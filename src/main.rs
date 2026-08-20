mod control;
mod profile;
mod service;
mod usb;

use std::time::Duration;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};

use crate::{
    control::{CONTROLS, ControlSpec, ControlValue, find_control},
    profile::{Profile, profile_path},
    usb::{Camera, VendorControl, vendor_control},
};

#[derive(Parser)]
#[command(
    name = "kiyo",
    version,
    about = "Persistent macOS controls for the Razer Kiyo Pro"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Confirm the camera is connected and inspect its UVC control units.
    Diagnose,
    /// Show supported controls, current values, ranges, and saved Razer values.
    Show,
    /// Read one standard UVC control.
    Get { control: String },
    /// Set and remember one or more key=value assignments.
    Set {
        #[arg(required = true)]
        assignments: Vec<String>,
    },
    /// Capture the camera's current standard controls into the profile.
    Save,
    /// Reapply the saved profile now.
    Apply,
    /// Install and start the per-user reconnect service.
    Install {
        #[arg(long, default_value_t = 2)]
        interval: u64,
    },
    /// Stop and remove the reconnect service.
    Uninstall,
    /// Run the reconnect watcher in the foreground (used by launchd).
    #[command(hide = true)]
    Service {
        #[arg(long, default_value_t = 2)]
        interval: u64,
    },
}

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    match Cli::parse().command {
        Command::Diagnose => diagnose(),
        Command::Show => show(),
        Command::Get { control } => get(&control),
        Command::Set { assignments } => set(&assignments),
        Command::Save => save(),
        Command::Apply => apply(),
        Command::Install { interval } => {
            if interval == 0 {
                bail!("interval must be at least one second");
            }
            let profile = Profile::load()?;
            if profile.controls.is_empty() && profile.razer.is_empty() {
                bail!("profile is empty; run `kiyo save` or `kiyo set key=value` first");
            }
            let path = service::install(interval)?;
            println!("Reconnect service installed: {}", path.display());
            Ok(())
        }
        Command::Uninstall => {
            service::uninstall()?;
            println!("Reconnect service removed");
            Ok(())
        }
        Command::Service { interval } => {
            if interval == 0 {
                bail!("interval must be at least one second");
            }
            service::run(Duration::from_secs(interval))
        }
    }
}

fn diagnose() -> Result<()> {
    let identifier = Camera::identifier()?.context("Razer Kiyo Pro 1532:0e05 is not connected")?;
    let camera = Camera::open()?;
    println!("Razer Kiyo Pro 1532:0e05 at USB {identifier}");
    println!("{}", camera.unit_summary());
    let modes = camera
        .stream_modes()
        .iter()
        .map(|(width, height)| format!("{width}x{height}"))
        .collect::<Vec<_>>()
        .join(", ");
    println!("USB video-stream resolutions: {modes}");
    println!(
        "Note: capture resolution is selected by each video app; UVC controls cannot override it globally."
    );
    Ok(())
}

fn show() -> Result<()> {
    let camera = Camera::open()?;
    let profile = Profile::load()?;
    for spec in CONTROLS {
        match camera.range(spec) {
            Ok(range) => {
                let name = format!("{} ({})", spec.key, spec.label);
                println!(
                    "{name:<48} {:>8}  min {:>8}  max {:>8}  step {:>6}  default {:>8}",
                    spec.display(range.current),
                    display_optional(spec, range.minimum),
                    display_optional(spec, range.maximum),
                    display_optional(spec, range.resolution),
                    display_optional(spec, range.default),
                )
            }
            Err(error) => println!("{:<48} unsupported ({error})", spec.key),
        }
    }
    println!("\nRazer write-only controls (saved profile):");
    for (key, choices) in [
        ("hdr", "on|off"),
        ("hdr_mode", "dark|bright"),
        ("fov", "wide|medium|narrow"),
        ("autofocus_mode", "responsive|passive"),
    ] {
        let value = profile
            .razer
            .get(key)
            .map(String::as_str)
            .unwrap_or("not saved");
        println!("{key:<22} {value:<12} ({choices})");
    }

    fn display_optional(
        spec: &control::ControlSpec,
        value: Option<control::ControlValue>,
    ) -> String {
        value
            .map(|value| spec.display(value))
            .unwrap_or_else(|| "-".to_string())
    }
    Ok(())
}

fn get(key: &str) -> Result<()> {
    let spec =
        find_control(key).with_context(|| format!("unknown standard UVC control '{key}'"))?;
    let camera = Camera::open()?;
    println!("{}", spec.display(camera.get(spec, 0x81)?));
    Ok(())
}

fn set(assignments: &[String]) -> Result<()> {
    enum Pending {
        Standard(&'static ControlSpec, ControlValue),
        Vendor(String, VendorControl, String),
    }

    let mut pending = Vec::new();
    for assignment in assignments {
        let (key, raw) = assignment
            .split_once('=')
            .with_context(|| format!("expected key=value, got '{assignment}'"))?;
        if let Some(spec) = find_control(key) {
            pending.push(Pending::Standard(spec, spec.parse(raw)?));
        } else if let Some(control) = vendor_control(key) {
            let value = raw.to_ascii_lowercase();
            validate_vendor_value(control, &value)?;
            pending.push(Pending::Vendor(key.to_string(), control, value));
        } else {
            bail!("unknown control '{key}'; run `kiyo show` for the supported controls");
        }
    }
    pending.sort_by_key(|assignment| match assignment {
        Pending::Standard(spec, _) => match spec.key {
            "white_balance_auto" | "focus_auto" | "auto_exposure" => 0,
            _ => 1,
        },
        Pending::Vendor(_, _, _) => 2,
    });

    let camera = Camera::open()?;
    let mut profile = Profile::load()?;
    let mut vendor_changed = false;

    for assignment in pending {
        match assignment {
            Pending::Standard(spec, value) => {
                camera.set(spec, value)?;
                profile.controls.insert(spec.key.to_string(), value);
                println!("{}={}", spec.key, spec.display(value));
            }
            Pending::Vendor(key, control, value) => {
                camera.set_vendor(control, &value)?;
                profile.razer.insert(key.clone(), value.clone());
                vendor_changed = true;
                println!("{key}={value}");
            }
        }
    }

    if vendor_changed {
        camera.save_vendor_settings()?;
    }
    let path = profile.save()?;
    println!("Remembered in {}", path.display());
    Ok(())
}

fn validate_vendor_value(control: VendorControl, value: &str) -> Result<()> {
    let accepted: &[&str] = match control {
        VendorControl::Hdr => &["on", "off"],
        VendorControl::HdrMode => &["dark", "bright"],
        VendorControl::FieldOfView => &["wide", "medium", "narrow"],
        VendorControl::AutofocusMode => &["responsive", "passive"],
    };
    if accepted.contains(&value) {
        Ok(())
    } else {
        bail!("value must be one of: {}", accepted.join(", "))
    }
}

fn save() -> Result<()> {
    let camera = Camera::open()?;
    let profile = Profile::capture(&camera, Profile::load()?)?;
    let count = profile.controls.len();
    let path = profile.save()?;
    println!("Saved {count} standard controls to {}", path.display());
    Ok(())
}

fn apply() -> Result<()> {
    let path = profile_path()?;
    let profile = Profile::load()?;
    if profile.controls.is_empty() && profile.razer.is_empty() {
        bail!("profile is empty; run `kiyo save` or `kiyo set key=value` first");
    }
    let camera = Camera::open()?;
    let applied = profile.apply(&camera)?;
    println!("Applied {} from {}", applied.join(", "), path.display());
    Ok(())
}
