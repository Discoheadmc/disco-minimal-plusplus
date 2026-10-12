//! Windows EXE ikonunu ve sürüm bilgilerini gömer.

fn main() {
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("icons/icon.ico");

        let version = env!("CARGO_PKG_VERSION");
        res.set("CompanyName", "Discoheadmc");
        res.set("ProductName", "Disco Minimal++");
        res.set("FileDescription", "Disco Minimal++ — Minecraft launcher");
        res.set("LegalCopyright", "© 2026 Discoheadmc");
        res.set("OriginalFilename", "disco_app.exe");
        res.set("InternalName", "disco_app");
        res.set("FileVersion", version);
        res.set("ProductVersion", version);

        res.compile()
            .expect("Windows ikon kaynağı gömülemedi");
    }
    println!("cargo:rerun-if-changed=icons/icon.ico");
}
