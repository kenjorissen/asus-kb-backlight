use std::process::ExitCode;
use std::sync::mpsc;

use asus_kbdlight::backlight::{BacklightController, Brightness};
use asus_kbdlight::config::{Config, config_path, event_log_path};
use asus_kbdlight::monitor::{self, EventLogger};
use asus_kbdlight::service;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Show the firmware-reported keyboard backlight state.
    Status {
        /// Emit machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
    /// Save and immediately apply a brightness level.
    Set {
        /// off/low/medium/high, 0-3, or on (an alias for high).
        brightness: Brightness,
    },
    /// Run the enforcing monitor in the foreground.
    Watch,
    /// Install, control, or remove the Windows boot service (run as Administrator).
    Service {
        #[command(subcommand)]
        action: ServiceAction,
    },
    /// Internal Windows Service Control Manager entry point.
    #[command(hide = true)]
    ServiceRun,
    /// Print configuration and event-log paths.
    Paths,
}

#[derive(Subcommand)]
enum ServiceAction {
    Install,
    Uninstall,
    Start,
    Stop,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let cli = Cli::parse();
    match cli.command {
        Command::Status { json } => {
            let controller = BacklightController::connect().map_err(|error| error.to_string())?;
            let state = controller.status().map_err(|error| error.to_string())?;
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&state).map_err(|error| error.to_string())?
                );
            } else {
                println!(
                    "{} (raw=0x{:08X}, control=0x{:02X})",
                    state.brightness, state.raw_status, state.control_byte
                );
            }
        }
        Command::Set { brightness } => {
            let mut config = Config::load_or_default().map_err(|error| error.to_string())?;
            config.brightness = brightness;
            config.save().map_err(|error| {
                format!(
                    "save {}: {error}; if the service is installed, retry from an elevated terminal",
                    config_path().display()
                )
            })?;
            let controller = BacklightController::connect().map_err(|error| error.to_string())?;
            let state = controller
                .set(brightness)
                .map_err(|error| error.to_string())?;
            println!(
                "set {} (raw=0x{:08X}); the service will enforce this setting",
                state.brightness, state.raw_status
            );
        }
        Command::Watch => {
            let (sender, receiver) = mpsc::channel();
            let _keepalive = sender;
            monitor::run(receiver, EventLogger::console());
        }
        Command::Service { action } => {
            match action {
                ServiceAction::Install => service::install(),
                ServiceAction::Uninstall => service::uninstall(),
                ServiceAction::Start => service::start(),
                ServiceAction::Stop => service::stop(),
            }?;
            println!("service command completed");
        }
        Command::ServiceRun => service::dispatch().map_err(|error| error.to_string())?,
        Command::Paths => {
            println!("config: {}", config_path().display());
            println!("events: {}", event_log_path().display());
        }
    }
    Ok(())
}
