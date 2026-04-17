fn main() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "windows" {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("icons/black-icon-for-windows-only.ico");
        res.set("ProductName", "Odd Box");
        res.set("FileDescription", "a dead simple reverse proxy server and web server");
        res.set("InternalName", "odd-box");
        res.set("OriginalFilename", "odd-box.exe");
        res.set("LegalCopyright", "Copyright © Olof Blomqvist");

        if let Err(err) = res.compile() {
            panic!("failed to compile Windows resources: {err}");
        }
    }
}
