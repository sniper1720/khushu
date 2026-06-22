use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use gtk4::glib;
use gtk4::glib::prelude::*;
use gtk4::glib::subclass::prelude::*;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum LocationMode {
    Manual,
    City,
    Auto,
}

#[derive(Serialize, Deserialize, Default, Debug, Clone, PartialEq)]
pub enum PrayerTimesSource {
    #[default]
    Calculated,
    Mawaqit,
}

#[derive(Serialize, Deserialize, Default, Debug, Clone, PartialEq)]
#[allow(clippy::upper_case_acronyms)]
pub enum CalculationMethod {
    #[default]
    MWL,
    ISNA,
    Egypt,
    Makkah,
    Karachi,
    Dubai,
    MoonsightingCommittee,
    Kuwait,
    Qatar,
    Singapore,
    Turkey,
    #[serde(rename = "kemenag")]
    Kemenag,
    #[serde(rename = "france")]
    France,
    #[serde(rename = "algeria")]
    Algeria,
}

#[derive(Serialize, Deserialize, Default, Debug, Clone, PartialEq)]
pub enum MadhabChoice {
    #[default]
    Shafi,
    Hanafi,
}

#[derive(Serialize, Deserialize, Default, Debug, Clone, PartialEq)]
pub enum ThemeMode {
    #[serde(rename = "system")]
    #[default]
    System,
    #[serde(rename = "light")]
    Light,
    #[serde(rename = "dark")]
    Dark,
}

#[derive(Serialize, Deserialize, Default, Debug, Clone, PartialEq)]
pub enum TimezoneMode {
    #[serde(rename = "auto")]
    #[default]
    Auto,
    #[serde(rename = "named")]
    Named(String),
    #[serde(rename = "utc_offset")]
    UtcOffset(i32),
}

fn default_volume() -> f32 {
    1.0
}

fn default_autostart() -> bool {
    false
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StopCondition {
    #[serde(rename = "none")]
    None,
    #[serde(rename = "ayat")]
    #[default]
    Ayat,
    #[serde(rename = "page")]
    Page,
    #[serde(rename = "juz")]
    Juz,
    #[serde(rename = "surah")]
    Surah,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct QuranBookmark {
    pub page: u32,
    #[serde(default)]
    pub surah: u32,
    #[serde(default)]
    pub verse: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MawaqitCache {
    pub url: String,
    #[serde(default)]
    pub mosque_name: Option<String>,
    #[serde(default)]
    pub timezone: Option<String>,
    #[serde(default)]
    pub latitude: Option<f64>,
    #[serde(default)]
    pub longitude: Option<f64>,
    #[serde(default)]
    pub country_code: Option<String>,
    pub year: i32,
    #[serde(default)]
    pub months: Vec<std::collections::BTreeMap<u32, [String; 6]>>,
    #[serde(default)]
    pub fetched_on: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AppConfigData {
    pub latitude: f64,
    pub longitude: f64,
    pub method: CalculationMethod,
    pub madhab: MadhabChoice,
    pub location_mode: LocationMode,
    pub city_name: Option<String>,
    pub adhan_sound_path: Option<String>,
    pub pre_prayer_notify: bool,
    pub pre_prayer_minutes: u32,
    pub hijri_offset: i64,
    #[serde(default)]
    pub favorites: Vec<String>,
    #[serde(default)]
    pub adkar_notification_enabled: bool,
    #[serde(default)]
    pub iqamah_notify: bool,
    #[serde(default)]
    pub adhan_only_mode: bool,
    #[serde(default)]
    pub language: String,
    #[serde(default)]
    pub theme: ThemeMode,
    #[serde(default)]
    pub is_configured: bool,
    #[serde(default = "default_volume")]
    pub adhan_volume: f32,
    #[serde(default)]
    pub adhan_muted: bool,
    #[serde(default = "default_autostart")]
    pub autostart: bool,
    #[serde(default)]
    pub quran_bookmark_surah: Option<u32>,
    #[serde(default)]
    pub quran_bookmark_page: Option<u32>,
    #[serde(default)]
    pub quran_bookmarks: Vec<QuranBookmark>,
    #[serde(default)]
    pub quran_last_surah: Option<u32>,
    #[serde(default)]
    pub quran_last_page: Option<u32>,
    #[serde(default)]
    pub prayer_times_source: PrayerTimesSource,
    #[serde(default)]
    pub mawaqit_url: Option<String>,
    #[serde(default)]
    pub mawaqit_auto_refresh_daily: bool,
    #[serde(default)]
    pub mawaqit_cache: Option<MawaqitCache>,
    #[serde(default)]
    pub timezone_override_minutes: Option<i32>,
    #[serde(default)]
    pub timezone_mode: TimezoneMode,
    #[serde(default = "default_quran_arabic_font_px")]
    pub quran_arabic_font_px: f64,
    #[serde(default = "default_quran_translation_font_px")]
    pub quran_translation_font_px: f64,
    #[serde(default = "default_quran_line_height")]
    pub quran_line_height: f64,
    #[serde(default = "default_global_arabic_font_family")]
    pub global_arabic_font_family: String,
    #[serde(default = "default_global_ui_font_family")]
    pub global_ui_font_family: String,
    #[serde(default = "default_iqamah_minutes")]
    pub iqamah_minutes: HashMap<String, u32>,
    #[serde(default = "default_reciter_slug")]
    pub reciter_slug: String,
    #[serde(default)]
    pub stop_condition: StopCondition,
    #[serde(default)]
    pub installed_reciters: Vec<String>,
}

impl Default for AppConfigData {
    fn default() -> Self {
        Self {
            latitude: 36.75,
            longitude: 3.05,
            method: CalculationMethod::MWL,
            madhab: MadhabChoice::Shafi,
            location_mode: LocationMode::Manual,
            city_name: None,
            adhan_sound_path: None,
            pre_prayer_notify: true,
            pre_prayer_minutes: 15,
            hijri_offset: 0,
            favorites: Vec::new(),
            adkar_notification_enabled: true,
            iqamah_notify: true,
            adhan_only_mode: false,
            language: "auto".to_string(),
            theme: ThemeMode::System,
            is_configured: false,
            adhan_volume: 1.0,
            adhan_muted: false,
            autostart: true,
            quran_bookmark_surah: None,
            quran_bookmark_page: None,
            quran_bookmarks: Vec::new(),
            quran_last_surah: None,
            quran_last_page: None,
            prayer_times_source: PrayerTimesSource::Calculated,
            mawaqit_url: None,
            mawaqit_auto_refresh_daily: false,
            mawaqit_cache: None,
            timezone_override_minutes: None,
            timezone_mode: TimezoneMode::Auto,
            quran_arabic_font_px: default_quran_arabic_font_px(),
            quran_translation_font_px: default_quran_translation_font_px(),
            quran_line_height: default_quran_line_height(),
            global_arabic_font_family: default_global_arabic_font_family(),
            global_ui_font_family: default_global_ui_font_family(),
            iqamah_minutes: default_iqamah_minutes(),
            reciter_slug: default_reciter_slug(),
            stop_condition: StopCondition::default(),
            installed_reciters: Vec::new(),
        }
    }
}

fn default_quran_arabic_font_px() -> f64 {
    22.0
}

fn default_quran_translation_font_px() -> f64 {
    14.0
}

fn default_quran_line_height() -> f64 {
    1.0
}

fn default_global_arabic_font_family() -> String {
    "Amiri, Noto Sans Arabic".to_string()
}

fn default_global_ui_font_family() -> String {
    "Cantarell, sans-serif".to_string()
}

fn default_reciter_slug() -> String {
    "Minshawy_Murattal_128kbps".to_string()
}

fn default_iqamah_minutes() -> HashMap<String, u32> {
    let mut m = HashMap::new();
    m.insert("Fajr".to_string(), 20);
    m.insert("Dhuhr".to_string(), 10);
    m.insert("Asr".to_string(), 10);
    m.insert("Maghrib".to_string(), 5);
    m.insert("Isha".to_string(), 10);
    m
}

mod imp {
    use super::*;
    use std::sync::LazyLock;

    #[derive(Default)]
    pub struct AppConfig {
        pub data: RefCell<AppConfigData>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for AppConfig {
        const NAME: &'static str = "KhushuAppConfig";
        type Type = super::AppConfig;
    }

    impl ObjectImpl for AppConfig {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
                vec![
                    glib::ParamSpecDouble::builder("latitude")
                        .nick("Latitude")
                        .minimum(-90.0)
                        .maximum(90.0)
                        .default_value(36.75)
                        .build(),
                    glib::ParamSpecDouble::builder("longitude")
                        .nick("Longitude")
                        .minimum(-180.0)
                        .maximum(180.0)
                        .default_value(3.05)
                        .build(),
                    glib::ParamSpecString::builder("method")
                        .nick("Calculation Method")
                        .read_only()
                        .build(),
                    glib::ParamSpecString::builder("madhab")
                        .nick("Madhab")
                        .read_only()
                        .build(),
                    glib::ParamSpecString::builder("language")
                        .nick("Language")
                        .read_only()
                        .build(),
                    glib::ParamSpecString::builder("city-name")
                        .nick("City Name")
                        .read_only()
                        .build(),
                    glib::ParamSpecString::builder("prayer-times-source")
                        .nick("Prayer Times Source")
                        .read_only()
                        .build(),
                    glib::ParamSpecInt::builder("timezone-override-minutes")
                        .nick("Timezone Override")
                        .read_only()
                        .build(),
                    glib::ParamSpecString::builder("timezone-mode")
                        .nick("Timezone Mode")
                        .read_only()
                        .build(),
                    glib::ParamSpecString::builder("location-mode")
                        .nick("Location Mode")
                        .read_only()
                        .build(),
                ]
            });
            PROPERTIES.as_ref()
        }

        fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            let obj = self.obj();
            match pspec.name() {
                "latitude" => obj.latitude().to_value(),
                "longitude" => obj.longitude().to_value(),
                "method" => format!("{:?}", obj.method()).to_value(),
                "madhab" => format!("{:?}", obj.madhab()).to_value(),
                "language" => obj.language().to_value(),
                "city-name" => obj.city_name().to_value(),
                "prayer-times-source" => format!("{:?}", obj.prayer_times_source()).to_value(),
                "timezone-override-minutes" => {
                    obj.timezone_override_minutes().unwrap_or(-1).to_value()
                }
                "timezone-mode" => format!("{:?}", obj.timezone_mode()).to_value(),
                "location-mode" => format!("{:?}", obj.location_mode()).to_value(),
                _ => unimplemented!("property {:?}", pspec.name()),
            }
        }

        fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            let obj = self.obj();
            match pspec.name() {
                "latitude" => obj.set_latitude(value.get().expect("latitude param value")),
                "longitude" => obj.set_longitude(value.get().expect("longitude param value")),
                _ => unimplemented!("set_property {:?}", pspec.name()),
            }
        }
    }
}

glib::wrapper! {
    pub struct AppConfig(ObjectSubclass<imp::AppConfig>);
}

impl Default for AppConfig {
    fn default() -> Self {
        glib::Object::new()
    }
}

thread_local! {
    static CONFIG_INSTANCE: std::cell::RefCell<Option<AppConfig>> = const { std::cell::RefCell::new(None) };
}

impl AppConfig {
    pub fn language(&self) -> String {
        self.imp().data.borrow().language.clone()
    }
    pub fn set_language(&self, val: &str) {
        self.imp().data.borrow_mut().language = val.to_string();
        self.notify("language");
    }

    pub fn theme(&self) -> ThemeMode {
        self.imp().data.borrow().theme.clone()
    }
    pub fn set_theme(&self, val: ThemeMode) {
        self.imp().data.borrow_mut().theme = val;
    }

    pub fn latitude(&self) -> f64 {
        self.imp().data.borrow().latitude
    }
    pub fn set_latitude(&self, val: f64) {
        if (self.latitude() - val).abs() > 1e-10 {
            self.imp().data.borrow_mut().latitude = val;
            self.notify("latitude");
        }
    }

    pub fn longitude(&self) -> f64 {
        self.imp().data.borrow().longitude
    }
    pub fn set_longitude(&self, val: f64) {
        if (self.longitude() - val).abs() > 1e-10 {
            self.imp().data.borrow_mut().longitude = val;
            self.notify("longitude");
        }
    }

    pub fn city_name(&self) -> Option<String> {
        self.imp().data.borrow().city_name.clone()
    }
    pub fn set_city_name(&self, val: Option<String>) {
        self.imp().data.borrow_mut().city_name = val;
        self.notify("city-name");
    }

    pub fn method(&self) -> CalculationMethod {
        self.imp().data.borrow().method.clone()
    }
    pub fn set_method(&self, val: CalculationMethod) {
        self.imp().data.borrow_mut().method = val;
        self.notify("method");
    }

    pub fn madhab(&self) -> MadhabChoice {
        self.imp().data.borrow().madhab.clone()
    }
    pub fn set_madhab(&self, val: MadhabChoice) {
        self.imp().data.borrow_mut().madhab = val;
        self.notify("madhab");
    }

    pub fn location_mode(&self) -> LocationMode {
        self.imp().data.borrow().location_mode.clone()
    }
    pub fn set_location_mode(&self, val: LocationMode) {
        if self.location_mode() != val {
            self.imp().data.borrow_mut().location_mode = val;
            self.notify("location-mode");
        }
    }

    pub fn adhan_sound_path(&self) -> Option<String> {
        self.imp().data.borrow().adhan_sound_path.clone()
    }
    pub fn set_adhan_sound_path(&self, val: Option<String>) {
        self.imp().data.borrow_mut().adhan_sound_path = val;
    }

    pub fn pre_prayer_notify(&self) -> bool {
        self.imp().data.borrow().pre_prayer_notify
    }
    pub fn set_pre_prayer_notify(&self, val: bool) {
        self.imp().data.borrow_mut().pre_prayer_notify = val;
    }

    pub fn pre_prayer_minutes(&self) -> u32 {
        self.imp().data.borrow().pre_prayer_minutes
    }
    pub fn set_pre_prayer_minutes(&self, val: u32) {
        self.imp().data.borrow_mut().pre_prayer_minutes = val;
    }

    pub fn hijri_offset(&self) -> i64 {
        self.imp().data.borrow().hijri_offset
    }
    pub fn set_hijri_offset(&self, val: i64) {
        self.imp().data.borrow_mut().hijri_offset = val;
    }

    pub fn favorites(&self) -> Vec<String> {
        self.imp().data.borrow().favorites.clone()
    }
    pub fn set_favorites(&self, val: Vec<String>) {
        self.imp().data.borrow_mut().favorites = val;
    }

    pub fn adkar_notification_enabled(&self) -> bool {
        self.imp().data.borrow().adkar_notification_enabled
    }
    pub fn set_adkar_notification_enabled(&self, val: bool) {
        self.imp().data.borrow_mut().adkar_notification_enabled = val;
    }

    pub fn iqamah_notify(&self) -> bool {
        self.imp().data.borrow().iqamah_notify
    }
    pub fn set_iqamah_notify(&self, val: bool) {
        self.imp().data.borrow_mut().iqamah_notify = val;
    }

    pub fn adhan_only_mode(&self) -> bool {
        self.imp().data.borrow().adhan_only_mode
    }
    pub fn set_adhan_only_mode(&self, val: bool) {
        self.imp().data.borrow_mut().adhan_only_mode = val;
    }

    pub fn is_configured(&self) -> bool {
        self.imp().data.borrow().is_configured
    }
    pub fn set_is_configured(&self, val: bool) {
        self.imp().data.borrow_mut().is_configured = val;
    }

    pub fn adhan_volume(&self) -> f32 {
        self.imp().data.borrow().adhan_volume
    }
    pub fn set_adhan_volume(&self, val: f32) {
        self.imp().data.borrow_mut().adhan_volume = val;
    }

    pub fn adhan_muted(&self) -> bool {
        self.imp().data.borrow().adhan_muted
    }
    pub fn set_adhan_muted(&self, val: bool) {
        self.imp().data.borrow_mut().adhan_muted = val;
    }

    pub fn autostart(&self) -> bool {
        self.imp().data.borrow().autostart
    }
    pub fn set_autostart(&self, val: bool) {
        self.imp().data.borrow_mut().autostart = val;
    }

    pub fn quran_bookmark_surah(&self) -> Option<u32> {
        self.imp().data.borrow().quran_bookmark_surah
    }
    pub fn set_quran_bookmark_surah(&self, val: Option<u32>) {
        self.imp().data.borrow_mut().quran_bookmark_surah = val;
    }

    pub fn quran_bookmark_page(&self) -> Option<u32> {
        self.imp().data.borrow().quran_bookmark_page
    }
    pub fn set_quran_bookmark_page(&self, val: Option<u32>) {
        self.imp().data.borrow_mut().quran_bookmark_page = val;
    }

    pub fn quran_bookmarks(&self) -> Vec<QuranBookmark> {
        self.imp().data.borrow().quran_bookmarks.clone()
    }
    pub fn set_quran_bookmarks(&self, val: Vec<QuranBookmark>) {
        self.imp().data.borrow_mut().quran_bookmarks = val;
    }

    pub fn quran_last_surah(&self) -> Option<u32> {
        self.imp().data.borrow().quran_last_surah
    }
    pub fn set_quran_last_surah(&self, val: Option<u32>) {
        self.imp().data.borrow_mut().quran_last_surah = val;
    }

    pub fn quran_last_page(&self) -> Option<u32> {
        self.imp().data.borrow().quran_last_page
    }
    pub fn set_quran_last_page(&self, val: Option<u32>) {
        self.imp().data.borrow_mut().quran_last_page = val;
    }

    pub fn prayer_times_source(&self) -> PrayerTimesSource {
        self.imp().data.borrow().prayer_times_source.clone()
    }
    pub fn set_prayer_times_source(&self, val: PrayerTimesSource) {
        self.imp().data.borrow_mut().prayer_times_source = val;
        self.notify("prayer-times-source");
    }

    pub fn mawaqit_url(&self) -> Option<String> {
        self.imp().data.borrow().mawaqit_url.clone()
    }
    pub fn set_mawaqit_url(&self, val: Option<String>) {
        self.imp().data.borrow_mut().mawaqit_url = val;
    }

    pub fn mawaqit_auto_refresh_daily(&self) -> bool {
        self.imp().data.borrow().mawaqit_auto_refresh_daily
    }
    pub fn set_mawaqit_auto_refresh_daily(&self, val: bool) {
        self.imp().data.borrow_mut().mawaqit_auto_refresh_daily = val;
    }

    pub fn mawaqit_cache(&self) -> Option<MawaqitCache> {
        self.imp().data.borrow().mawaqit_cache.clone()
    }
    pub fn set_mawaqit_cache(&self, val: Option<MawaqitCache>) {
        self.imp().data.borrow_mut().mawaqit_cache = val;
    }

    pub fn timezone_override_minutes(&self) -> Option<i32> {
        self.imp().data.borrow().timezone_override_minutes
    }
    pub fn set_timezone_override_minutes(&self, val: Option<i32>) {
        self.imp().data.borrow_mut().timezone_override_minutes = val;
        self.notify("timezone-override-minutes");
    }

    pub fn timezone_mode(&self) -> TimezoneMode {
        self.imp().data.borrow().timezone_mode.clone()
    }
    pub fn set_timezone_mode(&self, val: TimezoneMode) {
        self.imp().data.borrow_mut().timezone_mode = val;
        self.notify("timezone-mode");
    }

    pub fn quran_arabic_font_px(&self) -> f64 {
        self.imp().data.borrow().quran_arabic_font_px
    }
    pub fn set_quran_arabic_font_px(&self, val: f64) {
        self.imp().data.borrow_mut().quran_arabic_font_px = val;
    }

    pub fn quran_translation_font_px(&self) -> f64 {
        self.imp().data.borrow().quran_translation_font_px
    }
    pub fn set_quran_translation_font_px(&self, val: f64) {
        self.imp().data.borrow_mut().quran_translation_font_px = val;
    }

    pub fn quran_line_height(&self) -> f64 {
        self.imp().data.borrow().quran_line_height
    }
    pub fn set_quran_line_height(&self, val: f64) {
        self.imp().data.borrow_mut().quran_line_height = val;
    }

    pub fn global_arabic_font_family(&self) -> String {
        self.imp().data.borrow().global_arabic_font_family.clone()
    }
    pub fn set_global_arabic_font_family(&self, val: &str) {
        self.imp().data.borrow_mut().global_arabic_font_family = val.to_string();
    }

    pub fn global_ui_font_family(&self) -> String {
        self.imp().data.borrow().global_ui_font_family.clone()
    }
    pub fn set_global_ui_font_family(&self, val: &str) {
        self.imp().data.borrow_mut().global_ui_font_family = val.to_string();
    }

    pub fn iqamah_minutes(&self) -> HashMap<String, u32> {
        self.imp().data.borrow().iqamah_minutes.clone()
    }
    pub fn set_iqamah_minutes(&self, val: HashMap<String, u32>) {
        self.imp().data.borrow_mut().iqamah_minutes = val;
    }

    pub fn reciter_slug(&self) -> String {
        self.imp().data.borrow().reciter_slug.clone()
    }
    pub fn set_reciter_slug(&self, val: &str) {
        self.imp().data.borrow_mut().reciter_slug = val.to_string();
    }

    pub fn stop_condition(&self) -> StopCondition {
        self.imp().data.borrow().stop_condition
    }
    pub fn set_stop_condition(&self, val: StopCondition) {
        self.imp().data.borrow_mut().stop_condition = val;
    }

    pub fn installed_reciters(&self) -> Vec<String> {
        self.imp().data.borrow().installed_reciters.clone()
    }
    pub fn set_installed_reciters(&self, val: Vec<String>) {
        self.imp().data.borrow_mut().installed_reciters = val;
    }
    pub fn add_installed_reciter(&self, slug: &str) {
        let mut data = self.imp().data.borrow_mut();
        if !data.installed_reciters.contains(&slug.to_string()) {
            data.installed_reciters.push(slug.to_string());
        }
    }
    pub fn remove_installed_reciter(&self, slug: &str) {
        self.imp()
            .data
            .borrow_mut()
            .installed_reciters
            .retain(|s| s != slug);
    }

    fn load_data() -> AppConfigData {
        let path = Self::config_path();
        if let Ok(content) = fs::read_to_string(&path)
            && let Ok(config) = serde_json::from_str::<AppConfigData>(&content)
        {
            log::info!("Configuration loaded from {:?}", path);
            return config;
        }
        log::info!("No existing configuration found, using defaults");
        AppConfigData::default()
    }

    pub fn load() -> Self {
        CONFIG_INSTANCE.with(|cell| {
            cell.borrow_mut()
                .get_or_insert_with(|| {
                    let data = Self::load_data();
                    let config: Self = glib::Object::new();
                    let imp = config.imp();
                    *imp.data.borrow_mut() = data;
                    config
                })
                .clone()
        })
    }

    fn to_data(&self) -> AppConfigData {
        self.imp().data.borrow().clone()
    }

    pub fn save(&self) {
        Self::write_to_disk(&self.to_data());
    }

    fn write_to_disk(data: &AppConfigData) {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(content) = serde_json::to_string_pretty(data) {
            let tmp_path = path.with_extension("json.tmp");
            if let Ok(mut file) = std::fs::File::create(&tmp_path)
                && file.write_all(content.as_bytes()).is_ok()
                && file.flush().is_ok()
                && std::fs::rename(&tmp_path, &path).is_ok()
            {
                let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
                log::info!("Configuration saved to {:?}", path);
                return;
            }
            let _ = std::fs::remove_file(&tmp_path);
            log::error!("Failed to save configuration to {:?}", path);
        } else {
            log::error!("Failed to serialize configuration");
        }
    }

    pub fn save_shared(config: &Self) {
        config.save();
    }

    pub fn config_path() -> PathBuf {
        let mut path = glib::user_config_dir();
        path.push("khushu");
        path.push("config.json");
        path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_arabic_font_family() {
        let config = AppConfigData::default();
        assert_eq!(config.global_arabic_font_family, "Amiri, Noto Sans Arabic");
    }

    #[test]
    fn test_default_config_ui_font_family() {
        let config = AppConfigData::default();
        assert_eq!(config.global_ui_font_family, "Cantarell, sans-serif");
    }

    #[test]
    fn test_default_config_quran_font_sizes() {
        let config = AppConfigData::default();
        assert_eq!(config.quran_arabic_font_px, 22.0);
        assert_eq!(config.quran_translation_font_px, 14.0);
        assert_eq!(config.quran_line_height, 1.0);
    }

    #[test]
    fn test_default_config_location() {
        let config = AppConfigData::default();
        assert_eq!(config.latitude, 36.75);
        assert_eq!(config.longitude, 3.05);
        assert_eq!(config.location_mode, LocationMode::Manual);
    }

    #[test]
    fn test_default_config_iqamah_minutes() {
        let config = AppConfigData::default();
        let iqamah = &config.iqamah_minutes;
        assert_eq!(iqamah.get("Fajr"), Some(&20));
        assert_eq!(iqamah.get("Dhuhr"), Some(&10));
        assert_eq!(iqamah.get("Asr"), Some(&10));
        assert_eq!(iqamah.get("Maghrib"), Some(&5));
        assert_eq!(iqamah.get("Isha"), Some(&10));
    }
}
