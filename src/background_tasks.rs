use std::cell::RefCell;
use std::rc::Rc;

use adw::Application;
use adw::ApplicationWindow;
use gtk4 as gtk;
use gtk4::prelude::*;
use libadwaita as adw;

pub fn start_background_tasks(
    _app: &Application,
    window: &ApplicationWindow,
    view_stack: Rc<adw::ViewStack>,
    refresh_qibla: Rc<dyn Fn()>,
) {
    let compass_paused = Rc::new(RefCell::new(true));
    let refresh_qibla_loop = refresh_qibla.clone();
    let compass_paused_timer = compass_paused.clone();
    let window_visibility_check = window.clone();

    gtk::glib::timeout_add_local(std::time::Duration::from_millis(16), move || {
        if !*compass_paused_timer.borrow() && window_visibility_check.is_visible() {
            refresh_qibla_loop();
        }
        gtk::glib::ControlFlow::Continue
    });

    let compass_paused_notify = compass_paused.clone();
    view_stack.connect_notify_local(Some("visible-child-name"), move |stack, _| {
        let is_qibla_visible = stack
            .visible_child_name()
            .map(|name| name.as_str() == "qibla")
            .unwrap_or(false);
        *compass_paused_notify.borrow_mut() = !is_qibla_visible;
    });
}
