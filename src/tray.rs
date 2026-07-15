use std::sync::Arc;
use std::thread;

use tray_icon::menu::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIconBuilder};
use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, MB_ICONINFORMATION, MB_OK, MSG, MessageBoxW, PostThreadMessageW,
    TranslateMessage, WM_QUIT,
};
use windows::core::HSTRING;

use crate::backlight::{BacklightController, Brightness};
use crate::config::Config;

pub fn run() -> Result<(), String> {
    let menu = Menu::new();
    let status = MenuItem::with_id("status", "Status...", true, None);
    let status_separator = PredefinedMenuItem::separator();
    let off = MenuItem::with_id("off", "Off", true, None);
    let low = MenuItem::with_id("low", "Low", true, None);
    let medium = MenuItem::with_id("medium", "Medium", true, None);
    let high = MenuItem::with_id("high", "High", true, None);
    let separator = PredefinedMenuItem::separator();
    let quit = MenuItem::with_id("quit", "Exit tray icon", true, None);
    menu.append_items(&[
        &status,
        &status_separator,
        &off,
        &low,
        &medium,
        &high,
        &separator,
        &quit,
    ])
    .map_err(|error| format!("create tray menu: {error}"))?;

    let _tray = TrayIconBuilder::new()
        .with_tooltip("ASUS Keyboard Backlight")
        .with_icon(make_icon()?)
        .with_menu(Box::new(menu))
        .build()
        .map_err(|error| format!("create tray icon: {error}"))?;

    let thread_id = unsafe { GetCurrentThreadId() };
    let ids = Arc::new(MenuIds {
        status: status.id().clone(),
        off: off.id().clone(),
        low: low.id().clone(),
        medium: medium.id().clone(),
        high: high.id().clone(),
        quit: quit.id().clone(),
    });
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        if event.id == ids.quit {
            let _ = unsafe { PostThreadMessageW(thread_id, WM_QUIT, WPARAM(0), LPARAM(0)) };
            return;
        }
        if event.id == ids.status {
            thread::spawn(show_status);
            return;
        }
        let brightness = if event.id == ids.off {
            Some(Brightness::Off)
        } else if event.id == ids.low {
            Some(Brightness::Low)
        } else if event.id == ids.medium {
            Some(Brightness::Medium)
        } else if event.id == ids.high {
            Some(Brightness::High)
        } else {
            None
        };
        if let Some(brightness) = brightness {
            thread::spawn(move || apply(brightness));
        }
    }));

    let mut message = MSG::default();
    loop {
        let result = unsafe { GetMessageW(&mut message, None, 0, 0) };
        if result.0 <= 0 {
            break;
        }
        unsafe {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
    Ok(())
}

struct MenuIds {
    status: MenuId,
    off: MenuId,
    low: MenuId,
    medium: MenuId,
    high: MenuId,
    quit: MenuId,
}

pub fn show_status() {
    let config_line = match Config::load_or_default() {
        Ok(config) => format!("Desired brightness: {}", config.brightness),
        Err(error) => format!("Configuration error: {error}"),
    };
    let service_line = service_status();
    let firmware_line =
        match BacklightController::connect().and_then(|controller| controller.status()) {
            Ok(state) => format!(
                "Firmware brightness: {}\nRaw status: 0x{:08X}",
                state.brightness, state.raw_status
            ),
            Err(error) => format!("Firmware status unavailable:\n{error}"),
        };
    let message = HSTRING::from(format!("{config_line}\n{service_line}\n\n{firmware_line}"));
    let title = HSTRING::from("ASUS Keyboard Backlight Status");
    unsafe {
        let _ = MessageBoxW(None, &message, &title, MB_OK | MB_ICONINFORMATION);
    }
}

fn service_status() -> String {
    match std::process::Command::new("sc.exe")
        .args(["query", crate::service::SERVICE_NAME])
        .output()
    {
        Ok(output) if output.status.success() => {
            let text = String::from_utf8_lossy(&output.stdout);
            if text.contains("RUNNING") {
                "Service: running".to_owned()
            } else if text.contains("STOPPED") {
                "Service: stopped".to_owned()
            } else {
                "Service: installed (transitional state)".to_owned()
            }
        }
        _ => "Service: not installed".to_owned(),
    }
}

fn apply(brightness: Brightness) {
    let result = (|| -> Result<(), String> {
        let mut config = Config::load_or_default().map_err(|error| error.to_string())?;
        config.brightness = brightness;
        config.save().map_err(|error| error.to_string())?;

        // Apply immediately for responsiveness. The service will retry and enforce it.
        if let Ok(controller) = BacklightController::connect() {
            let _ = controller.set(brightness);
        }
        Ok(())
    })();
    if let Err(error) = result {
        // A tray app has no console. Preserve a compact diagnostic next to the config.
        let path = crate::config::data_directory().join("tray-error.log");
        let _ = std::fs::write(path, error);
    }
}

fn make_icon() -> Result<Icon, String> {
    let rgba = include_bytes!("../assets/asus-kbdlight-32.rgba").to_vec();
    Icon::from_rgba(rgba, 32, 32).map_err(|error| format!("create tray icon pixels: {error}"))
}
