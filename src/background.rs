use crate::i18n::tr;
use crate::platform::{get_flatpak_tray_icon_path, is_flatpak, is_sandboxed, is_snap};
use adw::prelude::*;
use ksni::{MenuItem, Tray};
use libadwaita as adw;
use std::sync::{Arc, OnceLock, RwLock};

struct KhushuTrayData {
    open_label: String,
    quit_label: String,
}

struct KhushuTray {
    data: Arc<RwLock<KhushuTrayData>>,
}

static TRAY_DATA: OnceLock<Arc<RwLock<KhushuTrayData>>> = OnceLock::new();
static TRAY_HANDLE: OnceLock<ksni::Handle<KhushuTray>> = OnceLock::new();

fn get_tray_data() -> Arc<RwLock<KhushuTrayData>> {
    TRAY_DATA
        .get_or_init(|| {
            Arc::new(RwLock::new(KhushuTrayData {
                open_label: String::new(),
                quit_label: String::new(),
            }))
        })
        .clone()
}

fn get_flatpak_tray_icon() -> Option<std::path::PathBuf> {
    if !is_flatpak() {
        return None;
    }
    get_flatpak_tray_icon_path()
}

impl Tray for KhushuTray {
    fn icon_name(&self) -> String {
        if let Ok(snap) = std::env::var("SNAP") {
            let svg_path = format!("{snap}/meta/gui/io.github.sniper1720.khushu-symbolic.svg");
            if std::path::Path::new(&svg_path).exists() {
                return svg_path;
            }
        }
        if let Some(path) = get_flatpak_tray_icon() {
            return path.to_string_lossy().to_string();
        }
        "io.github.sniper1720.khushu".into()
    }

    fn icon_theme_path(&self) -> String {
        if is_snap() {
            return format!(
                "{}/usr/share/icons",
                std::env::var("SNAP").expect("SNAP env set by snap runtime")
            );
        }
        if is_flatpak() {
            return "/app/share/icons".to_string();
        }
        if std::path::Path::new("/usr/share/icons").exists() {
            return "/usr/share/icons".to_string();
        }
        String::new()
    }

    fn id(&self) -> String {
        "io.github.sniper1720.khushu".into()
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
        let data = self.data.read().expect("KhushuTray data lock poisoned");
        vec![
            StandardItem {
                label: data.open_label.clone(),
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
                label: data.quit_label.clone(),
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

async fn setup_tray_icon() {
    use ksni::TrayMethods;
    use std::sync::atomic::{AtomicBool, Ordering};

    static TRAY_SPAWNED: AtomicBool = AtomicBool::new(false);
    if TRAY_HANDLE.get().is_some() {
        return;
    }
    if TRAY_SPAWNED
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return;
    }

    let lang = std::env::var("LANGUAGE").unwrap_or_default();
    let lang_ref = if lang.is_empty() { "en" } else { &lang };

    let data = get_tray_data();
    {
        let mut d = data.write().expect("KhushuTray data lock poisoned");
        d.open_label = tr("Open Khushu", lang_ref);
        d.quit_label = tr("Quit", lang_ref);
    }

    let tray = KhushuTray { data };

    let is_sandboxed = is_sandboxed();

    let tray_builder = tray
        .disable_dbus_name(is_sandboxed)
        .assume_sni_available(is_sandboxed);

    match tray_builder.spawn().await {
        Ok(handle) => {
            let _ = TRAY_HANDLE.set(handle);
        }
        Err(e) => {
            TRAY_SPAWNED.store(false, Ordering::SeqCst);
            log::warn!(
                "System tray unavailable: {} \
                 (requires org.kde.StatusNotifierWatcher on the session bus; \
                 this is expected in sandboxed or minimal desktop environments \
                 and does not affect functionality)",
                e
            );
        }
    }
}

pub fn update_tray_labels(lang: &str) {
    let data = get_tray_data();
    {
        let mut d = data.write().expect("KhushuTray data lock poisoned");
        d.open_label = tr("Open Khushu", lang);
        d.quit_label = tr("Quit", lang);
    }

    if let Some(handle) = TRAY_HANDLE.get().cloned() {
        gtk4::glib::spawn_future_local(async move {
            let _ = handle.update(|_| {}).await;
        });
    }
}

pub fn setup_background() {
    gtk4::glib::spawn_future_local(async move {
        setup_tray_icon().await;
    });
}
