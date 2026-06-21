use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use adw::prelude::*;
use adw::{ComboRow, PreferencesGroup};
use gtk::glib::WeakRef;
use gtk::{Button, ListBox, StringList};
use gtk4 as gtk;
use libadwaita as adw;

use crate::config::{
    AppConfig, CalculationMethod, LocationMode, MadhabChoice, PrayerTimesSource, TimezoneMode,
};
use crate::i18n::tr;
use crate::location;
use crate::notifications;

struct AudioButtonEntry {
    btn: WeakRef<Button>,
    idle_label_key: &'static str,
}

thread_local! {
    static AUDIO_BUTTONS: RefCell<Vec<AudioButtonEntry>> = const { RefCell::new(Vec::new()) };
}

pub fn register_audio_button(btn: &Button, idle_label_key: &'static str) {
    set_audio_toggle_button_label(btn, idle_label_key, crate::audio::is_playing());
    AUDIO_BUTTONS.with(|reg| {
        reg.borrow_mut().push(AudioButtonEntry {
            btn: btn.downgrade(),
            idle_label_key,
        });
    });
}

pub fn on_audio_state_changed(is_playing: bool) {
    gtk::glib::MainContext::default().invoke(move || {
        AUDIO_BUTTONS.with(|reg| {
            let reg = reg.borrow();
            for entry in reg.iter() {
                if let Some(btn) = entry.btn.upgrade() {
                    set_audio_toggle_button_label(&btn, entry.idle_label_key, is_playing);
                }
            }
        });
    });
}

pub fn find_toast_overlay(window: &adw::ApplicationWindow) -> Option<adw::ToastOverlay> {
    fn search(widget: &gtk::Widget) -> Option<adw::ToastOverlay> {
        if let Some(overlay) = widget.downcast_ref::<adw::ToastOverlay>() {
            return Some(overlay.clone());
        }
        let mut child = widget.first_child();
        while let Some(c) = child {
            if let Some(found) = search(&c) {
                return Some(found);
            }
            child = c.next_sibling();
        }
        None
    }
    window.content().as_ref().and_then(search)
}

fn set_audio_toggle_button_label(btn: &Button, idle_label_key: &str, is_playing: bool) {
    let label = if is_playing {
        tr("⏹ Stop Adhan")
    } else {
        tr(idle_label_key)
    };
    btn.set_label(&label);
}

fn bind_audio_toggle_button_sync(btn: &Button, idle_label_key: &'static str) {
    register_audio_button(btn, idle_label_key);
}

fn finish_entry_row_interaction(row: &adw::EntryRow) {
    if let Some(root) = row.root() {
        root.set_focus(Option::<&gtk::Widget>::None);
    }
}

fn append_settings_section_heading(
    settings_box: &gtk::Box,
    title: &str,
    description: Option<&str>,
    margin_top: i32,
) -> (gtk::Label, Option<gtk::Label>) {
    let heading = gtk::Label::builder()
        .label(title)
        .css_classes(["title-4"])
        .halign(gtk::Align::Start)
        .xalign(0.0)
        .margin_top(margin_top)
        .margin_bottom(if description.is_some() { 4 } else { 12 })
        .build();
    settings_box.append(&heading);

    let desc_label = if let Some(desc) = description {
        let d = gtk::Label::builder()
            .label(desc)
            .css_classes(["dim-label"])
            .hexpand(true)
            .halign(gtk::Align::Fill)
            .xalign(0.0)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .margin_bottom(12)
            .build();
        settings_box.append(&d);
        Some(d)
    } else {
        None
    };

    (heading, desc_label)
}

pub struct SettingsUiParams<'a> {
    pub settings_box: &'a gtk::Box,
    pub config: AppConfig,
    pub list_box_rc: Rc<ListBox>,
    pub window: &'a adw::ApplicationWindow,
    pub current_lang: Rc<RefCell<String>>,
    pub loc_tx: std::sync::mpsc::Sender<(f64, f64, Option<String>)>,
    pub refresh_calendar: Rc<dyn Fn()>,
}

#[allow(dead_code)]
pub struct SettingsUiContext {
    pub config: AppConfig,
    pub list_box_rc: Rc<ListBox>,
    pub window: adw::ApplicationWindow,
    pub current_lang: Rc<RefCell<String>>,
    pub loc_tx: std::sync::mpsc::Sender<(f64, f64, Option<String>)>,
    pub refresh_calendar: Rc<dyn Fn()>,
    pub settings_box: gtk::Box,

    pub general_heading: gtk::Label,
    pub general_desc: gtk::Label,
    pub lang_row: ComboRow,
    pub lang_model: gtk::StringList,
    pub theme_row: ComboRow,
    pub theme_model: gtk::StringList,
    pub autostart_toggle: adw::SwitchRow,

    pub prayer_setup_heading: gtk::Label,
    pub prayer_setup_desc: gtk::Label,
    pub location_group: PreferencesGroup,
    pub mode_row: ComboRow,
    pub mode_model: gtk::StringList,
    pub lat_row: adw::SpinRow,
    pub lon_row: adw::SpinRow,
    pub status_row: adw::ActionRow,
    pub city_row: adw::EntryRow,
    pub city_btn: Button,
    pub auto_row: adw::ActionRow,
    pub auto_btn: Button,
    pub source_row: ComboRow,
    pub source_model: gtk::StringList,
    pub url_row: adw::EntryRow,
    pub auto_refresh_row: adw::SwitchRow,
    pub mawaqit_status_row: adw::ActionRow,
    pub refresh_btn: Button,

    pub travel_group: PreferencesGroup,
    pub tz_mode_row: ComboRow,
    pub tz_mode_model: gtk::StringList,
    pub tz_named_row: adw::EntryRow,
    pub tz_offset_row: adw::SpinRow,

    pub calc_group: PreferencesGroup,
    pub hijri_row: adw::SpinRow,
    pub method_row: ComboRow,
    pub method_model: gtk::StringList,
    pub madhab_row: ComboRow,
    pub madhab_model: gtk::StringList,
    pub note_row: adw::ActionRow,

    pub iqamah_group: PreferencesGroup,
    pub iqamah_rows: Vec<adw::SpinRow>,

    pub notif_audio_heading: gtk::Label,
    pub notif_audio_desc: gtk::Label,
    pub notify_toggle: adw::SwitchRow,
    pub notify_time: adw::SpinRow,
    pub iqamah_notify_toggle: adw::SwitchRow,
    pub adkar_toggle: adw::SwitchRow,
    pub adhan_only_toggle: adw::SwitchRow,
    pub test_notify_btn: Button,

    pub audio_group: PreferencesGroup,
    pub sound_combo: ComboRow,
    pub sound_model: gtk::StringList,
    pub preset_files: Vec<String>,
    pub mute_toggle: adw::SwitchRow,
    pub volume_row: adw::SpinRow,
    pub test_audio_btn: Button,
}

pub fn setup_settings_ui<'a>(
    params: SettingsUiParams<'a>,
) -> (adw::ComboRow, Rc<RefCell<SettingsUiContext>>) {
    let SettingsUiParams {
        settings_box,
        config,
        list_box_rc,
        window,
        current_lang,
        loc_tx,
        refresh_calendar,
    } = params;
    let lang_val = current_lang.borrow().clone();

    let (general_heading, general_desc) = append_settings_section_heading(
        settings_box,
        &tr("General"),
        Some(&tr("Customize the app's appearance and startup behavior.")),
        0,
    );
    let general_desc = general_desc.expect("general section description label");

    let general_group = PreferencesGroup::new();
    general_group.set_margin_bottom(24);
    settings_box.append(&general_group);

    let lang_model = StringList::new(&[
        &tr("System Default"),
        &tr("English"),
        &tr("Arabic"),
        &tr("French"),
        &tr("Spanish"),
        &tr("Turkish"),
        &tr("Indonesian"),
    ]);
    let lang_row = ComboRow::builder()
        .title(tr("Language"))
        .model(&lang_model)
        .build();

    match lang_val.as_str() {
        "en" => lang_row.set_selected(1),
        "ar" => lang_row.set_selected(2),
        "fr" => lang_row.set_selected(3),
        "es" => lang_row.set_selected(4),
        "tr" => lang_row.set_selected(5),
        "id" => lang_row.set_selected(6),
        _ => lang_row.set_selected(0),
    }

    general_group.add(&lang_row);

    let theme_model = StringList::new(&[&tr("System Default"), &tr("Light"), &tr("Dark")]);
    let theme_row = ComboRow::builder()
        .title(tr("Theme"))
        .model(&theme_model)
        .build();

    match config.theme() {
        crate::config::ThemeMode::Light => theme_row.set_selected(1),
        crate::config::ThemeMode::Dark => theme_row.set_selected(2),
        _ => theme_row.set_selected(0),
    }

    let config_for_theme = config.clone();
    theme_row.connect_selected_notify(move |row| {
        let new_theme = match row.selected() {
            1 => crate::config::ThemeMode::Light,
            2 => crate::config::ThemeMode::Dark,
            _ => crate::config::ThemeMode::System,
        };

        let sm = adw::StyleManager::default();
        sm.set_color_scheme(match new_theme {
            crate::config::ThemeMode::Light => adw::ColorScheme::ForceLight,
            crate::config::ThemeMode::Dark => adw::ColorScheme::PreferDark,
            crate::config::ThemeMode::System => adw::ColorScheme::Default,
        });
        config_for_theme.set_theme(new_theme);
        config_for_theme.save();
    });
    general_group.add(&theme_row);

    let autostart_toggle = adw::SwitchRow::builder()
        .title(tr("Start Automatically"))
        .subtitle(tr("Run Khushu in the background when you log in."))
        .build();
    autostart_toggle.set_active(config.autostart());
    let config_autostart = config.clone();

    let window_autostart = window.clone();
    autostart_toggle.connect_active_notify(move |row| {
        let is_active = row.is_active();
        let was_active = !is_active;
        config_autostart.set_autostart(is_active);
        config_autostart.save();

        if let Some(handle) = crate::autostart::sync(is_active) {
            let row_ref = row.clone();
            let window_ref = window_autostart.clone();
            let config_future = config_autostart.clone();
            gtk::glib::spawn_future_local(async move {
                let granted = handle.await.unwrap_or(false);
                if !granted && is_active {
                    row_ref.set_active(was_active);
                    config_future.set_autostart(was_active);
                    config_future.save();
                    crate::autostart::sync(was_active);
                    if let Some(overlay) = find_toast_overlay(&window_ref) {
                        overlay
                            .add_toast(adw::Toast::new(&tr("Autostart was denied by the system.")));
                    }
                }
            });
        }
    });
    general_group.add(&autostart_toggle);

    let (prayer_setup_heading, prayer_setup_desc) = append_settings_section_heading(
        settings_box,
        &tr("Prayer Setup"),
        Some(&tr(
            "Set your location, prayer times source, timezone, calculation methods, and Iqamah delays for each prayer.",
        )),
        24,
    );
    let prayer_setup_desc = prayer_setup_desc.expect("prayer setup description label");

    let location_group = PreferencesGroup::builder()
        .title(gtk::glib::markup_escape_text(&tr("Location & Source")))
        .description(tr(
            "Set your location and choose the prayer times data source.",
        ))
        .build();
    location_group.set_margin_top(0);
    location_group.set_margin_bottom(24);
    settings_box.append(&location_group);

    let modes_strings = [
        tr("Manual Coordinates"),
        tr("City Selection"),
        tr("Auto (GPS/Network)"),
    ];
    let modes_slices: Vec<&str> = modes_strings.iter().map(|s| s.as_str()).collect();
    let modes = StringList::new(&modes_slices);
    let mode_row = ComboRow::builder()
        .title(tr("Location Method"))
        .model(&modes)
        .build();

    let current_mode = config.location_mode();
    mode_row.set_selected(match current_mode {
        LocationMode::Manual => 0,
        LocationMode::City => 1,
        LocationMode::Auto => 2,
    });

    let lat_row = adw::SpinRow::builder()
        .title(tr("Latitude"))
        .adjustment(&gtk::Adjustment::new(
            config.latitude(),
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
        let lat = adj.value();
        config_lat.set_latitude(lat);
        config_lat.save();
        refresh_prayers(&config_lat, &list_box_lat);
    });

    let lon_row = adw::SpinRow::builder()
        .title(tr("Longitude"))
        .adjustment(&gtk::Adjustment::new(
            config.longitude(),
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
        let lon = adj.value();
        config_lon.set_longitude(lon);
        config_lon.save();
        refresh_prayers(&config_lon, &list_box_lon);
    });

    let status_row = adw::ActionRow::builder()
        .title(tr("Location Status"))
        .visible(false)
        .build();
    status_row.add_css_class("error");
    let status_row_clone = status_row.clone();
    let status_row_clone2 = status_row.clone();

    let city_row = adw::EntryRow::builder().title(tr("City Search")).build();

    if config.location_mode() == LocationMode::City {
        let city_name = config.city_name();
        let mawaqit_cache = if config.prayer_times_source() == PrayerTimesSource::Mawaqit {
            config.mawaqit_cache()
        } else {
            None
        };
        if let Some(text) =
            location::display_city_label(city_name.as_deref(), mawaqit_cache.as_ref(), &lang_val)
        {
            city_row.set_text(&text);
        }
    }

    let city_btn = Button::with_label(&tr("Search"));
    city_btn.set_valign(gtk::Align::Center);
    city_btn.set_halign(gtk::Align::End);
    city_btn.set_hexpand(false);
    city_btn.set_vexpand(false);
    let city_tx = loc_tx.clone();
    let current_lang_search = current_lang.clone();

    let city_row_clone = city_row.clone();
    let status_row_clone = status_row_clone.clone();
    let perform_search = Rc::new(move || {
        let query = city_row_clone.text().to_string();
        if query.trim().is_empty() {
            return;
        }

        let lang = current_lang_search.borrow().clone();

        city_row_clone.remove_css_class("error");
        city_row_clone.remove_css_class("success");

        let tx = city_tx.clone();
        let city_row_for_update = city_row_clone.clone();
        let status_row_clone = status_row_clone.clone();

        gtk::glib::spawn_future_local(async move {
            let result = location::search_city(&query, &lang).await;
            match result {
                Ok((lat, lon, name, _timezone)) => {
                    let _ = tx.send((lat, lon, Some(name.clone())));
                    city_row_for_update.set_text(&location::short_city_with_country(&name));
                    city_row_for_update.add_css_class("success");
                    status_row_clone.set_visible(false);
                }
                Err(e) => {
                    log::error!("City search failed: {}", e);
                    city_row_for_update.add_css_class("error");
                    status_row_clone.set_subtitle(&tr("City not found. Please try again."));
                    status_row_clone.set_visible(true);
                }
            }
        });
    });

    let search_fn = perform_search.clone();
    city_row.connect_entry_activated(move |row| {
        search_fn();
        finish_entry_row_interaction(row);
    });

    let search_fn_btn = perform_search.clone();
    city_btn.connect_clicked(move |_| {
        search_fn_btn();
    });

    city_row.add_suffix(&city_btn);

    let auto_row = adw::ActionRow::builder()
        .title(tr("Auto Detection"))
        .build();
    if let Some(name) = &config.city_name() {
        auto_row.set_subtitle(&location::short_city_with_country(name));
    }
    let auto_btn = Button::with_label(&tr("Update Now"));
    auto_btn.set_valign(gtk::Align::Center);
    auto_btn.set_halign(gtk::Align::End);
    auto_btn.set_hexpand(false);
    auto_btn.set_vexpand(false);

    let auto_tx = loc_tx.clone();
    let auto_row_clone = auto_row.clone();
    let status_row_auto = status_row_clone2;
    let current_lang_auto = current_lang.clone();

    let auto_btn_click = auto_btn.clone();
    auto_btn.connect_clicked(move |_| {
        auto_btn_click.set_sensitive(false);
        auto_row_clone.remove_css_class("error");
        auto_row_clone.remove_css_class("success");
        status_row_auto.set_visible(false);

        let tx = auto_tx.clone();
        let auto_row_for_update = auto_row_clone.clone();
        let status_for_update = status_row_auto.clone();
        let btn = auto_btn_click.clone();

        let lang = current_lang_auto.borrow().clone();

        gtk::glib::spawn_future_local(async move {
            let result = location::fetch_auto_location(&lang).await;
            match result {
                Ok((lat, lon, name)) => {
                    let _ = tx.send((lat, lon, Some(name.clone())));
                    auto_row_for_update.set_subtitle(&location::short_city_with_country(&name));
                    auto_row_for_update.add_css_class("success");
                }
                Err(e) => {
                    log::error!("Auto-location failed: {}", e);
                    auto_row_for_update.add_css_class("error");
                    status_for_update.set_subtitle(&tr(&e));
                    status_for_update.set_visible(true);
                }
            }
            btn.set_sensitive(true);
        });
    });

    auto_row.add_suffix(&auto_btn);

    let source_items = [tr("Calculated (Offline)"), tr("Connected Mosque (URL)")];
    let source_refs: Vec<&str> = source_items.iter().map(|s| s.as_str()).collect();
    let source_model = StringList::new(&source_refs);
    let source_row = ComboRow::builder()
        .title(tr("Prayer Times Source"))
        .model(&source_model)
        .build();
    source_row.set_selected(match config.prayer_times_source() {
        PrayerTimesSource::Calculated => 0,
        PrayerTimesSource::Mawaqit => 1,
    });
    location_group.add(&source_row);

    let url_row = adw::EntryRow::builder()
        .title(tr("Connected Mosque URL (mawaqit.net)"))
        .visible(config.prayer_times_source() == PrayerTimesSource::Mawaqit)
        .build();
    if let Some(url) = &config.mawaqit_url() {
        url_row.set_text(url);
    } else if let Some(cache) = config.mawaqit_cache().as_ref() {
        url_row.set_text(&cache.url);
    }
    location_group.add(&url_row);

    let auto_refresh_row = adw::SwitchRow::builder()
        .title(tr("Auto refresh daily"))
        .subtitle(tr(
            "Refresh mosque prayer times once per day while the app is open.",
        ))
        .visible(config.prayer_times_source() == PrayerTimesSource::Mawaqit)
        .build();
    auto_refresh_row.set_active(config.mawaqit_auto_refresh_daily());
    location_group.add(&auto_refresh_row);

    let mawaqit_status_row = adw::ActionRow::builder()
        .title(tr("Connected Mosque"))
        .visible(config.prayer_times_source() == PrayerTimesSource::Mawaqit)
        .build();
    if let Some(cache) = config.mawaqit_cache().as_ref() {
        let title = cache
            .mosque_name
            .clone()
            .unwrap_or_else(|| cache.url.clone());
        let tz = cache.timezone.clone().unwrap_or_default();
        let tz_label = if tz.is_empty() {
            String::new()
        } else {
            location::localized_time_zone_label(&tz, &lang_val)
        };
        let subtitle = if tz_label.is_empty() {
            format!("{} • {}", tr("Last updated"), cache.fetched_on)
        } else {
            format!(
                "{} • {} • {}",
                tz_label,
                tr("Last updated"),
                cache.fetched_on
            )
        };
        mawaqit_status_row.set_subtitle(&subtitle);
        mawaqit_status_row.set_title(&title);
    } else {
        mawaqit_status_row.set_subtitle(&tr("Not configured"));
    }

    let refresh_btn = Button::with_label(&tr("Refresh now"));
    refresh_btn.set_valign(gtk::Align::Center);
    refresh_btn.set_halign(gtk::Align::End);
    mawaqit_status_row.add_suffix(&refresh_btn);
    location_group.add(&mawaqit_status_row);

    location_group.add(&mode_row);
    location_group.add(&lat_row);
    location_group.add(&lon_row);
    location_group.add(&city_row);
    location_group.add(&auto_row);
    location_group.add(&status_row);

    let config_for_auto = config.clone();
    auto_refresh_row.connect_active_notify(move |row| {
        config_for_auto.set_mawaqit_auto_refresh_daily(row.is_active());
        config_for_auto.save();
    });

    let config_for_source = config.clone();
    let list_box_for_source = list_box_rc.clone();
    let url_row_for_source = url_row.clone();
    let auto_row_for_source = auto_refresh_row.clone();
    let status_for_source = mawaqit_status_row.clone();
    let refresh_btn_for_source = refresh_btn.clone();
    source_row.connect_selected_notify(move |row| {
        let show = row.selected() == 1;
        config_for_source.set_prayer_times_source(if show {
            crate::config::PrayerTimesSource::Mawaqit
        } else {
            crate::config::PrayerTimesSource::Calculated
        });
        config_for_source.save();
        url_row_for_source.set_visible(show);
        auto_row_for_source.set_visible(show);
        status_for_source.set_visible(show);
        refresh_btn_for_source.set_visible(show);
        refresh_prayers(&config_for_source, &list_box_for_source);
    });

    let config_for_fetch = config.clone();
    let list_box_for_fetch = list_box_rc.clone();
    let status_for_fetch = mawaqit_status_row.clone();
    let url_row_for_fetch = url_row.clone();
    let loc_tx_for_fetch = loc_tx.clone();
    let current_lang_for_fetch = current_lang.clone();
    let refresh_calendar_for_fetch = refresh_calendar.clone();
    let do_fetch: Rc<dyn Fn()> = Rc::new(move || {
        let raw = url_row_for_fetch.text().to_string();
        if raw.trim().is_empty() {
            status_for_fetch.set_subtitle(&tr("Invalid Mawaqit URL"));
            status_for_fetch.add_css_class("error");
            return;
        }
        let lang = current_lang_for_fetch.borrow().clone();
        status_for_fetch.remove_css_class("error");
        status_for_fetch.set_subtitle(&tr("Fetching..."));
        let cfg = config_for_fetch.clone();
        let list_box = list_box_for_fetch.clone();
        let status = status_for_fetch.clone();
        let tx = loc_tx_for_fetch.clone();
        let refresh_calendar = refresh_calendar_for_fetch.clone();
        gtk::glib::spawn_future_local(async move {
            match crate::mawaqit::fetch_mawaqit_cache(&raw).await {
                Ok(cache) => {
                    let mut maybe_loc_update: Option<(f64, f64, Option<String>)> = None;
                    {
                        cfg.set_mawaqit_url(Some(cache.url.clone()));
                        cfg.set_mawaqit_cache(Some(cache.clone()));
                        if let (Some(lat), Some(lon)) = (cache.latitude, cache.longitude) {
                            cfg.set_latitude(lat);
                            cfg.set_longitude(lon);
                            cfg.set_location_mode(LocationMode::City);
                            let fallback_city = crate::location::localized_mawaqit_city_name(
                                None,
                                cache.timezone.as_deref(),
                                cache.mosque_name.as_deref(),
                                &lang,
                            );
                            if let Some(city) = fallback_city.clone() {
                                cfg.set_city_name(Some(city.clone()));
                                maybe_loc_update = Some((lat, lon, Some(city)));
                            } else {
                                maybe_loc_update = Some((lat, lon, None));
                            }
                        }

                        if let Some(ref tz) = cache.timezone
                            && let Some(ref sys_tz) = crate::location::system_time_zone_id()
                            && !tz.eq_ignore_ascii_case(sys_tz)
                        {
                            cfg.set_timezone_mode(TimezoneMode::Named(tz.clone()));
                            log::info!(
                                "Timezone auto-updated to {} (Mawaqit, different from system {})",
                                tz,
                                sys_tz
                            );
                        }
                        cfg.save();
                    }
                    if let Some((lat, lon, None)) = &maybe_loc_update {
                        let lat = *lat;
                        let lon = *lon;
                        let cfg2 = cfg.clone();
                        let tx2 = tx.clone();
                        let lang2 = lang.clone();
                        gtk::glib::spawn_future_local(async move {
                            if let Ok(name) =
                                crate::location::resolve_city_name(lat, lon, &lang2).await
                            {
                                cfg2.set_city_name(Some(name.clone()));
                                cfg2.save();
                                let _ = tx2.send((lat, lon, Some(name)));
                            }
                        });
                    }
                    if let Some((lat, lon, name)) = maybe_loc_update {
                        let _ = tx.send((lat, lon, name));
                    }
                    let title = cache
                        .mosque_name
                        .clone()
                        .unwrap_or_else(|| cache.url.clone());
                    let tz = cache.timezone.clone().unwrap_or_default();
                    let tz_label = if tz.is_empty() {
                        String::new()
                    } else {
                        location::localized_time_zone_label(&tz, &lang)
                    };
                    let subtitle = if tz_label.is_empty() {
                        format!("{} • {}", tr("Last updated"), cache.fetched_on)
                    } else {
                        format!(
                            "{} • {} • {}",
                            tz_label,
                            tr("Last updated"),
                            cache.fetched_on
                        )
                    };
                    status.set_title(&title);
                    status.set_subtitle(&subtitle);
                    status.remove_css_class("error");
                    refresh_prayers(&cfg, &list_box);
                    refresh_calendar();
                }
                Err(e) => {
                    status.add_css_class("error");
                    status.set_subtitle(&tr(&e));
                }
            }
        });
    });

    let do_fetch_btn = do_fetch.clone();
    refresh_btn.connect_clicked(move |_| {
        do_fetch_btn();
    });
    let do_fetch_entry = do_fetch.clone();
    url_row.connect_entry_activated(move |row| {
        do_fetch_entry();
        finish_entry_row_interaction(row);
    });

    let travel_group = PreferencesGroup::builder()
        .title(gtk::glib::markup_escape_text(&tr("Timezone & Travel")))
        .description(tr("Override the timezone for prayer time calculations."))
        .build();
    travel_group.set_margin_top(12);
    travel_group.set_margin_bottom(24);
    settings_box.append(&travel_group);

    let tz_mode_strings = [
        tr("Automatic (System)"),
        tr("Custom Timezone (IANA)"),
        tr("Manual UTC Offset"),
    ];
    let tz_mode_slices: Vec<&str> = tz_mode_strings.iter().map(|s| s.as_str()).collect();
    let tz_modes = StringList::new(&tz_mode_slices);
    let tz_mode_row = ComboRow::builder()
        .title(tr("Timezone Mode"))
        .subtitle(tr("How prayer times are adjusted for your timezone."))
        .model(&tz_modes)
        .build();

    let current_tz_mode = config.timezone_mode();
    let tz_init_selected = match &current_tz_mode {
        TimezoneMode::Auto => 0u32,
        TimezoneMode::Named(_) => 1,
        TimezoneMode::UtcOffset(_) => 2,
    };
    tz_mode_row.set_selected(tz_init_selected);
    travel_group.add(&tz_mode_row);

    let tz_named_init = match &current_tz_mode {
        TimezoneMode::Named(s) => s.clone(),
        _ => location::system_time_zone_id().unwrap_or_default(),
    };
    let tz_named_row = adw::EntryRow::builder()
        .title(tr("IANA Timezone"))
        .text(&tz_named_init)
        .show_apply_button(false)
        .visible(tz_init_selected == 1)
        .build();
    tz_named_row.set_input_hints(gtk::InputHints::NO_SPELLCHECK);
    tz_named_row.set_direction(gtk::TextDirection::Ltr);
    tz_named_row.add_prefix(&gtk::Image::from_icon_name("mark-location-symbolic"));
    travel_group.add(&tz_named_row);

    let current_lang_tz_val = current_lang.clone();
    let update_tz_named_validation = Rc::new({
        let tz_named_row = tz_named_row.clone();
        move |text: &str, keep_success_state: bool| {
            let lang = current_lang_tz_val.borrow().clone();
            tz_named_row.remove_css_class("error");
            tz_named_row.remove_css_class("success");

            if let Some(name) = location::validated_time_zone_id(text) {
                if keep_success_state && !text.trim().is_empty() {
                    tz_named_row.add_css_class("success");
                }
                tz_named_row
                    .set_tooltip_text(Some(&location::localized_time_zone_label(&name, &lang)));
            } else if text.trim().is_empty() {
                tz_named_row.set_tooltip_text(None);
            } else {
                tz_named_row.add_css_class("error");
                tz_named_row.set_tooltip_text(None);
            }
        }
    });
    update_tz_named_validation(&tz_named_init, false);

    let tz_adj = gtk::Adjustment::new(0.0, -12.0, 14.0, 0.5, 0.0, 0.0);
    if let TimezoneMode::UtcOffset(mins) = &current_tz_mode {
        tz_adj.set_value(*mins as f64 / 60.0);
    }
    let tz_offset_row = adw::SpinRow::builder()
        .title(tr("UTC Offset (hours)"))
        .subtitle(tr("Example: +2.0 for UTC+2, -5.0 for UTC-5"))
        .adjustment(&tz_adj)
        .digits(1)
        .visible(tz_init_selected == 2)
        .build();
    travel_group.add(&tz_offset_row);

    let tz_named_vis = tz_named_row.clone();
    let tz_offset_vis = tz_offset_row.clone();
    let config_tz_mode = config.clone();
    let list_box_tz = list_box_rc.clone();
    let tz_adj_for_mode = tz_adj.clone();
    let update_tz_named_for_mode = update_tz_named_validation.clone();
    tz_mode_row.connect_selected_notify(move |combo| {
        let sel = combo.selected();
        tz_named_vis.set_visible(sel == 1);
        tz_offset_vis.set_visible(sel == 2);
        let tz_named_text = tz_named_vis.text().to_string();
        let existing_named = match config_tz_mode.timezone_mode() {
            TimezoneMode::Named(name) if !name.trim().is_empty() => Some(name),
            _ => None,
        };
        let new_mode = match sel {
            1 => {
                if let Some(name) = location::validated_time_zone_id(&tz_named_text) {
                    tz_named_vis.set_text(&name);
                    TimezoneMode::Named(name)
                } else if let Some(name) = existing_named {
                    TimezoneMode::Named(name)
                } else {
                    TimezoneMode::Auto
                }
            }
            2 => TimezoneMode::UtcOffset((tz_adj_for_mode.value() * 60.0) as i32),
            _ => TimezoneMode::Auto,
        };
        config_tz_mode.set_timezone_mode(new_mode);
        config_tz_mode.save();
        refresh_prayers(&config_tz_mode, &list_box_tz);
        update_tz_named_for_mode(&tz_named_vis.text(), sel == 1 && tz_named_vis.has_focus());
    });

    let update_tz_named_for_change = update_tz_named_validation.clone();
    tz_named_row.connect_changed(move |row| {
        let text = row.text().to_string();
        update_tz_named_for_change(&text, row.has_focus());
    });

    let config_tz_named = config.clone();
    let list_box_tz_named = list_box_rc.clone();
    let tz_mode_row_for_apply = tz_mode_row.clone();
    let tz_named_row_for_apply = tz_named_row.clone();
    let update_tz_named_for_apply = update_tz_named_validation.clone();
    let apply_named_timezone = Rc::new(move || {
        if tz_mode_row_for_apply.selected() != 1 {
            return;
        }
        let raw_text = tz_named_row_for_apply.text().to_string();
        update_tz_named_for_apply(&raw_text, tz_named_row_for_apply.has_focus());

        if let Some(name) = location::validated_time_zone_id(&raw_text) {
            tz_named_row_for_apply.set_text(&name);
            update_tz_named_for_apply(&name, tz_named_row_for_apply.has_focus());
            config_tz_named.set_timezone_mode(crate::config::TimezoneMode::Named(name));
            config_tz_named.save();
            refresh_prayers(&config_tz_named, &list_box_tz_named);
        }
    });

    let apply_named_timezone_from_enter = apply_named_timezone.clone();
    tz_named_row.connect_entry_activated(move |row| {
        apply_named_timezone_from_enter();
        finish_entry_row_interaction(row);
    });

    let apply_named_timezone_on_blur = apply_named_timezone.clone();
    let update_tz_named_for_focus = update_tz_named_validation.clone();
    tz_named_row.connect_has_focus_notify(move |row| {
        if !row.has_focus() {
            apply_named_timezone_on_blur();
        }
        let text = row.text().to_string();
        update_tz_named_for_focus(&text, row.has_focus());
    });

    let config_tz_offset = config.clone();
    let list_box_tz_offset = list_box_rc.clone();
    tz_adj.connect_value_changed(move |adj| {
        if let TimezoneMode::UtcOffset(_) = config_tz_offset.timezone_mode() {
            config_tz_offset.set_timezone_mode(crate::config::TimezoneMode::UtcOffset(
                (adj.value() * 60.0) as i32,
            ));
            config_tz_offset.save();
            refresh_prayers(&config_tz_offset, &list_box_tz_offset);
        }
    });

    let calc_group = PreferencesGroup::builder().title(tr("Calculation")).build();
    calc_group.set_margin_top(12);
    calc_group.set_margin_bottom(24);
    settings_box.append(&calc_group);

    let hijri_adj = gtk::Adjustment::new(config.hijri_offset() as f64, -2.0, 2.0, 1.0, 0.0, 0.0);
    let hijri_row = adw::SpinRow::builder()
        .title(tr("Hijri Date Correction"))
        .subtitle(tr("Adjust Hijri date by +/- days"))
        .adjustment(&hijri_adj)
        .digits(0)
        .build();

    let config_hijri = config.clone();
    let refresh_calendar_hijri = refresh_calendar.clone();
    hijri_adj.connect_value_changed(move |adj| {
        config_hijri.set_hijri_offset(adj.value() as i64);
        config_hijri.save();
        refresh_calendar_hijri();
    });
    calc_group.add(&hijri_row);

    let methods_strings = [
        tr("MWL"),
        tr("ISNA"),
        tr("Egypt"),
        tr("Makkah"),
        tr("Karachi"),
        tr("Dubai"),
        tr("MoonsightingCommittee"),
        tr("Kuwait"),
        tr("Qatar"),
        tr("Singapore"),
        tr("Turkey"),
        tr("KEMENAG"),
        tr("France (UOIF)"),
        tr("Algeria"),
    ];
    let methods_slices: Vec<&str> = methods_strings.iter().map(|s| s.as_str()).collect();
    let methods = StringList::new(&methods_slices);
    let method_row = ComboRow::builder()
        .title(tr("Calculation Method"))
        .model(&methods)
        .build();

    let current_method = config.method();
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
        CalculationMethod::Kemenag => 11,
        CalculationMethod::France => 12,
        CalculationMethod::Algeria => 13,
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
            11 => CalculationMethod::Kemenag,
            12 => CalculationMethod::France,
            13 => CalculationMethod::Algeria,
            _ => CalculationMethod::MWL,
        };
        config_method.set_method(method);
        config_method.save();
        refresh_prayers(&config_method, &list_box_method);
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
    let source_row_for_mode = source_row.clone();
    let url_row_for_mode = url_row.clone();
    let auto_row_for_mode = auto_refresh_row.clone();
    let status_row_for_mode = mawaqit_status_row.clone();
    let refresh_btn_for_mode = refresh_btn.clone();
    mode_row.connect_selected_notify(move |combo| {
        let mode = match combo.selected() {
            0 => LocationMode::Manual,
            1 => LocationMode::City,
            2 => LocationMode::Auto,
            _ => LocationMode::Manual,
        };
        let was_mawaqit = config_mode.prayer_times_source() == PrayerTimesSource::Mawaqit;
        if was_mawaqit {
            config_mode.set_prayer_times_source(crate::config::PrayerTimesSource::Calculated);
        }
        config_mode.set_latitude(config_mode.latitude());
        config_mode.set_longitude(config_mode.longitude());
        config_mode.set_location_mode(mode.clone());
        config_mode.save();
        if was_mawaqit {
            source_row_for_mode.set_selected(0);
            url_row_for_mode.set_visible(false);
            auto_row_for_mode.set_visible(false);
            status_row_for_mode.set_visible(false);
            refresh_btn_for_mode.set_visible(false);
        }
        AppConfig::save_shared(&config_mode);
        update_vis_clone(&mode);
        refresh_prayers(&config_mode, &list_box_mode);
    });

    let madhab_strings = [tr("Shafi (Standard/Maliki/Hanbali)"), tr("Hanafi")];
    let madhab_slices: Vec<&str> = madhab_strings.iter().map(|s| s.as_str()).collect();
    let madhabs = StringList::new(&madhab_slices);
    let madhab_row = ComboRow::builder()
        .title(tr("Asr Calculation (Madhab)"))
        .model(&madhabs)
        .build();

    let current_madhab = config.madhab();
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
        config_madhab.set_madhab(m);
        config_madhab.save();
        refresh_prayers(&config_madhab, &list_box_madhab);
    });
    calc_group.add(&madhab_row);

    let note_row = adw::ActionRow::builder()
        .title(tr("Note"))
        .subtitle(tr("Maliki/Hanbali use Standard (Shafi) for Asr."))
        .build();
    calc_group.add(&note_row);

    let iqamah_group = PreferencesGroup::builder()
        .title(tr("Iqamah Delays"))
        .description(tr(
            "Minutes to wait after the Adhan before the Iqamah (second call to prayer).",
        ))
        .build();
    iqamah_group.set_margin_top(12);
    iqamah_group.set_margin_bottom(24);
    settings_box.append(&iqamah_group);

    let (notif_audio_heading, notif_audio_desc) = append_settings_section_heading(
        settings_box,
        &tr("Notifications & Audio"),
        Some(&tr(
            "Choose when and how you receive prayer reminders and the Adhan sound.",
        )),
        24,
    );
    let notif_audio_desc = notif_audio_desc.expect("notifications description label");

    let notif_group = PreferencesGroup::new();
    notif_group.set_margin_top(0);
    notif_group.set_margin_bottom(12);
    settings_box.append(&notif_group);

    let notify_toggle = adw::SwitchRow::builder()
        .title(tr("Pre-Prayer Alert"))
        .subtitle(tr("Get notified before the prayer time."))
        .build();
    notify_toggle.set_active(config.pre_prayer_notify());

    let iqamah_notify_toggle = adw::SwitchRow::builder()
        .title(tr("Iqamah Alert"))
        .subtitle(tr("Get notified when it's time for Iqamah."))
        .build();
    let adkar_toggle = adw::SwitchRow::builder()
        .title(tr("Adkar"))
        .subtitle(tr("Morning, evening, and night invocation reminders."))
        .build();

    iqamah_notify_toggle.set_active(config.iqamah_notify());
    adkar_toggle.set_active(config.adkar_notification_enabled());

    let notify_toggle_for_sync = notify_toggle.clone();
    let iqamah_toggle_for_sync = iqamah_notify_toggle.clone();
    let adkar_toggle_for_sync = adkar_toggle.clone();

    let adhan_only_toggle = adw::SwitchRow::builder()
        .title(tr("Adhan Only Mode"))
        .subtitle(tr(
            "Show only the Adhan notification. Disables all other notifications.",
        ))
        .build();
    adhan_only_toggle.set_active(config.adhan_only_mode());

    let sync_ui = move |enabled: bool| {
        notify_toggle_for_sync.set_sensitive(!enabled);
        iqamah_toggle_for_sync.set_sensitive(!enabled);
        adkar_toggle_for_sync.set_sensitive(!enabled);
        if enabled {
            notify_toggle_for_sync.set_active(false);
            iqamah_toggle_for_sync.set_active(false);
            adkar_toggle_for_sync.set_active(false);
        } else {
            notify_toggle_for_sync.set_active(true);
            iqamah_toggle_for_sync.set_active(true);
            adkar_toggle_for_sync.set_active(true);
        }
    };

    notify_toggle.set_sensitive(!config.adhan_only_mode());
    iqamah_notify_toggle.set_sensitive(!config.adhan_only_mode());
    adkar_toggle.set_sensitive(!config.adhan_only_mode());

    let config_only = config.clone();
    adhan_only_toggle.connect_active_notify(move |row| {
        let enabled = row.is_active();
        config_only.set_adhan_only_mode(enabled);
        if enabled {
            config_only.set_pre_prayer_notify(false);
            config_only.set_pre_prayer_minutes(config_only.pre_prayer_minutes());
            config_only.set_iqamah_notify(false);
            config_only.set_adkar_notification_enabled(false);
        } else {
            config_only.set_pre_prayer_notify(true);
            config_only.set_pre_prayer_minutes(config_only.pre_prayer_minutes());
            config_only.set_iqamah_notify(true);
            config_only.set_adkar_notification_enabled(true);
        }
        config_only.save();
        sync_ui(enabled);
    });

    notif_group.add(&notify_toggle);

    let notify_time = adw::SpinRow::builder()
        .title(tr("Alert Time"))
        .subtitle(tr("Minutes before prayer"))
        .adjustment(&gtk::Adjustment::new(
            config.pre_prayer_minutes() as f64,
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
        let new_minutes = adj.value() as u32;
        config_time.set_pre_prayer_minutes(new_minutes);
        config_time.save();
    });
    notif_group.add(&notify_time);

    let time_row_clone = notify_time.clone();
    notify_toggle.connect_active_notify(move |row| {
        time_row_clone.set_visible(row.is_active());
    });
    notify_time.set_visible(config.pre_prayer_notify());

    notif_group.add(&iqamah_notify_toggle);
    notif_group.add(&adkar_toggle);
    notif_group.add(&adhan_only_toggle);

    let config_notify = config.clone();
    notify_toggle.connect_active_notify(move |row| {
        config_notify.set_pre_prayer_notify(row.is_active());
        config_notify.set_pre_prayer_minutes(config_notify.pre_prayer_minutes());
        config_notify.save();
    });

    let config_iq = config.clone();
    iqamah_notify_toggle.connect_active_notify(move |row| {
        config_iq.set_iqamah_notify(row.is_active());
        config_iq.save();
    });

    let config_adkar = config.clone();
    adkar_toggle.connect_active_notify(move |row| {
        config_adkar.set_adkar_notification_enabled(row.is_active());
        config_adkar.save();
    });

    let test_notify_btn = Button::builder()
        .label(tr("Test Notification"))
        .margin_top(12)
        .build();

    let config_test_notif = config.clone();
    bind_audio_toggle_button_sync(&test_notify_btn, "Test Notification");
    test_notify_btn.connect_clicked(move |btn| {
        if crate::audio::is_playing() {
            crate::audio::stop();
            set_audio_toggle_button_label(btn, "Test Notification", false);
        } else {
            notifications::show_notification(
                &tr("It's time for"),
                &tr("This is a test notification from Khushu. May your prayers be accepted."),
                true,
                &tr("Open Khushu"),
                &tr("Stop Adhan"),
            );
            if !config_test_notif.adhan_muted() {
                let path = config_test_notif
                    .adhan_sound_path()
                    .unwrap_or_else(|| "assets/audio/Madinah.mp3".to_string());
                crate::audio::play_adhan(&path, config_test_notif.adhan_volume());
                set_audio_toggle_button_label(btn, "Test Notification", true);
            }
        }
    });

    notif_group.add(&test_notify_btn);

    let prayer_iqamah_defs = [
        ("Fajr", 20u32),
        ("Dhuhr", 10u32),
        ("Asr", 10u32),
        ("Maghrib", 5u32),
        ("Isha", 10u32),
    ];

    let mut iqamah_rows = Vec::new();
    for (prayer_name, default_mins) in prayer_iqamah_defs {
        let current = config
            .iqamah_minutes()
            .get(prayer_name)
            .copied()
            .unwrap_or(default_mins);
        let iq_adj = gtk::Adjustment::new(current as f64, 0.0, 60.0, 1.0, 5.0, 0.0);
        let iq_row = adw::SpinRow::builder()
            .title(tr(prayer_name))
            .subtitle(tr("Minutes"))
            .adjustment(&iq_adj)
            .digits(0)
            .build();
        iqamah_rows.push(iq_row.clone());
        iqamah_group.add(&iq_row);

        let config_iq = config.clone();
        let prayer_key = prayer_name.to_string();
        iq_adj.connect_value_changed(move |adj| {
            let mut mins = config_iq.iqamah_minutes();
            mins.insert(prayer_key.clone(), adj.value() as u32);
            config_iq.set_iqamah_minutes(mins);
            config_iq.save();
        });
    }

    let audio_group = PreferencesGroup::new();
    audio_group.set_margin_bottom(12);
    settings_box.append(&audio_group);

    let preset_files: Vec<String> = vec!["Madinah.mp3".to_string(), "Makkah.mp3".to_string()];

    let mut preset_labels: Vec<String> = Vec::new();
    preset_labels.push(tr("Default"));
    preset_labels.push(tr("Custom File..."));
    for name in &preset_files {
        preset_labels.push(adhan_preset_label(name));
    }

    let label_refs: Vec<&str> = preset_labels.iter().map(|s| s.as_str()).collect();
    let model = gtk::StringList::new(&label_refs);

    let sound_combo = ComboRow::builder()
        .title(tr("Adhan Sound"))
        .model(&model)
        .build();

    let current_path = config.adhan_sound_path();
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
        sound_combo.set_subtitle(&tr("Using builtin default"));
    }

    let window_clone_sound = window.clone();
    let config_sound = config.clone();
    let preset_files_clone = preset_files.clone();

    sound_combo.connect_selected_notify(move |combo| {
        let index = combo.selected() as usize;

        if index == 0 {
            config_sound.set_adhan_sound_path(None);
            config_sound.save();
            combo.set_subtitle(&tr("Using builtin default"));
        } else if index == 1 {
            let file_filter = gtk::FileFilter::new();
            file_filter.set_name(Some(&tr("Audio Files")));
            file_filter.add_mime_type("audio/mpeg");
            file_filter.add_mime_type("audio/mp3");
            file_filter.add_mime_type("audio/ogg");

            let filters = gtk::gio::ListStore::new::<gtk::FileFilter>();
            filters.append(&file_filter);

            let dialog = gtk::FileDialog::builder()
                .title(tr("Select Adhan Sound"))
                .modal(true)
                .filters(&filters)
                .build();

            let _config_dialog = config_sound.clone();
            let combo_dialog = combo.clone();
            let parent_window = window_clone_sound.clone();

            gtk::glib::spawn_future_local(async move {
                if let Ok(file) = dialog.open_future(Some(&parent_window)).await
                    && let Some(path) = file.path()
                    && let Some(path_str) = path.to_str()
                {
                    let combo = combo_dialog.clone();
                    let parent = parent_window.clone();
                    crate::audio::validate_audio_async(path_str.to_string(), combo, parent);
                }
            });
        } else {
            let mut path = PathBuf::from("assets/audio");
            let file_name = &preset_files_clone[index - 2];
            path.push(file_name);
            if let Some(path_str) = path.to_str() {
                config_sound.set_adhan_sound_path(Some(path_str.to_string()));
                config_sound.save();
                combo.set_subtitle(path_str);
            }
        }
    });

    audio_group.add(&sound_combo);

    let mute_toggle = adw::SwitchRow::builder()
        .title(tr("Mute Adhan"))
        .subtitle(tr("Silence the Adhan sound at prayer time."))
        .build();
    mute_toggle.set_active(config.adhan_muted());
    let config_mute = config.clone();
    mute_toggle.connect_active_notify(move |row| {
        config_mute.set_adhan_muted(row.is_active());
        config_mute.save();
    });
    audio_group.add(&mute_toggle);

    let volume_adj = gtk::Adjustment::new(
        (config.adhan_volume() * 100.0) as f64,
        0.0,
        100.0,
        5.0,
        10.0,
        0.0,
    );
    let volume_row = adw::SpinRow::builder()
        .title(tr("Adhan Volume"))
        .subtitle(tr("Volume level (0–100%)"))
        .adjustment(&volume_adj)
        .digits(0)
        .build();
    volume_row.set_visible(!config.adhan_muted());

    let config_vol = config.clone();
    volume_adj.connect_value_changed(move |adj| {
        config_vol.set_adhan_volume((adj.value() / 100.0) as f32);
        config_vol.save();
    });
    audio_group.add(&volume_row);

    let volume_row_clone = volume_row.clone();
    mute_toggle.connect_active_notify(move |row| {
        volume_row_clone.set_visible(!row.is_active());
    });

    let test_audio_btn = Button::builder()
        .label(tr("▶ Preview Adhan"))
        .margin_top(8)
        .build();

    let config_test = config.clone();
    bind_audio_toggle_button_sync(&test_audio_btn, "▶ Preview Adhan");
    test_audio_btn.connect_clicked(move |btn| {
        if crate::audio::is_playing() {
            crate::audio::stop();
            set_audio_toggle_button_label(btn, "▶ Preview Adhan", false);
        } else {
            if config_test.adhan_muted() {
                return;
            }
            let path = config_test
                .adhan_sound_path()
                .unwrap_or_else(|| "assets/audio/Madinah.mp3".to_string());

            crate::audio::play_adhan(&path, config_test.adhan_volume());
            set_audio_toggle_button_label(btn, "▶ Preview Adhan", true);
        }
    });
    audio_group.add(&test_audio_btn);

    let ctx = SettingsUiContext {
        config: config.clone(),
        list_box_rc: list_box_rc.clone(),
        window: window.clone(),
        current_lang: current_lang.clone(),
        loc_tx: loc_tx.clone(),
        refresh_calendar: refresh_calendar.clone(),
        settings_box: settings_box.clone(),

        general_heading,
        general_desc,
        lang_row: lang_row.clone(),
        lang_model: lang_model.clone(),
        theme_row: theme_row.clone(),
        theme_model: theme_model.clone(),
        autostart_toggle,

        prayer_setup_heading,
        prayer_setup_desc,
        location_group: location_group.clone(),
        mode_row: mode_row.clone(),
        mode_model: modes.clone(),
        lat_row: lat_row.clone(),
        lon_row: lon_row.clone(),
        status_row: status_row.clone(),
        city_row: city_row.clone(),
        city_btn: city_btn.clone(),
        auto_row: auto_row.clone(),
        auto_btn: auto_btn.clone(),
        source_row: source_row.clone(),
        source_model: source_model.clone(),
        url_row: url_row.clone(),
        auto_refresh_row: auto_refresh_row.clone(),
        mawaqit_status_row: mawaqit_status_row.clone(),
        refresh_btn: refresh_btn.clone(),

        travel_group: travel_group.clone(),
        tz_mode_row: tz_mode_row.clone(),
        tz_mode_model: tz_modes.clone(),
        tz_named_row: tz_named_row.clone(),
        tz_offset_row: tz_offset_row.clone(),

        calc_group: calc_group.clone(),
        hijri_row: hijri_row.clone(),
        method_row: method_row.clone(),
        method_model: methods.clone(),
        madhab_row: madhab_row.clone(),
        madhab_model: madhabs.clone(),
        note_row: note_row.clone(),

        iqamah_group: iqamah_group.clone(),
        iqamah_rows,

        notif_audio_heading,
        notif_audio_desc,
        notify_toggle: notify_toggle.clone(),
        notify_time: notify_time.clone(),
        iqamah_notify_toggle: iqamah_notify_toggle.clone(),
        adkar_toggle: adkar_toggle.clone(),
        adhan_only_toggle: adhan_only_toggle.clone(),
        test_notify_btn: test_notify_btn.clone(),

        audio_group: audio_group.clone(),
        sound_combo: sound_combo.clone(),
        sound_model: model.clone(),
        preset_files,
        mute_toggle: mute_toggle.clone(),
        volume_row: volume_row.clone(),
        test_audio_btn,
    };

    (lang_row, Rc::new(RefCell::new(ctx)))
}

pub fn update_settings_ui_lang(ctx: &SettingsUiContext, lang: &str) {
    let cfg = &ctx.config;

    ctx.general_heading.set_label(&tr("General"));
    ctx.general_desc
        .set_label(&tr("Customize the app's appearance and startup behavior."));

    ctx.lang_row.set_title(&tr("Language"));
    let lang_items = [
        tr("System Default"),
        tr("English"),
        tr("Arabic"),
        tr("French"),
        tr("Spanish"),
        tr("Turkish"),
        tr("Indonesian"),
    ];
    let lang_refs: Vec<&str> = lang_items.iter().map(|s| s.as_str()).collect();
    ctx.lang_model
        .splice(0, ctx.lang_model.n_items(), &lang_refs);

    ctx.theme_row.set_title(&tr("Theme"));
    let theme_items = [tr("System Default"), tr("Light"), tr("Dark")];
    let theme_refs: Vec<&str> = theme_items.iter().map(|s| s.as_str()).collect();
    ctx.theme_model
        .splice(0, ctx.theme_model.n_items(), &theme_refs);

    ctx.autostart_toggle.set_title(&tr("Start Automatically"));
    ctx.autostart_toggle
        .set_subtitle(&tr("Run Khushu in the background when you log in."));

    ctx.prayer_setup_heading.set_label(&tr("Prayer Setup"));
    ctx.prayer_setup_desc.set_label(&tr("Set your location, prayer times source, timezone, calculation methods, and Iqamah delays for each prayer."));

    ctx.location_group
        .set_title(&gtk::glib::markup_escape_text(&tr("Location & Source")));
    ctx.location_group.set_description(Some(&tr(
        "Set your location and choose the prayer times data source.",
    )));

    let mode_items = [
        tr("Manual Coordinates"),
        tr("City Selection"),
        tr("Auto (GPS/Network)"),
    ];
    let mode_refs: Vec<&str> = mode_items.iter().map(|s| s.as_str()).collect();
    ctx.mode_model
        .splice(0, ctx.mode_model.n_items(), &mode_refs);
    ctx.mode_row.set_title(&tr("Location Method"));

    ctx.lat_row.set_title(&tr("Latitude"));
    ctx.lon_row.set_title(&tr("Longitude"));

    ctx.status_row.set_title(&tr("Location Status"));

    ctx.city_row.set_title(&tr("City Search"));
    ctx.city_btn.set_label(&tr("Search"));

    let city_row_reloc = ctx.city_row.clone();
    let auto_row_reloc = ctx.auto_row.clone();
    let current_lang_reloc = ctx.current_lang.clone();
    let loc_mode = cfg.location_mode();
    let lat_reloc = cfg.latitude();
    let lon_reloc = cfg.longitude();
    let lang_reloc = lang.to_string();

    if cfg.location_mode() == crate::config::LocationMode::City
        && let Some(text) = location::display_city_label(
            cfg.city_name().as_deref(),
            cfg.mawaqit_cache().as_ref(),
            lang,
        )
    {
        ctx.city_row.set_text(&text);
    }

    ctx.auto_row.set_title(&tr("Auto Detection"));
    ctx.auto_btn.set_label(&tr("Update Now"));

    if cfg.location_mode() == crate::config::LocationMode::Auto
        && let Some(name) = &cfg.city_name()
    {
        ctx.auto_row
            .set_subtitle(&location::short_city_with_country(name));
    }

    if matches!(
        loc_mode,
        crate::config::LocationMode::City | crate::config::LocationMode::Auto
    ) && cfg.mawaqit_cache().is_none()
    {
        gtk::glib::spawn_future_local(async move {
            if let Ok(name) =
                crate::location::resolve_city_name(lat_reloc, lon_reloc, &lang_reloc).await
                && current_lang_reloc.borrow().as_str() == lang_reloc
            {
                let short = crate::location::short_city_with_country(&name);
                if loc_mode == crate::config::LocationMode::City {
                    city_row_reloc.set_text(&short);
                } else {
                    auto_row_reloc.set_subtitle(&short);
                }
            }
        });
    }

    let source_items = [tr("Calculated (Offline)"), tr("Connected Mosque (URL)")];
    let source_refs: Vec<&str> = source_items.iter().map(|s| s.as_str()).collect();
    ctx.source_model
        .splice(0, ctx.source_model.n_items(), &source_refs);
    ctx.source_row.set_title(&tr("Prayer Times Source"));

    ctx.url_row
        .set_title(&tr("Connected Mosque URL (mawaqit.net)"));

    ctx.auto_refresh_row.set_title(&tr("Auto refresh daily"));
    ctx.auto_refresh_row.set_subtitle(&tr(
        "Refresh mosque prayer times once per day while the app is open.",
    ));

    ctx.mawaqit_status_row.set_title(&tr("Connected Mosque"));
    if let Some(cache) = cfg.mawaqit_cache().as_ref() {
        let tz = cache.timezone.clone().unwrap_or_default();
        let tz_label = if tz.is_empty() {
            String::new()
        } else {
            location::localized_time_zone_label(&tz, lang)
        };
        let subtitle = if tz_label.is_empty() {
            format!("{} • {}", tr("Last updated"), cache.fetched_on)
        } else {
            format!(
                "{} • {} • {}",
                tz_label,
                tr("Last updated"),
                cache.fetched_on
            )
        };
        ctx.mawaqit_status_row.set_subtitle(&subtitle);
    } else {
        ctx.mawaqit_status_row.set_subtitle(&tr("Not configured"));
    }
    ctx.refresh_btn.set_label(&tr("Refresh now"));

    ctx.travel_group
        .set_title(&gtk::glib::markup_escape_text(&tr("Timezone & Travel")));
    ctx.travel_group.set_description(Some(&tr(
        "Override the timezone for prayer time calculations.",
    )));

    let tz_mode_items = [
        tr("Automatic (System)"),
        tr("Custom Timezone (IANA)"),
        tr("Manual UTC Offset"),
    ];
    let tz_mode_refs: Vec<&str> = tz_mode_items.iter().map(|s| s.as_str()).collect();
    ctx.tz_mode_model
        .splice(0, ctx.tz_mode_model.n_items(), &tz_mode_refs);
    ctx.tz_mode_row.set_title(&tr("Timezone Mode"));
    ctx.tz_mode_row
        .set_subtitle(&tr("How prayer times are adjusted for your timezone."));

    ctx.tz_named_row.set_title(&tr("IANA Timezone"));

    ctx.tz_offset_row.set_title(&tr("UTC Offset (hours)"));
    ctx.tz_offset_row
        .set_subtitle(&tr("Example: +2.0 for UTC+2, -5.0 for UTC-5"));

    ctx.calc_group
        .set_title(&gtk::glib::markup_escape_text(&tr("Calculation")));

    ctx.hijri_row.set_title(&tr("Hijri Date Correction"));
    ctx.hijri_row
        .set_subtitle(&tr("Adjust Hijri date by +/- days"));

    let method_items = [
        tr("MWL"),
        tr("ISNA"),
        tr("Egypt"),
        tr("Makkah"),
        tr("Karachi"),
        tr("Dubai"),
        tr("MoonsightingCommittee"),
        tr("Kuwait"),
        tr("Qatar"),
        tr("Singapore"),
        tr("Turkey"),
        tr("KEMENAG"),
        tr("France (UOIF)"),
        tr("Algeria"),
    ];
    let method_refs: Vec<&str> = method_items.iter().map(|s| s.as_str()).collect();
    ctx.method_model
        .splice(0, ctx.method_model.n_items(), &method_refs);
    ctx.method_row.set_title(&tr("Calculation Method"));

    let madhab_items = [tr("Shafi (Standard/Maliki/Hanbali)"), tr("Hanafi")];
    let madhab_refs: Vec<&str> = madhab_items.iter().map(|s| s.as_str()).collect();
    ctx.madhab_model
        .splice(0, ctx.madhab_model.n_items(), &madhab_refs);
    ctx.madhab_row.set_title(&tr("Asr Calculation (Madhab)"));

    ctx.note_row.set_title(&tr("Note"));
    ctx.note_row
        .set_subtitle(&tr("Maliki/Hanbali use Standard (Shafi) for Asr."));

    ctx.iqamah_group.set_title(&tr("Iqamah Delays"));
    ctx.iqamah_group.set_description(Some(&tr(
        "Minutes to wait after the Adhan before the Iqamah (second call to prayer).",
    )));

    let prayer_names = ["Fajr", "Dhuhr", "Asr", "Maghrib", "Isha"];
    for (i, name) in prayer_names.iter().enumerate() {
        if let Some(row) = ctx.iqamah_rows.get(i) {
            row.set_title(&tr(name));
            row.set_subtitle(&tr("Minutes"));
        }
    }

    ctx.notif_audio_heading
        .set_label(&tr("Notifications & Audio"));
    ctx.notif_audio_desc.set_label(&tr(
        "Choose when and how you receive prayer reminders and the Adhan sound.",
    ));

    ctx.notify_toggle.set_title(&tr("Pre-Prayer Alert"));
    ctx.notify_toggle
        .set_subtitle(&tr("Get notified before the prayer time."));

    ctx.notify_time.set_title(&tr("Alert Time"));
    ctx.notify_time.set_subtitle(&tr("Minutes before prayer"));

    ctx.iqamah_notify_toggle.set_title(&tr("Iqamah Alert"));
    ctx.iqamah_notify_toggle
        .set_subtitle(&tr("Get notified when it's time for Iqamah."));

    ctx.adkar_toggle.set_title(&tr("Adkar"));
    ctx.adkar_toggle
        .set_subtitle(&tr("Morning, evening, and night invocation reminders."));

    ctx.adhan_only_toggle.set_title(&tr("Adhan Only Mode"));
    ctx.adhan_only_toggle.set_subtitle(&tr(
        "Show only the Adhan notification. Disables all other notifications.",
    ));

    ctx.test_notify_btn.set_label(&tr("Test Notification"));

    let mut preset_labels: Vec<String> = Vec::new();
    preset_labels.push(tr("Default"));
    preset_labels.push(tr("Custom File..."));
    for name in &ctx.preset_files {
        preset_labels.push(adhan_preset_label(name));
    }
    let preset_refs: Vec<&str> = preset_labels.iter().map(|s| s.as_str()).collect();
    ctx.sound_model
        .splice(0, ctx.sound_model.n_items(), &preset_refs);
    ctx.sound_combo.set_title(&tr("Adhan Sound"));

    let current_path = ctx.config.adhan_sound_path();
    if let Some(path) = &current_path {
        let path_obj = PathBuf::from(path);
        if let Some(name) = path_obj.file_name().and_then(|n| n.to_str()) {
            if let Some(pos) = ctx.preset_files.iter().position(|p| p.as_str() == name) {
                ctx.sound_combo.set_selected((pos + 2) as u32);
            } else {
                ctx.sound_combo.set_selected(1);
                ctx.sound_combo.set_subtitle(path);
            }
        } else {
            ctx.sound_combo.set_selected(1);
            ctx.sound_combo.set_subtitle(path);
        }
    } else {
        ctx.sound_combo.set_selected(0);
        ctx.sound_combo.set_subtitle(&tr("Using builtin default"));
    }

    ctx.mute_toggle.set_title(&tr("Mute Adhan"));
    ctx.mute_toggle
        .set_subtitle(&tr("Silence the Adhan sound at prayer time."));

    ctx.volume_row.set_title(&tr("Adhan Volume"));
    ctx.volume_row.set_subtitle(&tr("Volume level (0–100%)"));

    ctx.test_audio_btn.set_label(&tr("▶ Preview Adhan"));
}

fn adhan_preset_label(file_name: &str) -> String {
    let stem = std::path::Path::new(file_name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(file_name);
    match stem {
        "Makkah" => tr("Makkah Adhan"),
        "Madinah" => tr("Madinah Adhan"),
        _ => stem.to_string(),
    }
}

pub fn refresh_prayers(config: &AppConfig, list_box: &ListBox) {
    while let Some(child) = list_box.first_child() {
        list_box.remove(&child);
    }

    let today = crate::time::effective_today(config);
    if let Some(schedule) = crate::time::schedule_for_config(config, today) {
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
                .title(tr(name))
                .subtitle(time.format("%H:%M").to_string())
                .name(name)
                .build();
            list_box.append(&row);
        }
    }
}
