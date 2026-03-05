use crate::config::{AppConfig, LocationMode, ThemeMode};
use crate::i18n::tr;
use crate::location;
use adw::prelude::*;
use adw::{ActionRow, Application, ApplicationWindow, ComboRow, EntryRow, PreferencesGroup};
use gtk::{Button, Orientation};
use gtk4 as gtk;
use libadwaita as adw;
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Clone, Debug)]
enum LocationState {
    Initial,
    Searching,
    Success(String, f64, f64),
    Error(String),
}

pub fn build_welcome_window<F>(app: &Application, config: Rc<RefCell<AppConfig>>, on_done: F)
where
    F: Fn() + 'static,
{
    let current_lang = Rc::new(RefCell::new(config.borrow().language.clone()));
    let location_state = Rc::new(RefCell::new(LocationState::Initial));

    let window = ApplicationWindow::builder()
        .application(app)
        .title("Welcome to Khushu")
        .default_width(600)
        .default_height(650)
        .build();

    let content_box = gtk::Box::new(Orientation::Vertical, 0);

    let header_bar = adw::HeaderBar::new();
    header_bar.set_show_end_title_buttons(true);

    let toolbar_view = adw::ToolbarView::new();
    toolbar_view.add_top_bar(&header_bar);
    toolbar_view.set_content(Some(&content_box));

    window.set_content(Some(&toolbar_view));

    let status_page = adw::StatusPage::builder()
        .title("Welcome to Khushu")
        .description("Please configure your location to get accurate prayer times.")
        .icon_name("io.github.sniper1720.khushu")
        .vexpand(true)
        .build();

    let settings_container = gtk::Box::new(Orientation::Vertical, 0);
    settings_container.set_margin_top(24);
    settings_container.set_margin_bottom(24);
    settings_container.set_margin_start(12);
    settings_container.set_margin_end(12);
    settings_container.set_spacing(12);

    let clamp = adw::Clamp::builder()
        .maximum_size(500)
        .child(&settings_container)
        .build();

    status_page.set_child(Some(&clamp));
    content_box.append(&status_page);

    let appearance_group = PreferencesGroup::builder().title("Appearance").build();
    settings_container.append(&appearance_group);

    let theme_model = gtk::StringList::new(&["System Default", "Light", "Dark"]);

    let theme_row = ComboRow::builder()
        .title("Theme")
        .model(&theme_model)
        .build();

    match config.borrow().theme {
        ThemeMode::Light => theme_row.set_selected(1),
        ThemeMode::Dark => theme_row.set_selected(2),
        ThemeMode::System => theme_row.set_selected(0),
    }

    appearance_group.add(&theme_row);

    let config_theme = config.clone();
    theme_row.connect_selected_notify(move |row| {
        let manager = adw::StyleManager::default();
        let theme = match row.selected() {
            1 => ThemeMode::Light,
            2 => ThemeMode::Dark,
            _ => ThemeMode::System,
        };

        match theme {
            ThemeMode::Light => manager.set_color_scheme(adw::ColorScheme::ForceLight),
            ThemeMode::Dark => manager.set_color_scheme(adw::ColorScheme::PreferDark),
            ThemeMode::System => manager.set_color_scheme(adw::ColorScheme::Default),
        }

        config_theme.borrow_mut().theme = theme;
    });

    let lang_group = PreferencesGroup::builder().title("Language").build();
    settings_container.append(&lang_group);

    let lang_model = gtk::StringList::new(&[
        "System Default",
        "English",
        "Arabic",
        "French",
        "Spanish",
        "Turkish",
    ]);

    let lang_row = ComboRow::builder()
        .title("Language")
        .model(&lang_model)
        .build();

    match current_lang.borrow().as_str() {
        "en" => lang_row.set_selected(1),
        "ar" => lang_row.set_selected(2),
        "fr" => lang_row.set_selected(3),
        "es" => lang_row.set_selected(4),
        "tr" => lang_row.set_selected(5),
        _ => lang_row.set_selected(0),
    }

    lang_group.add(&lang_row);

    let location_group = PreferencesGroup::builder()
        .title("Location Settings")
        .build();
    settings_container.append(&location_group);

    let modes =
        gtk::StringList::new(&["Manual Coordinates", "City Selection", "Auto (GPS/Network)"]);

    let mode_row = ComboRow::builder()
        .title("Location Method")
        .model(&modes)
        .build();

    mode_row.set_selected(match config.borrow().location_mode {
        LocationMode::Manual => 0,
        LocationMode::City => 1,
        LocationMode::Auto => 2,
    });

    location_group.add(&mode_row);

    let lat_row = EntryRow::builder()
        .title("Latitude")
        .text(config.borrow().latitude.to_string())
        .build();
    let lon_row = EntryRow::builder()
        .title("Longitude")
        .text(config.borrow().longitude.to_string())
        .build();

    let city_row = EntryRow::builder().title("City Name").build();
    if let Some(city) = &config.borrow().city_name {
        city_row.set_text(city);
    }

    let auto_status_row = ActionRow::builder()
        .title("Status")
        .subtitle("Enable location services in system settings.")
        .build();
    let detect_btn = Button::builder()
        .label("Detect Now")
        .valign(gtk::Align::Center)
        .build();
    auto_status_row.add_suffix(&detect_btn);

    location_group.add(&lat_row);
    location_group.add(&lon_row);
    location_group.add(&city_row);
    location_group.add(&auto_status_row);

    let app_clone = app.clone();
    let config_close = config.clone();
    window.connect_close_request(move |_| {
        if !config_close.borrow().is_configured {
            app_clone.quit();
        }
        gtk::glib::Propagation::Proceed
    });

    let continue_btn = Button::builder()
        .label("Continue")
        .css_classes(["suggested-action", "pill"])
        .margin_top(12)
        .margin_bottom(24)
        .halign(gtk::Align::Center)
        .width_request(200)
        .build();

    settings_container.append(&continue_btn);

    let update_visibility = Rc::new({
        let mode_row = mode_row.clone();
        let lat_row = lat_row.clone();
        let lon_row = lon_row.clone();
        let city_row = city_row.clone();
        let auto_status_row = auto_status_row.clone();

        move || {
            let selected = mode_row.selected();
            lat_row.set_visible(selected == 0);
            lon_row.set_visible(selected == 0);
            city_row.set_visible(selected == 1);
            auto_status_row.set_visible(selected == 2);
        }
    });

    update_visibility();
    let update_vis_clone = update_visibility.clone();
    mode_row.connect_selected_notify(move |_| {
        update_vis_clone();
    });

    let city_search_btn = Button::builder()
        .label("Search")
        .valign(gtk::Align::Center)
        .build();
    city_row.add_suffix(&city_search_btn);

    let update_translations = Rc::new({
        let status_page = status_page.clone();
        let appearance_group = appearance_group.clone();
        let theme_row = theme_row.clone();
        let lang_group = lang_group.clone();
        let lang_row = lang_row.clone();
        let location_group = location_group.clone();
        let mode_row = mode_row.clone();
        let lat_row = lat_row.clone();
        let lon_row = lon_row.clone();
        let city_row = city_row.clone();
        let city_search_btn = city_search_btn.clone();
        let auto_status_row = auto_status_row.clone();
        let detect_btn = detect_btn.clone();
        let continue_btn = continue_btn.clone();
        let window = window.clone();
        let _current_lang = current_lang.clone();
        let modes = modes.clone();
        let theme_model = theme_model.clone();
        let lang_model = lang_model.clone();

        let location_state = location_state.clone();
        move |lang_code: &str| {
            let l = lang_code;

            if l == "ar" {
                gtk::Widget::set_default_direction(gtk::TextDirection::Rtl);
                window.set_direction(gtk::TextDirection::Rtl);
            } else {
                gtk::Widget::set_default_direction(gtk::TextDirection::Ltr);
                window.set_direction(gtk::TextDirection::Ltr);
            }

            crate::apply_font_css(l);

            window.set_title(Some(&tr("Welcome to Khushu", l)));
            status_page.set_title(&tr("Welcome to Khushu", l));
            status_page.set_description(Some(&tr(
                "Please configure your location to get accurate prayer times.",
                l,
            )));

            appearance_group.set_title(&tr("Appearance", l));
            theme_row.set_title(&tr("Theme", l));
            theme_model.splice(
                0,
                3,
                &[&tr("System Default", l), &tr("Light", l), &tr("Dark", l)],
            );

            lang_group.set_title(&tr("Language", l));
            lang_row.set_title(&tr("Language", l));
            lang_model.splice(
                0,
                6,
                &[
                    &tr("System Default", l),
                    &tr("English", l),
                    &tr("Arabic", l),
                    &tr("French", l),
                    &tr("Spanish", l),
                    &tr("Turkish", l),
                ],
            );

            location_group.set_title(&tr("Location Settings", l));
            mode_row.set_title(&tr("Location Method", l));
            modes.splice(
                0,
                3,
                &[
                    &tr("Manual Coordinates", l),
                    &tr("City Selection", l),
                    &tr("Auto (GPS/Network)", l),
                ],
            );

            lat_row.set_title(&tr("Latitude", l));
            lon_row.set_title(&tr("Longitude", l));
            city_row.set_title(&tr("City Name", l));

            auto_status_row.set_title(&tr("Status", l));

            let state = location_state.borrow().clone();
            match state {
                LocationState::Initial => {
                    auto_status_row
                        .set_subtitle(&tr("Enable location services in system settings.", l));
                }
                LocationState::Searching => {
                    auto_status_row.set_subtitle(&tr("Detecting...", l));
                }
                LocationState::Success(city, lat, lon) => {
                    auto_status_row.set_subtitle(&format!(
                        "{}: {} ({:.2}, {:.2})",
                        tr("Found", l),
                        city,
                        lat,
                        lon
                    ));
                }
                LocationState::Error(key) => {
                    auto_status_row.set_subtitle(&tr(&key, l));
                }
            }

            detect_btn.set_label(&tr("Detect Now", l));
            continue_btn.set_label(&tr("Continue", l));
            city_search_btn.set_label(&tr("Search", l));
        }
    });

    update_translations(&current_lang.borrow());

    let update_translations_clone = update_translations.clone();
    let current_lang_clone = current_lang.clone();
    lang_row.connect_selected_notify(move |row| {
        let next_lang = match row.selected() {
            1 => "en",
            2 => "ar",
            3 => "fr",
            4 => "es",
            5 => "tr",
            _ => "auto",
        }
        .to_string();

        let changed = { *current_lang_clone.borrow() != next_lang };
        if changed {
            {
                *current_lang_clone.borrow_mut() = next_lang.clone();
            }
            crate::i18n::update_locale(&next_lang);
            update_translations_clone(&next_lang);
        }
    });

    let city_row_for_search = city_row.clone();
    let city_search_btn_for_search = city_search_btn.clone();
    let config_for_city_search = config.clone();
    let perform_city_search = std::rc::Rc::new(move || {
        let query = city_row_for_search.text().to_string();
        if query.trim().is_empty() {
            return;
        }

        city_row_for_search.remove_css_class("error");
        city_row_for_search.remove_css_class("success");

        let city_row_for_update = city_row_for_search.clone();
        let config_clone = config_for_city_search.clone();

        gtk::glib::spawn_future_local(async move {
            let result = location::search_city(&query).await;
            if let Ok((lat, lon, name)) = result {
                let mut cfg = config_clone.borrow_mut();
                cfg.latitude = lat;
                cfg.longitude = lon;
                cfg.city_name = Some(name.clone());
                cfg.location_mode = LocationMode::City;

                city_row_for_update.set_text(&location::short_city_with_country(&name));
                city_row_for_update.add_css_class("success");
            } else {
                city_row_for_update.add_css_class("error");
            }
        });
    });

    let perform_city_search_entry = perform_city_search.clone();
    city_row.connect_entry_activated(move |_| {
        perform_city_search_entry();
    });

    let perform_city_search_btn = perform_city_search.clone();
    city_search_btn_for_search.connect_clicked(move |_| {
        perform_city_search_btn();
    });

    let auto_status_label = Rc::new(RefCell::new(auto_status_row.clone()));
    let config_clone = config.clone();
    let current_lang_for_detect = current_lang.clone();
    let location_state_for_detect = location_state.clone();
    detect_btn.connect_clicked(move |_| {
        let label_row = auto_status_label.borrow().clone();
        let lang = current_lang_for_detect.borrow().clone();

        label_row.remove_css_class("success");
        label_row.remove_css_class("error");
        label_row.set_subtitle(&tr("Detecting...", &lang));
        *location_state_for_detect.borrow_mut() = LocationState::Searching;

        let config_clone = config_clone.clone();
        let current_lang_for_status = current_lang_for_detect.clone();
        let state_clone = location_state_for_detect.clone();
        gtk::glib::spawn_future_local(async move {
            let result = location::fetch_auto_location().await;
            match result {
                Ok((lat, lon, city)) => {
                    let mut cfg = config_clone.borrow_mut();
                    cfg.latitude = lat;
                    cfg.longitude = lon;
                    cfg.city_name = Some(city.clone());
                    cfg.location_mode = LocationMode::Auto;

                    let lang = current_lang_for_status.borrow().clone();
                    label_row.set_subtitle(&format!(
                        "{}: {} ({:.2}, {:.2})",
                        tr("Found", &lang),
                        city,
                        lat,
                        lon
                    ));
                    label_row.add_css_class("success");
                    *state_clone.borrow_mut() = LocationState::Success(city, lat, lon);
                }
                Err(e) => {
                    let lang = current_lang_for_status.borrow().clone();
                    label_row.set_subtitle(&tr(&e, &lang));
                    label_row.add_css_class("error");
                    *state_clone.borrow_mut() = LocationState::Error(e);
                }
            }
        });
    });

    let config_final = config.clone();
    let window_final = window.clone();
    let on_done_rc = Rc::new(on_done);

    continue_btn.connect_clicked(move |_| {
        {
            let mut cfg = config_final.borrow_mut();

            match mode_row.selected() {
                0 => {
                    cfg.location_mode = LocationMode::Manual;
                    cfg.latitude = lat_row.text().parse().unwrap_or(cfg.latitude);
                    cfg.longitude = lon_row.text().parse().unwrap_or(cfg.longitude);
                }
                1 => {
                    cfg.location_mode = LocationMode::City;
                    let city = city_row.text().to_string();
                    if !city.is_empty() {
                        cfg.city_name = Some(city);
                    }
                }
                2 => {
                    cfg.location_mode = LocationMode::Auto;
                }
                _ => {}
            }

            let lang_idx = lang_row.selected();
            cfg.language = match lang_idx {
                1 => "en",
                2 => "ar",
                3 => "fr",
                4 => "es",
                5 => "tr",
                _ => "auto",
            }
            .to_string();

            let theme_idx = theme_row.selected();
            cfg.theme = match theme_idx {
                1 => ThemeMode::Light,
                2 => ThemeMode::Dark,
                _ => ThemeMode::System,
            };

            cfg.is_configured = true;
            cfg.save();
        }

        window_final.close();
        on_done_rc();
    });

    window.present();
}
