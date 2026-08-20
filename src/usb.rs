use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use rusb::{Device, DeviceHandle, GlobalContext};

use crate::control::{ControlSpec, ControlValue, Unit};

pub const VENDOR_ID: u16 = 0x1532;
pub const PRODUCT_ID: u16 = 0x0e05;

const TIMEOUT: Duration = Duration::from_millis(750);
const GET_CUR: u8 = 0x81;
const GET_MIN: u8 = 0x82;
const GET_MAX: u8 = 0x83;
const GET_RES: u8 = 0x84;
const GET_DEF: u8 = 0x87;
const SET_CUR: u8 = 0x01;
const REQUEST_GET: u8 = 0xa1;
const REQUEST_SET: u8 = 0x21;
const RAZER_SELECTOR: u8 = 0x01;

const RAZER_EXTENSION_GUID: [u8; 16] = [
    0xd0, 0x9e, 0xe4, 0x23, 0x78, 0x11, 0x31, 0x4f, 0xae, 0x52, 0xd2, 0xfb, 0x8a, 0x8d, 0x3b, 0x48,
];

const SAVE_TO_NVRAM: [u8; 8] = [0xc0, 0x03, 0xa8, 0, 0, 0, 0, 0];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VendorControl {
    Hdr,
    HdrMode,
    FieldOfView,
    AutofocusMode,
}

#[derive(Clone, Copy, Debug)]
pub struct ControlRange {
    pub current: ControlValue,
    pub minimum: Option<ControlValue>,
    pub maximum: Option<ControlValue>,
    pub resolution: Option<ControlValue>,
    pub default: Option<ControlValue>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct Units {
    interface: u8,
    processing: u8,
    camera_terminal: u8,
    razer_extension: u8,
}

pub struct Camera {
    handle: DeviceHandle<GlobalContext>,
    units: Units,
    stream_modes: Vec<(u16, u16)>,
}

impl Camera {
    pub fn open() -> Result<Self> {
        let device = find_device()?.ok_or_else(|| anyhow!("Razer Kiyo Pro is not connected"))?;
        let handle = device
            .open()
            .context("opening the Kiyo Pro USB control endpoint")?;
        let descriptors = read_configuration_descriptor(&handle)?;
        let units = parse_units(&descriptors)?;
        let stream_modes = parse_stream_modes(&descriptors)?;

        Ok(Self {
            handle,
            units,
            stream_modes,
        })
    }

    pub fn identifier() -> Result<Option<String>> {
        Ok(find_device()?
            .map(|device| format!("{:03}:{:03}", device.bus_number(), device.address())))
    }

    pub fn unit_summary(&self) -> String {
        format!(
            "VC interface {}, camera terminal {}, processing unit {}, Razer extension {}",
            self.units.interface,
            self.units.camera_terminal,
            self.units.processing,
            self.units.razer_extension
        )
    }

    pub fn stream_modes(&self) -> &[(u16, u16)] {
        &self.stream_modes
    }

    pub fn get(&self, spec: &ControlSpec, request: u8) -> Result<ControlValue> {
        let unit = self.unit_id(spec.unit)?;
        let mut buffer = vec![0_u8; spec.len];
        let read = self.handle.read_control(
            REQUEST_GET,
            request,
            u16::from(spec.selector) << 8,
            (u16::from(unit) << 8) | u16::from(self.units.interface),
            &mut buffer,
            TIMEOUT,
        );
        std::thread::sleep(Duration::from_millis(50));

        match read {
            Ok(size) if size == spec.len => spec.decode(&buffer),
            Ok(size) => bail!("{} returned {size} bytes, expected {}", spec.key, spec.len),
            Err(rusb::Error::Pipe) => bail!("{} is not supported by this camera", spec.key),
            Err(error) => Err(error).with_context(|| format!("reading {}", spec.key)),
        }
    }

    pub fn range(&self, spec: &ControlSpec) -> Result<ControlRange> {
        Ok(ControlRange {
            current: self.get(spec, GET_CUR)?,
            minimum: self.get(spec, GET_MIN).ok(),
            maximum: self.get(spec, GET_MAX).ok(),
            resolution: self.get(spec, GET_RES).ok(),
            default: self.get(spec, GET_DEF).ok(),
        })
    }

    pub fn set(&self, spec: &ControlSpec, value: ControlValue) -> Result<()> {
        if let Ok(range) = self.range(spec)
            && let (Some(minimum), Some(maximum)) = (range.minimum, range.maximum)
        {
            let number = numeric(value);
            if number < numeric(minimum) || number > numeric(maximum) {
                bail!(
                    "{} must be between {} and {}",
                    spec.key,
                    spec.display(minimum),
                    spec.display(maximum)
                );
            }
        }

        let unit = self.unit_id(spec.unit)?;
        self.write(unit, spec.selector, &spec.encode(value)?)
            .with_context(|| format!("setting {}", spec.key))
    }

    pub fn set_vendor(&self, control: VendorControl, value: &str) -> Result<()> {
        let normalized = value.to_ascii_lowercase();
        let messages: &[[u8; 8]] = match (control, normalized.as_str()) {
            (VendorControl::Hdr, "off") => &[[0xff, 0x02, 0x00, 0, 0, 0, 0, 0]],
            (VendorControl::Hdr, "on") => &[[0xff, 0x02, 0x01, 0, 0, 0, 0, 0]],
            (VendorControl::HdrMode, "dark") => &[[0xff, 0x07, 0x00, 0, 0, 0, 0, 0]],
            (VendorControl::HdrMode, "bright") => &[[0xff, 0x07, 0x01, 0, 0, 0, 0, 0]],
            (VendorControl::AutofocusMode, "responsive") => &[[0xff, 0x06, 0x00, 0, 0, 0, 0, 0]],
            (VendorControl::AutofocusMode, "passive") => &[[0xff, 0x06, 0x01, 0, 0, 0, 0, 0]],
            (VendorControl::FieldOfView, "wide") => &[[0xff, 0x01, 0x00, 0x03, 0x00, 0, 0, 0]],
            (VendorControl::FieldOfView, "medium") => &[
                [0xff, 0x01, 0x00, 0x03, 0x01, 0, 0, 0],
                [0xff, 0x01, 0x01, 0x03, 0x01, 0, 0, 0],
            ],
            (VendorControl::FieldOfView, "narrow") => &[
                [0xff, 0x01, 0x00, 0x03, 0x02, 0, 0, 0],
                [0xff, 0x01, 0x01, 0x03, 0x02, 0, 0, 0],
            ],
            (VendorControl::Hdr, _) => bail!("hdr accepts on or off"),
            (VendorControl::HdrMode, _) => bail!("hdr_mode accepts dark or bright"),
            (VendorControl::FieldOfView, _) => bail!("fov accepts wide, medium, or narrow"),
            (VendorControl::AutofocusMode, _) => {
                bail!("autofocus_mode accepts responsive or passive")
            }
        };

        if self.units.razer_extension == 0 {
            bail!("the Razer extension unit was not found");
        }
        for message in messages {
            self.write(self.units.razer_extension, RAZER_SELECTOR, message)?;
        }
        Ok(())
    }

    pub fn save_vendor_settings(&self) -> Result<()> {
        if self.units.razer_extension == 0 {
            bail!("the Razer extension unit was not found");
        }
        self.write(self.units.razer_extension, RAZER_SELECTOR, &SAVE_TO_NVRAM)
            .context("saving Razer settings to camera NVRAM")
    }

    fn unit_id(&self, unit: Unit) -> Result<u8> {
        let id = match unit {
            Unit::Processing => self.units.processing,
            Unit::CameraTerminal => self.units.camera_terminal,
        };
        if id == 0 {
            bail!("{unit:?} unit was not found in the camera descriptors");
        }
        Ok(id)
    }

    fn write(&self, unit: u8, selector: u8, data: &[u8]) -> Result<()> {
        let written = self
            .handle
            .write_control(
                REQUEST_SET,
                SET_CUR,
                u16::from(selector) << 8,
                (u16::from(unit) << 8) | u16::from(self.units.interface),
                data,
                TIMEOUT,
            )
            .context("USB control transfer failed")?;
        // Kiyo Pro firmware can wedge when class requests are sent back-to-back.
        std::thread::sleep(Duration::from_millis(50));
        if written != data.len() {
            bail!(
                "USB control transfer wrote {written} bytes, expected {}",
                data.len()
            );
        }
        Ok(())
    }
}

pub fn vendor_control(key: &str) -> Option<VendorControl> {
    match key {
        "hdr" => Some(VendorControl::Hdr),
        "hdr_mode" => Some(VendorControl::HdrMode),
        "fov" => Some(VendorControl::FieldOfView),
        "autofocus_mode" => Some(VendorControl::AutofocusMode),
        _ => None,
    }
}

fn numeric(value: ControlValue) -> i64 {
    match value {
        ControlValue::Number(value) => value,
        ControlValue::Toggle(value) => i64::from(value),
    }
}

fn find_device() -> Result<Option<Device<GlobalContext>>> {
    Ok(rusb::devices()
        .context("enumerating USB devices")?
        .iter()
        .find(|device| {
            device
                .device_descriptor()
                .map(|descriptor| {
                    descriptor.vendor_id() == VENDOR_ID && descriptor.product_id() == PRODUCT_ID
                })
                .unwrap_or(false)
        }))
}

fn read_configuration_descriptor(handle: &DeviceHandle<GlobalContext>) -> Result<Vec<u8>> {
    let mut header = [0_u8; 9];
    let header_length = handle
        .read_control(0x80, 0x06, 0x0200, 0, &mut header, TIMEOUT)
        .context("reading USB configuration descriptor header")?;
    if header_length != header.len() {
        bail!(
            "USB configuration descriptor header returned {header_length} bytes, expected {}",
            header.len()
        );
    }
    let total = usize::from(u16::from_le_bytes([header[2], header[3]]));
    let mut descriptors = vec![0_u8; total];
    let read = handle
        .read_control(0x80, 0x06, 0x0200, 0, &mut descriptors, TIMEOUT)
        .context("reading USB configuration descriptors")?;
    descriptors.truncate(read);
    Ok(descriptors)
}

fn parse_units(descriptors: &[u8]) -> Result<Units> {
    let mut units = Units::default();
    let mut in_video_control = false;
    let mut offset = 0;

    while offset + 2 <= descriptors.len() {
        let length = usize::from(descriptors[offset]);
        if length < 2 || offset + length > descriptors.len() {
            bail!("malformed USB descriptor at byte {offset}");
        }

        let descriptor = &descriptors[offset..offset + length];

        match descriptor[1] {
            0x04 if length >= 7 => {
                in_video_control = descriptor[5] == 0x0e && descriptor[6] == 0x01;
                if in_video_control {
                    units.interface = descriptor[2];
                }
            }
            0x24 if in_video_control && length >= 4 => match descriptor[2] {
                0x02 if length >= 6 && descriptor[4..6] == [0x01, 0x02] => {
                    units.camera_terminal = descriptor[3];
                }
                0x05 => units.processing = descriptor[3],
                0x06 if length >= 20 && descriptor[4..20] == RAZER_EXTENSION_GUID => {
                    units.razer_extension = descriptor[3];
                }
                _ => {}
            },
            _ => {}
        }

        offset += length;
    }

    if units.processing == 0 && units.camera_terminal == 0 && units.razer_extension == 0 {
        bail!("no UVC control units were found");
    }
    Ok(units)
}

fn parse_stream_modes(descriptors: &[u8]) -> Result<Vec<(u16, u16)>> {
    let mut modes = Vec::new();
    let mut in_video_streaming = false;
    let mut offset = 0;

    while offset + 2 <= descriptors.len() {
        let length = usize::from(descriptors[offset]);
        if length < 2 || offset + length > descriptors.len() {
            bail!("malformed USB descriptor at byte {offset}");
        }
        let descriptor = &descriptors[offset..offset + length];

        match descriptor[1] {
            0x04 if length >= 7 => {
                in_video_streaming = descriptor[5] == 0x0e && descriptor[6] == 0x02;
            }
            0x24 if in_video_streaming
                && length >= 9
                && matches!(descriptor[2], 0x05 | 0x07 | 0x11) =>
            {
                let width = u16::from_le_bytes([descriptor[5], descriptor[6]]);
                let height = u16::from_le_bytes([descriptor[7], descriptor[8]]);
                if width > 0 && height > 0 && !modes.contains(&(width, height)) {
                    modes.push((width, height));
                }
            }
            _ => {}
        }

        offset += length;
    }

    modes.sort_unstable_by(|left, right| {
        (left.0 as u32 * left.1 as u32).cmp(&(right.0 as u32 * right.1 as u32))
    });
    Ok(modes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_uvc_units() {
        let mut data = vec![
            9, 4, 2, 0, 1, 0x0e, 0x01, 0, 0, // VC interface 2
            8, 0x24, 0x02, 4, 0x01, 0x02, 0, 0, // camera terminal 4
            4, 0x24, 0x05, 6, // processing unit 6
        ];
        let mut extension = vec![20, 0x24, 0x06, 8];
        extension.extend_from_slice(&RAZER_EXTENSION_GUID);
        data.extend(extension);

        assert_eq!(
            parse_units(&data).unwrap(),
            Units {
                interface: 2,
                camera_terminal: 4,
                processing: 6,
                razer_extension: 8,
            }
        );
    }

    #[test]
    fn rejects_truncated_descriptors() {
        assert!(parse_units(&[9, 4, 0]).is_err());
    }

    #[test]
    fn parses_unique_stream_resolutions() {
        let data = [
            9, 4, 1, 0, 1, 0x0e, 0x02, 0, 0, // VS interface
            9, 0x24, 0x07, 1, 0, 0x80, 0x07, 0x38, 0x04, // MJPEG 1920x1080
            9, 0x24, 0x07, 2, 0, 0x00, 0x05, 0xd0, 0x02, // MJPEG 1280x720
            9, 0x24, 0x07, 3, 0, 0x80, 0x07, 0x38, 0x04, // duplicate 1080p
        ];
        assert_eq!(
            parse_stream_modes(&data).unwrap(),
            vec![(1280, 720), (1920, 1080)]
        );
    }
}
