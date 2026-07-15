use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::backlight::Brightness;

pub const APP_DIRECTORY: &str = "AsusKbdLight";
pub const CONFIG_FILE: &str = "config.json";
pub const EVENT_LOG_FILE: &str = "events.jsonl";
pub const DEFAULT_POLL_INTERVAL_MS: u64 = 30_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub brightness: Brightness,
    pub poll_interval_ms: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            brightness: Brightness::High,
            poll_interval_ms: DEFAULT_POLL_INTERVAL_MS,
        }
    }
}

impl Config {
    pub fn load_or_default() -> io::Result<Self> {
        let path = config_path();
        match fs::read_to_string(path) {
            Ok(contents) => serde_json::from_str(&contents)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(error),
        }
    }

    pub fn save(&self) -> io::Result<()> {
        let directory = config_directory();
        fs::create_dir_all(&directory)?;
        let bytes = serde_json::to_vec_pretty(self)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        fs::write(directory.join(CONFIG_FILE), bytes)
    }
}

pub fn data_directory() -> PathBuf {
    env::var_os("PROGRAMDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\ProgramData"))
        .join(APP_DIRECTORY)
}

pub fn config_directory() -> PathBuf {
    data_directory().join("config")
}

pub fn config_path() -> PathBuf {
    config_directory().join(CONFIG_FILE)
}

pub fn event_log_path() -> PathBuf {
    data_directory().join(EVENT_LOG_FILE)
}

pub fn ensure_data_directories() -> io::Result<()> {
    fs::create_dir_all(config_directory())
}

pub fn display_path(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}
