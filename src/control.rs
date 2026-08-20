use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Unit {
    Processing,
    CameraTerminal,
}

#[derive(Clone, Copy, Debug)]
pub enum ValueKind {
    Integer,
    Boolean,
    Menu(&'static [(i64, &'static str)]),
}

#[derive(Clone, Copy, Debug)]
pub struct ControlSpec {
    pub key: &'static str,
    pub label: &'static str,
    pub unit: Unit,
    pub selector: u8,
    pub len: usize,
    pub signed: bool,
    pub kind: ValueKind,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(untagged)]
pub enum ControlValue {
    Number(i64),
    Toggle(bool),
}

const POWER_LINE: &[(i64, &str)] = &[(0, "disabled"), (1, "50hz"), (2, "60hz")];
const AUTO_EXPOSURE: &[(i64, &str)] = &[
    (1, "manual"),
    (2, "auto"),
    (4, "shutter_priority"),
    (8, "aperture_priority"),
];

pub const CONTROLS: &[ControlSpec] = &[
    ControlSpec::new("brightness", "Brightness", Unit::Processing, 0x02, 2, true),
    ControlSpec::new("contrast", "Contrast", Unit::Processing, 0x03, 2, false),
    ControlSpec::new("saturation", "Saturation", Unit::Processing, 0x07, 2, false),
    ControlSpec::new("sharpness", "Sharpness", Unit::Processing, 0x08, 2, false),
    ControlSpec::new("gamma", "Gamma", Unit::Processing, 0x09, 2, false),
    ControlSpec::new("hue", "Hue", Unit::Processing, 0x06, 2, true),
    ControlSpec::new("gain", "Gain", Unit::Processing, 0x04, 2, false),
    ControlSpec::new(
        "backlight",
        "Backlight compensation",
        Unit::Processing,
        0x01,
        2,
        false,
    ),
    ControlSpec::menu(
        "power_line",
        "Power-line frequency",
        Unit::Processing,
        0x05,
        1,
        POWER_LINE,
    ),
    ControlSpec::menu(
        "auto_exposure",
        "Auto exposure",
        Unit::CameraTerminal,
        0x02,
        1,
        AUTO_EXPOSURE,
    ),
    ControlSpec::new(
        "exposure_time",
        "Exposure time",
        Unit::CameraTerminal,
        0x04,
        4,
        false,
    ),
    ControlSpec::boolean(
        "white_balance_auto",
        "Auto white balance",
        Unit::Processing,
        0x0b,
    ),
    ControlSpec::new(
        "white_balance",
        "White-balance temperature",
        Unit::Processing,
        0x0a,
        2,
        false,
    ),
    ControlSpec::boolean("focus_auto", "Autofocus", Unit::CameraTerminal, 0x08),
    ControlSpec::new("focus", "Focus", Unit::CameraTerminal, 0x06, 2, false),
    ControlSpec::new("zoom", "Zoom", Unit::CameraTerminal, 0x0b, 2, false),
];

impl ControlSpec {
    const fn new(
        key: &'static str,
        label: &'static str,
        unit: Unit,
        selector: u8,
        len: usize,
        signed: bool,
    ) -> Self {
        Self {
            key,
            label,
            unit,
            selector,
            len,
            signed,
            kind: ValueKind::Integer,
        }
    }

    const fn boolean(key: &'static str, label: &'static str, unit: Unit, selector: u8) -> Self {
        Self {
            key,
            label,
            unit,
            selector,
            len: 1,
            signed: false,
            kind: ValueKind::Boolean,
        }
    }

    const fn menu(
        key: &'static str,
        label: &'static str,
        unit: Unit,
        selector: u8,
        len: usize,
        choices: &'static [(i64, &'static str)],
    ) -> Self {
        Self {
            key,
            label,
            unit,
            selector,
            len,
            signed: false,
            kind: ValueKind::Menu(choices),
        }
    }

    pub fn parse(&self, input: &str) -> Result<ControlValue> {
        match self.kind {
            ValueKind::Integer => Ok(ControlValue::Number(input.parse()?)),
            ValueKind::Boolean => match input.to_ascii_lowercase().as_str() {
                "on" | "true" | "1" => Ok(ControlValue::Toggle(true)),
                "off" | "false" | "0" => Ok(ControlValue::Toggle(false)),
                _ => bail!("{} accepts on or off", self.key),
            },
            ValueKind::Menu(choices) => {
                let normalized = input.to_ascii_lowercase().replace('-', "_");
                choices
                    .iter()
                    .find(|(_, name)| *name == normalized)
                    .map(|(value, _)| ControlValue::Number(*value))
                    .or_else(|| input.parse().ok().map(ControlValue::Number))
                    .ok_or_else(|| {
                        let names = choices
                            .iter()
                            .map(|(_, name)| *name)
                            .collect::<Vec<_>>()
                            .join(", ");
                        anyhow::anyhow!("{} accepts: {names}", self.key)
                    })
            }
        }
    }

    pub fn display(&self, value: ControlValue) -> String {
        match (self.kind, value) {
            (ValueKind::Boolean, ControlValue::Toggle(value)) => {
                if value { "on" } else { "off" }.to_string()
            }
            (ValueKind::Menu(choices), ControlValue::Number(value)) => choices
                .iter()
                .find(|(candidate, _)| *candidate == value)
                .map(|(_, label)| (*label).to_string())
                .unwrap_or_else(|| value.to_string()),
            (_, ControlValue::Number(value)) => value.to_string(),
            (_, ControlValue::Toggle(value)) => i64::from(value).to_string(),
        }
    }

    pub fn encode(&self, value: ControlValue) -> Result<Vec<u8>> {
        let number = match value {
            ControlValue::Number(value) => value,
            ControlValue::Toggle(value) => i64::from(value),
        };
        let bits = self.len * 8;
        let (minimum, maximum) = if self.signed {
            (-(1_i128 << (bits - 1)), (1_i128 << (bits - 1)) - 1)
        } else {
            (0, (1_i128 << bits) - 1)
        };
        if i128::from(number) < minimum || i128::from(number) > maximum {
            bail!(
                "{} value {number} does not fit in {} {} byte{}",
                self.key,
                if self.signed {
                    "a signed"
                } else {
                    "an unsigned"
                },
                self.len,
                if self.len == 1 { "" } else { "s" }
            );
        }
        let bytes = number.to_le_bytes();
        Ok(bytes[..self.len].to_vec())
    }

    pub fn decode(&self, bytes: &[u8]) -> Result<ControlValue> {
        if bytes.len() != self.len {
            bail!(
                "{} returned {} bytes, expected {}",
                self.key,
                bytes.len(),
                self.len
            );
        }

        let mut raw = [0_u8; 8];
        raw[..bytes.len()].copy_from_slice(bytes);
        let value = if self.signed {
            let shift = 64 - (bytes.len() * 8);
            (i64::from_le_bytes(raw) << shift) >> shift
        } else {
            u64::from_le_bytes(raw) as i64
        };

        Ok(match self.kind {
            ValueKind::Boolean => ControlValue::Toggle(value != 0),
            _ => ControlValue::Number(value),
        })
    }
}

pub fn find_control(key: &str) -> Option<&'static ControlSpec> {
    CONTROLS.iter().find(|spec| spec.key == key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signed_values_round_trip() {
        let brightness = find_control("brightness").unwrap();
        let encoded = brightness.encode(ControlValue::Number(-12)).unwrap();
        assert_eq!(
            brightness.decode(&encoded).unwrap(),
            ControlValue::Number(-12)
        );
    }

    #[test]
    fn menus_accept_names() {
        let exposure = find_control("auto_exposure").unwrap();
        assert_eq!(
            exposure.parse("shutter-priority").unwrap(),
            ControlValue::Number(4)
        );
    }

    #[test]
    fn encoding_rejects_values_that_do_not_fit() {
        let zoom = find_control("zoom").unwrap();
        assert!(zoom.encode(ControlValue::Number(100_000)).is_err());
        assert!(zoom.encode(ControlValue::Number(-1)).is_err());

        let brightness = find_control("brightness").unwrap();
        assert!(
            brightness
                .encode(ControlValue::Number(i64::from(i16::MAX) + 1))
                .is_err()
        );
    }
}
