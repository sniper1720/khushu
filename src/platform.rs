pub fn is_flatpak() -> bool {
    std::path::Path::new("/app/.flatpak-info").exists()
        || std::env::var_os("FLATPAK_ID").is_some()
        || std::env::var("container").as_deref() == Ok("flatpak")
}

pub fn is_snap() -> bool {
    std::env::var_os("SNAP").is_some()
}

pub fn is_sandboxed() -> bool {
    is_flatpak() || is_snap()
}
