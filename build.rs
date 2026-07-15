fn main() {
    println!("cargo:rerun-if-changed=assets/asus-kbdlight.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut resource = winresource::WindowsResource::new();
        resource
            .set_icon("assets/asus-kbdlight.ico")
            .set("FileDescription", "ASUS Keyboard Backlight")
            .set("ProductName", "ASUS Keyboard Backlight");
        resource.compile().expect("compile Windows icon resource");
    }
}
