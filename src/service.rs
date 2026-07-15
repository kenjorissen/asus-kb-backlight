use std::ffi::OsString;
use std::process::Command;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use windows_service::define_windows_service;
use windows_service::service::{
    ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus, ServiceType,
};
use windows_service::service_control_handler::{self, ServiceControlHandlerResult};
use windows_service::service_dispatcher;

use crate::config::{Config, config_directory, config_path, ensure_data_directories};
use crate::monitor::{self, EventLogger, MonitorCommand};

pub const SERVICE_NAME: &str = "AsusKbdLight";
pub const SERVICE_DISPLAY_NAME: &str = "ASUS Keyboard Backlight Enforcer";

define_windows_service!(ffi_service_main, service_main);

pub fn dispatch() -> windows_service::Result<()> {
    service_dispatcher::start(SERVICE_NAME, ffi_service_main)
}

fn service_main(_arguments: Vec<OsString>) {
    if let Err(error) = run_service() {
        eprintln!("service failure: {error}");
    }
}

fn run_service() -> windows_service::Result<()> {
    let (sender, receiver) = mpsc::channel();
    let control_sender = sender.clone();
    let event_handler = move |event| match event {
        ServiceControl::Stop | ServiceControl::Shutdown | ServiceControl::Preshutdown => {
            let _ = control_sender.send(MonitorCommand::Stop);
            ServiceControlHandlerResult::NoError
        }
        ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
        ServiceControl::PowerEvent(details) => {
            let _ = control_sender.send(MonitorCommand::EnforceNow(format!(
                "power event: {details:?}"
            )));
            ServiceControlHandlerResult::NoError
        }
        ServiceControl::SessionChange(details) => {
            let _ = control_sender.send(MonitorCommand::EnforceNow(format!(
                "session event: {details:?}"
            )));
            ServiceControlHandlerResult::NoError
        }
        ServiceControl::HardwareProfileChange(details) => {
            let _ = control_sender.send(MonitorCommand::EnforceNow(format!(
                "hardware profile event: {details:?}"
            )));
            ServiceControlHandlerResult::NoError
        }
        ServiceControl::ParamChange | ServiceControl::Continue => {
            let _ = control_sender.send(MonitorCommand::EnforceNow(format!(
                "service event: {event:?}"
            )));
            ServiceControlHandlerResult::NoError
        }
        _ => ServiceControlHandlerResult::NotImplemented,
    };

    let status_handle = service_control_handler::register(SERVICE_NAME, event_handler)?;
    status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Running,
        controls_accepted: ServiceControlAccept::STOP
            | ServiceControlAccept::SHUTDOWN
            | ServiceControlAccept::PRESHUTDOWN
            | ServiceControlAccept::POWER_EVENT
            | ServiceControlAccept::SESSION_CHANGE
            | ServiceControlAccept::HARDWARE_PROFILE_CHANGE
            | ServiceControlAccept::PARAM_CHANGE,
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    })?;

    let logger = EventLogger::file(false).unwrap_or_else(|_| EventLogger::console());
    monitor::run(receiver, logger);

    status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Stopped,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    })
}

pub fn install() -> Result<(), String> {
    ensure_data_directories().map_err(|error| format!("create data directory: {error}"))?;
    if !config_path().exists() {
        Config::default()
            .save()
            .map_err(|error| format!("write default configuration: {error}"))?;
    }

    let executable =
        std::env::current_exe().map_err(|error| format!("locate current executable: {error}"))?;
    let binary_path = format!("\"{}\" service-run", executable.display());

    if service_exists() {
        let _ = stop();
        run_sc(&[
            "config",
            SERVICE_NAME,
            "binPath=",
            &binary_path,
            "start=",
            "auto",
            "DisplayName=",
            SERVICE_DISPLAY_NAME,
        ])?;
    } else {
        run_sc(&[
            "create",
            SERVICE_NAME,
            "binPath=",
            &binary_path,
            "start=",
            "auto",
            "DisplayName=",
            SERVICE_DISPLAY_NAME,
        ])?;
    }
    run_sc(&[
        "description",
        SERVICE_NAME,
        "Keeps the ASUS keyboard backlight at the configured brightness and logs state changes.",
    ])?;
    run_sc(&[
        "failure",
        SERVICE_NAME,
        "reset=",
        "86400",
        "actions=",
        "restart/5000/restart/15000/restart/60000",
    ])?;

    let directory = config_directory().to_string_lossy().into_owned();
    let acl_status = Command::new("icacls.exe")
        .args([&directory, "/grant", "*S-1-5-32-545:(OI)(CI)(M)"])
        .status()
        .map_err(|error| format!("start icacls: {error}"))?;
    if !acl_status.success() {
        return Err(format!("icacls failed with {acl_status}"));
    }

    run_sc(&["start", SERVICE_NAME])
}

pub fn uninstall() -> Result<(), String> {
    let _ = stop();
    run_sc(&["delete", SERVICE_NAME])
}

pub fn start() -> Result<(), String> {
    run_sc(&["start", SERVICE_NAME])
}

pub fn stop() -> Result<(), String> {
    if service_is_stopped() {
        return Ok(());
    }
    run_sc(&["stop", SERVICE_NAME])?;
    for _ in 0..100 {
        if service_is_stopped() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(100));
    }
    Err(format!(
        "service {SERVICE_NAME} did not stop within 10 seconds"
    ))
}

fn run_sc(arguments: &[&str]) -> Result<(), String> {
    let output = Command::new("sc.exe")
        .args(arguments)
        .output()
        .map_err(|error| format!("start sc.exe: {error}"))?;
    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        Err(format!(
            "sc.exe {} failed with {}: {}{}",
            arguments.join(" "),
            output.status,
            stdout,
            stderr
        ))
    }
}

fn service_exists() -> bool {
    Command::new("sc.exe")
        .args(["query", SERVICE_NAME])
        .output()
        .is_ok_and(|output| output.status.success())
}

fn service_is_stopped() -> bool {
    Command::new("sc.exe")
        .args(["query", SERVICE_NAME])
        .output()
        .is_ok_and(|output| {
            output.status.success() && String::from_utf8_lossy(&output.stdout).contains("STOPPED")
        })
}
