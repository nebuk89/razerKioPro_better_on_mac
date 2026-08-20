use std::{
    env, fs,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    process::Command,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, bail};

use crate::{
    profile::{Profile, ensure_parent, log_path},
    usb::Camera,
};

const LABEL: &str = "com.github.nebuk89.kiyo-control";

pub fn run(interval: Duration) -> Result<()> {
    let log = log_path()?;
    ensure_parent(&log)?;
    let mut previous = None;

    loop {
        match Camera::identifier() {
            Ok(identifier) if identifier != previous => {
                if let Some(ref device) = identifier {
                    match Camera::open().and_then(|camera| Profile::load()?.apply(&camera)) {
                        Ok(applied) => write_log(
                            &log,
                            &format!("camera {device} connected; applied {}", applied.join(", ")),
                        )?,
                        Err(error) => write_log(
                            &log,
                            &format!("camera {device} connected; apply failed: {error:#}"),
                        )?,
                    }
                } else if previous.is_some() {
                    write_log(&log, "camera disconnected")?;
                }
                previous = identifier;
            }
            Ok(_) => {}
            Err(error) => write_log(&log, &format!("USB scan failed: {error:#}"))?,
        }
        thread::sleep(interval);
    }
}

pub fn install(interval_seconds: u64) -> Result<PathBuf> {
    let source = env::current_exe().context("finding the current executable")?;
    let home = dirs::home_dir().context("home directory is unavailable")?;
    let binary = home.join(".local/bin/kiyo");
    ensure_parent(&binary)?;
    fs::copy(&source, &binary)
        .with_context(|| format!("copying {} to {}", source.display(), binary.display()))?;
    fs::set_permissions(&binary, fs::Permissions::from_mode(0o755))?;

    let launch_agents = home.join("Library/LaunchAgents");
    fs::create_dir_all(&launch_agents)?;
    let plist = launch_agents.join(format!("{LABEL}.plist"));
    let stdout = log_path()?;
    ensure_parent(&stdout)?;
    let contents = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>{LABEL}</string>
  <key>ProgramArguments</key>
  <array>
    <string>{binary}</string>
    <string>service</string>
    <string>--interval</string>
    <string>{interval_seconds}</string>
  </array>
  <key>RunAtLoad</key>
  <true/>
  <key>KeepAlive</key>
  <true/>
  <key>StandardOutPath</key>
  <string>{stdout}</string>
  <key>StandardErrorPath</key>
  <string>{stdout}</string>
</dict>
</plist>
"#,
        binary = xml_escape(&binary.display().to_string()),
        stdout = xml_escape(&stdout.display().to_string()),
    );
    fs::write(&plist, contents)?;

    let domain = format!("gui/{}", unsafe { libc_getuid() });
    let _ = Command::new("launchctl")
        .args(["bootout", &domain, &plist.display().to_string()])
        .output();
    let status = Command::new("launchctl")
        .args(["bootstrap", &domain, &plist.display().to_string()])
        .status()
        .context("starting the Kiyo launch agent")?;
    if !status.success() {
        bail!("launchctl could not start {}", plist.display());
    }
    Ok(plist)
}

pub fn uninstall() -> Result<()> {
    let home = dirs::home_dir().context("home directory is unavailable")?;
    let plist = home
        .join("Library/LaunchAgents")
        .join(format!("{LABEL}.plist"));
    if plist.exists() {
        let domain = format!("gui/{}", unsafe { libc_getuid() });
        let _ = Command::new("launchctl")
            .args(["bootout", &domain, &plist.display().to_string()])
            .output();
        fs::remove_file(&plist)?;
    }
    Ok(())
}

fn write_log(path: &PathBuf, message: &str) -> Result<()> {
    use std::io::Write;

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    writeln!(file, "{timestamp} {message}")?;
    Ok(())
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

unsafe extern "C" {
    fn getuid() -> u32;
}

unsafe fn libc_getuid() -> u32 {
    unsafe { getuid() }
}
