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

fn is_sandboxed() -> bool {
    std::path::Path::new("/.flatpak-info").exists() || std::env::var_os("SNAP").is_some()
}

fn enable_fs() {
    let path = get_autostart_path();

    if let Some(parent) = path.parent().filter(|p| !p.exists()) {
        let _ = fs::create_dir_all(parent);
    }

    let desktop_content = r#"[Desktop Entry]
Name=Khushu
Comment=An all-in-one Muslim app for Linux.
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
        let mut p = glib::user_config_dir();
        p.push("autostart");
        p.push("khushu.desktop");
        p
    };

    if path.exists() && fs::remove_file(&path).is_ok() {
        log::info!("Autostart disabled (filesystem): removed {:?}", path);
    }
    if old_path.exists() && fs::remove_file(&old_path).is_ok() {
        log::info!(
            "Legacy autostart disabled (filesystem): removed {:?}",
            old_path
        );
    }
}

async fn request_portal(enable: bool) -> Result<(), ashpd::Error> {
    use ashpd::desktop::background::Background;

    let response = Background::request()
        .reason("Allow Khushu to start automatically at login for prayer notifications.")
        .auto_start(enable)
        .dbus_activatable(false)
        .send()
        .await?
        .response()?;

    log::info!(
        "Portal autostart response: auto_start={}, background={}",
        response.auto_start(),
        response.run_in_background()
    );
    Ok(())
}

pub fn sync(should_enable: bool) {
    if is_sandboxed() {
        glib::spawn_future_local(async move {
            if let Err(e) = request_portal(should_enable).await {
                log::warn!("Portal autostart failed: {e}, falling back to filesystem");
                if should_enable {
                    enable_fs();
                } else {
                    disable_fs();
                }
            }
        });
    } else if should_enable {
        enable_fs();
    } else {
        disable_fs();
    }
}
