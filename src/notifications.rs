use gtk::prelude::*;
use gtk4 as gtk;

fn get_notification_icon() -> gtk::gio::Icon {
    if let Ok(snap) = std::env::var("SNAP") {
        let icon_path = format!(
            "{}/usr/share/icons/hicolor/scalable/apps/io.github.sniper1720.khushu.svg",
            snap
        );
        if std::path::Path::new(&icon_path).exists() {
            let file = gtk::gio::File::for_path(&icon_path);
            let icon = gtk::gio::FileIcon::new(&file);
            return icon.upcast();
        }
    }
    gtk::gio::ThemedIcon::new("io.github.sniper1720.khushu").upcast()
}

pub fn show_notification(title: &str, body: &str, is_adhan: bool, open_lbl: &str, stop_lbl: &str) {
    if let Some(app) = gtk::gio::Application::default() {
        log::debug!("Sending notification via GApplication (Portal-compatible)");
        let notification = gtk::gio::Notification::new(title);
        notification.set_body(Some(body));
        let icon = get_notification_icon();
        notification.set_icon(&icon);
        notification.set_default_action("app.open-main");
        notification.add_button(open_lbl, "app.open-main");
        if is_adhan {
            notification.add_button(stop_lbl, "app.stop-adhan");
        }
        app.send_notification(Some("khushu-notification"), &notification);
        log::info!("Notification sent via GApplication: {}", title);
    } else {
        log::debug!("Sending notification via notify-rust (Legacy/Non-GApp fallback)");
        let title = title.to_string();
        let body = body.to_string();
        let open_lbl = open_lbl.to_string();
        let stop_lbl = stop_lbl.to_string();
        std::thread::spawn(move || {
            let mut builder = notify_rust::Notification::new();
            builder.summary(&title).body(&body).appname("Khushu");

            if let Ok(snap) = std::env::var("SNAP") {
                let icon_path = format!(
                    "{}/usr/share/icons/hicolor/scalable/apps/io.github.sniper1720.khushu.svg",
                    snap
                );
                if std::path::Path::new(&icon_path).exists() {
                    builder.icon(&icon_path);
                } else {
                    builder.icon("io.github.sniper1720.khushu");
                }
            } else {
                builder.icon("io.github.sniper1720.khushu");
            }

            builder
                .hint(notify_rust::Hint::DesktopEntry(
                    "io.github.sniper1720.khushu".to_string(),
                ))
                .action("open", &open_lbl);

            if is_adhan {
                builder.action("stop", &stop_lbl);
            }

            match builder.show() {
                Ok(handle) => {
                    log::info!("Notification sent via notify-rust: {}", title);
                    let ctx = gtk::glib::MainContext::default();
                    handle.wait_for_action(move |action| {
                        if action == "open" {
                            ctx.invoke(|| {
                                if let Some(app) = gtk::gio::Application::default() {
                                    app.activate();
                                }
                            });
                        } else if action == "stop" {
                            ctx.invoke(|| {
                                if let Some(app) = gtk::gio::Application::default() {
                                    app.activate_action("stop-adhan", None);
                                }
                            });
                        }
                    });
                }
                Err(err) => log::error!("Failed to send notification: {}", err),
            }
        });
    }
}
