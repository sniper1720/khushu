use crate::platform::{is_flatpak, is_snap};
use gtk4::glib;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

fn get_autostart_path() -> PathBuf {
    let mut path = glib::user_config_dir();
    path.push("autostart");
    path.push("io.github.sniper1720.khushu.desktop");
    path
}

fn get_snap_autostart_path() -> Option<PathBuf> {
    std::env::var("SNAP_USER_DATA").ok().map(|snap_data| {
        let mut path = PathBuf::from(snap_data);
        path.push(".config/autostart/io.github.sniper1720.khushu.desktop");
        path
    })
}

fn enable_fs() {
    let path = get_autostart_path();

    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    let desktop_content = r#"[Desktop Entry]
Name=Khushu
Comment=An all-in-one Muslim app for Linux
Exec=khushu --background
Icon=io.github.sniper1720.khushu
Terminal=false
Type=Application
Categories=Utility;
Keywords=Prayer;Islam;Salah;
StartupNotify=true
"#;

    if fs::write(&path, desktop_content).is_ok() {
        if let Ok(mut perms) = fs::metadata(&path).map(|m| m.permissions()) {
            perms.set_mode(0o644);
            let _ = fs::set_permissions(&path, perms);
        }
        log::info!("Autostart enabled (filesystem): {:?}", path);
    } else {
        log::error!("Failed to create autostart desktop file at {:?}", path);
    }
}

fn disable_fs() {
    let path = get_autostart_path();
    let old_path = {
        let mut autostart_path = glib::user_config_dir();
        autostart_path.push("autostart");
        autostart_path.push("khushu.desktop");
        autostart_path
    };

    if fs::remove_file(&path).is_ok() {
        log::info!("Autostart disabled (filesystem): removed {:?}", path);
    }
    if fs::remove_file(&old_path).is_ok() {
        log::info!(
            "Legacy autostart disabled (filesystem): removed {:?}",
            old_path
        );
    }
}

fn enable_snap_autostart() {
    if let Some(path) = get_snap_autostart_path() {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        let desktop_content = r#"[Desktop Entry]
Name=Khushu
Comment=An all-in-one Muslim app for Linux
Exec=khushu --background
Icon=io.github.sniper1720.khushu
Terminal=false
Type=Application
Categories=Utility;
Keywords=Prayer;Islam;Salah;
StartupNotify=true
"#;

        if fs::write(&path, desktop_content).is_ok() {
            if let Ok(mut perms) = fs::metadata(&path).map(|m| m.permissions()) {
                perms.set_mode(0o644);
                let _ = fs::set_permissions(&path, perms);
            }
            log::info!("Autostart enabled (snap native): {:?}", path);
        } else {
            log::error!("Failed to create snap autostart desktop file at {:?}", path);
        }
    }
}

fn disable_snap_autostart() {
    if let Some(path) = get_snap_autostart_path()
        && fs::remove_file(&path).is_ok()
    {
        log::info!("Autostart disabled (snap native): removed {:?}", path);
    }
}

async fn request_portal(enable: bool) -> Result<bool, ashpd::Error> {
    use ashpd::desktop::background::Background;

    let response = Background::request()
        .reason("Allow Khushu to start automatically at login for prayer notifications.")
        .auto_start(enable)
        .dbus_activatable(false)
        .command(&["khushu", "--background"])
        .send()
        .await?
        .response()?;

    log::info!(
        "Portal autostart response: auto_start={}, background={}",
        response.auto_start(),
        response.run_in_background()
    );

    if !response.auto_start() {
        log::warn!("Portal denied autostart request (auto_start=false)");
    }

    Ok(response.auto_start())
}

fn sync_flatpak(should_enable: bool) -> glib::JoinHandle<bool> {
    glib::spawn_future_local(async move {
        match request_portal(should_enable).await {
            Ok(granted) => granted,
            Err(e) => {
                log::error!("Portal autostart failed: {e}");
                false
            }
        }
    })
}

pub fn sync(should_enable: bool) -> Option<glib::JoinHandle<bool>> {
    if is_snap() {
        if should_enable {
            enable_snap_autostart();
        } else {
            disable_snap_autostart();
        }
        None
    } else if is_flatpak() {
        Some(sync_flatpak(should_enable))
    } else if should_enable {
        enable_fs();
        None
    } else {
        disable_fs();
        None
    }
}
