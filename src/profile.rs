use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::{
    control::{CONTROLS, ControlValue, find_control},
    usb::{Camera, vendor_control},
};

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Profile {
    #[serde(default)]
    pub controls: BTreeMap<String, ControlValue>,
    #[serde(default)]
    pub razer: BTreeMap<String, String>,
}

impl Profile {
    pub fn load() -> Result<Self> {
        let path = profile_path()?;
        if !path.exists() {
            return Ok(Self::default());
        }
        let data =
            fs::read(&path).with_context(|| format!("reading profile {}", path.display()))?;
        serde_json::from_slice(&data).with_context(|| format!("parsing profile {}", path.display()))
    }

    pub fn save(&self) -> Result<PathBuf> {
        let path = profile_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
        }
        let temporary = path.with_extension("json.tmp");
        fs::write(&temporary, serde_json::to_vec_pretty(self)?)
            .with_context(|| format!("writing {}", temporary.display()))?;
        fs::rename(&temporary, &path).with_context(|| format!("replacing {}", path.display()))?;
        Ok(path)
    }

    pub fn capture(camera: &Camera, previous: Self) -> Result<Self> {
        let mut profile = Self {
            razer: previous.razer,
            ..Self::default()
        };
        for spec in CONTROLS {
            if let Ok(value) = camera.get(spec, 0x81) {
                profile.controls.insert(spec.key.to_string(), value);
            }
        }
        Ok(profile)
    }

    pub fn apply(&self, camera: &Camera) -> Result<Vec<String>> {
        let mut applied = Vec::new();
        let priority = ["white_balance_auto", "focus_auto", "auto_exposure"];

        for key in priority {
            if let Some(value) = self.controls.get(key)
                && let Some(spec) = find_control(key)
            {
                camera.set(spec, *value)?;
                applied.push(key.to_string());
            }
        }

        for (key, value) in &self.controls {
            if priority.contains(&key.as_str()) {
                continue;
            }
            if self.control_is_locked(key) {
                continue;
            }
            if let Some(spec) = find_control(key) {
                camera.set(spec, *value)?;
                applied.push(key.clone());
            }
        }

        for (key, value) in &self.razer {
            if let Some(control) = vendor_control(key) {
                camera.set_vendor(control, value)?;
                applied.push(key.clone());
            }
        }
        if !self.razer.is_empty() {
            camera.save_vendor_settings()?;
        }
        Ok(applied)
    }

    fn control_is_locked(&self, key: &str) -> bool {
        match key {
            "white_balance" => self.toggle_is_on("white_balance_auto"),
            "focus" => self.toggle_is_on("focus_auto"),
            "exposure_time" => self
                .controls
                .get("auto_exposure")
                .is_some_and(|value| !matches!(value, ControlValue::Number(1 | 4))),
            _ => false,
        }
    }

    fn toggle_is_on(&self, key: &str) -> bool {
        matches!(self.controls.get(key), Some(ControlValue::Toggle(true)))
    }
}

pub fn profile_path() -> Result<PathBuf> {
    let base = dirs::data_dir().context("macOS application-support directory is unavailable")?;
    Ok(base.join("KiyoControl").join("profile.json"))
}

pub fn log_path() -> Result<PathBuf> {
    let path = profile_path()?;
    Ok(path.with_file_name("service.log"))
}

pub fn ensure_parent(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_missing_sections_as_empty() {
        let profile: Profile = serde_json::from_str("{}").unwrap();
        assert!(profile.controls.is_empty());
        assert!(profile.razer.is_empty());
    }

    #[test]
    fn automatic_modes_lock_manual_values() {
        let profile = Profile {
            controls: BTreeMap::from([
                ("white_balance_auto".to_string(), ControlValue::Toggle(true)),
                ("focus_auto".to_string(), ControlValue::Toggle(true)),
                ("auto_exposure".to_string(), ControlValue::Number(2)),
            ]),
            razer: BTreeMap::new(),
        };

        assert!(profile.control_is_locked("white_balance"));
        assert!(profile.control_is_locked("focus"));
        assert!(profile.control_is_locked("exposure_time"));
    }
}
