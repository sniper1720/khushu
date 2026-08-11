use std::cell::RefCell;
use std::rc::Rc;

use adw::ActionRow;
use adw::prelude::*;
use gtk::{Box, ListBox, ListBoxRow, Orientation, SelectionMode};
use gtk4 as gtk;
use libadwaita as adw;

use crate::i18n::tr;

pub fn build_sidebar(split_view: &adw::OverlaySplitView) -> ListBox {
    let sidebar_box = Box::new(Orientation::Vertical, 0);
    sidebar_box.set_vexpand(true);

    let sidebar_list = ListBox::builder()
        .selection_mode(SelectionMode::Single)
        .css_classes(["navigation-sidebar"])
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(12)
        .margin_end(12)
        .build();

    let nav_items = vec![
        ("home", tr("Home"), "user-home-symbolic"),
        ("calendar", tr("Calendar"), "x-office-calendar-symbolic"),
        ("qibla", tr("Qibla"), "qibla-symbolic"),
        ("adkar", tr("Adkar"), "emblem-documents-symbolic"),
        ("quran", tr("Noble Quran"), "noble-quran-symbolic"),
        ("settings", tr("Settings"), "emblem-system-symbolic"),
        ("about", tr("About"), "help-about-symbolic"),
    ];

    for (id, title, icon) in nav_items {
        let row = ActionRow::builder().title(&title).build();
        let image = if icon.starts_with('/') {
            gtk::Image::from_resource(icon)
        } else {
            gtk::Image::from_icon_name(icon)
        };
        row.add_prefix(&image);

        let list_row = ListBoxRow::new();
        list_row.set_child(Some(&row));
        list_row.set_widget_name(id);

        sidebar_list.append(&list_row);
    }

    sidebar_list.select_row(sidebar_list.row_at_index(0).as_ref());

    let sidebar_scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .child(&sidebar_list)
        .build();

    sidebar_box.append(&sidebar_scroll);
    split_view.set_sidebar(Some(&sidebar_box));

    sidebar_list
}

pub fn page_title(name: &str) -> String {
    match name {
        "home" => tr("Prayer Times"),
        "calendar" => tr("Calendar"),
        "qibla" => tr("Qibla"),
        "adkar" => tr("Adkar"),
        "quran" => tr("Noble Quran"),
        "settings" => tr("Settings"),
        _ => "Khushu".to_string(),
    }
}

pub fn navigate_to(
    name: &str,
    sidebar_list: &ListBox,
    view_stack: &adw::ViewStack,
    window_title: &adw::WindowTitle,
    current_lang: &str,
    split_view: &adw::OverlaySplitView,
    config: &crate::config::AppConfig,
) {
    if name == "quran" {
        crate::quran::open_last_read_or_list(view_stack, current_lang, config.clone());
    } else {
        view_stack.set_visible_child_name(name);
    }

    if split_view.is_collapsed() {
        split_view.set_show_sidebar(false);
    }

    window_title.set_title(&page_title(name));
    if let Some(row) = sidebar_list.row_at_index(0) {
        let mut child = Some(row.upcast::<gtk::Widget>());
        while let Some(widget) = child {
            if let Some(list_row) = widget.downcast_ref::<ListBoxRow>()
                && list_row.widget_name() == name
            {
                sidebar_list.select_row(Some(list_row));
                break;
            }
            child = widget.next_sibling();
        }
    }
}

pub fn connect_sidebar_navigation(
    sidebar_list: &ListBox,
    view_stack: Rc<adw::ViewStack>,
    window_title: &adw::WindowTitle,
    current_lang: Rc<RefCell<String>>,
    split_view: &adw::OverlaySplitView,
    window: &adw::ApplicationWindow,
    config: crate::config::AppConfig,
) {
    let view_stack_clone = view_stack.clone();
    let last_valid_row = Rc::new(RefCell::new(sidebar_list.row_at_index(0)));

    let split_view_hide = split_view.clone();
    let last_valid_row_act = last_valid_row.clone();
    let current_lang_sidebar = current_lang.clone();
    let window_sidebar = window.clone();
    let window_title_sidebar = window_title.clone();

    sidebar_list.connect_row_activated(move |list, row| {
        let name = row.widget_name();
        if name == "about" {
            crate::show_about_window(&window_sidebar);
            let prev = last_valid_row_act.borrow().as_ref().cloned();
            if let Some(prev_row) = prev {
                list.select_row(Some(&prev_row));
            }
        } else if !name.is_empty() {
            *last_valid_row_act.borrow_mut() = Some(row.clone());
            navigate_to(
                &name,
                list,
                &view_stack_clone,
                &window_title_sidebar,
                &current_lang_sidebar.borrow(),
                &split_view_hide,
                &config,
            );
        }
    });

    let last_valid_row_sel = last_valid_row.clone();
    sidebar_list.connect_selected_rows_changed(move |list| {
        if let Some(row) = list.selected_row()
            && row.widget_name() != "about"
        {
            *last_valid_row_sel.borrow_mut() = Some(row);
        }
    });
}
