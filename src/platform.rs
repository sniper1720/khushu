use std::path::PathBuf;
use std::sync::OnceLock;

static FLATPAK_ICON: OnceLock<Option<PathBuf>> = OnceLock::new();

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

pub fn get_flatpak_tray_icon_path() -> Option<PathBuf> {
    FLATPAK_ICON
        .get_or_init(|| {
            let runtime_dir = std::env::var("XDG_RUNTIME_DIR").ok()?;
            let tray_dir = PathBuf::from(&runtime_dir).join("tray-icon");
            let icon_path = tray_dir.join("io.github.sniper1720.khushu.svg");

            if icon_path.exists() {
                return Some(icon_path);
            }

            if std::fs::create_dir_all(&tray_dir).is_err() {
                return None;
            }

            let source =
                "/app/share/icons/hicolor/symbolic/apps/io.github.sniper1720.khushu-symbolic.svg";
            if let Ok(data) = std::fs::read(source)
                && std::fs::write(&icon_path, &data).is_ok()
            {
                return Some(icon_path);
            }

            None
        })
        .clone()
}
