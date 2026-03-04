use gtk::prelude::*;
use gtk4 as gtk;

pub fn show_notification(title: &str, body: &str, is_adhan: bool, open_lbl: &str, stop_lbl: &str) {
    if let Some(app) = gtk::gio::Application::default() {
        log::debug!("Sending notification via GApplication (Portal-compatible)");
        let notification = gtk::gio::Notification::new(title);
        notification.set_body(Some(body));
        let icon = gtk::gio::ThemedIcon::new("com.github.sniper1720.khushu");
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
            builder
                .summary(&title)
                .body(&body)
                .appname("Khushu")
                .icon("com.github.sniper1720.khushu")
                .hint(notify_rust::Hint::DesktopEntry(
                    "com.github.sniper1720.khushu".to_string(),
                ))
                .action("open", &open_lbl);

            if is_adhan {
                builder.action("stop", &stop_lbl);
            }

            match builder.show() {
                Ok(handle) => {
                    log::info!("Notification sent via notify-rust: {}", title);
                    handle.wait_for_action(|action| {
                        if action == "open" {
                            gtk::glib::idle_add_local(|| {
                                if let Some(app) = gtk::gio::Application::default() {
                                    app.activate();
                                }
                                gtk::glib::ControlFlow::Break
                            });
                        } else if action == "stop" {
                            gtk::glib::idle_add_local(|| {
                                if let Some(app) = gtk::gio::Application::default() {
                                    app.activate_action("stop-adhan", None);
                                }
                                gtk::glib::ControlFlow::Break
                            });
                        }
                    });
                }
                Err(e) => log::error!("Failed to send notification: {}", e),
            }
        });
    }
}
