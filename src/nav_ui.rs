use std::cell::RefCell;
use std::rc::Rc;

use adw::ActionRow;
use adw::prelude::*;
use gtk::{Box, Label, ListBox, ListBoxRow, Orientation, SelectionMode};
use gtk4 as gtk;
use libadwaita as adw;

use crate::i18n::tr;

pub fn build_sidebar(
    split_view: &adw::OverlaySplitView,
    current_lang: &Rc<RefCell<String>>,
) -> ListBox {
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
        (
            "home",
            tr("Home", &current_lang.borrow()),
            "user-home-symbolic",
        ),
        (
            "calendar",
            tr("Calendar", &current_lang.borrow()),
            "x-office-calendar-symbolic",
        ),
        (
            "qibla",
            tr("Qibla", &current_lang.borrow()),
            "qibla-symbolic",
        ),
        (
            "adkar",
            tr("Adkar", &current_lang.borrow()),
            "emblem-documents-symbolic",
        ),
        (
            "settings",
            tr("Settings", &current_lang.borrow()),
            "emblem-system-symbolic",
        ),
        (
            "about",
            tr("About", &current_lang.borrow()),
            "help-about-symbolic",
        ),
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

pub fn connect_sidebar_navigation(
    sidebar_list: &ListBox,
    view_stack: Rc<adw::ViewStack>,
    window_title: &Label,
    current_lang: Rc<RefCell<String>>,
    split_view: &adw::OverlaySplitView,
    window: &adw::ApplicationWindow,
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
            crate::show_about_window(&window_sidebar, &current_lang_sidebar.borrow());
            let prev = last_valid_row_act.borrow().as_ref().cloned();
            if let Some(p) = prev {
                list.select_row(Some(&p));
            }
        } else if !name.is_empty() {
            *last_valid_row_act.borrow_mut() = Some(row.clone());
            view_stack_clone.set_visible_child_name(&name);

            if split_view_hide.is_collapsed() {
                split_view_hide.set_show_sidebar(false);
            }

            let lang = current_lang_sidebar.borrow();
            let title = match name.as_str() {
                "home" => tr("Prayer Times", &lang),
                "calendar" => tr("Calendar", &lang),
                "qibla" => tr("Qibla", &lang),
                "adkar" => tr("Adkar", &lang),
                "settings" => tr("Settings", &lang),
                _ => "Khushu".to_string(),
            };
            window_title_sidebar.set_label(&title);
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
