#![windows_subsystem = "windows"]

fn main() {
    if std::env::args().nth(1).as_deref() == Some("status") {
        asus_kbdlight::tray::show_status();
    } else {
        let _ = asus_kbdlight::tray::run();
    }
}
