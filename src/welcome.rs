use crate::config::{AppConfig, LocationMode, ThemeMode};
use crate::i18n::tr;
use crate::location;
use crate::platform::is_flatpak;
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

fn finish_entry_row_interaction(row: &EntryRow) {
    if let Some(root) = row.root() {
        root.set_focus(Option::<&gtk::Widget>::None);
    }
}

pub fn build_welcome_window<F>(app: &Application, config: AppConfig, on_done: F)
where
    F: Fn() + 'static,
{
    let current_language = Rc::new(RefCell::new(config.language()));
    let location_state = Rc::new(RefCell::new(LocationState::Initial));

    let window = ApplicationWindow::builder()
        .application(app)
        .title("Welcome to Khushu")
        .default_width(600)
        .default_height(650)
        .width_request(360)
        .height_request(294)
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

    match config.theme() {
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

        config_theme.set_theme(theme);
    });

    let language_group = PreferencesGroup::builder().title("Language").build();
    settings_container.append(&language_group);

    let language_model = gtk::StringList::new(&[
        "System Default",
        "English",
        "Arabic",
        "French",
        "Spanish",
        "Turkish",
        "Indonesian",
    ]);

    let language_row = ComboRow::builder()
        .title("Language")
        .model(&language_model)
        .build();

    language_row.set_selected(crate::i18n::language_index_from_code(
        current_language.borrow().as_str(),
    ));

    language_group.add(&language_row);

    let behavior_group = PreferencesGroup::builder().title("Autostart").build();
    settings_container.append(&behavior_group);

    let autostart_row = adw::SwitchRow::builder()
        .title("Start Automatically")
        .subtitle("Run Khushu in the background when you log in.")
        .active(config.autostart())
        .build();
    behavior_group.add(&autostart_row);

    let config_autostart = config.clone();
    autostart_row.connect_active_notify(move |row| {
        config_autostart.set_autostart(row.is_active());
    });

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

    mode_row.set_selected(match config.location_mode() {
        LocationMode::Manual => 0,
        LocationMode::City => 1,
        LocationMode::Auto => 2,
    });

    location_group.add(&mode_row);

    let latitude_row = EntryRow::builder()
        .title("Latitude")
        .text(config.latitude().to_string())
        .build();
    let longitude_row = EntryRow::builder()
        .title("Longitude")
        .text(config.longitude().to_string())
        .build();

    let city_row = EntryRow::builder().title("City Name").build();
    if let Some(city) = config.city_name() {
        city_row.set_text(&city);
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

    location_group.add(&latitude_row);
    location_group.add(&longitude_row);
    location_group.add(&city_row);
    location_group.add(&auto_status_row);

    let prayer_group = PreferencesGroup::builder().title("Prayer Setup").build();
    settings_container.append(&prayer_group);

    let method_model = gtk::StringList::new(&[
        "MWL",
        "ISNA",
        "Egypt",
        "Makkah",
        "Karachi",
        "Dubai",
        "MoonsightingCommittee",
        "Kuwait",
        "Qatar",
        "Singapore",
        "Turkey",
        "KEMENAG",
        "France (UOIF)",
        "Algeria",
    ]);

    let method_row = ComboRow::builder()
        .title("Calculation Method")
        .model(&method_model)
        .build();

    method_row.set_selected(match config.method() {
        crate::config::CalculationMethod::MWL => 0,
        crate::config::CalculationMethod::ISNA => 1,
        crate::config::CalculationMethod::Egypt => 2,
        crate::config::CalculationMethod::Makkah => 3,
        crate::config::CalculationMethod::Karachi => 4,
        crate::config::CalculationMethod::Dubai => 5,
        crate::config::CalculationMethod::MoonsightingCommittee => 6,
        crate::config::CalculationMethod::Kuwait => 7,
        crate::config::CalculationMethod::Qatar => 8,
        crate::config::CalculationMethod::Singapore => 9,
        crate::config::CalculationMethod::Turkey => 10,
        crate::config::CalculationMethod::Kemenag => 11,
        crate::config::CalculationMethod::France => 12,
        crate::config::CalculationMethod::Algeria => 13,
    });
    prayer_group.add(&method_row);

    let config_method = config.clone();
    method_row.connect_selected_notify(move |row| {
        let method = match row.selected() {
            0 => crate::config::CalculationMethod::MWL,
            1 => crate::config::CalculationMethod::ISNA,
            2 => crate::config::CalculationMethod::Egypt,
            3 => crate::config::CalculationMethod::Makkah,
            4 => crate::config::CalculationMethod::Karachi,
            5 => crate::config::CalculationMethod::Dubai,
            6 => crate::config::CalculationMethod::MoonsightingCommittee,
            7 => crate::config::CalculationMethod::Kuwait,
            8 => crate::config::CalculationMethod::Qatar,
            9 => crate::config::CalculationMethod::Singapore,
            10 => crate::config::CalculationMethod::Turkey,
            11 => crate::config::CalculationMethod::Kemenag,
            12 => crate::config::CalculationMethod::France,
            13 => crate::config::CalculationMethod::Algeria,
            _ => crate::config::CalculationMethod::MWL,
        };
        config_method.set_method(method);
    });

    let app_clone = app.clone();
    let config_for_auto_detect_updatelose = config.clone();
    window.connect_close_request(move |_| {
        if !config_for_auto_detect_updatelose.is_configured() {
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
        let mode_row_c = mode_row.clone();
        let latitude_row_c = latitude_row.clone();
        let longitude_row_c = longitude_row.clone();
        let city_row_c = city_row.clone();
        let auto_status_row_c = auto_status_row.clone();

        move || {
            let selected = mode_row_c.selected();
            latitude_row_c.set_visible(selected == 0);
            longitude_row_c.set_visible(selected == 0);
            city_row_c.set_visible(selected == 1);
            auto_status_row_c.set_visible(selected == 2);
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
        let status_page_c = status_page.clone();
        let appearance_group_c = appearance_group.clone();
        let theme_row_c = theme_row.clone();
        let language_group_c = language_group.clone();
        let language_row_c = language_row.clone();
        let behavior_group_c = behavior_group.clone();
        let autostart_row_c = autostart_row.clone();
        let prayer_group_c = prayer_group.clone();
        let method_row_c = method_row.clone();
        let location_group_c = location_group.clone();
        let mode_row_c = mode_row.clone();
        let latitude_row_c = latitude_row.clone();
        let longitude_row_c = longitude_row.clone();
        let city_row_c = city_row.clone();
        let city_search_btn_c = city_search_btn.clone();
        let auto_status_row_c = auto_status_row.clone();
        let detect_btn_c = detect_btn.clone();
        let continue_btn_c = continue_btn.clone();
        let window_c = window.clone();
        let modes_c = modes.clone();
        let theme_model_c = theme_model.clone();
        let language_model_c = language_model.clone();
        let method_model_c = method_model.clone();
        let config_font = config.clone();

        let location_state_c = location_state.clone();
        move |language_code: &str| {
            let detected = crate::i18n::resolved_locale(language_code);
            let locale = &detected;

            if crate::i18n::is_rtl(locale) {
                gtk::Widget::set_default_direction(gtk::TextDirection::Rtl);
                window_c.set_direction(gtk::TextDirection::Rtl);
            } else {
                gtk::Widget::set_default_direction(gtk::TextDirection::Ltr);
                window_c.set_direction(gtk::TextDirection::Ltr);
            }

            crate::i18n::update_locale(&detected);
            crate::apply_font_css(&config_font);

            window_c.set_title(Some(&tr("Welcome to Khushu")));
            status_page_c.set_title(&tr("Welcome to Khushu"));
            status_page_c.set_description(Some(&tr(
                "Please configure your location to get accurate prayer times.",
            )));

            appearance_group_c.set_title(&tr("Appearance"));
            theme_row_c.set_title(&tr("Theme"));
            theme_model_c.splice(0, 3, &[&tr("System Default"), &tr("Light"), &tr("Dark")]);

            language_group_c.set_title(&tr("Language"));
            language_row_c.set_title(&tr("Language"));
            language_model_c.splice(
                0,
                7,
                &[
                    &tr("System Default"),
                    &tr("English"),
                    &tr("Arabic"),
                    &tr("French"),
                    &tr("Spanish"),
                    &tr("Turkish"),
                    &tr("Indonesian"),
                ],
            );

            behavior_group_c.set_title(&tr("Autostart"));
            autostart_row_c.set_title(&tr("Start Automatically"));
            autostart_row_c.set_subtitle(&tr("Run Khushu in the background when you log in."));

            prayer_group_c.set_title(&tr("Prayer Setup"));
            method_row_c.set_title(&tr("Calculation Method"));
            method_model_c.splice(
                0,
                14,
                &[
                    &tr("MWL"),
                    &tr("ISNA"),
                    &tr("Egypt"),
                    &tr("Makkah"),
                    &tr("Karachi"),
                    &tr("Dubai"),
                    &tr("MoonsightingCommittee"),
                    &tr("Kuwait"),
                    &tr("Qatar"),
                    &tr("Singapore"),
                    &tr("Turkey"),
                    &tr("KEMENAG"),
                    &tr("France (UOIF)"),
                    &tr("Algeria"),
                ],
            );

            location_group_c.set_title(&tr("Location Settings"));
            mode_row_c.set_title(&tr("Location Method"));
            modes_c.splice(
                0,
                3,
                &[
                    &tr("Manual Coordinates"),
                    &tr("City Selection"),
                    &tr("Auto (GPS/Network)"),
                ],
            );

            latitude_row_c.set_title(&tr("Latitude"));
            longitude_row_c.set_title(&tr("Longitude"));
            city_row_c.set_title(&tr("City Name"));

            auto_status_row_c.set_title(&tr("Status"));

            let state = location_state_c.borrow().clone();
            match state {
                LocationState::Initial => {
                    auto_status_row_c
                        .set_subtitle(&tr("Enable location services in system settings."));
                }
                LocationState::Searching => {
                    auto_status_row_c.set_subtitle(&tr("Detecting..."));
                }
                LocationState::Success(city, latitude, longitude) => {
                    auto_status_row_c.set_subtitle(&format!(
                        "{}: {} ({:.2}, {:.2})",
                        tr("Found"),
                        city,
                        latitude,
                        longitude
                    ));
                }
                LocationState::Error(key) => {
                    auto_status_row_c.set_subtitle(&tr(&key));
                }
            }

            detect_btn_c.set_label(&tr("Detect Now"));
            continue_btn_c.set_label(&tr("Continue"));
            city_search_btn_c.set_label(&tr("Search"));
        }
    });

    update_translations(&current_language.borrow());

    let update_translations_clone = update_translations.clone();
    let current_language_clone = current_language.clone();
    language_row.connect_selected_notify(move |row| {
        log::info!(
            "selected-notify (welcome): selected={}, current_language={}",
            row.selected(),
            current_language_clone.borrow(),
        );
        let next_language = crate::i18n::language_code_from_index(row.selected()).to_string();

        let changed = { *current_language_clone.borrow() != next_language };
        if changed {
            {
                *current_language_clone.borrow_mut() = next_language.clone();
            }
            let detected_for_locale = crate::i18n::resolved_locale(&next_language);
            crate::i18n::update_locale(&detected_for_locale);
            update_translations_clone(&detected_for_locale);
        }
    });

    let city_row_for_search = city_row.clone();
    let city_search_btn_for_search = city_search_btn.clone();
    let config_for_city_search = config.clone();
    let current_language_for_search = current_language.clone();
    let perform_city_search = std::rc::Rc::new(move || {
        let query = city_row_for_search.text().to_string();
        if query.trim().is_empty() {
            return;
        }

        let current_language_clone_search = current_language_for_search.clone();

        city_row_for_search.remove_css_class("error");
        city_row_for_search.remove_css_class("success");

        let city_row_for_update = city_row_for_search.clone();
        let config_for_city_update = config_for_city_search.clone();

        gtk::glib::spawn_future_local(async move {
            let language = current_language_clone_search.borrow().clone();
            let result = location::search_city(&query, &language).await;
            if let Ok((latitude, longitude, name, _detected_timezone)) = result {
                config_for_city_update.set_latitude(latitude);
                config_for_city_update.set_longitude(longitude);
                config_for_city_update.set_city_name(Some(name.clone()));
                config_for_city_update.set_location_mode(LocationMode::City);

                city_row_for_update.set_text(&location::short_city_with_country(&name));
                city_row_for_update.add_css_class("success");
            } else {
                city_row_for_update.add_css_class("error");
            }
        });
    });

    let perform_city_search_entry = perform_city_search.clone();
    city_row.connect_entry_activated(move |row| {
        perform_city_search_entry();
        finish_entry_row_interaction(row);
    });

    let perform_city_search_btn = perform_city_search.clone();
    city_search_btn_for_search.connect_clicked(move |_| {
        perform_city_search_btn();
    });

    let auto_status_label = Rc::new(RefCell::new(auto_status_row.clone()));
    let config_for_auto_detect = config.clone();
    let current_language_for_detect = current_language.clone();
    let location_state_for_detect = location_state.clone();
    detect_btn.connect_clicked(move |_| {
        let label_row = auto_status_label.borrow().clone();

        label_row.remove_css_class("success");
        label_row.remove_css_class("error");
        label_row.set_subtitle(&tr("Detecting..."));
        *location_state_for_detect.borrow_mut() = LocationState::Searching;

        let config_for_auto_detect_update = config_for_auto_detect.clone();
        let current_language_for_status = current_language_for_detect.clone();
        let state_clone = location_state_for_detect.clone();
        gtk::glib::spawn_future_local(async move {
            let language = current_language_for_status.borrow().clone();
            let result = location::fetch_auto_location(&language).await;
            match result {
                Ok((latitude, longitude, city)) => {
                    config_for_auto_detect_update.set_latitude(latitude);
                    config_for_auto_detect_update.set_longitude(longitude);
                    config_for_auto_detect_update.set_city_name(Some(city.clone()));
                    config_for_auto_detect_update.set_location_mode(LocationMode::Auto);

                    label_row.set_subtitle(&format!(
                        "{}: {} ({:.2}, {:.2})",
                        tr("Found"),
                        city,
                        latitude,
                        longitude
                    ));
                    label_row.add_css_class("success");
                    *state_clone.borrow_mut() = LocationState::Success(city, latitude, longitude);
                }
                Err(err) => {
                    label_row.set_subtitle(&tr(&err));
                    label_row.add_css_class("error");
                    *state_clone.borrow_mut() = LocationState::Error(err);
                }
            }
        });
    });

    let config_final = config.clone();
    let window_final = window.clone();
    let on_done_rc = Rc::new(on_done);
    let autostart_row_final = autostart_row.clone();

    continue_btn.connect_clicked(move |_| {
        let should_autostart = config_final.autostart();
        let is_flatpak_runtime = is_flatpak();

        match mode_row.selected() {
            0 => {
                config_final.set_location_mode(LocationMode::Manual);
                let latitude = latitude_row
                    .text()
                    .parse()
                    .unwrap_or(config_final.latitude());
                let longitude = longitude_row
                    .text()
                    .parse()
                    .unwrap_or(config_final.longitude());
                config_final.set_latitude(latitude);
                config_final.set_longitude(longitude);
            }
            1 => {
                config_final.set_location_mode(LocationMode::City);
                let city = city_row.text().to_string();
                if !city.is_empty() {
                    config_final.set_city_name(Some(city));
                }
            }
            2 => {
                config_final.set_location_mode(LocationMode::Auto);
            }
            _ => {}
        }

        let language_index = language_row.selected();
        let language = crate::i18n::language_code_from_index(language_index);
        config_final.set_language(language);

        let theme_idx = theme_row.selected();
        let theme = match theme_idx {
            1 => ThemeMode::Light,
            2 => ThemeMode::Dark,
            _ => ThemeMode::System,
        };
        config_final.set_theme(theme);

        config_final.set_is_configured(true);
        config_final.save();

        if should_autostart && is_flatpak_runtime {
            let config_for_welcome_continue = config_final.clone();
            let window_ref = window_final.clone();
            let on_done_ref = on_done_rc.clone();
            let window_close_ref = window_final.clone();
            let row_ref = autostart_row_final.clone();
            gtk::glib::spawn_future_local(async move {
                if let Some(handle) = crate::autostart::sync(true) {
                    let granted = handle.await.unwrap_or(false);
                    if granted {
                        window_close_ref.close();
                        on_done_ref();
                    } else {
                        config_for_welcome_continue.set_autostart(false);
                        config_for_welcome_continue.save();
                        row_ref.set_active(false);
                        if let Some(overlay) = crate::settings_ui::find_toast_overlay(&window_ref) {
                            overlay.add_toast(adw::Toast::new(&tr(
                                "Autostart was denied by the system.",
                            )));
                        }
                    }
                }
            });
        } else {
            if should_autostart {
                crate::autostart::sync(true);
            }
            window_final.close();
            on_done_rc();
        }
    });

    window.present();
}
