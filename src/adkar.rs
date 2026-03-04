use crate::i18n::tr;
use gtk::prelude::*;
use gtk4 as gtk;
use libadwaita as adw;
use std::rc::Rc;

use rand::seq::SliceRandom;
use serde::Deserialize;

#[derive(Clone, Deserialize)]
pub struct Dikr {
    pub category: String,
    pub count: u32,
    pub arabic: String,
    pub translation: String,
    pub reference: String,
}

use std::sync::OnceLock;

static ADKAR_CACHE: OnceLock<Vec<Dikr>> = OnceLock::new();

pub fn load_adkar() -> Vec<Dikr> {
    ADKAR_CACHE
        .get_or_init(|| {
            if let Ok(bytes) = gtk::gio::resources_lookup_data(
                "/com/github/sniper1720/khushu/adkar.json",
                gtk::gio::ResourceLookupFlags::NONE,
            ) {
                if let Ok(content) = std::str::from_utf8(&bytes) {
                    if let Ok(adkar) = serde_json::from_str::<Vec<Dikr>>(content) {
                        return adkar;
                    } else {
                        log::error!("Failed to deserialize Adkar JSON from GResource");
                    }
                } else {
                    log::error!("Adkar GResource was not valid UTF-8");
                }
            } else {
                log::error!("Failed to locate adkar.json inside the compiled binary resources");
            }
            vec![]
        })
        .clone()
}

pub fn get_morning_adkar() -> Vec<Dikr> {
    load_adkar()
        .into_iter()
        .filter(|d| d.category == "morning")
        .collect()
}

pub fn get_evening_adkar() -> Vec<Dikr> {
    load_adkar()
        .into_iter()
        .filter(|d| d.category == "evening")
        .collect()
}

pub fn get_night_adkar() -> Vec<Dikr> {
    load_adkar()
        .into_iter()
        .filter(|d| d.category == "night")
        .collect()
}

pub fn get_n_random_dikrs(category: &str, n: usize) -> Vec<Dikr> {
    let adkars: Vec<Dikr> = load_adkar()
        .into_iter()
        .filter(|d| d.category == category)
        .collect();

    let mut rng = rand::thread_rng();
    adkars.choose_multiple(&mut rng, n).cloned().collect()
}

use crate::config::AppConfig;
use std::cell::RefCell;

pub fn create_adkar_page(config: Rc<RefCell<AppConfig>>) -> (gtk::Box, Rc<dyn Fn()>) {
    let container = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let stack = adw::ViewStack::new();
    stack.set_hhomogeneous(false);
    stack.set_vhomogeneous(false);

    let switcher = adw::ViewSwitcher::new();
    switcher.set_stack(Some(&stack));
    switcher.set_halign(gtk::Align::Center);
    switcher.set_margin_top(6);
    switcher.set_margin_bottom(6);

    let switcher_clamp = adw::Clamp::builder()
        .child(&switcher)
        .maximum_size(340)
        .build();
    container.append(&switcher_clamp);

    let morning_scroll = gtk::ScrolledWindow::builder()
        .vexpand(true)
        .propagate_natural_width(false)
        .propagate_natural_height(false)
        .build();
    let morning_box = gtk::ListBox::new();
    morning_box.add_css_class("boxed-list");
    morning_box.set_selection_mode(gtk::SelectionMode::None);
    morning_box.set_margin_top(12);
    morning_box.set_margin_bottom(12);
    morning_box.set_margin_start(12);
    morning_box.set_margin_end(12);

    let morning_clamp = adw::Clamp::builder()
        .maximum_size(800)
        .tightening_threshold(600)
        .child(&morning_box)
        .build();
    morning_scroll.set_child(Some(&morning_clamp));

    let evening_scroll = gtk::ScrolledWindow::builder()
        .vexpand(true)
        .propagate_natural_width(false)
        .propagate_natural_height(false)
        .build();
    let evening_box = gtk::ListBox::new();
    evening_box.add_css_class("boxed-list");
    evening_box.set_selection_mode(gtk::SelectionMode::None);
    evening_box.set_margin_top(12);
    evening_box.set_margin_bottom(12);
    evening_box.set_margin_start(12);
    evening_box.set_margin_end(12);

    let evening_clamp = adw::Clamp::builder()
        .maximum_size(800)
        .tightening_threshold(600)
        .child(&evening_box)
        .build();
    evening_scroll.set_child(Some(&evening_clamp));

    let night_scroll = gtk::ScrolledWindow::builder()
        .vexpand(true)
        .propagate_natural_width(false)
        .propagate_natural_height(false)
        .build();
    let night_box = gtk::ListBox::new();
    night_box.add_css_class("boxed-list");
    night_box.set_selection_mode(gtk::SelectionMode::None);
    night_box.set_margin_top(12);
    night_box.set_margin_bottom(12);
    night_box.set_margin_start(12);
    night_box.set_margin_end(12);

    let night_clamp = adw::Clamp::builder()
        .maximum_size(800)
        .tightening_threshold(600)
        .child(&night_box)
        .build();
    night_scroll.set_child(Some(&night_clamp));

    let morning_box_rc = Rc::new(morning_box);
    let evening_box_rc = Rc::new(evening_box);
    let night_box_rc = Rc::new(night_box);

    let morning_box_clone = morning_box_rc.clone();
    let evening_box_clone = evening_box_rc.clone();
    let night_box_clone = night_box_rc.clone();

    let rebuild_lists = Rc::new(move |config_ref: Rc<RefCell<AppConfig>>| {
        let m_box = &*morning_box_clone;
        let e_box = &*evening_box_clone;
        let n_box = &*night_box_clone;

        while let Some(child) = m_box.first_child() {
            m_box.remove(&child);
        }
        while let Some(child) = e_box.first_child() {
            e_box.remove(&child);
        }
        while let Some(child) = n_box.first_child() {
            n_box.remove(&child);
        }

        let favs = config_ref.borrow().favorites.clone();
        let all_boxes = [m_box.clone(), e_box.clone(), n_box.clone()];

        for dikr in get_morning_adkar() {
            let id = dikr.arabic.trim().to_string();
            m_box.append(&create_dikr_row(
                &dikr,
                favs.contains(&id),
                config_ref.clone(),
                all_boxes.clone(),
            ));
        }
        for dikr in get_evening_adkar() {
            let id = dikr.arabic.trim().to_string();
            e_box.append(&create_dikr_row(
                &dikr,
                favs.contains(&id),
                config_ref.clone(),
                all_boxes.clone(),
            ));
        }
        for dikr in get_night_adkar() {
            let id = dikr.arabic.trim().to_string();
            n_box.append(&create_dikr_row(
                &dikr,
                favs.contains(&id),
                config_ref.clone(),
                all_boxes.clone(),
            ));
        }
    });

    rebuild_lists(config.clone());

    let initial_lang = config.borrow().language.clone();
    let morning_page = stack.add_titled(
        &morning_scroll,
        Some("morning"),
        &tr("Morning", &initial_lang),
    );
    morning_page.set_icon_name(Some("weather-clear-symbolic"));

    let evening_page = stack.add_titled(
        &evening_scroll,
        Some("evening"),
        &tr("Evening", &initial_lang),
    );
    evening_page.set_icon_name(Some("weather-few-clouds-night-symbolic"));

    let night_page = stack.add_titled(&night_scroll, Some("night"), &tr("Night", &initial_lang));
    night_page.set_icon_name(Some("weather-clear-night-symbolic"));

    container.append(&stack);

    let rebuild_lists_refresh = rebuild_lists.clone();
    let config_refresh = config.clone();
    let refresh_ui = Rc::new(move || {
        let lang = config_refresh.borrow().language.clone();
        morning_page.set_title(Some(&tr("Morning", &lang)));
        evening_page.set_title(Some(&tr("Evening", &lang)));
        night_page.set_title(Some(&tr("Night", &lang)));
        rebuild_lists_refresh(config_refresh.clone());
    });

    (container, refresh_ui)
}

fn create_dikr_row(
    dikr: &Dikr,
    is_favorite: bool,
    config: Rc<RefCell<AppConfig>>,
    all_lists: [gtk::ListBox; 3],
) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    row.add_css_class("activatable");

    let vbox = gtk::Box::new(gtk::Orientation::Vertical, 6);
    vbox.set_margin_top(12);
    vbox.set_margin_bottom(12);
    vbox.set_margin_start(12);
    vbox.set_margin_end(12);

    let top_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    spacer.set_hexpand(true);

    let fav_btn = gtk::Button::builder()
        .icon_name("user-bookmarks-symbolic")
        .has_frame(false)
        .build();
    if is_favorite {
        fav_btn.add_css_class("accent");
    }
    fav_btn.set_widget_name(dikr.arabic.trim());

    let dikr_id = dikr.arabic.trim().to_string();
    let config_fav = config.clone();
    let all_lists_clone = all_lists.clone();
    let dikr_id_signal = dikr_id.clone();

    fav_btn.connect_clicked(move |_btn| {
        let mut cfg = config_fav.borrow_mut();
        let currently_fav = cfg.favorites.contains(&dikr_id_signal);
        if currently_fav {
            cfg.favorites.retain(|x| x != &dikr_id_signal);
        } else {
            cfg.favorites.push(dikr_id_signal.clone());
        }
        cfg.save();

        let new_fav_status = !currently_fav;
        for list in &all_lists_clone {
            let mut curr = list.first_child();
            while let Some(row_child) = curr {
                if let Some(lb_row) = row_child.downcast_ref::<gtk::ListBoxRow>()
                    && let Some(v_box) = lb_row.child().and_then(|c| c.downcast::<gtk::Box>().ok())
                    && let Some(t_box) = v_box
                        .first_child()
                        .and_then(|c| c.downcast::<gtk::Box>().ok())
                    && let Some(target_btn) = t_box
                        .last_child()
                        .and_then(|c| c.downcast::<gtk::Button>().ok())
                    && target_btn.widget_name() == dikr_id_signal
                {
                    if new_fav_status {
                        target_btn.add_css_class("accent");
                    } else {
                        target_btn.remove_css_class("accent");
                    }
                }
                curr = row_child.next_sibling();
            }
        }
    });

    top_box.append(&spacer);
    top_box.append(&fav_btn);
    vbox.append(&top_box);

    let lbl_arabic = gtk::Label::builder()
        .label(&dikr.arabic)
        .wrap(true)
        .justify(gtk::Justification::Center)
        .build();
    let attrs = gtk::pango::AttrList::new();
    let font_desc = gtk::pango::FontDescription::from_string("Amiri, Amiri-Regular 22");
    attrs.insert(gtk::pango::AttrFontDesc::new(&font_desc));
    lbl_arabic.set_attributes(Some(&attrs));

    let lbl_trans = gtk::Label::builder()
        .label(&dikr.translation)
        .wrap(true)
        .justify(gtk::Justification::Center)
        .css_classes(["caption"])
        .build();

    let hbox_meta = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    hbox_meta.set_halign(gtk::Align::Center);
    hbox_meta.append(
        &gtk::Label::builder()
            .label(format!("{}x", dikr.count))
            .css_classes(["numeric", "badge"])
            .build(),
    );
    hbox_meta.append(
        &gtk::Label::builder()
            .label(&dikr.reference)
            .css_classes(["dim-label", "caption-heading"])
            .build(),
    );

    vbox.append(&lbl_arabic);
    if config.borrow().language != "ar" {
        vbox.append(&lbl_trans);
    }
    vbox.append(&hbox_meta);

    row.set_child(Some(&vbox));
    row
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn morning_adkar_have_correct_category() {
        let items = get_morning_adkar();
        if items.is_empty() {
            return;
        }
        for d in &items {
            assert_eq!(
                d.category, "morning",
                "Expected 'morning', got '{}'",
                d.category
            );
        }
    }

    #[test]
    fn evening_adkar_have_correct_category() {
        let items = get_evening_adkar();
        if items.is_empty() {
            return;
        }
        for d in &items {
            assert_eq!(d.category, "evening");
        }
    }

    #[test]
    fn night_adkar_have_correct_category() {
        let items = get_night_adkar();
        if items.is_empty() {
            return;
        }
        for d in &items {
            assert_eq!(d.category, "night");
        }
    }

    #[test]
    fn random_morning_dikr_returns_some_when_data_exists() {
        let items = get_morning_adkar();
        if items.is_empty() {
            return;
        }
        let pick = get_n_random_dikrs("morning", 1);
        assert!(
            !pick.is_empty(),
            "Random pick should return elements when data exists"
        );
    }

    #[test]
    fn adkar_entries_have_non_empty_fields() {
        let all = load_adkar();
        if all.is_empty() {
            return;
        }
        for d in &all {
            assert!(!d.arabic.trim().is_empty(), "Arabic text must not be empty");
            assert!(
                !d.translation.trim().is_empty(),
                "Translation must not be empty"
            );
            assert!(
                !d.reference.trim().is_empty(),
                "Reference must not be empty"
            );
            assert!(d.count > 0, "Count must be positive");
        }
    }
}
