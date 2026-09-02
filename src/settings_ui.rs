use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;

use chrono::Datelike;

use adw::prelude::*;
use adw::{ComboRow, PreferencesGroup};
use gtk::glib::WeakRef;
use gtk::{Button, ListBox, StringList};
use gtk4 as gtk;
use libadwaita as adw;

use crate::config::{
    AppConfig, CalculationMethod, HighLatitudeChoice, LocationMode, MadhabChoice,
    PolarEstimationMethod, PrayerTimesSource, TimezoneMode,
};
use crate::i18n::tr;
use crate::location;
use crate::notifications;

struct AudioButtonEntry {
    audio_button: WeakRef<Button>,
    idle_label_key: &'static str,
}

thread_local! {
    static AUDIO_BUTTONS: RefCell<Vec<AudioButtonEntry>> = const { RefCell::new(Vec::new()) };
}

pub fn register_audio_button(button: &Button, idle_label_key: &'static str) {
    set_audio_toggle_button_label(button, idle_label_key, crate::audio::is_adhan());
    AUDIO_BUTTONS.with(|reg| {
        reg.borrow_mut().push(AudioButtonEntry {
            audio_button: button.downgrade(),
            idle_label_key,
        });
    });
}

pub fn on_audio_state_changed(is_adhan: bool) {
    gtk::glib::MainContext::default().invoke(move || {
        AUDIO_BUTTONS.with(|reg| {
            let reg_guard = reg.borrow();
            for entry in reg_guard.iter() {
                if let Some(button) = entry.audio_button.upgrade() {
                    set_audio_toggle_button_label(&button, entry.idle_label_key, is_adhan);
                }
            }
        });
    });
}

thread_local! {
    static OPEN_FONTS_SETTINGS: RefCell<Option<Rc<dyn Fn()>>> = const { RefCell::new(None) };
}

pub fn register_open_fonts_settings(callback: Rc<dyn Fn()>) {
    OPEN_FONTS_SETTINGS.with(|cell| *cell.borrow_mut() = Some(callback));
}

pub fn open_fonts_settings() {
    OPEN_FONTS_SETTINGS.with(|cell| {
        if let Some(callback) = cell.borrow().clone() {
            callback();
        }
    });
}

pub fn find_toast_overlay(window: &adw::ApplicationWindow) -> Option<adw::ToastOverlay> {
    fn search(widget: &gtk::Widget) -> Option<adw::ToastOverlay> {
        if let Some(overlay) = widget.downcast_ref::<adw::ToastOverlay>() {
            return Some(overlay.clone());
        }
        let mut child = widget.first_child();
        while let Some(child_widget) = child {
            if let Some(found) = search(&child_widget) {
                return Some(found);
            }
            child = child_widget.next_sibling();
        }
        None
    }
    window.content().as_ref().and_then(search)
}

/// Builds the "Connected Mosque" status subtitle from a cached Mawaqit schedule.
pub fn mawaqit_status_subtitle(cache: &crate::config::MawaqitCache, language: &str) -> String {
    let timezone = cache.timezone.clone().unwrap_or_default();
    let timezone_label = if timezone.is_empty() {
        String::new()
    } else {
        location::localized_time_zone_label(&timezone, language)
    };
    if timezone_label.is_empty() {
        format!("{} • {}", tr("Last updated"), cache.fetched_on)
    } else {
        format!(
            "{} • {} • {}",
            timezone_label,
            tr("Last updated"),
            cache.fetched_on
        )
    }
}

fn apply_sound_combo_state(combo: &ComboRow, preset_files: &[String], current_path: Option<&str>) {
    if let Some(path) = current_path {
        let path_obj = PathBuf::from(path);
        if let Some(name) = path_obj
            .file_name()
            .and_then(|file_name| file_name.to_str())
        {
            if let Some(pos) = preset_files
                .iter()
                .position(|preset| preset.as_str() == name)
            {
                // Entries 0 (Default) and 1 (Custom File...) precede the preset files.
                combo.set_selected((pos + 2) as u32);
            } else {
                combo.set_selected(1);
                combo.set_subtitle(path);
            }
        } else {
            combo.set_selected(1);
            combo.set_subtitle(path);
        }
    } else {
        combo.set_selected(0);
        combo.set_subtitle(&tr("Using builtin default"));
    }
}

fn set_audio_toggle_button_label(button: &Button, idle_label_key: &str, is_adhan: bool) {
    let label = if is_adhan {
        tr("⏹ Stop Adhan")
    } else {
        tr(idle_label_key)
    };
    button.set_label(&label);
}

fn bind_audio_toggle_button_sync(button: &Button, idle_label_key: &'static str) {
    register_audio_button(button, idle_label_key);
}

fn finish_entry_row_interaction(row: &adw::EntryRow) {
    if let Some(root) = row.root() {
        root.set_focus(Option::<&gtk::Widget>::None);
    }
}

struct FontSettingRow {
    row: adw::ActionRow,
    font_button: gtk::FontDialogButton,
    reset_btn: Button,
    dialog: gtk::FontDialog,
    default_desc: gtk::pango::FontDescription,
}

fn system_font_description() -> gtk::pango::FontDescription {
    let name = gtk::Settings::default()
        .and_then(|settings| settings.gtk_font_name())
        .map(|font_name| font_name.to_string())
        .unwrap_or_else(|| "Cantarell 11".to_string());
    gtk::pango::FontDescription::from_string(&name)
}

fn set_font_row_state(state: &FontSettingRow, current: Option<&str>) {
    match current {
        Some(family) => {
            state
                .font_button
                .set_font_desc(&gtk::pango::FontDescription::from_string(family));
            state.reset_btn.set_sensitive(true);
        }
        None => {
            state.font_button.set_font_desc(&state.default_desc);
            state.reset_btn.set_sensitive(false);
        }
    }
}

struct FontRowOptions {
    dialog_title: String,
    row_title: String,
    subtitle: String,
    default_desc: gtk::pango::FontDescription,
    current: Option<String>,
}

type FontFamilySetter = dyn Fn(&AppConfig, Option<String>);

fn setup_font_setting_row(
    config: &AppConfig,
    fonts_group: &PreferencesGroup,
    options: FontRowOptions,
    apply_family: impl Fn(&AppConfig, Option<String>) + 'static,
) -> FontSettingRow {
    let dialog = gtk::FontDialog::builder()
        .title(options.dialog_title)
        .build();
    let font_button = gtk::FontDialogButton::new(Some(dialog.clone()));
    font_button.set_level(gtk::FontLevel::Family);
    font_button.set_valign(gtk::Align::Center);
    let reset_btn = Button::builder()
        .icon_name("edit-undo-symbolic")
        .tooltip_text(tr("Restore to default"))
        .has_frame(false)
        .valign(gtk::Align::Center)
        .build();

    let row = adw::ActionRow::builder()
        .title(options.row_title)
        .subtitle(options.subtitle)
        .build();
    row.set_activatable_widget(Some(&font_button));
    row.add_suffix(&font_button);
    row.add_suffix(&reset_btn);

    let state = FontSettingRow {
        row: row.clone(),
        font_button: font_button.clone(),
        reset_btn: reset_btn.clone(),
        dialog: dialog.clone(),
        default_desc: options.default_desc,
    };
    set_font_row_state(&state, options.current.as_deref());
    fonts_group.add(&row);

    let setter: Rc<FontFamilySetter> = Rc::new(apply_family);
    let suppress = Rc::new(Cell::new(false));

    let config_for_pick = config.clone();
    let row_pick = row.clone();
    let reset_pick = reset_btn.clone();
    let setter_pick = setter.clone();
    let suppress_pick = suppress.clone();
    font_button.connect_font_desc_notify(move |button| {
        if suppress_pick.get() {
            return;
        }
        if let Some(family) = button.font_desc().and_then(|fd| fd.family()) {
            let family = family.to_string();
            setter_pick(&config_for_pick, Some(family.clone()));
            config_for_pick.save();
            crate::apply_font_css(&config_for_pick);
            row_pick.set_subtitle(&family);
            reset_pick.set_sensitive(true);
        }
    });

    let config_for_reset = config.clone();
    let reset_reset = reset_btn.clone();
    let font_btn_reset = font_button.clone();
    let setter_reset = setter;
    let default_desc_reset = state.default_desc.clone();
    let suppress_reset = suppress;
    reset_btn.connect_clicked(move |_| {
        suppress_reset.set(true);
        setter_reset(&config_for_reset, None);
        config_for_reset.save();
        crate::apply_font_css(&config_for_reset);
        font_btn_reset.set_font_desc(&default_desc_reset);
        reset_reset.set_sensitive(false);
        suppress_reset.set(false);
    });

    state
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
        let label = gtk::Label::builder()
            .label(desc)
            .css_classes(["dim-label"])
            .hexpand(true)
            .halign(gtk::Align::Fill)
            .xalign(0.0)
            .wrap(true)
            .wrap_mode(gtk::pango::WrapMode::Word)
            .margin_bottom(12)
            .build();
        settings_box.append(&label);
        Some(label)
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
    pub current_language: Rc<RefCell<String>>,
    pub loc_tx: std::sync::mpsc::Sender<(f64, f64, Option<String>)>,
    pub refresh_calendar: Rc<dyn Fn()>,
}

pub struct SettingsUiContext {
    pub config: AppConfig,
    pub current_language: Rc<RefCell<String>>,
    pub sync_lock: Rc<std::cell::Cell<bool>>,

    pub general_heading: gtk::Label,
    pub general_desc: gtk::Label,
    pub language_row: ComboRow,
    pub language_model: gtk::StringList,
    pub theme_row: ComboRow,
    pub theme_model: gtk::StringList,
    pub autostart_toggle: adw::SwitchRow,

    pub fonts_heading: gtk::Label,
    pub fonts_desc: gtk::Label,
    pub ui_font_row: adw::ActionRow,
    pub ui_font_reset: Button,
    pub ui_font_dialog: gtk::FontDialog,
    pub arabic_font_row: adw::ActionRow,
    pub arabic_font_reset: Button,
    pub arabic_font_dialog: gtk::FontDialog,
    pub quran_font_row: adw::ActionRow,
    pub quran_font_reset: Button,
    pub quran_font_dialog: gtk::FontDialog,

    pub prayer_setup_heading: gtk::Label,
    pub prayer_setup_desc: gtk::Label,
    pub location_group: PreferencesGroup,
    pub mode_row: ComboRow,
    pub mode_model: gtk::StringList,
    pub latitude_row: adw::SpinRow,
    pub longitude_row: adw::SpinRow,
    pub status_row: adw::ActionRow,
    pub city_row: adw::EntryRow,
    pub city_button: Button,
    pub auto_row: adw::ActionRow,
    pub auto_button: Button,
    pub source_row: ComboRow,
    pub source_model: gtk::StringList,
    pub url_row: adw::EntryRow,
    pub auto_refresh_row: adw::SwitchRow,
    pub mawaqit_status_row: adw::ActionRow,
    pub refresh_button: Button,

    pub travel_group: PreferencesGroup,
    pub timezone_mode_row: ComboRow,
    pub timezone_mode_model: gtk::StringList,
    pub timezone_named_row: adw::ActionRow,
    pub timezone_offset_row: adw::SpinRow,

    pub calc_group: PreferencesGroup,
    pub hijri_row: adw::SpinRow,
    pub method_row: ComboRow,
    pub method_model: gtk::StringList,
    pub madhab_row: ComboRow,
    pub madhab_model: gtk::StringList,
    pub high_latitude_row: ComboRow,
    pub high_latitude_model: gtk::StringList,
    pub polar_row: ComboRow,
    pub polar_model: gtk::StringList,

    pub iqamah_group: PreferencesGroup,
    pub iqamah_rows: Vec<adw::SpinRow>,

    pub notif_audio_heading: gtk::Label,
    pub notif_audio_desc: gtk::Label,
    pub notify_toggle: adw::SwitchRow,
    pub notify_time: adw::SpinRow,
    pub iqamah_notify_toggle: adw::SwitchRow,
    pub adkar_toggle: adw::SwitchRow,
    pub adhan_only_toggle: adw::SwitchRow,
    pub test_notification_button: Button,

    pub sound_combo: ComboRow,
    pub sound_model: gtk::StringList,
    pub preset_files: Vec<String>,
    pub mute_toggle: adw::SwitchRow,
    pub volume_row: adw::SpinRow,
    pub test_audio_button: Button,
}

impl SettingsUiContext {
    /// Re-renders the Location Mode group and syncs the mode and timezone
    /// rows from the config. Under Connected Mosque the Location Mode group
    /// is hidden and the timezone rows show the effective mosque timezone
    /// disabled; the user's stored mode and timezone are never overwritten.
    /// Selection sets are guarded by `sync_lock` so the handlers do not
    /// re-enter and mutate config.
    fn sync_location_and_timezone(&self, language: &str) {
        let config = &self.config;
        let is_mawaqit = config.prayer_times_source() == crate::config::PrayerTimesSource::Mawaqit;
        let mode = config.location_mode();

        self.sync_lock.set(true);

        self.mode_row.set_visible(!is_mawaqit);
        self.latitude_row
            .set_visible(!is_mawaqit && mode == LocationMode::Manual);
        self.longitude_row
            .set_visible(!is_mawaqit && mode == LocationMode::Manual);
        self.city_row
            .set_visible(!is_mawaqit && mode == LocationMode::City);
        self.auto_row
            .set_visible(!is_mawaqit && mode == LocationMode::Auto);
        if is_mawaqit {
            self.status_row.set_visible(false);
        }
        if !is_mawaqit {
            self.mode_row.set_selected(match mode {
                LocationMode::Manual => 0,
                LocationMode::City => 1,
                LocationMode::Auto => 2,
            });
            if mode == LocationMode::Manual {
                let latitude = config.latitude();
                let longitude = config.longitude();
                if (self.latitude_row.adjustment().value() - latitude).abs() > 1e-4 {
                    self.latitude_row.adjustment().set_value(latitude);
                }
                if (self.longitude_row.adjustment().value() - longitude).abs() > 1e-4 {
                    self.longitude_row.adjustment().set_value(longitude);
                }
            } else if mode == LocationMode::Auto {
                let auto_mode_city = config.active_location().city_name;
                let subtitle = auto_mode_city
                    .as_deref()
                    .filter(|name| !name.is_empty())
                    .map(location::short_city_with_country)
                    .unwrap_or_default();
                if self.auto_row.subtitle().as_deref() != Some(subtitle.as_str()) {
                    self.auto_row.set_subtitle(&subtitle);
                }
            }
        }

        let effective = crate::time::effective_timezone(config);
        match &effective {
            TimezoneMode::Auto => {
                self.timezone_mode_row.set_selected(0);
                self.timezone_named_row.set_visible(false);
                self.timezone_offset_row.set_visible(false);
            }
            TimezoneMode::Named(name) => {
                self.timezone_mode_row.set_selected(1);
                self.timezone_named_row.set_visible(true);
                self.timezone_offset_row.set_visible(false);
                let label = location::localized_time_zone_label(name, language);
                self.timezone_named_row.set_subtitle(&if label.is_empty() {
                    name.clone()
                } else {
                    label
                });
            }
            TimezoneMode::UtcOffset(_) => {
                self.timezone_mode_row.set_selected(2);
                self.timezone_named_row.set_visible(false);
                self.timezone_offset_row.set_visible(true);
            }
        }

        self.timezone_mode_row.set_sensitive(!is_mawaqit);
        self.timezone_named_row.set_sensitive(!is_mawaqit);
        self.timezone_offset_row.set_sensitive(!is_mawaqit);

        self.sync_lock.set(false);
    }
}

pub fn setup_settings_ui<'a>(
    params: SettingsUiParams<'a>,
) -> (adw::ComboRow, Rc<RefCell<SettingsUiContext>>) {
    let SettingsUiParams {
        settings_box,
        config,
        list_box_rc,
        window,
        current_language,
        loc_tx,
        refresh_calendar,
    } = params;
    let selected_language = current_language.borrow().clone();

    // Guard against handler re-entry while the UI is programmatically synced,
    // so setting a combo selection never mutates config.
    let sync_lock = Rc::new(std::cell::Cell::new(false));
    // Late-bound so the internal handlers can trigger a full location/timezone
    // sync once the context exists.
    let ctx_slot: Rc<RefCell<Option<Rc<RefCell<SettingsUiContext>>>>> = Rc::new(RefCell::new(None));
    let sync_closure_current_language = current_language.clone();
    let trigger_sync_closures = ctx_slot.clone();
    let trigger_sync = Rc::new(move || {
        if let Some(ctx) = trigger_sync_closures.borrow().as_ref() {
            let language = sync_closure_current_language.borrow().clone();
            ctx.borrow().sync_location_and_timezone(&language);
        }
    });
    let trigger_sync_for_source = trigger_sync.clone();
    let trigger_sync_for_fetch = trigger_sync.clone();

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

    let language_model = StringList::new(&[
        &tr("System Default"),
        &tr("English"),
        &tr("Arabic"),
        &tr("French"),
        &tr("Spanish"),
        &tr("Turkish"),
        &tr("Indonesian"),
    ]);
    let language_row = ComboRow::builder()
        .title(tr("Language"))
        .model(&language_model)
        .build();

    language_row.set_selected(crate::i18n::language_index_from_code(
        selected_language.as_str(),
    ));

    general_group.add(&language_row);

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

        let style_manager = adw::StyleManager::default();
        style_manager.set_color_scheme(match new_theme {
            crate::config::ThemeMode::Light => adw::ColorScheme::ForceLight,
            crate::config::ThemeMode::Dark => adw::ColorScheme::PreferDark,
            crate::config::ThemeMode::System => adw::ColorScheme::Default,
        });
        config_for_theme.set_theme(new_theme);
        config_for_theme.save();
    });
    general_group.add(&theme_row);

    let time_format_model = StringList::new(&[&tr("Auto"), &tr("24-hour"), &tr("12-hour")]);
    let time_format_row = ComboRow::builder()
        .title(tr("Time Format"))
        .subtitle(tr(
            "Auto follows the time convention of your selected language.",
        ))
        .model(&time_format_model)
        .build();

    match config.time_format() {
        crate::config::TimeFormat::Auto => time_format_row.set_selected(0),
        crate::config::TimeFormat::Hours24 => time_format_row.set_selected(1),
        crate::config::TimeFormat::Hours12 => time_format_row.set_selected(2),
    }

    let config_for_time_format = config.clone();
    let list_box_for_time_format = list_box_rc.clone();
    let window_for_time_format = window.clone();
    time_format_row.connect_selected_notify(move |row| {
        let new_format = match row.selected() {
            1 => crate::config::TimeFormat::Hours24,
            2 => crate::config::TimeFormat::Hours12,
            _ => crate::config::TimeFormat::Auto,
        };
        config_for_time_format.set_time_format(new_format);
        config_for_time_format.save();
        if let Some(result) = refresh_prayers(&config_for_time_format, &list_box_for_time_format) {
            update_lre_toast(&config_for_time_format, &result, &window_for_time_format);
            update_fallback_toast(&config_for_time_format, &result, &window_for_time_format);
        }
    });
    general_group.add(&time_format_row);

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

    let (fonts_heading, fonts_desc) = append_settings_section_heading(
        settings_box,
        &tr("Fonts"),
        Some(&tr(
            "Choose the fonts used across the app. 'System default' follows your desktop font settings.",
        )),
        24,
    );
    let fonts_desc = fonts_desc.expect("fonts section description label");

    let fonts_group = PreferencesGroup::new();
    fonts_group.set_margin_top(0);
    fonts_group.set_margin_bottom(24);
    settings_box.append(&fonts_group);

    let system_desc = system_font_description();
    let amiri_quran_desc = gtk::pango::FontDescription::from_string("Amiri Quran");

    let ui_font_row = setup_font_setting_row(
        &config,
        &fonts_group,
        FontRowOptions {
            dialog_title: tr("Select Interface Font"),
            row_title: tr("UI Font"),
            subtitle: String::new(),
            default_desc: system_desc.clone(),
            current: config.ui_font_family(),
        },
        |config, font_family| config.set_ui_font_family(font_family),
    );

    let arabic_font_row = setup_font_setting_row(
        &config,
        &fonts_group,
        FontRowOptions {
            dialog_title: tr("Select Arabic Font"),
            row_title: tr("Arabic Font"),
            subtitle: tr("Used for Arabic content, such as the adkar texts."),
            default_desc: system_desc,
            current: config.arabic_font_family(),
        },
        |config, font_family| config.set_arabic_font_family(font_family),
    );

    let quran_font_row = setup_font_setting_row(
        &config,
        &fonts_group,
        FontRowOptions {
            dialog_title: tr("Select Quran Font"),
            row_title: tr("Quran Font"),
            subtitle: tr(
                "Must support Uthmani Quran script; otherwise verses may render incorrectly.",
            ),
            default_desc: amiri_quran_desc,
            current: config.quran_font_family(),
        },
        |config, font_family| config.set_quran_font_family(font_family),
    );

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
    let modes_slices: Vec<&str> = modes_strings.iter().map(|item| item.as_str()).collect();
    let modes = StringList::new(&modes_slices);
    let mode_row = ComboRow::builder()
        .title(tr("Location Mode"))
        .model(&modes)
        .build();

    let current_mode = config.location_mode();
    mode_row.set_selected(match current_mode {
        LocationMode::Manual => 0,
        LocationMode::City => 1,
        LocationMode::Auto => 2,
    });

    let latitude_row = adw::SpinRow::builder()
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

    let config_latitude = config.clone();
    let list_box_latitude = list_box_rc.clone();
    let window_latitude = window.clone();
    let sync_lock_latitude = sync_lock.clone();
    latitude_row
        .adjustment()
        .connect_value_changed(move |adjustment| {
            if sync_lock_latitude.get() {
                return;
            }
            let latitude = adjustment.value();
            config_latitude.set_latitude(latitude);
            config_latitude.save();
            if let Some(result) = refresh_prayers(&config_latitude, &list_box_latitude) {
                update_lre_toast(&config_latitude, &result, &window_latitude);
                update_fallback_toast(&config_latitude, &result, &window_latitude);
            }
        });

    let longitude_row = adw::SpinRow::builder()
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

    let config_longitude = config.clone();
    let list_box_longitude = list_box_rc.clone();
    let window_longitude = window.clone();
    let sync_lock_longitude = sync_lock.clone();
    longitude_row
        .adjustment()
        .connect_value_changed(move |adjustment| {
            if sync_lock_longitude.get() {
                return;
            }
            let longitude = adjustment.value();
            config_longitude.set_longitude(longitude);
            config_longitude.save();
            if let Some(result) = refresh_prayers(&config_longitude, &list_box_longitude) {
                update_lre_toast(&config_longitude, &result, &window_longitude);
                update_fallback_toast(&config_longitude, &result, &window_longitude);
            }
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
        if let Some(text) = location::display_city_label(city_name.as_deref(), &selected_language) {
            city_row.set_text(&text);
        }
    }

    let city_button = Button::with_label(&tr("Search"));
    city_button.set_valign(gtk::Align::Center);
    city_button.set_halign(gtk::Align::End);
    city_button.set_hexpand(false);
    city_button.set_vexpand(false);
    let city_tx = loc_tx.clone();
    let current_language_search = current_language.clone();

    let city_row_clone = city_row.clone();
    let status_row_c = status_row_clone.clone();
    let perform_search = Rc::new(move || {
        let query = city_row_clone.text().to_string();
        if query.trim().is_empty() {
            return;
        }

        let language = current_language_search.borrow().clone();

        city_row_clone.remove_css_class("error");
        city_row_clone.remove_css_class("success");

        let sender = city_tx.clone();
        let city_row_for_update = city_row_clone.clone();
        let status_row_async_c = status_row_c.clone();

        gtk::glib::spawn_future_local(async move {
            let result = location::search_city(&query, &language).await;
            match result {
                Ok((latitude, longitude, name, _timezone)) => {
                    let _ = sender.send((latitude, longitude, Some(name.clone())));
                    city_row_for_update.set_text(&location::short_city_with_country(&name));
                    city_row_for_update.add_css_class("success");
                    status_row_async_c.set_visible(false);
                }
                Err(err) => {
                    log::error!("City search failed: {}", err);
                    city_row_for_update.add_css_class("error");
                    status_row_async_c.set_subtitle(&tr("City not found. Please try again."));
                    status_row_async_c.set_visible(true);
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
    city_button.connect_clicked(move |_| {
        search_fn_btn();
    });

    city_row.add_suffix(&city_button);

    let auto_row = adw::ActionRow::builder()
        .title(tr("Auto Detection"))
        .build();
    if let Some(name) = &config.active_location().city_name {
        auto_row.set_subtitle(&location::short_city_with_country(name));
    }
    let auto_button = Button::with_label(&tr("Update Now"));
    auto_button.set_valign(gtk::Align::Center);
    auto_button.set_halign(gtk::Align::End);
    auto_button.set_hexpand(false);
    auto_button.set_vexpand(false);

    let auto_tx = loc_tx.clone();
    let auto_row_clone = auto_row.clone();
    let status_row_auto = status_row_clone2;
    let current_language_auto = current_language.clone();

    let auto_button_click = auto_button.clone();
    auto_button.connect_clicked(move |_| {
        auto_button_click.set_sensitive(false);
        auto_row_clone.remove_css_class("error");
        auto_row_clone.remove_css_class("success");
        status_row_auto.set_visible(false);

        let sender = auto_tx.clone();
        let auto_row_for_update = auto_row_clone.clone();
        let status_for_update = status_row_auto.clone();
        let button_for_auto = auto_button_click.clone();

        let language = current_language_auto.borrow().clone();

        gtk::glib::spawn_future_local(async move {
            let result = location::fetch_auto_location(&language).await;
            match result {
                Ok((latitude, longitude, name)) => {
                    let _ = sender.send((latitude, longitude, Some(name.clone())));
                    auto_row_for_update.set_subtitle(&location::short_city_with_country(&name));
                    auto_row_for_update.add_css_class("success");
                }
                Err(err) => {
                    log::error!("Auto-location failed: {}", err);
                    auto_row_for_update.add_css_class("error");
                    status_for_update.set_subtitle(&tr(&err));
                    status_for_update.set_visible(true);
                }
            }
            button_for_auto.set_sensitive(true);
        });
    });

    auto_row.add_suffix(&auto_button);

    let source_items = [tr("Calculated (Offline)"), tr("Connected Mosque (URL)")];
    let source_refs: Vec<&str> = source_items.iter().map(|item| item.as_str()).collect();
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
        mawaqit_status_row.set_subtitle(&mawaqit_status_subtitle(cache, &selected_language));
        mawaqit_status_row.set_title(&title);
    } else {
        mawaqit_status_row.set_subtitle(&tr("Not configured"));
    }

    let refresh_button = Button::with_label(&tr("Refresh now"));
    refresh_button.set_valign(gtk::Align::Center);
    refresh_button.set_halign(gtk::Align::End);
    mawaqit_status_row.add_suffix(&refresh_button);
    location_group.add(&mawaqit_status_row);

    location_group.add(&mode_row);
    location_group.add(&latitude_row);
    location_group.add(&longitude_row);
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
    let refresh_button_for_source = refresh_button.clone();
    let window_for_source = window.clone();
    let sync_lock_for_source = sync_lock.clone();
    source_row.connect_selected_notify(move |row| {
        if sync_lock_for_source.get() {
            return;
        }
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
        refresh_button_for_source.set_visible(show);
        trigger_sync_for_source();
        if let Some(result) = refresh_prayers(&config_for_source, &list_box_for_source) {
            update_lre_toast(&config_for_source, &result, &window_for_source);
            update_fallback_toast(&config_for_source, &result, &window_for_source);
        }
    });

    let config_for_fetch = config.clone();
    let list_box_for_fetch = list_box_rc.clone();
    let status_for_fetch = mawaqit_status_row.clone();
    let url_row_for_fetch = url_row.clone();
    let loc_tx_for_fetch = loc_tx.clone();
    let current_language_for_fetch = current_language.clone();
    let refresh_calendar_for_fetch = refresh_calendar.clone();
    let window_for_fetch = window.clone();
    let do_fetch: Rc<dyn Fn()> = Rc::new(move || {
        let url_text = url_row_for_fetch.text().to_string();
        if url_text.trim().is_empty() {
            status_for_fetch.set_subtitle(&tr("Invalid Mawaqit URL"));
            status_for_fetch.add_css_class("error");
            return;
        }
        let language = current_language_for_fetch.borrow().clone();
        status_for_fetch.remove_css_class("error");
        status_for_fetch.set_subtitle(&tr("Fetching..."));
        let config_for_mawaqit_save = config_for_fetch.clone();
        let list_box = list_box_for_fetch.clone();
        let status = status_for_fetch.clone();
        let sender = loc_tx_for_fetch.clone();
        let refresh_calendar_fetch = refresh_calendar_for_fetch.clone();
        let window_bg = window_for_fetch.clone();
        let trigger_sync_fetch = trigger_sync_for_fetch.clone();
        gtk::glib::spawn_future_local(async move {
            match crate::mawaqit::fetch_mawaqit_cache(&url_text).await {
                Ok(cache) => {
                    let mut fetched_coordinates: Option<(f64, f64)> = None;
                    {
                        config_for_mawaqit_save.set_mawaqit_url(Some(cache.url.clone()));
                        config_for_mawaqit_save.set_mawaqit_cache(Some(cache.clone()));
                        if let (Some(latitude), Some(longitude)) = (cache.latitude, cache.longitude)
                        {
                            config_for_mawaqit_save.set_latitude(latitude);
                            config_for_mawaqit_save.set_longitude(longitude);
                            fetched_coordinates = Some((latitude, longitude));
                        }
                        config_for_mawaqit_save.save();
                    }
                    if let Some((latitude, longitude)) = fetched_coordinates {
                        let config_for_mawaqit_resolve = config_for_mawaqit_save.clone();
                        let inner_sender = sender.clone();
                        let language_for_city = language.clone();
                        gtk::glib::spawn_future_local(async move {
                            if let Some(name) = crate::location::resolve_mawaqit_city(
                                latitude,
                                longitude,
                                &language_for_city,
                            )
                            .await
                            {
                                if let Some(mut stored_cache) =
                                    config_for_mawaqit_resolve.mawaqit_cache()
                                {
                                    stored_cache.resolved_city = Some(name);
                                    config_for_mawaqit_resolve
                                        .set_mawaqit_cache(Some(stored_cache));
                                    config_for_mawaqit_resolve.save();
                                }
                                let _ = inner_sender.send((latitude, longitude, None));
                            }
                        });
                    }
                    if let Some((latitude, longitude)) = fetched_coordinates {
                        let _ = sender.send((latitude, longitude, None));
                    }
                    let title = cache
                        .mosque_name
                        .clone()
                        .unwrap_or_else(|| cache.url.clone());
                    status.set_title(&title);
                    status.set_subtitle(&mawaqit_status_subtitle(&cache, &language));
                    status.remove_css_class("error");
                    if let Some(result) = refresh_prayers(&config_for_mawaqit_save, &list_box) {
                        update_lre_toast(&config_for_mawaqit_save, &result, &window_bg);
                        update_fallback_toast(&config_for_mawaqit_save, &result, &window_bg);
                    }
                    refresh_calendar_fetch();
                    trigger_sync_fetch();
                }
                Err(err) => {
                    status.add_css_class("error");
                    status.set_subtitle(&tr(&err));
                }
            }
        });
    });

    let do_fetch_btn = do_fetch.clone();
    refresh_button.connect_clicked(move |_| {
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

    let timezone_mode_strings = [
        tr("Automatic (System)"),
        tr("Custom Timezone (IANA)"),
        tr("Manual UTC Offset"),
    ];
    let timezone_mode_slices: Vec<&str> = timezone_mode_strings
        .iter()
        .map(|item| item.as_str())
        .collect();
    let timezone_modes = StringList::new(&timezone_mode_slices);
    let timezone_mode_row = ComboRow::builder()
        .title(tr("Timezone Mode"))
        .subtitle(tr("How prayer times are adjusted for your timezone."))
        .model(&timezone_modes)
        .build();

    let current_timezone_mode = config.timezone_mode();
    let timezone_init_selected = match &current_timezone_mode {
        TimezoneMode::Auto => 0u32,
        TimezoneMode::Named(_) => 1,
        TimezoneMode::UtcOffset(_) => 2,
    };
    timezone_mode_row.set_selected(timezone_init_selected);
    travel_group.add(&timezone_mode_row);

    let timezone_named_init = match &current_timezone_mode {
        TimezoneMode::Named(timezone_name) => timezone_name.clone(),
        _ => location::system_time_zone_id().unwrap_or_default(),
    };

    let timezone_named_label =
        location::localized_time_zone_label(&timezone_named_init, &selected_language);
    let timezone_named_row = adw::ActionRow::builder()
        .title(tr("IANA Timezone"))
        .subtitle(if timezone_named_label.is_empty() {
            &timezone_named_init
        } else {
            &timezone_named_label
        })
        .activatable(true)
        .visible(timezone_init_selected == 1)
        .build();
    timezone_named_row.add_prefix(&gtk::Image::from_icon_name("mark-location-symbolic"));
    let timezone_named_arrow = gtk::Image::from_icon_name("go-next-symbolic");
    timezone_named_row.add_suffix(&timezone_named_arrow);
    travel_group.add(&timezone_named_row);

    let config_for_timezone_named = config.clone();
    let list_box_timezone_named = list_box_rc.clone();
    let timezone_mode_row_for_apply = timezone_mode_row.clone();
    let timezone_gesture_click = gtk::GestureClick::builder()
        .button(gtk::gdk::BUTTON_PRIMARY)
        .build();
    let timezone_named_row_for_gesture = timezone_named_row.clone();
    let window_timezone_gesture = window.clone();
    let config_for_timezone_dialog = config_for_timezone_named.clone();
    let list_box_timezone_gesture = list_box_timezone_named.clone();
    let current_language_tz_gesture = current_language.clone();
    timezone_gesture_click.connect_pressed(move |_, _, _, _| {
        if timezone_mode_row_for_apply.selected() != 1 {
            return;
        }
        let on_select = {
            let config_for_tz = config_for_timezone_dialog.clone();
            let list_box_timezone_gesture_c = list_box_timezone_gesture.clone();
            let win = window_timezone_gesture.clone();
            let row = timezone_named_row_for_gesture.clone();
            let language_rc = current_language_tz_gesture.clone();
            Rc::new(move |zone: &str| {
                let language = language_rc.borrow().clone();
                let label = location::localized_time_zone_label(zone, &language);
                let subtitle = if label.is_empty() {
                    zone.to_string()
                } else {
                    label
                };
                row.set_subtitle(&subtitle);
                config_for_tz.set_timezone_mode(TimezoneMode::Named(zone.to_string()));
                config_for_tz.save();
                if let Some(result) = refresh_prayers(&config_for_tz, &list_box_timezone_gesture_c)
                {
                    update_lre_toast(&config_for_tz, &result, &win);
                    update_fallback_toast(&config_for_tz, &result, &win);
                }
            })
        };
        let root_parent = timezone_named_row_for_gesture
            .root()
            .and_then(|root| root.downcast::<adw::ApplicationWindow>().ok());
        if let Some(parent_window) = root_parent {
            let language = current_language_tz_gesture.borrow().clone();
            crate::tz_dialog::open_tz_dialog(&parent_window, on_select, &language);
        }
    });
    timezone_named_row.add_controller(timezone_gesture_click);

    let timezone_offset_adjustment = gtk::Adjustment::new(0.0, -12.0, 14.0, 0.5, 0.0, 0.0);
    if let TimezoneMode::UtcOffset(mins) = &current_timezone_mode {
        timezone_offset_adjustment.set_value(*mins as f64 / 60.0);
    }
    let timezone_offset_row = adw::SpinRow::builder()
        .title(tr("UTC Offset (hours)"))
        .subtitle(tr("Example: +2.0 for UTC+2, -5.0 for UTC-5"))
        .adjustment(&timezone_offset_adjustment)
        .digits(1)
        .visible(timezone_init_selected == 2)
        .build();
    travel_group.add(&timezone_offset_row);

    let timezone_named_vis = timezone_named_row.clone();
    let timezone_offset_vis = timezone_offset_row.clone();
    let config_for_timezone_mode = config.clone();
    let list_box_timezone = list_box_rc.clone();
    let timezone_offset_adjustment_for_mode = timezone_offset_adjustment.clone();
    let window_timezone = window.clone();
    let sync_lock_for_timezone_mode = sync_lock.clone();
    timezone_mode_row.connect_selected_notify(move |combo| {
        if sync_lock_for_timezone_mode.get() {
            return;
        }
        let selected_index = combo.selected();
        timezone_named_vis.set_visible(selected_index == 1);
        timezone_offset_vis.set_visible(selected_index == 2);
        let new_mode = match selected_index {
            1 => {
                let existing = match config_for_timezone_mode.timezone_mode() {
                    TimezoneMode::Named(name) if !name.trim().is_empty() => Some(name),
                    _ => None,
                };
                if let Some(name) = existing {
                    TimezoneMode::Named(name)
                } else if let Some(name) = location::system_time_zone_id() {
                    TimezoneMode::Named(name)
                } else {
                    TimezoneMode::Auto
                }
            }
            2 => {
                TimezoneMode::UtcOffset((timezone_offset_adjustment_for_mode.value() * 60.0) as i32)
            }
            _ => TimezoneMode::Auto,
        };
        config_for_timezone_mode.set_timezone_mode(new_mode);
        config_for_timezone_mode.save();
        if let Some(result) = refresh_prayers(&config_for_timezone_mode, &list_box_timezone) {
            update_lre_toast(&config_for_timezone_mode, &result, &window_timezone);
            update_fallback_toast(&config_for_timezone_mode, &result, &window_timezone);
        }
    });

    let config_for_timezone_offset = config.clone();
    let list_box_timezone_offset = list_box_rc.clone();
    let window_timezone_offset = window.clone();
    timezone_offset_adjustment.connect_value_changed(move |adjustment| {
        if let TimezoneMode::UtcOffset(_) = config_for_timezone_offset.timezone_mode() {
            config_for_timezone_offset.set_timezone_mode(crate::config::TimezoneMode::UtcOffset(
                (adjustment.value() * 60.0) as i32,
            ));
            config_for_timezone_offset.save();
            if let Some(result) =
                refresh_prayers(&config_for_timezone_offset, &list_box_timezone_offset)
            {
                update_lre_toast(
                    &config_for_timezone_offset,
                    &result,
                    &window_timezone_offset,
                );
                update_fallback_toast(
                    &config_for_timezone_offset,
                    &result,
                    &window_timezone_offset,
                );
            }
        }
    });

    let calc_group = PreferencesGroup::builder().title(tr("Calculation")).build();
    calc_group.set_margin_top(12);
    calc_group.set_margin_bottom(24);
    settings_box.append(&calc_group);

    let hijri_offset_adjustment =
        gtk::Adjustment::new(config.hijri_offset() as f64, -2.0, 2.0, 1.0, 0.0, 0.0);
    let hijri_row = adw::SpinRow::builder()
        .title(tr("Hijri Date Correction"))
        .subtitle(tr("Adjust Hijri date by +/- days"))
        .adjustment(&hijri_offset_adjustment)
        .digits(0)
        .build();

    let config_hijri = config.clone();
    let refresh_calendar_hijri = refresh_calendar.clone();
    hijri_offset_adjustment.connect_value_changed(move |adjustment| {
        config_hijri.set_hijri_offset(adjustment.value() as i64);
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
    let methods_slices: Vec<&str> = methods_strings.iter().map(|item| item.as_str()).collect();
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
    let window_method = window.clone();
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
        if let Some(result) = refresh_prayers(&config_method, &list_box_method) {
            update_lre_toast(&config_method, &result, &window_method);
            update_fallback_toast(&config_method, &result, &window_method);
        }
    });
    calc_group.add(&method_row);

    let latitude_row_visibility = latitude_row.clone();
    let longitude_row_visibility = longitude_row.clone();
    let city_row_visibility = city_row.clone();
    let auto_row_visibility = auto_row.clone();

    let update_visibility = Rc::new(move |mode: &LocationMode| {
        latitude_row_visibility.set_visible(*mode == LocationMode::Manual);
        longitude_row_visibility.set_visible(*mode == LocationMode::Manual);
        city_row_visibility.set_visible(*mode == LocationMode::City);
        auto_row_visibility.set_visible(*mode == LocationMode::Auto);
    });

    update_visibility(&current_mode);

    let update_vis_clone = update_visibility.clone();
    let config_mode = config.clone();
    let list_box_mode = list_box_rc.clone();
    let source_row_for_mode = source_row.clone();
    let url_row_for_mode = url_row.clone();
    let auto_row_for_mode = auto_refresh_row.clone();
    let status_row_for_mode = mawaqit_status_row.clone();
    let refresh_button_for_mode = refresh_button.clone();
    let window_mode = window.clone();
    let sync_lock_for_mode = sync_lock.clone();
    let trigger_sync_for_mode = trigger_sync.clone();
    mode_row.connect_selected_notify(move |combo| {
        if sync_lock_for_mode.get() {
            return;
        }
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
        config_mode.set_location_mode(mode.clone());
        config_mode.save();
        if was_mawaqit {
            source_row_for_mode.set_selected(0);
            url_row_for_mode.set_visible(false);
            auto_row_for_mode.set_visible(false);
            status_row_for_mode.set_visible(false);
            refresh_button_for_mode.set_visible(false);
        }
        update_vis_clone(&mode);
        if let Some(result) = refresh_prayers(&config_mode, &list_box_mode) {
            update_lre_toast(&config_mode, &result, &window_mode);
            update_fallback_toast(&config_mode, &result, &window_mode);
        }
        trigger_sync_for_mode();
    });

    let madhab_strings = [tr("Shafi (Maliki/Hanbali)"), tr("Hanafi")];
    let madhab_slices: Vec<&str> = madhab_strings.iter().map(|item| item.as_str()).collect();
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
    let window_madhab = window.clone();
    madhab_row.connect_selected_notify(move |combo| {
        let index = combo.selected();
        let madhab_choice = if index == 1 {
            MadhabChoice::Hanafi
        } else {
            MadhabChoice::Shafi
        };
        config_madhab.set_madhab(madhab_choice);
        config_madhab.save();
        if let Some(result) = refresh_prayers(&config_madhab, &list_box_madhab) {
            update_lre_toast(&config_madhab, &result, &window_madhab);
            update_fallback_toast(&config_madhab, &result, &window_madhab);
        }
    });
    calc_group.add(&madhab_row);

    let high_latitude_strings = [
        tr("Auto (Recommended)"),
        tr("Middle of the Night"),
        tr("Seventh of the Night"),
        tr("Twilight Angle"),
        tr("Local Relative Estimation"),
    ];
    let high_latitude_slices: Vec<&str> = high_latitude_strings
        .iter()
        .map(|item| item.as_str())
        .collect();
    let high_latitude_model = StringList::new(&high_latitude_slices);
    let high_latitude_row = ComboRow::builder()
        .title(tr("Fajr/Isha Approximation"))
        .subtitle(tr(
            "How to approximate Fajr and Isha when the twilight angle is not reachable.",
        ))
        .model(&high_latitude_model)
        .build();
    high_latitude_row.set_subtitle_lines(2);

    match config.high_latitude_rule() {
        HighLatitudeChoice::Auto => high_latitude_row.set_selected(0),
        HighLatitudeChoice::MiddleOfTheNight => high_latitude_row.set_selected(1),
        HighLatitudeChoice::SeventhOfTheNight => high_latitude_row.set_selected(2),
        HighLatitudeChoice::TwilightAngle => high_latitude_row.set_selected(3),
        HighLatitudeChoice::LocalRelativeEstimation => high_latitude_row.set_selected(4),
    }

    let config_high_latitude = config.clone();
    let list_box_high_latitude = list_box_rc.clone();
    let window_high_latitude = window.clone();
    let high_latitude_updating = Rc::new(Cell::new(false));
    let high_latitude_updating_inner = high_latitude_updating.clone();
    high_latitude_row.connect_selected_notify(move |combo| {
        if high_latitude_updating_inner.get() {
            return;
        }
        let choice = match combo.selected() {
            0 => HighLatitudeChoice::Auto,
            1 => HighLatitudeChoice::MiddleOfTheNight,
            2 => HighLatitudeChoice::SeventhOfTheNight,
            3 => HighLatitudeChoice::TwilightAngle,
            4 => HighLatitudeChoice::LocalRelativeEstimation,
            _ => HighLatitudeChoice::Auto,
        };
        config_high_latitude.set_high_latitude_rule(choice);
        config_high_latitude.save();
        if let Some(result) = refresh_prayers(&config_high_latitude, &list_box_high_latitude) {
            if result.lre_blocked {
                high_latitude_updating_inner.set(true);
                combo.set_selected(0);
                high_latitude_updating_inner.set(false);
                config_high_latitude.set_high_latitude_rule(HighLatitudeChoice::Auto);
                config_high_latitude.save();
            }
            update_lre_toast(&config_high_latitude, &result, &window_high_latitude);
            update_fallback_toast(&config_high_latitude, &result, &window_high_latitude);
        }
    });
    calc_group.add(&high_latitude_row);

    let polar_strings = [tr("Nearest Latitude"), tr("Reference Latitude 45°")];
    let polar_slices: Vec<&str> = polar_strings.iter().map(|item| item.as_str()).collect();
    let polar_model = StringList::new(&polar_slices);
    let polar_row = ComboRow::builder()
        .title(tr("Polar Estimation Method"))
        .subtitle(tr(
            "Fallback applied when the sun never rises or sets (above ~66.5° latitude).",
        ))
        .model(&polar_model)
        .build();
    polar_row.set_subtitle_lines(2);

    match config.polar_estimation_method() {
        PolarEstimationMethod::NearestLatitude => polar_row.set_selected(0),
        PolarEstimationMethod::Reference45 => polar_row.set_selected(1),
    }

    let config_for_polar_estimation = config.clone();
    let list_box_pf = list_box_rc.clone();
    let window_pf = window.clone();
    polar_row.connect_selected_notify(move |combo| {
        let choice = if combo.selected() == 0 {
            PolarEstimationMethod::NearestLatitude
        } else {
            PolarEstimationMethod::Reference45
        };
        config_for_polar_estimation.set_polar_estimation_method(choice);
        config_for_polar_estimation.save();
        if let Some(result) = refresh_prayers(&config_for_polar_estimation, &list_box_pf) {
            update_lre_toast(&config_for_polar_estimation, &result, &window_pf);
            update_fallback_toast(&config_for_polar_estimation, &result, &window_pf);
        }
    });
    calc_group.add(&polar_row);

    // Moonsighting Committee ignores both the high-latitude rule and polar
    // estimation (they have no effect on its output), so hide the rows under
    // that method; otherwise fall back to the latitude-zone gating.
    let high_latitude_row_clone = high_latitude_row.clone();
    let polar_row_clone = polar_row.clone();
    let update_calculation_rows = Rc::new(move |config: &AppConfig| {
        let is_moonsighting = config.method() == CalculationMethod::MoonsightingCommittee;
        let latitude_zone = config.latitude_zone();
        high_latitude_row_clone.set_visible(!is_moonsighting && latitude_zone >= 2);
        polar_row_clone.set_visible(!is_moonsighting && latitude_zone >= 3);
    });
    update_calculation_rows(&config);

    let update_calculation_rows_listen = update_calculation_rows.clone();
    let config_calculation = config.clone();
    crate::connect_to_properties(&config, crate::CONFIG_REFRESH_PROPERTIES, move || {
        update_calculation_rows_listen(&config_calculation)
    });

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

    let config_for_adhan_only = config.clone();
    adhan_only_toggle.connect_active_notify(move |row| {
        let enabled = row.is_active();
        config_for_adhan_only.set_adhan_only_mode(enabled);
        if enabled {
            config_for_adhan_only.set_pre_prayer_notify(false);
            config_for_adhan_only.set_iqamah_notify(false);
            config_for_adhan_only.set_adkar_notification_enabled(false);
        } else {
            config_for_adhan_only.set_pre_prayer_notify(true);
            config_for_adhan_only.set_iqamah_notify(true);
            config_for_adhan_only.set_adkar_notification_enabled(true);
        }
        config_for_adhan_only.save();
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

    let config_for_pre_prayer_minutes = config.clone();
    notify_time
        .adjustment()
        .connect_value_changed(move |adjustment| {
            let new_minutes = adjustment.value() as u32;
            config_for_pre_prayer_minutes.set_pre_prayer_minutes(new_minutes);
            config_for_pre_prayer_minutes.save();
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
        config_notify.save();
    });

    let config_for_iqamah_notify = config.clone();
    iqamah_notify_toggle.connect_active_notify(move |row| {
        config_for_iqamah_notify.set_iqamah_notify(row.is_active());
        config_for_iqamah_notify.save();
    });

    let config_adkar = config.clone();
    adkar_toggle.connect_active_notify(move |row| {
        config_adkar.set_adkar_notification_enabled(row.is_active());
        config_adkar.save();
    });

    let test_notification_button = Button::builder()
        .label(tr("Test Notification"))
        .margin_top(12)
        .build();

    let config_for_test_notification = config.clone();
    bind_audio_toggle_button_sync(&test_notification_button, "Test Notification");
    test_notification_button.connect_clicked(move |button| {
        if crate::audio::is_adhan() {
            crate::audio::stop();
            set_audio_toggle_button_label(button, "Test Notification", false);
        } else {
            notifications::show_notification(
                &tr("It's time for"),
                &tr("This is a test notification from Khushu. May your prayers be accepted."),
                true,
                &tr("Open Khushu"),
                &tr("Stop Adhan"),
            );
            if !config_for_test_notification.adhan_muted() {
                let path = config_for_test_notification
                    .adhan_sound_path()
                    .unwrap_or_else(|| "assets/audio/Madinah.mp3".to_string());
                crate::audio::play_adhan(&path, config_for_test_notification.adhan_volume());
                set_audio_toggle_button_label(button, "Test Notification", true);
            }
        }
    });

    notif_group.add(&test_notification_button);

    let mut iqamah_rows = Vec::new();
    for (prayer_name, default_mins) in crate::config::DEFAULT_IQAMAH_MINUTES {
        let current = config
            .iqamah_minutes()
            .get(prayer_name)
            .copied()
            .unwrap_or(default_mins);
        let iqamah_adjustment = gtk::Adjustment::new(current as f64, 0.0, 60.0, 1.0, 5.0, 0.0);
        let iq_row = adw::SpinRow::builder()
            .title(tr(prayer_name))
            .subtitle(tr("Minutes"))
            .adjustment(&iqamah_adjustment)
            .digits(0)
            .build();
        iqamah_rows.push(iq_row.clone());
        iqamah_group.add(&iq_row);

        let config_for_iqamah_minutes = config.clone();
        let prayer_key = prayer_name.to_string();
        iqamah_adjustment.connect_value_changed(move |adjustment| {
            let mut mins = config_for_iqamah_minutes.iqamah_minutes();
            mins.insert(prayer_key.clone(), adjustment.value() as u32);
            config_for_iqamah_minutes.set_iqamah_minutes(mins);
            config_for_iqamah_minutes.save();
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

    let label_refs: Vec<&str> = preset_labels.iter().map(|item| item.as_str()).collect();
    let model = gtk::StringList::new(&label_refs);

    let sound_combo = ComboRow::builder()
        .title(tr("Adhan Sound"))
        .model(&model)
        .build();

    apply_sound_combo_state(
        &sound_combo,
        &preset_files,
        config.adhan_sound_path().as_deref(),
    );

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

            let combo_dialog = combo.clone();
            let parent_window = window_clone_sound.clone();

            gtk::glib::spawn_future_local(async move {
                if let Ok(file) = dialog.open_future(Some(&parent_window)).await
                    && let Some(path) = file.path()
                    && let Some(path_str) = path.to_str()
                {
                    let combo_owned = combo_dialog.clone();
                    let parent = parent_window.clone();
                    crate::audio::validate_audio_async(path_str.to_string(), combo_owned, parent);
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

    let volume_adjustment = gtk::Adjustment::new(
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
        .adjustment(&volume_adjustment)
        .digits(0)
        .build();
    volume_row.set_visible(!config.adhan_muted());

    let config_vol = config.clone();
    volume_adjustment.connect_value_changed(move |adjustment| {
        config_vol.set_adhan_volume((adjustment.value() / 100.0) as f32);
        config_vol.save();
    });
    audio_group.add(&volume_row);

    let volume_row_clone = volume_row.clone();
    mute_toggle.connect_active_notify(move |row| {
        volume_row_clone.set_visible(!row.is_active());
    });

    let test_audio_button = Button::builder()
        .label(tr("▶ Preview Adhan"))
        .margin_top(8)
        .build();

    let config_test = config.clone();
    bind_audio_toggle_button_sync(&test_audio_button, "▶ Preview Adhan");
    test_audio_button.connect_clicked(move |button| {
        if crate::audio::is_adhan() {
            crate::audio::stop();
            set_audio_toggle_button_label(button, "▶ Preview Adhan", false);
        } else {
            if config_test.adhan_muted() {
                return;
            }
            let path = config_test
                .adhan_sound_path()
                .unwrap_or_else(|| "assets/audio/Madinah.mp3".to_string());

            crate::audio::play_adhan(&path, config_test.adhan_volume());
            set_audio_toggle_button_label(button, "▶ Preview Adhan", true);
        }
    });
    audio_group.add(&test_audio_button);

    let ctx = SettingsUiContext {
        config: config.clone(),
        current_language: current_language.clone(),
        sync_lock: sync_lock.clone(),

        general_heading,
        general_desc,
        language_row: language_row.clone(),
        language_model: language_model.clone(),
        theme_row: theme_row.clone(),
        theme_model: theme_model.clone(),
        autostart_toggle,

        fonts_heading,
        fonts_desc,
        ui_font_row: ui_font_row.row.clone(),
        ui_font_reset: ui_font_row.reset_btn.clone(),
        ui_font_dialog: ui_font_row.dialog.clone(),
        arabic_font_row: arabic_font_row.row.clone(),
        arabic_font_reset: arabic_font_row.reset_btn.clone(),
        arabic_font_dialog: arabic_font_row.dialog.clone(),
        quran_font_row: quran_font_row.row.clone(),
        quran_font_reset: quran_font_row.reset_btn.clone(),
        quran_font_dialog: quran_font_row.dialog.clone(),

        prayer_setup_heading,
        prayer_setup_desc,
        location_group: location_group.clone(),
        mode_row: mode_row.clone(),
        mode_model: modes.clone(),
        latitude_row: latitude_row.clone(),
        longitude_row: longitude_row.clone(),
        status_row: status_row.clone(),
        city_row: city_row.clone(),
        city_button: city_button.clone(),
        auto_row: auto_row.clone(),
        auto_button: auto_button.clone(),
        source_row: source_row.clone(),
        source_model: source_model.clone(),
        url_row: url_row.clone(),
        auto_refresh_row: auto_refresh_row.clone(),
        mawaqit_status_row: mawaqit_status_row.clone(),
        refresh_button: refresh_button.clone(),

        travel_group: travel_group.clone(),
        timezone_mode_row: timezone_mode_row.clone(),
        timezone_mode_model: timezone_modes.clone(),
        timezone_named_row: timezone_named_row.clone(),
        timezone_offset_row: timezone_offset_row.clone(),

        calc_group: calc_group.clone(),
        hijri_row: hijri_row.clone(),
        method_row: method_row.clone(),
        method_model: methods.clone(),
        madhab_row: madhab_row.clone(),
        madhab_model: madhabs.clone(),
        high_latitude_row: high_latitude_row.clone(),
        high_latitude_model: high_latitude_model.clone(),
        polar_row: polar_row.clone(),
        polar_model: polar_model.clone(),

        iqamah_group: iqamah_group.clone(),
        iqamah_rows,

        notif_audio_heading,
        notif_audio_desc,
        notify_toggle: notify_toggle.clone(),
        notify_time: notify_time.clone(),
        iqamah_notify_toggle: iqamah_notify_toggle.clone(),
        adkar_toggle: adkar_toggle.clone(),
        adhan_only_toggle: adhan_only_toggle.clone(),
        test_notification_button: test_notification_button.clone(),

        sound_combo: sound_combo.clone(),
        sound_model: model.clone(),
        preset_files,
        mute_toggle: mute_toggle.clone(),
        volume_row: volume_row.clone(),
        test_audio_button,
    };

    let ctx = Rc::new(RefCell::new(ctx));
    *ctx_slot.borrow_mut() = Some(ctx.clone());
    ctx.borrow().sync_location_and_timezone(&selected_language);

    (language_row, ctx)
}

pub fn update_settings_ui_lang(ctx: &SettingsUiContext, language: &str) {
    let app_config = &ctx.config;

    ctx.general_heading.set_label(&tr("General"));
    ctx.general_desc
        .set_label(&tr("Customize the app's appearance and startup behavior."));

    ctx.language_row.set_title(&tr("Language"));
    let language_items = [
        tr("System Default"),
        tr("English"),
        tr("Arabic"),
        tr("French"),
        tr("Spanish"),
        tr("Turkish"),
        tr("Indonesian"),
    ];
    let language_refs: Vec<&str> = language_items.iter().map(|item| item.as_str()).collect();
    ctx.language_model
        .splice(0, ctx.language_model.n_items(), &language_refs);

    ctx.theme_row.set_title(&tr("Theme"));
    let theme_items = [tr("System Default"), tr("Light"), tr("Dark")];
    let theme_refs: Vec<&str> = theme_items.iter().map(|item| item.as_str()).collect();
    ctx.theme_model
        .splice(0, ctx.theme_model.n_items(), &theme_refs);

    ctx.autostart_toggle.set_title(&tr("Start Automatically"));
    ctx.autostart_toggle
        .set_subtitle(&tr("Run Khushu in the background when you log in."));

    ctx.fonts_heading.set_label(&tr("Fonts"));
    ctx.fonts_desc.set_label(&tr(
        "Choose the fonts used across the app. 'System default' follows your desktop font settings.",
    ));

    ctx.ui_font_row.set_title(&tr("UI Font"));
    ctx.ui_font_reset
        .set_tooltip_text(Some(&tr("Restore to default")));
    ctx.ui_font_dialog.set_title(&tr("Select Interface Font"));

    ctx.arabic_font_row.set_title(&tr("Arabic Font"));
    ctx.arabic_font_reset
        .set_tooltip_text(Some(&tr("Restore to default")));
    ctx.arabic_font_dialog.set_title(&tr("Select Arabic Font"));
    ctx.arabic_font_row
        .set_subtitle(&tr("Used for Arabic content, such as the adkar texts."));

    ctx.quran_font_row.set_title(&tr("Quran Font"));
    ctx.quran_font_reset
        .set_tooltip_text(Some(&tr("Restore to default")));
    ctx.quran_font_dialog.set_title(&tr("Select Quran Font"));
    ctx.quran_font_row.set_subtitle(&tr(
        "Must support Uthmani Quran script; otherwise verses may render incorrectly.",
    ));

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
    let mode_refs: Vec<&str> = mode_items.iter().map(|item| item.as_str()).collect();
    ctx.mode_model
        .splice(0, ctx.mode_model.n_items(), &mode_refs);
    ctx.mode_row.set_title(&tr("Location Mode"));

    ctx.latitude_row.set_title(&tr("Latitude"));
    ctx.longitude_row.set_title(&tr("Longitude"));

    ctx.status_row.set_title(&tr("Location Status"));

    ctx.city_row.set_title(&tr("City Search"));
    ctx.city_button.set_label(&tr("Search"));

    let city_row_for_geocode = ctx.city_row.clone();
    let auto_row_for_geocode = ctx.auto_row.clone();
    let current_language_for_geocode = ctx.current_language.clone();
    let loc_mode = app_config.location_mode();
    let latitude_for_geocode = app_config.latitude();
    let longitude_for_geocode = app_config.longitude();
    let language_for_geocode = language.to_string();

    if app_config.location_mode() == crate::config::LocationMode::City
        && let Some(text) =
            location::display_city_label(app_config.city_name().as_deref(), language)
    {
        ctx.city_row.set_text(&text);
    }

    ctx.auto_row.set_title(&tr("Auto Detection"));
    ctx.auto_button.set_label(&tr("Update Now"));

    if app_config.location_mode() == crate::config::LocationMode::Auto
        && let Some(name) = &app_config.city_name()
    {
        ctx.auto_row
            .set_subtitle(&location::short_city_with_country(name));
    }

    if matches!(
        loc_mode,
        crate::config::LocationMode::City | crate::config::LocationMode::Auto
    ) && app_config.mawaqit_cache().is_none()
    {
        gtk::glib::spawn_future_local(async move {
            if let Ok(name) = crate::location::resolve_city_name(
                latitude_for_geocode,
                longitude_for_geocode,
                &language_for_geocode,
            )
            .await
                && crate::i18n::resolved_locale(&current_language_for_geocode.borrow())
                    == language_for_geocode
            {
                let short = crate::location::short_city_with_country(&name);
                if loc_mode == crate::config::LocationMode::City {
                    city_row_for_geocode.set_text(&short);
                } else {
                    auto_row_for_geocode.set_subtitle(&short);
                }
            }
        });
    }

    let source_items = [tr("Calculated (Offline)"), tr("Connected Mosque (URL)")];
    let source_refs: Vec<&str> = source_items.iter().map(|item| item.as_str()).collect();
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
    if let Some(cache) = app_config.mawaqit_cache().as_ref() {
        ctx.mawaqit_status_row
            .set_subtitle(&mawaqit_status_subtitle(cache, language));
    } else {
        ctx.mawaqit_status_row.set_subtitle(&tr("Not configured"));
    }
    ctx.refresh_button.set_label(&tr("Refresh now"));

    ctx.travel_group
        .set_title(&gtk::glib::markup_escape_text(&tr("Timezone & Travel")));
    ctx.travel_group.set_description(Some(&tr(
        "Override the timezone for prayer time calculations.",
    )));

    let timezone_mode_items = [
        tr("Automatic (System)"),
        tr("Custom Timezone (IANA)"),
        tr("Manual UTC Offset"),
    ];
    let timezone_mode_refs: Vec<&str> = timezone_mode_items
        .iter()
        .map(|item| item.as_str())
        .collect();
    ctx.timezone_mode_model
        .splice(0, ctx.timezone_mode_model.n_items(), &timezone_mode_refs);
    ctx.timezone_mode_row.set_title(&tr("Timezone Mode"));
    ctx.timezone_mode_row
        .set_subtitle(&tr("How prayer times are adjusted for your timezone."));

    ctx.timezone_named_row.set_title(&tr("IANA Timezone"));
    let current_timezone = match app_config.timezone_mode() {
        TimezoneMode::Named(name) => name,
        _ => location::system_time_zone_id().unwrap_or_default(),
    };
    let timezone_label = location::localized_time_zone_label(&current_timezone, language);
    ctx.timezone_named_row
        .set_subtitle(if timezone_label.is_empty() {
            &current_timezone
        } else {
            &timezone_label
        });

    ctx.timezone_offset_row.set_title(&tr("UTC Offset (hours)"));
    ctx.timezone_offset_row
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
    let method_refs: Vec<&str> = method_items.iter().map(|item| item.as_str()).collect();
    ctx.method_model
        .splice(0, ctx.method_model.n_items(), &method_refs);
    ctx.method_row.set_title(&tr("Calculation Method"));

    let madhab_items = [tr("Shafi (Maliki/Hanbali)"), tr("Hanafi")];
    let madhab_refs: Vec<&str> = madhab_items.iter().map(|item| item.as_str()).collect();
    ctx.madhab_model
        .splice(0, ctx.madhab_model.n_items(), &madhab_refs);
    ctx.madhab_row.set_title(&tr("Asr Calculation (Madhab)"));

    ctx.high_latitude_row
        .set_title(&tr("Fajr/Isha Approximation"));
    ctx.high_latitude_row.set_subtitle(&tr(
        "How to approximate Fajr and Isha when the twilight angle is not reachable.",
    ));
    let high_latitude_items = [
        tr("Auto (Recommended)"),
        tr("Middle of the Night"),
        tr("Seventh of the Night"),
        tr("Twilight Angle"),
        tr("Local Relative Estimation"),
    ];
    let high_latitude_refs: Vec<&str> = high_latitude_items
        .iter()
        .map(|item| item.as_str())
        .collect();
    ctx.high_latitude_model
        .splice(0, ctx.high_latitude_model.n_items(), &high_latitude_refs);

    ctx.polar_row.set_title(&tr("Polar Estimation Method"));
    ctx.polar_row.set_subtitle(&tr(
        "Fallback applied when the sun never rises or sets (above ~66.5° latitude).",
    ));
    let polar_items = [tr("Nearest Latitude"), tr("Reference Latitude 45°")];
    let polar_refs: Vec<&str> = polar_items.iter().map(|item| item.as_str()).collect();
    ctx.polar_model
        .splice(0, ctx.polar_model.n_items(), &polar_refs);

    ctx.iqamah_group.set_title(&tr("Iqamah Delays"));
    ctx.iqamah_group.set_description(Some(&tr(
        "Minutes to wait after the Adhan before the Iqamah (second call to prayer).",
    )));

    let prayer_names = ["Fajr", "Dhuhr", "Asr", "Maghrib", "Isha"];
    for (prayer_index, name) in prayer_names.iter().enumerate() {
        if let Some(row) = ctx.iqamah_rows.get(prayer_index) {
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

    ctx.test_notification_button
        .set_label(&tr("Test Notification"));

    let mut preset_labels: Vec<String> = Vec::new();
    preset_labels.push(tr("Default"));
    preset_labels.push(tr("Custom File..."));
    for name in &ctx.preset_files {
        preset_labels.push(adhan_preset_label(name));
    }
    let preset_refs: Vec<&str> = preset_labels.iter().map(|item| item.as_str()).collect();
    ctx.sound_model
        .splice(0, ctx.sound_model.n_items(), &preset_refs);
    ctx.sound_combo.set_title(&tr("Adhan Sound"));

    apply_sound_combo_state(
        &ctx.sound_combo,
        &ctx.preset_files,
        ctx.config.adhan_sound_path().as_deref(),
    );

    ctx.mute_toggle.set_title(&tr("Mute Adhan"));
    ctx.mute_toggle
        .set_subtitle(&tr("Silence the Adhan sound at prayer time."));

    ctx.volume_row.set_title(&tr("Adhan Volume"));
    ctx.volume_row.set_subtitle(&tr("Volume level (0–100%)"));

    ctx.test_audio_button.set_label(&tr("▶ Preview Adhan"));

    ctx.sync_location_and_timezone(language);
}

fn adhan_preset_label(file_name: &str) -> String {
    let stem = std::path::Path::new(file_name)
        .file_stem()
        .and_then(|os_str| os_str.to_str())
        .unwrap_or(file_name);
    match stem {
        "Makkah" => tr("Makkah Adhan"),
        "Madinah" => tr("Madinah Adhan"),
        _ => stem.to_string(),
    }
}

pub fn refresh_prayers(
    config: &AppConfig,
    list_box: &ListBox,
) -> Option<crate::time::PrayerResult> {
    while let Some(child) = list_box.first_child() {
        list_box.remove(&child);
    }

    let today = crate::time::effective_today(config);
    if let Ok(result) = crate::time::schedule_for_config(config, today) {
        // For Connected Mosque the times are the mosque's published
        // wall-clock strings; show them as-is (12/24h preference only) so the
        // label matches the mosque regardless of the system timezone.
        let published_times =
            if config.prayer_times_source() == crate::config::PrayerTimesSource::Mawaqit {
                config.mawaqit_cache().and_then(|cache| {
                    cache
                        .months
                        .get(today.month0() as usize)?
                        .get(&today.day())
                        .cloned()
                })
            } else {
                None
            };

        let prayers = [
            ("Fajr", result.schedule.fajr, 0),
            ("Sunrise", result.schedule.shurooq, 1),
            ("Dhuhr", result.schedule.dhuhr, 2),
            ("Asr", result.schedule.asr, 3),
            ("Maghrib", result.schedule.maghrib, 4),
            ("Isha", result.schedule.isha, 5),
        ];

        for (name, time, published_idx) in prayers {
            let subtitle = match &published_times {
                Some(times) => {
                    location::format_published_time(&times[published_idx], config.time_format())
                }
                None => {
                    location::localized_time_only(time, &config.language(), config.time_format())
                }
            };
            let row = adw::ActionRow::builder()
                .title(tr(name))
                .subtitle(&subtitle)
                .name(name)
                .build();
            list_box.append(&row);
        }
        if false {
            tr("Fajr");
            tr("Dhuhr");
            tr("Asr");
            tr("Maghrib");
            tr("Isha");
            tr("Sunrise");
        }
        Some(result)
    } else {
        None
    }
}

pub(crate) fn update_fallback_toast(
    config: &AppConfig,
    result: &crate::time::PrayerResult,
    window: &adw::ApplicationWindow,
) {
    let prev = config.fallback_was_active();
    if prev == result.fallback_active {
        return;
    }
    let Some(overlay) = find_toast_overlay(window) else {
        return;
    };
    let toast = if result.fallback_active {
        adw::Toast::new(&tr(
            "Prayer times are calculated at a substituted latitude.",
        ))
    } else {
        adw::Toast::new(&tr(
            "Prayer times are now calculated at your actual latitude.",
        ))
    };
    toast.set_timeout(0);
    overlay.add_toast(toast);
    config.set_fallback_was_active(result.fallback_active);
    config.save();
}

pub(crate) fn update_lre_toast(
    config: &AppConfig,
    result: &crate::time::PrayerResult,
    window: &adw::ApplicationWindow,
) {
    let prev = config.lre_was_blocked();
    if prev != result.lre_blocked {
        if result.lre_blocked
            && let Some(overlay) = find_toast_overlay(window)
        {
            overlay.add_toast(adw::Toast::new(&tr("LRE not available at this latitude.")));
        }
        config.set_lre_was_blocked(result.lre_blocked);
        config.save();
    }
}
