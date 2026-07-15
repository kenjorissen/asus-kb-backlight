use std::fmt;
use std::str::FromStr;
use std::thread;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use wmi::WMIConnection;

pub const ASUS_WMI_NAMESPACE: &str = r"root\WMI";
pub const ASUS_WMI_CLASS: &str = "AsusAtkWmi_WMNB";
pub const KEYBOARD_BACKLIGHT_DEVICE_ID: u32 = 0x0005_0021;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Brightness {
    Off,
    Low,
    Medium,
    High,
}

impl Brightness {
    pub const fn level(self) -> u8 {
        match self {
            Self::Off => 0,
            Self::Low => 1,
            Self::Medium => 2,
            Self::High => 3,
        }
    }

    pub const fn control_status(self) -> u32 {
        0x80 | self.level() as u32
    }

    pub const fn from_level(level: u8) -> Option<Self> {
        match level {
            0 => Some(Self::Off),
            1 => Some(Self::Low),
            2 => Some(Self::Medium),
            3 => Some(Self::High),
            _ => None,
        }
    }
}

impl fmt::Display for Brightness {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Off => "off",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        })
    }
}

impl FromStr for Brightness {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_lowercase().as_str() {
            "0" | "off" => Ok(Self::Off),
            "1" | "low" => Ok(Self::Low),
            "2" | "medium" | "med" => Ok(Self::Medium),
            "3" | "high" | "on" => Ok(Self::High),
            _ => Err(format!(
                "invalid brightness {value:?}; use off, low, medium, high, or 0-3"
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct BacklightState {
    pub raw_status: u32,
    pub control_byte: u8,
    pub brightness: Brightness,
}

impl BacklightState {
    pub fn from_raw(raw_status: u32) -> Self {
        let control_byte = raw_status as u8;
        let brightness = Brightness::from_level(control_byte & 0x03).unwrap_or(Brightness::Off);
        Self {
            raw_status,
            control_byte,
            brightness,
        }
    }
}

#[derive(Debug, Error)]
pub enum BacklightError {
    #[error("cannot access ASUS WMI namespace {ASUS_WMI_NAMESPACE}: {0}")]
    Connect(#[source] wmi::WMIError),
    #[error(
        "cannot query ASUS WMI class {ASUS_WMI_CLASS}; verify ASUS System Control Interface v3 is installed (interactive commands may require an Administrator terminal): {0}"
    )]
    ClassQuery(#[source] wmi::WMIError),
    #[error("ASUS WMI class {ASUS_WMI_CLASS} returned no instances")]
    NoInstance,
    #[error("ASUS WMI {method} failed: {source}")]
    Method {
        method: &'static str,
        #[source]
        source: wmi::WMIError,
    },
    #[error("ASUS firmware rejected brightness {brightness} (DEVS result={result})")]
    FirmwareRejected { brightness: Brightness, result: u32 },
    #[error(
        "ASUS firmware accepted {requested}, but read-back reported {actual} (raw 0x{raw_status:08X})"
    )]
    VerificationFailed {
        requested: Brightness,
        actual: Brightness,
        raw_status: u32,
    },
}

#[allow(non_snake_case)]
#[allow(non_camel_case_types)]
#[derive(Debug, Deserialize)]
struct AsusAtkWmi_WMNB {
    __Path: String,
}

#[allow(non_snake_case)]
#[derive(Serialize)]
struct DeviceStatusInput {
    Device_ID: u32,
}

#[derive(Deserialize)]
struct DeviceStatusOutput {
    device_status: u32,
}

#[allow(non_snake_case)]
#[derive(Serialize)]
struct DeviceControlInput {
    Device_ID: u32,
    Control_status: u32,
}

#[derive(Deserialize)]
struct DeviceControlOutput {
    result: u32,
}

pub struct BacklightController {
    connection: WMIConnection,
    instance_path: String,
}

impl BacklightController {
    pub fn connect() -> Result<Self, BacklightError> {
        let connection = WMIConnection::with_namespace_path(ASUS_WMI_NAMESPACE)
            .map_err(BacklightError::Connect)?;
        let instance = connection
            .get::<AsusAtkWmi_WMNB>()
            .map_err(BacklightError::ClassQuery)?;

        if instance.__Path.is_empty() {
            return Err(BacklightError::NoInstance);
        }

        Ok(Self {
            connection,
            instance_path: instance.__Path,
        })
    }

    pub fn status(&self) -> Result<BacklightState, BacklightError> {
        let output: DeviceStatusOutput = self
            .connection
            .exec_instance_method::<AsusAtkWmi_WMNB, _>(
                &self.instance_path,
                "DSTS",
                DeviceStatusInput {
                    Device_ID: KEYBOARD_BACKLIGHT_DEVICE_ID,
                },
            )
            .map_err(|source| BacklightError::Method {
                method: "DSTS",
                source,
            })?;
        Ok(BacklightState::from_raw(output.device_status))
    }

    pub fn set(&self, brightness: Brightness) -> Result<BacklightState, BacklightError> {
        let output: DeviceControlOutput = self
            .connection
            .exec_instance_method::<AsusAtkWmi_WMNB, _>(
                &self.instance_path,
                "DEVS",
                DeviceControlInput {
                    Device_ID: KEYBOARD_BACKLIGHT_DEVICE_ID,
                    Control_status: brightness.control_status(),
                },
            )
            .map_err(|source| BacklightError::Method {
                method: "DEVS",
                source,
            })?;

        if output.result != 1 {
            return Err(BacklightError::FirmwareRejected {
                brightness,
                result: output.result,
            });
        }

        let mut actual = self.status()?;
        for _ in 0..4 {
            if actual.brightness == brightness {
                return Ok(actual);
            }
            thread::sleep(Duration::from_millis(50));
            actual = self.status()?;
        }
        Err(BacklightError::VerificationFailed {
            requested: brightness,
            actual: actual.brightness,
            raw_status: actual.raw_status,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_values_match_asus_protocol() {
        assert_eq!(Brightness::Off.control_status(), 0x80);
        assert_eq!(Brightness::Low.control_status(), 0x81);
        assert_eq!(Brightness::Medium.control_status(), 0x82);
        assert_eq!(Brightness::High.control_status(), 0x83);
    }

    #[test]
    fn parses_documented_status() {
        let off = BacklightState::from_raw(0x0035_0000);
        let high = BacklightState::from_raw(0x0035_0083);
        assert_eq!(off.brightness, Brightness::Off);
        assert_eq!(high.brightness, Brightness::High);
        assert_eq!(high.control_byte, 0x83);
    }

    #[test]
    fn parses_names_numbers_and_on_alias() {
        assert_eq!("off".parse(), Ok(Brightness::Off));
        assert_eq!("2".parse(), Ok(Brightness::Medium));
        assert_eq!("on".parse(), Ok(Brightness::High));
    }
}
