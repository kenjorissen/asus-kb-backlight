use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;

use crate::backlight::{BacklightController, BacklightState, Brightness};
use crate::config::{Config, event_log_path};

const MAX_EVENT_LOG_BYTES: u64 = 5 * 1024 * 1024;
const RETAINED_EVENT_LOG_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug)]
pub enum MonitorCommand {
    Stop,
    Context(String),
    EnforceNow(String),
}

#[derive(Serialize)]
struct LogEvent<'a> {
    unix_ms: u128,
    event: &'a str,
    desired: Option<Brightness>,
    observed: Option<BacklightState>,
    message: Option<&'a str>,
}

pub struct EventLogger {
    file: Option<File>,
    path: Option<PathBuf>,
    echo: bool,
}

impl EventLogger {
    pub fn file(echo: bool) -> io::Result<Self> {
        let path = event_log_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new().create(true).append(true).open(&path)?;
        Ok(Self {
            file: Some(file),
            path: Some(path),
            echo,
        })
    }

    pub fn console() -> Self {
        Self {
            file: None,
            path: None,
            echo: true,
        }
    }

    fn log(
        &mut self,
        event: &str,
        desired: Option<Brightness>,
        observed: Option<BacklightState>,
        message: Option<&str>,
    ) {
        let record = LogEvent {
            unix_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis(),
            event,
            desired,
            observed,
            message,
        };
        let mut line = serde_json::to_string(&record)
            .unwrap_or_else(|_| r#"{"event":"logger_serialization_error"}"#.to_owned());
        line.push('\n');
        if self.echo {
            print!("{line}");
        }
        if let Some(file) = &mut self.file {
            let _ = file.write_all(line.as_bytes());
            let _ = file.flush();
        }
        self.compact_if_needed();
    }

    fn compact_if_needed(&mut self) {
        let Some(path) = self.path.clone() else {
            return;
        };
        let is_oversized = self
            .file
            .as_ref()
            .and_then(|file| file.metadata().ok())
            .is_some_and(|metadata| metadata.len() > MAX_EVENT_LOG_BYTES);
        if !is_oversized {
            return;
        }

        // Close the append handle while compacting, then reopen it regardless of
        // whether compaction succeeded so logging can continue.
        self.file.take();
        let _ = compact_log(&path);
        self.file = OpenOptions::new().create(true).append(true).open(path).ok();
    }
}

fn compact_log(path: &Path) -> io::Result<()> {
    let contents = fs::read(path)?;
    let retained = retained_complete_lines(&contents, RETAINED_EVENT_LOG_BYTES);
    fs::write(path, retained)
}

fn retained_complete_lines(contents: &[u8], retained_bytes: usize) -> &[u8] {
    if contents.len() <= retained_bytes {
        return contents;
    }
    let candidate = &contents[contents.len() - retained_bytes..];
    match candidate.iter().position(|byte| *byte == b'\n') {
        Some(boundary) => &candidate[boundary + 1..],
        None => &candidate[candidate.len()..],
    }
}

pub fn run(receiver: Receiver<MonitorCommand>, mut logger: EventLogger) {
    logger.log("monitor_started", None, None, None);
    let mut controller: Option<BacklightController> = None;
    let mut previous_state: Option<BacklightState> = None;
    let mut previous_error = String::new();

    loop {
        let config = match Config::load_or_default() {
            Ok(config) => config,
            Err(error) => {
                let message = format!("cannot read configuration: {error}");
                if message != previous_error {
                    logger.log("config_error", None, None, Some(&message));
                    previous_error = message;
                }
                Config::default()
            }
        };

        if controller.is_none() {
            match BacklightController::connect() {
                Ok(value) => {
                    logger.log("wmi_connected", Some(config.brightness), None, None);
                    controller = Some(value);
                    previous_error.clear();
                }
                Err(error) => {
                    let message = error.to_string();
                    if message != previous_error {
                        logger.log(
                            "wmi_connect_error",
                            Some(config.brightness),
                            None,
                            Some(&message),
                        );
                        previous_error = message;
                    }
                }
            }
        }

        if let Some(active) = &controller {
            match active.status() {
                Ok(state) => {
                    previous_error.clear();
                    if previous_state != Some(state) {
                        logger.log("state_changed", Some(config.brightness), Some(state), None);
                        previous_state = Some(state);
                    }

                    if state.brightness != config.brightness {
                        logger.log(
                            "correction_requested",
                            Some(config.brightness),
                            Some(state),
                            None,
                        );
                        match active.set(config.brightness) {
                            Ok(corrected) => {
                                logger.log(
                                    "correction_succeeded",
                                    Some(config.brightness),
                                    Some(corrected),
                                    None,
                                );
                                previous_state = Some(corrected);
                            }
                            Err(error) => {
                                let message = error.to_string();
                                logger.log(
                                    "correction_failed",
                                    Some(config.brightness),
                                    Some(state),
                                    Some(&message),
                                );
                                previous_error = message;
                            }
                        }
                    }
                }
                Err(error) => {
                    let message = error.to_string();
                    if message != previous_error {
                        logger.log(
                            "status_error",
                            Some(config.brightness),
                            previous_state,
                            Some(&message),
                        );
                        previous_error = message;
                    }
                    controller = None;
                }
            }
        }

        let wait = Duration::from_millis(config.poll_interval_ms.clamp(100, 60_000));
        match receiver.recv_timeout(wait) {
            Ok(MonitorCommand::Stop) | Err(RecvTimeoutError::Disconnected) => {
                logger.log(
                    "monitor_stopped",
                    Some(config.brightness),
                    previous_state,
                    None,
                );
                return;
            }
            Ok(MonitorCommand::Context(message)) => logger.log(
                "windows_event",
                Some(config.brightness),
                previous_state,
                Some(&message),
            ),
            Ok(MonitorCommand::EnforceNow(message)) => logger.log(
                "enforcement_triggered",
                Some(config.brightness),
                previous_state,
                Some(&message),
            ),
            Err(RecvTimeoutError::Timeout) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::retained_complete_lines;

    #[test]
    fn log_compaction_keeps_only_complete_newest_lines() {
        let contents = b"oldest\nolder\nnewer\nnewest\n";
        assert_eq!(retained_complete_lines(contents, 15), b"newer\nnewest\n");
    }

    #[test]
    fn log_compaction_drops_an_overlong_partial_record() {
        let contents = b"short\none-record-with-no-newline";
        assert_eq!(retained_complete_lines(contents, 8), b"");
    }
}
