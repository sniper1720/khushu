use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use adw::prelude::*;
use adw::{ApplicationWindow, ComboRow, PreferencesGroup};
use chrono::Local;
use gtk::{Box, Button, ListBox, StringList};
use gtk4 as gtk;
use libadwaita as adw;

use crate::config::{AppConfig, CalculationMethod, LocationMode, MadhabChoice};
use crate::i18n::tr;
use crate::location;
use crate::notifications;
use crate::time::PrayerEngine;

pub fn setup_settings_ui(
    settings_box: &Box,
    config: Rc<RefCell<AppConfig>>,
    list_box_rc: Rc<ListBox>,
    window: &ApplicationWindow,
    current_lang: Rc<RefCell<String>>,
    loc_tx: std::sync::mpsc::Sender<(f64, f64, Option<String>)>,
    refresh_calendar: Rc<dyn Fn()>,
) {
    let lang_val = current_lang.borrow().clone();

    while let Some(child) = settings_box.first_child() {
        settings_box.remove(&child);
    }

    let theme_group = PreferencesGroup::builder()
        .title(tr("Theme", &lang_val))
        .build();
    settings_box.append(&theme_group);

    let theme_model = StringList::new(&[
        &tr("System Default", &lang_val),
        &tr("Light", &lang_val),
        &tr("Dark", &lang_val),
    ]);
    let theme_row = ComboRow::builder()
        .title(tr("Theme", &lang_val))
        .model(&theme_model)
        .build();

    match config.borrow().theme {
        crate::config::ThemeMode::Light => theme_row.set_selected(1),
        crate::config::ThemeMode::Dark => theme_row.set_selected(2),
        _ => theme_row.set_selected(0),
    }

    let config_theme = config.clone();
    theme_row.connect_selected_notify(move |row| {
        let manager = adw::StyleManager::default();
        let new_theme = match row.selected() {
            1 => crate::config::ThemeMode::Light,
            2 => crate::config::ThemeMode::Dark,
            _ => crate::config::ThemeMode::System,
        };

        match new_theme {
            crate::config::ThemeMode::Light => {
                manager.set_color_scheme(adw::ColorScheme::ForceLight)
            }
            crate::config::ThemeMode::Dark => {
                manager.set_color_scheme(adw::ColorScheme::PreferDark)
            }
            crate::config::ThemeMode::System => manager.set_color_scheme(adw::ColorScheme::Default),
        }

        config_theme.borrow_mut().theme = new_theme;
        config_theme.borrow().save();
    });
    theme_group.add(&theme_row);

    let startup_group = PreferencesGroup::builder()
        .title(tr("Autostart", &lang_val))
        .build();
    startup_group.set_margin_top(24);
    settings_box.append(&startup_group);

    let autostart_toggle = adw::SwitchRow::builder()
        .title(tr("Start Automatically", &lang_val))
        .subtitle(tr(
            "Run Khushu in the background when you log in.",
            &lang_val,
        ))
        .build();
    autostart_toggle.set_active(config.borrow().autostart);

    let config_autostart = config.clone();
    autostart_toggle.connect_active_notify(move |row| {
        let is_active = row.is_active();
        config_autostart.borrow_mut().autostart = is_active;
        config_autostart.borrow().save();
        crate::autostart::sync(is_active);
    });
    startup_group.add(&autostart_toggle);

    let location_group = PreferencesGroup::builder()
        .title(tr("Location Settings", &lang_val))
        .build();
    location_group.set_margin_top(24);
    location_group.set_margin_bottom(24);
    settings_box.append(&location_group);

    let modes_strings = [
        tr("Manual Coordinates", &lang_val),
        tr("City Selection", &lang_val),
        tr("Auto (GPS/Network)", &lang_val),
    ];
    let modes_slices: Vec<&str> = modes_strings.iter().map(|s| s.as_str()).collect();
    let modes = StringList::new(&modes_slices);
    let mode_row = ComboRow::builder()
        .title(tr("Location Method", &lang_val))
        .model(&modes)
        .build();

    let current_mode = config.borrow().location_mode.clone();
    mode_row.set_selected(match current_mode {
        LocationMode::Manual => 0,
        LocationMode::City => 1,
        LocationMode::Auto => 2,
    });
    location_group.add(&mode_row);

    let lat_row = adw::SpinRow::builder()
        .title(tr("Latitude", &lang_val))
        .adjustment(&gtk::Adjustment::new(
            config.borrow().latitude,
            -90.0,
            90.0,
            0.01,
            0.0,
            0.0,
        ))
        .digits(4)
        .build();

    let config_lat = config.clone();
    let list_box_lat = list_box_rc.clone();
    lat_row.adjustment().connect_value_changed(move |adj| {
        config_lat.borrow_mut().latitude = adj.value();
        config_lat.borrow().save();
        refresh_prayers(&config_lat.borrow(), &list_box_lat);
    });

    let lon_row = adw::SpinRow::builder()
        .title(tr("Longitude", &lang_val))
        .adjustment(&gtk::Adjustment::new(
            config.borrow().longitude,
            -180.0,
            180.0,
            0.01,
            0.0,
            0.0,
        ))
        .digits(4)
        .build();

    let config_lon = config.clone();
    let list_box_lon = list_box_rc.clone();
    lon_row.adjustment().connect_value_changed(move |adj| {
        config_lon.borrow_mut().longitude = adj.value();
        config_lon.borrow().save();
        refresh_prayers(&config_lon.borrow(), &list_box_lon);
    });

    let status_row = adw::ActionRow::builder()
        .title(tr("Location Status", &lang_val))
        .visible(false)
        .build();
    status_row.add_css_class("error");
    let status_row_clone = status_row.clone();
    let status_row_clone2 = status_row.clone();

    let city_row = adw::EntryRow::builder()
        .title(tr("City Search", &lang_val))
        .build();

    if config.borrow().location_mode == LocationMode::City
        && let Some(name) = &config.borrow().city_name
    {
        city_row.set_text(&location::short_city_with_country(name));
    }

    let city_btn = Button::with_label(&tr("Search", &lang_val));
    city_btn.set_valign(gtk::Align::Center);
    city_btn.set_halign(gtk::Align::End);
    city_btn.set_hexpand(false);
    city_btn.set_vexpand(false);
    let city_tx = loc_tx.clone();

    let city_row_clone = city_row.clone();
    let lang_val_city = lang_val.clone();
    let perform_search = Rc::new(move || {
        let query = city_row_clone.text().to_string();
        if query.trim().is_empty() {
            return;
        }

        city_row_clone.remove_css_class("error");
        city_row_clone.remove_css_class("success");

        let tx = city_tx.clone();
        let city_row_for_update = city_row_clone.clone();
        let status_row_clone = status_row_clone.clone();
        let lang_val_clone = lang_val_city.clone();

        gtk::glib::spawn_future_local(async move {
            let result = location::search_city(&query).await;
            match result {
                Ok((lat, lon, name)) => {
                    let _ = tx.send((lat, lon, Some(name.clone())));
                    city_row_for_update.set_text(&location::short_city_with_country(&name));
                    city_row_for_update.add_css_class("success");
                    status_row_clone.set_visible(false);
                }
                Err(e) => {
                    log::error!("City search failed: {}", e);
                    city_row_for_update.add_css_class("error");
                    status_row_clone
                        .set_subtitle(&tr("City not found. Please try again.", &lang_val_clone));
                    status_row_clone.set_visible(true);
                }
            }
        });
    });

    let search_fn = perform_search.clone();
    city_row.connect_entry_activated(move |_| {
        search_fn();
    });

    let search_fn_btn = perform_search.clone();
    city_btn.connect_clicked(move |_| {
        search_fn_btn();
    });

    city_row.add_suffix(&city_btn);

    let auto_row = adw::ActionRow::builder()
        .title(tr("Auto Detection", &lang_val))
        .build();
    if let Some(name) = &config.borrow().city_name {
        auto_row.set_subtitle(&location::short_city_with_country(name));
    }
    let auto_btn = Button::with_label(&tr("Update Now", &lang_val));
    auto_btn.set_valign(gtk::Align::Center);
    auto_btn.set_halign(gtk::Align::End);
    auto_btn.set_hexpand(false);
    auto_btn.set_vexpand(false);

    let auto_tx = loc_tx.clone();
    let auto_row_clone = auto_row.clone();
    let status_row_auto = status_row_clone2;
    let lang_val_auto = lang_val.clone();

    auto_btn.connect_clicked(move |_| {
        auto_row_clone.remove_css_class("error");
        auto_row_clone.remove_css_class("success");
        status_row_auto.set_visible(false);

        let tx = auto_tx.clone();
        let auto_row_for_update = auto_row_clone.clone();
        let status_for_update = status_row_auto.clone();
        let lang_for_update = lang_val_auto.clone();

        gtk::glib::spawn_future_local(async move {
            let result = location::fetch_auto_location().await;
            match result {
                Ok((lat, lon, name)) => {
                    let _ = tx.send((lat, lon, Some(name.clone())));
                    auto_row_for_update.set_subtitle(&location::short_city_with_country(&name));
                    auto_row_for_update.add_css_class("success");
                }
                Err(e) => {
                    log::error!("Auto-location failed: {}", e);
                    auto_row_for_update.add_css_class("error");
                    status_for_update.set_subtitle(&tr(&e, &lang_for_update));
                    status_for_update.set_visible(true);
                }
            }
        });
    });

    auto_row.add_suffix(&auto_btn);

    location_group.add(&lat_row);
    location_group.add(&lon_row);
    location_group.add(&city_row);
    location_group.add(&auto_row);
    location_group.add(&status_row);

    let calc_group = PreferencesGroup::builder()
        .title(tr("Calculation Settings", &lang_val))
        .build();
    calc_group.set_margin_top(24);
    calc_group.set_margin_bottom(24);
    settings_box.append(&calc_group);

    let hijri_adj = gtk::Adjustment::new(
        config.borrow().hijri_offset as f64,
        -2.0,
        2.0,
        1.0,
        0.0,
        0.0,
    );
    let hijri_row = adw::SpinRow::builder()
        .title(tr("Hijri Date Correction", &lang_val))
        .subtitle(tr("Adjust Hijri date by +/- days", &lang_val))
        .adjustment(&hijri_adj)
        .digits(0)
        .build();

    let config_hijri = config.clone();
    let refresh_calendar_hijri = refresh_calendar.clone();
    hijri_adj.connect_value_changed(move |adj| {
        config_hijri.borrow_mut().hijri_offset = adj.value() as i64;
        config_hijri.borrow().save();
        refresh_calendar_hijri();
    });
    calc_group.add(&hijri_row);

    let methods_strings = [
        tr("MWL", &lang_val),
        tr("ISNA", &lang_val),
        tr("Egypt", &lang_val),
        tr("Makkah", &lang_val),
        tr("Karachi", &lang_val),
        tr("Dubai", &lang_val),
        tr("MoonsightingCommittee", &lang_val),
        tr("Kuwait", &lang_val),
        tr("Qatar", &lang_val),
        tr("Singapore", &lang_val),
        tr("Turkey", &lang_val),
    ];
    let methods_slices: Vec<&str> = methods_strings.iter().map(|s| s.as_str()).collect();
    let methods = StringList::new(&methods_slices);
    let method_row = ComboRow::builder()
        .title(tr("Calculation Method", &lang_val))
        .model(&methods)
        .build();

    let current_method = config.borrow().method.clone();
    method_row.set_selected(match current_method {
        CalculationMethod::MWL => 0,
        CalculationMethod::ISNA => 1,
        CalculationMethod::Egypt => 2,
        CalculationMethod::Makkah => 3,
        CalculationMethod::Karachi => 4,
        CalculationMethod::Dubai => 5,
        CalculationMethod::MoonsightingCommittee => 6,
        CalculationMethod::Kuwait => 7,
        CalculationMethod::Qatar => 8,
        CalculationMethod::Singapore => 9,
        CalculationMethod::Turkey => 10,
    });

    let config_method = config.clone();
    let list_box_method = list_box_rc.clone();
    method_row.connect_selected_notify(move |combo| {
        let method = match combo.selected() {
            0 => CalculationMethod::MWL,
            1 => CalculationMethod::ISNA,
            2 => CalculationMethod::Egypt,
            3 => CalculationMethod::Makkah,
            4 => CalculationMethod::Karachi,
            5 => CalculationMethod::Dubai,
            6 => CalculationMethod::MoonsightingCommittee,
            7 => CalculationMethod::Kuwait,
            8 => CalculationMethod::Qatar,
            9 => CalculationMethod::Singapore,
            10 => CalculationMethod::Turkey,
            _ => CalculationMethod::MWL,
        };
        config_method.borrow_mut().method = method;
        config_method.borrow().save();
        refresh_prayers(&config_method.borrow(), &list_box_method);
    });
    calc_group.add(&method_row);

    let lat_row_clone = lat_row.clone();
    let lon_row_clone = lon_row.clone();
    let city_row_clone = city_row.clone();
    let auto_row_clone = auto_row.clone();

    let update_visibility = Rc::new(move |mode: &LocationMode| {
        lat_row_clone.set_visible(*mode == LocationMode::Manual);
        lon_row_clone.set_visible(*mode == LocationMode::Manual);
        city_row_clone.set_visible(*mode == LocationMode::City);
        auto_row_clone.set_visible(*mode == LocationMode::Auto);
    });

    update_visibility(&current_mode);

    let update_vis_clone = update_visibility.clone();
    let config_mode = config.clone();
    let list_box_mode = list_box_rc.clone();
    mode_row.connect_selected_notify(move |combo| {
        let mode = match combo.selected() {
            0 => LocationMode::Manual,
            1 => LocationMode::City,
            2 => LocationMode::Auto,
            _ => LocationMode::Manual,
        };
        config_mode.borrow_mut().location_mode = mode.clone();
        config_mode.borrow().save();
        update_vis_clone(&mode);
        refresh_prayers(&config_mode.borrow(), &list_box_mode);
    });

    let madhab_strings = [
        tr("Shafi (Standard/Maliki/Hanbali)", &lang_val),
        tr("Hanafi", &lang_val),
    ];
    let madhab_slices: Vec<&str> = madhab_strings.iter().map(|s| s.as_str()).collect();
    let madhabs = StringList::new(&madhab_slices);
    let madhab_row = ComboRow::builder()
        .title(tr("Asr Calculation (Madhab)", &lang_val))
        .model(&madhabs)
        .build();

    let current_madhab = config.borrow().madhab.clone();
    if current_madhab == MadhabChoice::Hanafi {
        madhab_row.set_selected(1);
    } else {
        madhab_row.set_selected(0);
    }

    let config_madhab = config.clone();
    let list_box_madhab = list_box_rc.clone();
    madhab_row.connect_selected_notify(move |combo| {
        let index = combo.selected();
        let m = if index == 1 {
            MadhabChoice::Hanafi
        } else {
            MadhabChoice::Shafi
        };
        config_madhab.borrow_mut().madhab = m;
        config_madhab.borrow().save();
        refresh_prayers(&config_madhab.borrow(), &list_box_madhab);
    });
    calc_group.add(&madhab_row);

    let note_row = adw::ActionRow::builder()
        .title(tr("Note", &lang_val))
        .subtitle(tr(
            "Maliki/Hanbali use Standard (Shafi) for Asr.",
            &lang_val,
        ))
        .build();
    calc_group.add(&note_row);

    let notif_group = PreferencesGroup::new();
    notif_group.set_title(&tr("Notifications", &lang_val));
    notif_group.set_margin_top(24);
    notif_group.set_margin_bottom(24);
    settings_box.append(&notif_group);

    let notify_toggle = adw::SwitchRow::builder()
        .title(tr("Pre-Prayer Alert", &lang_val))
        .subtitle(tr("Get notified before the prayer time.", &lang_val))
        .build();
    notify_toggle.set_active(config.borrow().pre_prayer_notify);

    let config_notify = config.clone();
    notify_toggle.connect_active_notify(move |row| {
        config_notify.borrow_mut().pre_prayer_notify = row.is_active();
        config_notify.borrow().save();
    });
    notif_group.add(&notify_toggle);

    let notify_time = adw::SpinRow::builder()
        .title(tr("Alert Time", &lang_val))
        .subtitle(tr("Minutes before prayer", &lang_val))
        .adjustment(&gtk::Adjustment::new(
            config.borrow().pre_prayer_minutes as f64,
            1.0,
            60.0,
            1.0,
            5.0,
            0.0,
        ))
        .digits(0)
        .build();

    let config_time = config.clone();
    notify_time.adjustment().connect_value_changed(move |adj| {
        config_time.borrow_mut().pre_prayer_minutes = adj.value() as u32;
        config_time.borrow().save();
    });
    notif_group.add(&notify_time);

    let time_row_clone = notify_time.clone();
    notify_toggle.connect_active_notify(move |row| {
        time_row_clone.set_visible(row.is_active());
    });
    notify_time.set_visible(config.borrow().pre_prayer_notify);

    let test_notify_btn = Button::builder()
        .label(tr("Test Notification", &lang_val))
        .margin_top(12)
        .build();

    let config_test_notif = config.clone();
    let current_lang_notif = current_lang.clone();
    test_notify_btn.connect_clicked(move |_| {
        let lang = current_lang_notif.borrow();
        let cfg = config_test_notif.borrow();
        let title = tr("It's time for", &lang);
        let body = tr(
            "This is a test notification from Khushu. May your prayers be accepted.",
            &lang,
        );
        notifications::show_notification(
            &title,
            &body,
            true,
            &tr("Open Khushu", &lang),
            &tr("Stop Adhan", &lang),
        );
        if !cfg.adhan_muted {
            let path = cfg
                .adhan_sound_path
                .clone()
                .unwrap_or_else(|| "assets/audio/Madinah.mp3".to_string());
            let audio = crate::audio::AudioManager::new();
            audio.play_adhan(&path, cfg.adhan_volume);
        }
    });

    notif_group.add(&test_notify_btn);

    let audio_group = PreferencesGroup::new();
    audio_group.set_title(&tr("Audio", &lang_val));
    audio_group.set_margin_bottom(12);
    settings_box.append(&audio_group);

    let preset_files: Vec<String> = vec!["Madinah.mp3".to_string(), "Makkah.mp3".to_string()];

    let mut preset_labels: Vec<String> = Vec::new();
    preset_labels.push(tr("Default", &lang_val));
    preset_labels.push(tr("Custom File...", &lang_val));
    for name in &preset_files {
        preset_labels.push(adhan_preset_label(name, &lang_val));
    }

    let label_refs: Vec<&str> = preset_labels.iter().map(|s| s.as_str()).collect();
    let model = gtk::StringList::new(&label_refs);

    let sound_combo = ComboRow::builder()
        .title(tr("Adhan Sound", &lang_val))
        .model(&model)
        .build();

    let current_path = config.borrow().adhan_sound_path.clone();
    if let Some(path) = current_path {
        let path_obj = PathBuf::from(&path);
        if let Some(name) = path_obj.file_name().and_then(|n| n.to_str()) {
            if let Some(pos) = preset_files.iter().position(|p| p.as_str() == name) {
                sound_combo.set_selected((pos + 2) as u32);
            } else {
                sound_combo.set_selected(1);
                sound_combo.set_subtitle(&path);
            }
        } else {
            sound_combo.set_selected(1);
            sound_combo.set_subtitle(&path);
        }
    } else {
        sound_combo.set_selected(0);
        sound_combo.set_subtitle(&tr("Using builtin default", &lang_val));
    }

    let window_clone_sound = window.clone();
    let config_sound = config.clone();
    let preset_files_clone = preset_files.clone();
    let lang_for_audio = lang_val.clone();

    sound_combo.connect_selected_notify(move |combo| {
        let index = combo.selected() as usize;

        if index == 0 {
            config_sound.borrow_mut().adhan_sound_path = None;
            config_sound.borrow().save();
            combo.set_subtitle(&tr("Using builtin default", &lang_for_audio));
        } else if index == 1 {
            let file_filter = gtk::FileFilter::new();
            file_filter.set_name(Some(&tr("Audio Files", &lang_for_audio)));
            file_filter.add_mime_type("audio/mpeg");
            file_filter.add_mime_type("audio/mp3");
            file_filter.add_mime_type("audio/ogg");

            let dialog = gtk::FileChooserDialog::builder()
                .title(tr("Select Adhan Sound", &lang_for_audio))
                .action(gtk::FileChooserAction::Open)
                .modal(true)
                .transient_for(&window_clone_sound)
                .filter(&file_filter)
                .build();

            dialog.add_button(&tr("Cancel", &lang_for_audio), gtk::ResponseType::Cancel);
            dialog.add_button(&tr("Select", &lang_for_audio), gtk::ResponseType::Accept);

            let config_dialog = config_sound.clone();
            let combo_dialog = combo.clone();

            dialog.connect_response(move |d, response| {
                if response == gtk::ResponseType::Accept
                    && let Some(file) = d.file()
                    && let Some(path) = file.path()
                    && let Some(path_str) = path.to_str()
                {
                    config_dialog.borrow_mut().adhan_sound_path = Some(path_str.to_string());
                    config_dialog.borrow().save();
                    combo_dialog.set_subtitle(path_str);
                }
                d.close();
            });
            dialog.show();
        } else {
            let mut path = PathBuf::from("assets/audio");
            let file_name = &preset_files_clone[index - 2];
            path.push(file_name);
            if let Some(path_str) = path.to_str() {
                config_sound.borrow_mut().adhan_sound_path = Some(path_str.to_string());
                config_sound.borrow().save();
                combo.set_subtitle(path_str);
            }
        }
    });

    audio_group.add(&sound_combo);

    let mute_toggle = adw::SwitchRow::builder()
        .title(tr("Mute Adhan", &lang_val))
        .subtitle(tr("Silence the Adhan sound at prayer time.", &lang_val))
        .build();
    mute_toggle.set_active(config.borrow().adhan_muted);
    let config_mute = config.clone();
    mute_toggle.connect_active_notify(move |row| {
        config_mute.borrow_mut().adhan_muted = row.is_active();
        config_mute.borrow().save();
    });
    audio_group.add(&mute_toggle);

    let volume_adj = gtk::Adjustment::new(
        (config.borrow().adhan_volume * 100.0) as f64,
        0.0,
        100.0,
        5.0,
        10.0,
        0.0,
    );
    let volume_row = adw::SpinRow::builder()
        .title(tr("Adhan Volume", &lang_val))
        .subtitle(tr("Volume level (0–100%)", &lang_val))
        .adjustment(&volume_adj)
        .digits(0)
        .build();
    volume_row.set_visible(!config.borrow().adhan_muted);

    let config_vol = config.clone();
    volume_adj.connect_value_changed(move |adj| {
        config_vol.borrow_mut().adhan_volume = (adj.value() / 100.0) as f32;
        config_vol.borrow().save();
    });
    audio_group.add(&volume_row);

    let volume_row_clone = volume_row.clone();
    mute_toggle.connect_active_notify(move |row| {
        volume_row_clone.set_visible(!row.is_active());
    });

    let test_audio_btn = Button::builder()
        .label(tr("▶ Preview Adhan", &lang_val))
        .margin_top(8)
        .build();

    let config_test = config.clone();
    let audio_mgr = crate::audio::AudioManager::new();
    let is_playing = Rc::new(RefCell::new(false));

    let lang_for_btn = lang_val.clone();
    test_audio_btn.connect_clicked(move |btn| {
        let mut playing = is_playing.borrow_mut();
        if *playing {
            audio_mgr.stop();
            btn.set_label(&tr("▶ Preview Adhan", &lang_for_btn));
            *playing = false;
        } else {
            let cfg = config_test.borrow();
            if cfg.adhan_muted {
                return;
            }
            let path = cfg
                .adhan_sound_path
                .clone()
                .unwrap_or_else(|| "assets/audio/Madinah.mp3".to_string());

            audio_mgr.play_adhan(&path, cfg.adhan_volume);
            btn.set_label(&tr("⏹ Stop Adhan", &lang_for_btn));
            *playing = true;
        }
    });
    audio_group.add(&test_audio_btn);
}

fn adhan_preset_label(file_name: &str, lang: &str) -> String {
    let stem = std::path::Path::new(file_name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(file_name);
    match stem {
        "Makkah" => tr("Makkah Adhan", lang),
        "Madinah" => tr("Madinah Adhan", lang),
        _ => stem.to_string(),
    }
}

pub fn refresh_prayers(config: &AppConfig, list_box: &ListBox) {
    while let Some(child) = list_box.first_child() {
        list_box.remove(&child);
    }

    let engine = PrayerEngine::new(
        config.latitude,
        config.longitude,
        &config.method,
        &config.madhab,
    );
    let today = Local::now().date_naive();
    let current_lang_val = config.language.clone();

    if let Some(schedule) = engine.get_prayer_times(today) {
        let prayers = [
            ("Fajr", schedule.fajr),
            ("Sunrise", schedule.shurooq),
            ("Dhuhr", schedule.dhuhr),
            ("Asr", schedule.asr),
            ("Maghrib", schedule.maghrib),
            ("Isha", schedule.isha),
        ];

        for (name, time) in prayers {
            let row = adw::ActionRow::builder()
                .title(tr(name, &current_lang_val))
                .subtitle(time.format("%H:%M").to_string())
                .name(name)
                .build();
            list_box.append(&row);
        }
    }
}
