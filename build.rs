//! Windows only: the icon Explorer, the desktop and shortcuts show is a resource inside the
//! .exe, not the icon the window sets while the app runs. This embeds assets/icons/app-icon.ico
//! (16 to 256 px) with the product name and version. Other systems take their icon from the
//! bundle (macOS) or the .desktop file (Linux), so nothing happens there.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=assets/icons/app-icon.ico");
    #[cfg(windows)]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icons/app-icon.ico");
        res.set("ProductName", "DevTrack");
        res.set("FileDescription", "DevTrack");
        res.set("OriginalFilename", "DevTrack.exe");
        res.compile().expect("embedding the Windows icon and version information");
    }
}
