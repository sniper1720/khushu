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

#[derive(Serialize, Deserialize, Default, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TimeFormat {
    /// Follow app language convention via ICU4X locale.
    #[default]
    Auto,
    /// Force 24-hour format regardless of language.
    #[serde(rename = "24h")]
    Hours24,
    /// Force 12-hour (AM/PM) format regardless of language.
    #[serde(rename = "12h")]
    Hours12,
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
pub enum HighLatitudeChoice {
    #[default]
    Auto,
    MiddleOfTheNight,
    SeventhOfTheNight,
    TwilightAngle,
    LocalRelativeEstimation,
}

#[derive(Serialize, Deserialize, Default, Debug, Clone, PartialEq)]
pub enum PolarEstimationMethod {
    #[default]
    NearestLatitude,
    Reference45,
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
    true
}

fn default_adkar_notification_enabled() -> bool {
    true
}

fn default_iqamah_notify() -> bool {
    true
}

fn default_language() -> String {
    "auto".to_string()
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StopCondition {
    #[serde(rename = "none")]
    None,
    #[serde(rename = "ayah")]
    #[default]
    Ayah,
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
    #[serde(default, alias = "surah", alias = "surah_num")]
    pub surah_number: u32,
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

/// Config schema version, distinct from the app version.
/// `0` = files written before versioning; bump this to migrate old files once.
pub const CONFIG_SCHEMA_VERSION: u32 = 1;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AppConfigData {
    /// Schema version; `0` when absent (legacy files).
    #[serde(default)]
    pub schema_version: u32,
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
    #[serde(default = "default_adkar_notification_enabled")]
    pub adkar_notification_enabled: bool,
    #[serde(default = "default_iqamah_notify")]
    pub iqamah_notify: bool,
    #[serde(default)]
    pub adhan_only_mode: bool,
    #[serde(default = "default_language")]
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
    // Migrated from v0 single-bookmark fields — serde reads these via alias to migrate
    // into quran_bookmarks. Can be removed once all users are on schema version >= 1.
    #[serde(
        default,
        alias = "quran_bookmark_surah",
        alias = "quran_bookmark_surah_num",
        skip_serializing
    )]
    pub quran_bookmark_surah_number: Option<u32>,
    #[serde(default)]
    pub quran_bookmark_page: Option<u32>,
    #[serde(default)]
    pub quran_bookmarks: Vec<QuranBookmark>,
    #[serde(default, alias = "quran_last_surah", alias = "quran_last_surah_num")]
    pub quran_last_surah_number: Option<u32>,
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
    pub timezone_mode: TimezoneMode,
    #[serde(default = "default_quran_arabic_font_px")]
    pub quran_arabic_font_px: f64,
    #[serde(default = "default_quran_translation_font_px")]
    pub quran_translation_font_px: f64,
    #[serde(default = "default_quran_line_height")]
    pub quran_line_height: f64,
    #[serde(default)]
    pub ui_font_family: Option<String>,
    #[serde(default)]
    pub arabic_font_family: Option<String>,
    #[serde(default)]
    pub quran_font_family: Option<String>,
    #[serde(default = "default_iqamah_minutes")]
    pub iqamah_minutes: HashMap<String, u32>,
    #[serde(default = "default_reciter_slug")]
    pub reciter_slug: String,
    #[serde(default)]
    pub stop_condition: StopCondition,
    #[serde(default)]
    pub installed_reciters: Vec<String>,
    #[serde(default)]
    pub high_latitude_rule: HighLatitudeChoice,
    #[serde(default)]
    pub polar_estimation_method: PolarEstimationMethod,
    #[serde(default)]
    pub fallback_was_active: bool,
    #[serde(default)]
    pub lre_was_blocked: bool,
    #[serde(default)]
    pub time_format: TimeFormat,
}

impl Default for AppConfigData {
    fn default() -> Self {
        Self {
            schema_version: CONFIG_SCHEMA_VERSION,
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
            adkar_notification_enabled: default_adkar_notification_enabled(),
            iqamah_notify: default_iqamah_notify(),
            adhan_only_mode: false,
            language: default_language(),
            theme: ThemeMode::System,
            is_configured: false,
            adhan_volume: 1.0,
            adhan_muted: false,
            autostart: default_autostart(),
            quran_bookmark_surah_number: None,
            quran_bookmark_page: None,
            quran_bookmarks: Vec::new(),
            quran_last_surah_number: None,
            quran_last_page: None,
            prayer_times_source: PrayerTimesSource::Calculated,
            mawaqit_url: None,
            mawaqit_auto_refresh_daily: false,
            mawaqit_cache: None,
            timezone_mode: TimezoneMode::Auto,
            quran_arabic_font_px: default_quran_arabic_font_px(),
            quran_translation_font_px: default_quran_translation_font_px(),
            quran_line_height: default_quran_line_height(),
            ui_font_family: None,
            arabic_font_family: None,
            quran_font_family: None,
            iqamah_minutes: default_iqamah_minutes(),
            reciter_slug: default_reciter_slug(),
            stop_condition: StopCondition::default(),
            installed_reciters: Vec::new(),
            high_latitude_rule: HighLatitudeChoice::default(),
            polar_estimation_method: PolarEstimationMethod::default(),
            fallback_was_active: false,
            lre_was_blocked: false,
            time_format: TimeFormat::Auto,
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

fn default_reciter_slug() -> String {
    "Minshawy_Murattal_128kbps".to_string()
}

/// Order is the settings display order.
pub const DEFAULT_IQAMAH_MINUTES: [(&str, u32); 5] = [
    ("Fajr", 20),
    ("Dhuhr", 10),
    ("Asr", 10),
    ("Maghrib", 5),
    ("Isha", 10),
];

fn default_iqamah_minutes() -> HashMap<String, u32> {
    DEFAULT_IQAMAH_MINUTES
        .iter()
        .map(|(name, mins)| (name.to_string(), *mins))
        .collect()
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

/// Runs pending schema migrations, reporting whether the file must be rewritten.
fn migrate(data: &mut AppConfigData) -> bool {
    if data.schema_version >= CONFIG_SCHEMA_VERSION {
        return false;
    }

    // v0 → v1: Arabic-text favorites become ids; the single bookmark joins the list.
    // Can be removed once all users are on schema version >= 1.
    crate::adkar::migrate_favorites(data);
    if let (Some(surah_number), Some(page)) =
        (data.quran_bookmark_surah_number, data.quran_bookmark_page)
    {
        data.quran_bookmarks.push(QuranBookmark {
            page,
            surah_number,
            verse: 1,
        });
    }
    data.quran_bookmarks.sort_by_key(|bookmark| bookmark.page);
    data.quran_bookmarks.dedup_by_key(|bookmark| bookmark.page);
    data.quran_bookmark_surah_number = None;
    data.quran_bookmark_page = None;

    data.schema_version = CONFIG_SCHEMA_VERSION;
    true
}

thread_local! {
    static CONFIG_INSTANCE: std::cell::RefCell<Option<AppConfig>> = const { std::cell::RefCell::new(None) };
}

impl AppConfig {
    pub fn language(&self) -> String {
        self.imp().data.borrow().language.clone()
    }
    pub fn set_language(&self, language: &str) {
        self.imp().data.borrow_mut().language = language.to_string();
        self.notify("language");
    }

    pub fn theme(&self) -> ThemeMode {
        self.imp().data.borrow().theme.clone()
    }
    pub fn set_theme(&self, theme: ThemeMode) {
        self.imp().data.borrow_mut().theme = theme;
    }

    pub fn latitude(&self) -> f64 {
        self.imp().data.borrow().latitude
    }
    pub fn set_latitude(&self, latitude: f64) {
        if (self.latitude() - latitude).abs() > 1e-10 {
            self.imp().data.borrow_mut().latitude = latitude;
            self.notify("latitude");
        }
    }

    pub fn longitude(&self) -> f64 {
        self.imp().data.borrow().longitude
    }
    pub fn set_longitude(&self, longitude: f64) {
        if (self.longitude() - longitude).abs() > 1e-10 {
            self.imp().data.borrow_mut().longitude = longitude;
            self.notify("longitude");
        }
    }

    pub fn city_name(&self) -> Option<String> {
        self.imp().data.borrow().city_name.clone()
    }
    pub fn set_city_name(&self, city_name: Option<String>) {
        self.imp().data.borrow_mut().city_name = city_name;
        self.notify("city-name");
    }

    pub fn method(&self) -> CalculationMethod {
        self.imp().data.borrow().method.clone()
    }
    pub fn set_method(&self, method: CalculationMethod) {
        self.imp().data.borrow_mut().method = method;
        self.notify("method");
    }

    pub fn madhab(&self) -> MadhabChoice {
        self.imp().data.borrow().madhab.clone()
    }
    pub fn set_madhab(&self, madhab: MadhabChoice) {
        self.imp().data.borrow_mut().madhab = madhab;
        self.notify("madhab");
    }

    pub fn location_mode(&self) -> LocationMode {
        self.imp().data.borrow().location_mode.clone()
    }
    pub fn set_location_mode(&self, location_mode: LocationMode) {
        if self.location_mode() != location_mode {
            self.imp().data.borrow_mut().location_mode = location_mode;
            self.notify("location-mode");
        }
    }

    pub fn adhan_sound_path(&self) -> Option<String> {
        self.imp().data.borrow().adhan_sound_path.clone()
    }
    pub fn set_adhan_sound_path(&self, adhan_sound_path: Option<String>) {
        self.imp().data.borrow_mut().adhan_sound_path = adhan_sound_path;
    }

    pub fn pre_prayer_notify(&self) -> bool {
        self.imp().data.borrow().pre_prayer_notify
    }
    pub fn set_pre_prayer_notify(&self, pre_prayer_notify: bool) {
        self.imp().data.borrow_mut().pre_prayer_notify = pre_prayer_notify;
    }

    pub fn pre_prayer_minutes(&self) -> u32 {
        self.imp().data.borrow().pre_prayer_minutes
    }
    pub fn set_pre_prayer_minutes(&self, pre_prayer_minutes: u32) {
        self.imp().data.borrow_mut().pre_prayer_minutes = pre_prayer_minutes;
    }

    pub fn hijri_offset(&self) -> i64 {
        self.imp().data.borrow().hijri_offset
    }
    pub fn set_hijri_offset(&self, hijri_offset: i64) {
        self.imp().data.borrow_mut().hijri_offset = hijri_offset;
    }

    pub fn favorites(&self) -> Vec<String> {
        self.imp().data.borrow().favorites.clone()
    }
    pub fn set_favorites(&self, favorites: Vec<String>) {
        self.imp().data.borrow_mut().favorites = favorites;
    }

    pub fn adkar_notification_enabled(&self) -> bool {
        self.imp().data.borrow().adkar_notification_enabled
    }
    pub fn set_adkar_notification_enabled(&self, adkar_notification_enabled: bool) {
        self.imp().data.borrow_mut().adkar_notification_enabled = adkar_notification_enabled;
    }

    pub fn iqamah_notify(&self) -> bool {
        self.imp().data.borrow().iqamah_notify
    }
    pub fn set_iqamah_notify(&self, iqamah_notify: bool) {
        self.imp().data.borrow_mut().iqamah_notify = iqamah_notify;
    }

    pub fn adhan_only_mode(&self) -> bool {
        self.imp().data.borrow().adhan_only_mode
    }
    pub fn set_adhan_only_mode(&self, adhan_only_mode: bool) {
        self.imp().data.borrow_mut().adhan_only_mode = adhan_only_mode;
    }

    pub fn is_configured(&self) -> bool {
        self.imp().data.borrow().is_configured
    }
    pub fn set_is_configured(&self, is_configured: bool) {
        self.imp().data.borrow_mut().is_configured = is_configured;
    }

    pub fn adhan_volume(&self) -> f32 {
        self.imp().data.borrow().adhan_volume
    }
    pub fn set_adhan_volume(&self, adhan_volume: f32) {
        self.imp().data.borrow_mut().adhan_volume = adhan_volume;
    }

    pub fn adhan_muted(&self) -> bool {
        self.imp().data.borrow().adhan_muted
    }
    pub fn set_adhan_muted(&self, adhan_muted: bool) {
        self.imp().data.borrow_mut().adhan_muted = adhan_muted;
    }

    pub fn autostart(&self) -> bool {
        self.imp().data.borrow().autostart
    }
    pub fn set_autostart(&self, autostart: bool) {
        self.imp().data.borrow_mut().autostart = autostart;
    }

    pub fn quran_bookmarks(&self) -> Vec<QuranBookmark> {
        self.imp().data.borrow().quran_bookmarks.clone()
    }
    pub fn set_quran_bookmarks(&self, quran_bookmarks: Vec<QuranBookmark>) {
        self.imp().data.borrow_mut().quran_bookmarks = quran_bookmarks;
    }

    pub fn quran_last_surah_number(&self) -> Option<u32> {
        self.imp().data.borrow().quran_last_surah_number
    }
    pub fn set_quran_last_surah_number(&self, quran_last_surah_number: Option<u32>) {
        self.imp().data.borrow_mut().quran_last_surah_number = quran_last_surah_number;
    }

    pub fn quran_last_page(&self) -> Option<u32> {
        self.imp().data.borrow().quran_last_page
    }
    pub fn set_quran_last_page(&self, quran_last_page: Option<u32>) {
        self.imp().data.borrow_mut().quran_last_page = quran_last_page;
    }

    pub fn prayer_times_source(&self) -> PrayerTimesSource {
        self.imp().data.borrow().prayer_times_source.clone()
    }
    pub fn set_prayer_times_source(&self, prayer_times_source: PrayerTimesSource) {
        self.imp().data.borrow_mut().prayer_times_source = prayer_times_source;
        self.notify("prayer-times-source");
    }

    pub fn mawaqit_url(&self) -> Option<String> {
        self.imp().data.borrow().mawaqit_url.clone()
    }
    pub fn set_mawaqit_url(&self, mawaqit_url: Option<String>) {
        self.imp().data.borrow_mut().mawaqit_url = mawaqit_url;
    }

    pub fn mawaqit_auto_refresh_daily(&self) -> bool {
        self.imp().data.borrow().mawaqit_auto_refresh_daily
    }
    pub fn set_mawaqit_auto_refresh_daily(&self, mawaqit_auto_refresh_daily: bool) {
        self.imp().data.borrow_mut().mawaqit_auto_refresh_daily = mawaqit_auto_refresh_daily;
    }

    pub fn mawaqit_cache(&self) -> Option<MawaqitCache> {
        self.imp().data.borrow().mawaqit_cache.clone()
    }
    pub fn set_mawaqit_cache(&self, mawaqit_cache: Option<MawaqitCache>) {
        self.imp().data.borrow_mut().mawaqit_cache = mawaqit_cache;
    }

    pub fn timezone_mode(&self) -> TimezoneMode {
        self.imp().data.borrow().timezone_mode.clone()
    }
    pub fn set_timezone_mode(&self, timezone_mode: TimezoneMode) {
        self.imp().data.borrow_mut().timezone_mode = timezone_mode;
        self.notify("timezone-mode");
    }

    pub fn quran_arabic_font_px(&self) -> f64 {
        self.imp().data.borrow().quran_arabic_font_px
    }
    pub fn set_quran_arabic_font_px(&self, quran_arabic_font_px: f64) {
        self.imp().data.borrow_mut().quran_arabic_font_px = quran_arabic_font_px;
    }

    pub fn quran_translation_font_px(&self) -> f64 {
        self.imp().data.borrow().quran_translation_font_px
    }
    pub fn set_quran_translation_font_px(&self, quran_translation_font_px: f64) {
        self.imp().data.borrow_mut().quran_translation_font_px = quran_translation_font_px;
    }

    pub fn quran_line_height(&self) -> f64 {
        self.imp().data.borrow().quran_line_height
    }
    pub fn set_quran_line_height(&self, quran_line_height: f64) {
        self.imp().data.borrow_mut().quran_line_height = quran_line_height;
    }

    pub fn ui_font_family(&self) -> Option<String> {
        self.imp().data.borrow().ui_font_family.clone()
    }
    pub fn set_ui_font_family(&self, ui_font_family: Option<String>) {
        self.imp().data.borrow_mut().ui_font_family = ui_font_family;
    }

    pub fn arabic_font_family(&self) -> Option<String> {
        self.imp().data.borrow().arabic_font_family.clone()
    }
    pub fn set_arabic_font_family(&self, arabic_font_family: Option<String>) {
        self.imp().data.borrow_mut().arabic_font_family = arabic_font_family;
    }

    pub fn quran_font_family(&self) -> Option<String> {
        self.imp().data.borrow().quran_font_family.clone()
    }
    pub fn set_quran_font_family(&self, quran_font_family: Option<String>) {
        self.imp().data.borrow_mut().quran_font_family = quran_font_family;
    }

    pub fn iqamah_minutes(&self) -> HashMap<String, u32> {
        self.imp().data.borrow().iqamah_minutes.clone()
    }
    pub fn set_iqamah_minutes(&self, iqamah_minutes: HashMap<String, u32>) {
        self.imp().data.borrow_mut().iqamah_minutes = iqamah_minutes;
    }

    pub fn reciter_slug(&self) -> String {
        self.imp().data.borrow().reciter_slug.clone()
    }
    pub fn set_reciter_slug(&self, reciter_slug: &str) {
        self.imp().data.borrow_mut().reciter_slug = reciter_slug.to_string();
    }

    pub fn stop_condition(&self) -> StopCondition {
        self.imp().data.borrow().stop_condition
    }
    pub fn set_stop_condition(&self, stop_condition: StopCondition) {
        self.imp().data.borrow_mut().stop_condition = stop_condition;
    }

    pub fn installed_reciters(&self) -> Vec<String> {
        self.imp().data.borrow().installed_reciters.clone()
    }
    pub fn set_installed_reciters(&self, installed_reciters: Vec<String>) {
        self.imp().data.borrow_mut().installed_reciters = installed_reciters;
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
            .retain(|reciter| reciter != slug);
    }

    pub fn high_latitude_rule(&self) -> HighLatitudeChoice {
        self.imp().data.borrow().high_latitude_rule.clone()
    }
    pub fn set_high_latitude_rule(&self, high_latitude_rule: HighLatitudeChoice) {
        self.imp().data.borrow_mut().high_latitude_rule = high_latitude_rule;
    }

    pub fn polar_estimation_method(&self) -> PolarEstimationMethod {
        self.imp().data.borrow().polar_estimation_method.clone()
    }
    pub fn set_polar_estimation_method(&self, polar_estimation_method: PolarEstimationMethod) {
        self.imp().data.borrow_mut().polar_estimation_method = polar_estimation_method;
    }

    pub fn fallback_was_active(&self) -> bool {
        self.imp().data.borrow().fallback_was_active
    }
    pub fn set_fallback_was_active(&self, fallback_was_active: bool) {
        self.imp().data.borrow_mut().fallback_was_active = fallback_was_active;
    }

    pub fn lre_was_blocked(&self) -> bool {
        self.imp().data.borrow().lre_was_blocked
    }
    pub fn set_lre_was_blocked(&self, lre_was_blocked: bool) {
        self.imp().data.borrow_mut().lre_was_blocked = lre_was_blocked;
    }

    pub fn latitude_zone(&self) -> u8 {
        let latitude_abs = self.latitude().abs();
        if latitude_abs > 66.5 {
            3
        } else if latitude_abs > 48.6 {
            2
        } else {
            1
        }
    }

    fn load_data() -> AppConfigData {
        let path = Self::config_path();
        if let Ok(content) = fs::read_to_string(&path)
            && let Ok(mut config) = serde_json::from_str::<AppConfigData>(&content)
        {
            if migrate(&mut config) {
                Self::write_to_disk(&config);
                log::info!(
                    "Configuration migrated to schema version {}",
                    CONFIG_SCHEMA_VERSION
                );
            }
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
                    let config_data = Self::load_data();
                    let config: Self = glib::Object::new();
                    let imp = config.imp();
                    *imp.data.borrow_mut() = config_data;
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

    pub fn config_path() -> PathBuf {
        let mut path = glib::user_config_dir();
        path.push("khushu");
        path.push("config.json");
        path
    }

    pub fn time_format(&self) -> TimeFormat {
        self.imp().data.borrow().time_format
    }

    pub fn set_time_format(&self, time_format: TimeFormat) {
        self.imp().data.borrow_mut().time_format = time_format;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_font_families() {
        let config = AppConfigData::default();
        assert_eq!(config.ui_font_family, None);
        assert_eq!(config.arabic_font_family, None);
        assert_eq!(config.quran_font_family, None);
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

    #[test]
    fn legacy_singular_bookmark_migrates_to_list_and_bumps_version() {
        let json = serde_json::json!({
            "latitude": 36.75,
            "longitude": 3.05,
            "method": "MWL",
            "madhab": "Shafi",
            "location_mode": "Manual",
            "pre_prayer_notify": true,
            "pre_prayer_minutes": 15,
            "hijri_offset": 0,
            "quran_bookmark_surah": 2,
            "quran_bookmark_page": 8,
            "quran_bookmarks": [{"page": 8, "surah": 2, "verse": 1}]
        });
        let mut config: AppConfigData = serde_json::from_value(json).unwrap();
        assert_eq!(
            config.schema_version, 0,
            "missing key must default to legacy"
        );
        assert_eq!(
            config.quran_bookmarks.len(),
            1,
            "dedup must not duplicate the page"
        );

        assert!(migrate(&mut config));
        assert_eq!(config.schema_version, CONFIG_SCHEMA_VERSION);
        assert_eq!(
            config.quran_bookmarks,
            vec![QuranBookmark {
                page: 8,
                surah_number: 2,
                verse: 1
            }]
        );
        assert_eq!(config.quran_bookmark_surah_number, None);
        assert_eq!(config.quran_bookmark_page, None);
    }

    #[test]
    fn migrated_config_is_not_rewritten() {
        let mut config = AppConfigData {
            schema_version: CONFIG_SCHEMA_VERSION,
            ..AppConfigData::default()
        };
        config.quran_bookmarks.push(QuranBookmark {
            page: 42,
            surah_number: 29,
            verse: 1,
        });

        assert!(!migrate(&mut config), "current schema must be a no-op");
        assert_eq!(config.quran_bookmarks.len(), 1);
    }

    #[test]
    fn test_default_time_format() {
        let config = AppConfigData::default();
        assert_eq!(config.time_format, TimeFormat::Auto);
    }

    #[test]
    fn test_time_format_roundtrip() {
        let mut data = AppConfigData::default();
        assert_eq!(data.time_format, TimeFormat::Auto);

        data.time_format = TimeFormat::Hours12;
        assert_eq!(data.time_format, TimeFormat::Hours12);

        data.time_format = TimeFormat::Hours24;
        assert_eq!(data.time_format, TimeFormat::Hours24);

        data.time_format = TimeFormat::Auto;
        assert_eq!(data.time_format, TimeFormat::Auto);
    }

    #[test]
    fn test_time_format_serde() {
        let auto: TimeFormat = serde_json::from_str(r#""auto""#).unwrap();
        assert_eq!(auto, TimeFormat::Auto);

        let h24: TimeFormat = serde_json::from_str(r#""24h""#).unwrap();
        assert_eq!(h24, TimeFormat::Hours24);

        let h12: TimeFormat = serde_json::from_str(r#""12h""#).unwrap();
        assert_eq!(h12, TimeFormat::Hours12);

        assert_eq!(
            serde_json::to_string(&TimeFormat::Auto).unwrap(),
            r#""auto""#
        );
        assert_eq!(
            serde_json::to_string(&TimeFormat::Hours24).unwrap(),
            r#""24h""#
        );
        assert_eq!(
            serde_json::to_string(&TimeFormat::Hours12).unwrap(),
            r#""12h""#
        );
    }
}
