use adw::prelude::*;
use ksni::{MenuItem, Tray};
use libadwaita as adw;
use std::collections::HashMap;

struct KhushuTray {
    open_label: String,
    quit_label: String,
}

impl Tray for KhushuTray {
    fn icon_name(&self) -> String {
        "com.github.sniper1720.khushu-symbolic".into()
    }

    fn id(&self) -> String {
        "com.github.sniper1720.khushu".into()
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        Vec::new()
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        gtk4::glib::idle_add(move || {
            if let Some(app) = gtk4::gio::Application::default() {
                app.activate();
            }
            gtk4::glib::ControlFlow::Break
        });
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        use ksni::menu::*;
        vec![
            StandardItem {
                label: self.open_label.clone(),
                activate: Box::new(|_this: &mut Self| {
                    gtk4::glib::idle_add(move || {
                        if let Some(app) = gtk4::gio::Application::default() {
                            app.activate();
                        }
                        gtk4::glib::ControlFlow::Break
                    });
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: self.quit_label.clone(),
                activate: Box::new(|_this: &mut Self| {
                    gtk4::glib::idle_add(move || {
                        if let Some(app) = gtk4::gio::Application::default() {
                            use gtk4::prelude::*;
                            app.quit();
                        }
                        gtk4::glib::ControlFlow::Break
                    });
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}

async fn request_background_portal() -> Result<bool, zbus::Error> {
    log::info!("Attempting to request background portal...");
    let connection = zbus::Connection::session().await?;
    let proxy = zbus::Proxy::new(
        &connection,
        "org.freedesktop.portal.Desktop",
        "/org/freedesktop/portal/desktop",
        "org.freedesktop.portal.Background",
    )
    .await?;

    let mut options = HashMap::new();
    options.insert(
        "reason",
        zbus::zvariant::Value::from(
            "Khushu needs to run in the background to send prayer time notifications",
        ),
    );
    options.insert("autostart", zbus::zvariant::Value::from(true));
    options.insert(
        "commandline",
        zbus::zvariant::Value::from(vec!["khushu", "--background"]),
    );

    let _response_path: zbus::zvariant::OwnedObjectPath =
        proxy.call("RequestBackground", &("", options)).await?;

    log::info!("Background portal requested successfully.");
    Ok(true)
}

async fn setup_tray_icon() {
    use ksni::TrayMethods;
    use std::sync::atomic::{AtomicBool, Ordering};

    static TRAY_SPAWNED: AtomicBool = AtomicBool::new(false);
    if TRAY_SPAWNED.swap(true, Ordering::SeqCst) {
        return;
    }

    log::info!("Setting up KSNI fallback tray icon...");

    let lang = std::env::var("LANGUAGE").unwrap_or_default();
    let lang_ref = if lang.is_empty() { "en" } else { &lang };
    let tray = KhushuTray {
        open_label: crate::i18n::tr("Open Khushu", lang_ref),
        quit_label: crate::i18n::tr("Quit", lang_ref),
    };

    let is_sandboxed = std::path::Path::new("/.flatpak-info").exists();

    match tray.disable_dbus_name(is_sandboxed).spawn().await {
        Ok(handle) => {
            tokio::spawn(async move {
                let _h = handle;
                std::future::pending::<()>().await;
            });
        }
        Err(e) => {
            log::error!("Failed to spawn KSNI tray icon: {}", e);
        }
    }
}

pub fn setup_background() {
    gtk4::glib::spawn_future_local(async move {
        if request_background_portal().await.is_err() {
            log::info!("Background portal unavailable or failed.");
        }

        setup_tray_icon().await;
    });
}
