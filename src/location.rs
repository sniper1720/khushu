use crate::config::{AppConfig, LocationMode, PrayerTimesSource, TimeFormat};
use crate::i18n::{icu_locale_key, tr};
use chrono::{Datelike, Timelike};
use icu::calendar::{Date, Gregorian};
use icu::datetime::options::TimePrecision;
use icu::datetime::preferences::HourCycle;
use icu::datetime::{
    DateTimeFormatterPreferences, FixedCalendarDateTimeFormatter, NoCalendarFormatter,
    fieldsets::{
        ET, T,
        zone::{ExemplarCity, LocalizedOffsetLong, Location as TimeZoneLocation},
    },
};
use icu::time::Time;
use icu::time::zone::{TimeZone, UtcOffset};
use icu_experimental::displaynames::DisplayNamesOptions;
use icu_experimental::displaynames::multi::RegionDisplayNames;
use icu_locale::Locale;
use icu_locale::subtags::Region;
use reqwest::Client;
use serde::Deserialize;
use std::sync::OnceLock;

static HTTP_CLIENT: OnceLock<Client> = OnceLock::new();
static TIME_ZONE_LOOKUP: OnceLock<std::collections::HashMap<String, String>> = OnceLock::new();

fn client() -> &'static Client {
    HTTP_CLIENT.get_or_init(|| {
        Client::builder()
            .user_agent(crate::USER_AGENT)
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("Failed to create HTTP client")
    })
}

#[derive(Deserialize, Debug)]
pub struct GeocodeAddress {
    #[serde(rename = "country_code")]
    pub country_code: Option<String>,
    pub city: Option<String>,
    pub town: Option<String>,
    pub village: Option<String>,
    pub suburb: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct GeocodeResult {
    #[serde(rename = "lat")]
    pub latitude: String,
    #[serde(rename = "lon")]
    pub longitude: String,
    pub display_name: String,
    pub address: Option<GeocodeAddress>,
    #[serde(default)]
    pub timezone: Option<String>,
}

fn non_empty_text(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

pub fn validated_time_zone_id(timezone: &str) -> Option<String> {
    let trimmed = non_empty_text(timezone)?;
    if let Ok(timezone) = trimmed.parse::<chrono_tz::Tz>() {
        return Some(timezone.to_string());
    }

    TIME_ZONE_LOOKUP
        .get_or_init(|| {
            chrono_tz::TZ_VARIANTS
                .iter()
                .map(|timezone_variant| {
                    let canonical = timezone_variant.to_string();
                    (canonical.to_ascii_lowercase(), canonical)
                })
                .collect()
        })
        .get(&trimmed.to_ascii_lowercase())
        .cloned()
}

pub fn system_time_zone_id() -> Option<String> {
    std::env::var("TZ")
        .ok()
        .and_then(|timezone_value| validated_time_zone_id(&timezone_value))
        .or_else(|| {
            std::fs::read_to_string("/etc/timezone")
                .ok()
                .and_then(|timezone_value| validated_time_zone_id(&timezone_value))
        })
        .or_else(|| {
            std::fs::read_link("/etc/localtime")
                .ok()
                .and_then(|path| path.to_str().map(str::to_string))
                .and_then(|path| {
                    path.split_once("/zoneinfo/")
                        .map(|(_, timezone)| timezone.to_string())
                })
                .and_then(|timezone_value| validated_time_zone_id(&timezone_value))
        })
}

pub fn short_city_with_country(display_name: &str) -> String {
    let parts: Vec<&str> = display_name
        .split(',')
        .map(|part| part.trim())
        .filter(|part: &&str| !part.is_empty())
        .collect();
    if parts.len() >= 2 {
        format!("{}, {}", parts[0], parts[parts.len() - 1])
    } else if let Some(first) = parts.first() {
        first.to_string()
    } else {
        display_name.to_string()
    }
}

pub fn country_name_from_code(code: &str, language: &str) -> Option<String> {
    use std::cell::RefCell;
    use std::collections::HashMap;

    thread_local! {
        static CACHE: RefCell<HashMap<String, RegionDisplayNames>> = RefCell::new(HashMap::new());
    }

    let locale_str = icu_locale_key(language);

    let actual_code = if code.eq_ignore_ascii_case("IL") {
        "PS"
    } else {
        code
    };
    let region_code: Region = actual_code.parse().ok()?;

    CACHE.with(|cache| {
        let mut map = cache.borrow_mut();
        if !map.contains_key(&locale_str) {
            let locale: Locale = locale_str
                .parse()
                .unwrap_or_else(|_| "en".parse().expect("en is a valid locale string"));
            if let Ok(rdn) =
                RegionDisplayNames::try_new(locale.into(), DisplayNamesOptions::default())
            {
                map.insert(locale_str.clone(), rdn);
            } else {
                return None;
            }
        }
        map.get(&locale_str)?
            .of(region_code)
            .map(|city_name: &str| city_name.to_string())
    })
}

pub fn city_name_from_time_zone(timezone: &str, language: &str) -> Option<String> {
    use std::cell::RefCell;
    use std::collections::HashMap;

    thread_local! {
        static CACHE: RefCell<HashMap<String, NoCalendarFormatter<ExemplarCity>>> =
            RefCell::new(HashMap::new());
    }

    let locale_str = icu_locale_key(language);
    let time_zone = TimeZone::from_iana_id(timezone.trim());
    if time_zone == TimeZone::UNKNOWN {
        return None;
    }
    let time_zone_info = time_zone.with_offset(None);

    CACHE.with(|cache| {
        let mut map = cache.borrow_mut();
        if !map.contains_key(&locale_str) {
            let locale: Locale = locale_str.parse().ok()?;
            let formatter = NoCalendarFormatter::try_new(locale.into(), ExemplarCity).ok()?;
            map.insert(locale_str.clone(), formatter);
        }
        non_empty_text(&map.get(&locale_str)?.format(&time_zone_info).to_string())
    })
}

pub fn time_zone_location_name(timezone: &str, language: &str) -> Option<String> {
    use std::cell::RefCell;
    use std::collections::HashMap;

    thread_local! {
        static CACHE: RefCell<HashMap<String, NoCalendarFormatter<TimeZoneLocation>>> =
            RefCell::new(HashMap::new());
    }

    let locale_str = icu_locale_key(language);
    let time_zone = TimeZone::from_iana_id(timezone.trim());
    if time_zone == TimeZone::UNKNOWN {
        return None;
    }
    let time_zone_info = time_zone.without_offset();

    CACHE.with(|cache| {
        let mut map = cache.borrow_mut();
        if !map.contains_key(&locale_str) {
            let locale: Locale = locale_str.parse().ok()?;
            let formatter = NoCalendarFormatter::try_new(locale.into(), TimeZoneLocation).ok()?;
            map.insert(locale_str.clone(), formatter);
        }
        non_empty_text(&map.get(&locale_str)?.format(&time_zone_info).to_string())
    })
}

pub fn localized_time_zone_label(timezone: &str, language: &str) -> String {
    time_zone_location_name(timezone, language)
        .or_else(|| city_name_from_time_zone(timezone, language))
        .or_else(|| non_empty_text(timezone))
        .unwrap_or_else(|| timezone.to_string())
}

fn localize_iana_root(root: &str, language: &str) -> Option<String> {
    // IANA roots without UN M.49 region codes (expose to xgettext)
    if false {
        tr("Atlantic Ocean");
        tr("Indian Ocean");
        tr("Pacific Ocean");
        tr("Arctic");
    }

    let m49 = match root {
        "Africa" => "002",
        "America" => "019",
        "Antarctica" => "010",
        "Asia" => "142",
        "Australia" => "036",
        "Europe" => "150",
        "Atlantic" => return Some(tr("Atlantic Ocean")),
        "Indian" => return Some(tr("Indian Ocean")),
        "Pacific" => return Some(tr("Pacific Ocean")),
        "Arctic" => return Some(tr("Arctic")),
        _ => return None,
    };
    country_name_from_code(m49, language)
}

pub fn localized_zone(zone: &str, language: &str) -> String {
    let mut parts = zone.splitn(2, '/');
    let root = parts.next().unwrap_or(zone);
    let Some(city) = parts.next() else {
        return zone.to_string();
    };

    let localized_root = localize_iana_root(root, language).unwrap_or_else(|| root.to_string());
    let localized_city = if root == "Etc" {
        city.to_string()
    } else {
        city_name_from_time_zone(zone, language).unwrap_or_else(|| city.to_string())
    };
    format!("{}/{}", localized_root, localized_city)
}

fn fallback_offset(offset_secs: i32) -> String {
    let offset_sign = if offset_secs >= 0 { '+' } else { '-' };
    let offset_abs = offset_secs.unsigned_abs();
    let hours = offset_abs / 3600;
    let mins = (offset_abs % 3600) / 60;
    if mins == 0 {
        format!("UTC{}{:02}", offset_sign, hours)
    } else {
        format!("UTC{}{:02}:{:02}", offset_sign, hours, mins)
    }
}

pub fn localized_offset(offset_secs: i32, language: &str) -> String {
    use std::cell::RefCell;
    use std::collections::HashMap;

    thread_local! {
        static CACHE: RefCell<HashMap<String, NoCalendarFormatter<LocalizedOffsetLong>>> =
            RefCell::new(HashMap::new());
    }

    let locale_str = icu_locale_key(language);
    let offset = match UtcOffset::try_from_seconds(offset_secs) {
        Ok(offset) => offset,
        Err(_) => return fallback_offset(offset_secs),
    };

    CACHE.with(|cache| {
        let mut map = cache.borrow_mut();
        if !map.contains_key(&locale_str) {
            let locale: Locale = locale_str
                .parse()
                .unwrap_or_else(|_| "en".parse().expect("en is valid"));
            match NoCalendarFormatter::try_new(locale.into(), LocalizedOffsetLong) {
                Ok(fmt) => {
                    map.insert(locale_str.clone(), fmt);
                }
                Err(_) => {
                    return fallback_offset(offset_secs);
                }
            }
        }
        map.get(&locale_str)
            .map(|fmt| fmt.format(&offset).to_string())
            .unwrap_or_else(|| fallback_offset(offset_secs))
    })
}

fn fallback_time(datetime: chrono::DateTime<chrono_tz::Tz>) -> String {
    datetime.format("%a %H:%M").to_string()
}

fn fallback_time_only(
    datetime: chrono::DateTime<chrono::Local>,
    time_format: TimeFormat,
) -> String {
    match time_format {
        TimeFormat::Auto | TimeFormat::Hours24 => datetime.format("%H:%M").to_string(),
        TimeFormat::Hours12 => datetime.format("%I:%M %p").to_string(),
    }
}

/// Formats a Mawaqit-published "HH:MM" wall-clock string for display.
///
/// The mosque schedule is its local wall-clock, so the published string is
/// shown directly, converted only for the 12/24-hour preference. This keeps
/// the label identical to what the mosque publishes regardless of the system
/// timezone.
pub fn format_published_time(published: &str, time_format: TimeFormat) -> String {
    let (hours, minutes) = match published.split_once(':').and_then(|(hours, minutes)| {
        let hours = hours.trim().parse::<u32>().ok()?;
        let minutes = minutes.trim().parse::<u32>().ok()?;
        Some((hours, minutes))
    }) {
        Some(parsed) => parsed,
        None => return published.to_string(),
    };

    match time_format {
        TimeFormat::Auto | TimeFormat::Hours24 => format!("{hours:02}:{minutes:02}"),
        TimeFormat::Hours12 => {
            let (display_hour, period) = match hours {
                0 => (12, "AM"),
                12 => (12, "PM"),
                hour if hour > 12 => (hour - 12, "PM"),
                hour => (hour, "AM"),
            };
            format!("{display_hour:02}:{minutes:02} {period}")
        }
    }
}

pub fn localized_time_only(
    datetime: chrono::DateTime<chrono::Local>,
    language: &str,
    time_format: TimeFormat,
) -> String {
    use std::cell::RefCell;
    use std::collections::HashMap;

    thread_local! {
        static CACHE: RefCell<HashMap<String, NoCalendarFormatter<T>>> =
            RefCell::new(HashMap::new());
    }

    let time = match Time::try_new(
        datetime.hour() as u8,
        datetime.minute() as u8,
        datetime.second() as u8,
        0,
    ) {
        Ok(t) => t,
        Err(_) => return fallback_time_only(datetime, time_format),
    };

    let locale_str = icu_locale_key(language);
    let cache_key = format!("{}:{:?}", locale_str, time_format);

    CACHE.with(|cache| {
        let mut map = cache.borrow_mut();
        if !map.contains_key(&cache_key) {
            let locale: Locale = locale_str
                .parse()
                .unwrap_or_else(|_| "en".parse().expect("en is valid"));

            let mut prefs = DateTimeFormatterPreferences::from(locale);
            match time_format {
                TimeFormat::Auto => {} // Use locale default hour cycle
                TimeFormat::Hours24 => prefs.hour_cycle = Some(HourCycle::Clock24),
                TimeFormat::Hours12 => prefs.hour_cycle = Some(HourCycle::Clock12),
            }

            match NoCalendarFormatter::try_new(prefs, T::hm()) {
                Ok(fmt) => {
                    map.insert(cache_key.clone(), fmt);
                }
                Err(_) => {
                    return fallback_time_only(datetime, time_format);
                }
            }
        }
        map.get(&cache_key)
            .map(|fmt| fmt.format(&time).to_string())
            .unwrap_or_else(|| fallback_time_only(datetime, time_format))
    })
}

pub fn localized_time(datetime: chrono::DateTime<chrono_tz::Tz>, language: &str) -> String {
    use std::cell::RefCell;
    use std::collections::HashMap;

    thread_local! {
        static CACHE: RefCell<HashMap<String, FixedCalendarDateTimeFormatter<Gregorian, ET>>> =
            RefCell::new(HashMap::new());
    }

    let locale_str = icu_locale_key(language);
    let date = match Date::try_new_gregorian(
        datetime.year(),
        datetime.month() as u8,
        datetime.day() as u8,
    ) {
        Ok(gregorian_date) => gregorian_date,
        Err(_) => return fallback_time(datetime),
    };
    let time = match Time::try_new(
        datetime.hour() as u8,
        datetime.minute() as u8,
        datetime.second() as u8,
        0,
    ) {
        Ok(time_of_day) => time_of_day,
        Err(_) => return fallback_time(datetime),
    };
    let input = icu::time::DateTime { date, time };

    CACHE.with(|cache| {
        let mut map = cache.borrow_mut();
        if !map.contains_key(&locale_str) {
            let locale: Locale = locale_str
                .parse()
                .unwrap_or_else(|_| "en".parse().expect("en is valid"));
            match FixedCalendarDateTimeFormatter::<Gregorian, _>::try_new(
                locale.into(),
                ET::short().with_time_precision(TimePrecision::Minute),
            ) {
                Ok(fmt) => {
                    map.insert(locale_str.clone(), fmt);
                }
                Err(_) => {
                    return fallback_time(datetime);
                }
            }
        }
        map.get(&locale_str)
            .map(|fmt| fmt.format(&input).to_string())
            .unwrap_or_else(|| fallback_time(datetime))
    })
}

pub fn display_city_label(city_name: Option<&str>, language: &str) -> Option<String> {
    city_name
        .and_then(non_empty_text)
        .map(|city| append_country(short_city_with_country(&city), None, language))
}

fn append_country(city: String, country_code: Option<&str>, language: &str) -> String {
    let mut text = city;
    if let Some(code) = country_code
        && !text.contains(',')
        && let Some(country) = country_name_from_code(code, language)
        && !country.is_empty()
    {
        text = format!("{}, {}", text, country);
    }
    text
}

/// Produces the single authoritative location line used across the app.
///
/// Connected Mosque (Mawaqit): the mosque name, else the reverse-geocoded
/// resolved city (plus country), else the coordinates. Manual mode and any
/// missing data fall back to the coordinates.
pub fn display_location_label(config: &AppConfig, language: &str) -> String {
    let coordinates = format_coordinates(config.latitude(), config.longitude());

    if config.prayer_times_source() == PrayerTimesSource::Mawaqit {
        if let Some(cache) = config.mawaqit_cache() {
            if let Some(name) = non_empty_text(cache.mosque_name.as_deref().unwrap_or("")) {
                return append_country(
                    short_city_with_country(&name),
                    cache.country_code.as_deref(),
                    language,
                );
            }
            if let Some(city) = non_empty_text(cache.resolved_city.as_deref().unwrap_or("")) {
                return append_country(
                    short_city_with_country(&city),
                    cache.country_code.as_deref(),
                    language,
                );
            }
        }
        return coordinates;
    }

    if config.location_mode() == LocationMode::Manual {
        return coordinates;
    }

    display_city_label(config.city_name().as_deref(), language).unwrap_or(coordinates)
}

use ashpd::desktop::location::{Accuracy, CreateSessionOptions, LocationProxy};
use futures_util::StreamExt;

pub async fn fetch_auto_location(language: &str) -> Result<(f64, f64, String), String> {
    fetch_portal_location(language).await
}

pub async fn resolve_city_name(
    latitude: f64,
    longitude: f64,
    language: &str,
) -> Result<String, String> {
    reverse_geocode(latitude, longitude, language)
        .await
        .map(|name| short_city_with_country(&name))
}

/// Resolves the locality for a Mawaqit cache at fetch time.
///
/// Populates `MawaqitCache.resolved_city` from the mosque coordinates via the
/// Nominatim resolver; a failure yields `None` and the display falls back to
/// the mosque name or coordinates rather than the stored city.
pub async fn resolve_mawaqit_city(latitude: f64, longitude: f64, language: &str) -> Option<String> {
    resolve_city_name(latitude, longitude, language).await.ok()
}

async fn fetch_portal_location(language: &str) -> Result<(f64, f64, String), String> {
    log::info!("Attempting to fetch location via ASHPD Portal...");

    let proxy = LocationProxy::new().await.map_err(|err| {
        log::error!("Failed to create Location proxy: {}", err);
        tr("Location service unavailable. Please check system settings.")
    })?;

    let session = proxy
        .create_session(CreateSessionOptions::default().set_accuracy(Accuracy::City))
        .await
        .map_err(|err| {
            log::error!("Failed to create location session: {}", err);
            tr("Location access denied or unavailable.")
        })?;

    let mut stream = proxy.receive_location_updated().await.map_err(|err| {
        log::error!("Failed to receive location updates: {}", err);
        tr("Failed to receive location updates.")
    })?;

    proxy
        .start(&session, None, Default::default())
        .await
        .map_err(|err| {
            log::error!("Failed to start location session: {}", err);
            tr("Location access denied or unavailable.")
        })?;

    use futures_util::future::Either;

    let timeout = gtk4::glib::timeout_future_seconds(10);
    let location = match futures_util::future::select(timeout, stream.next()).await {
        Either::Right((Some(location), _)) => location,
        Either::Right((None, _)) => {
            let _ = session.close().await;
            log::error!("Location stream ended unexpectedly");
            return Err(tr("Location service disconnected unexpectedly."));
        }
        Either::Left((_, _)) => {
            let _ = session.close().await;
            log::error!("Location request timed out (possible permission denial)");
            return Err(tr(
                "Location request timed out. Please check your system settings.",
            ));
        }
    };

    let latitude = location.latitude();
    let longitude = location.longitude();

    let _ = session.close().await;

    log::info!("Portal location fetched: {}, {}", latitude, longitude);

    let city = match reverse_geocode(latitude, longitude, language).await {
        Ok(name) => {
            log::info!("Reverse geocoded to: {}", name);
            short_city_with_country(&name)
        }
        Err(err) => {
            log::warn!("Reverse geocode failed, using coordinates: {}", err);
            format_coordinates(latitude, longitude)
        }
    };

    Ok((latitude, longitude, city))
}

async fn reverse_geocode(latitude: f64, longitude: f64, language: &str) -> Result<String, String> {
    let http = client();
    let normalized_lang = icu_locale_key(language);

    let url = format!(
        "https://nominatim.openstreetmap.org/reverse?lat={}&lon={}&format=json&zoom=10&accept-language={}&addressdetails=1",
        latitude, longitude, normalized_lang
    );

    let resp = http
        .get(url)
        .send()
        .await
        .map_err(|_| tr("Network error while resolving city."))?;

    let result: GeocodeResult = resp
        .json()
        .await
        .map_err(|_| tr("Invalid response from location service."))?;

    if result.display_name.is_empty() {
        return Err(tr("Could not find city name for these coordinates."));
    }

    if let Some(ref addr) = result.address
        && addr
            .country_code
            .as_deref()
            .is_some_and(|country| country.eq_ignore_ascii_case("il"))
    {
        let city = addr
            .city
            .as_deref()
            .or(addr.town.as_deref())
            .or(addr.village.as_deref())
            .or(addr.suburb.as_deref())
            .unwrap_or("City");

        if let Some(country) = country_name_from_code("PS", language) {
            return Ok(format!("{}, {}", city, country));
        }
    }

    Ok(result.display_name)
}

fn format_coordinates(latitude: f64, longitude: f64) -> String {
    let latitude_dir = if latitude >= 0.0 { "N" } else { "S" };
    let longitude_dir = if longitude >= 0.0 { "E" } else { "W" };
    format!(
        "{:.2}°{}, {:.2}°{}",
        latitude.abs(),
        latitude_dir,
        longitude.abs(),
        longitude_dir
    )
}

pub async fn search_city(
    query: &str,
    language: &str,
) -> Result<(f64, f64, String, Option<String>), String> {
    log::info!("Searching for city: {}", query);
    let http = client();
    let normalized_lang = icu_locale_key(language);

    let url = format!(
        "https://nominatim.openstreetmap.org/search?q={}&format=json&limit=1&accept-language={}&addressdetails=1",
        urlencoding::encode(query),
        normalized_lang
    );

    let resp = http.get(url).send().await.map_err(|err| {
        log::error!("Geocoding request failed: {}", err);
        tr("Network error. Please check your connection.")
    })?;

    let results: Vec<GeocodeResult> = resp.json().await.map_err(|err| {
        log::error!("Geocoding JSON parsing failed: {}", err);
        tr("Invalid response from location service.")
    })?;

    if let Some(first_result) = results.first() {
        let latitude = first_result.latitude.parse::<f64>().map_err(|_| {
            log::error!("Invalid latitude from API: {}", first_result.latitude);
            tr("Invalid response from location service.")
        })?;
        let longitude = first_result.longitude.parse::<f64>().map_err(|_| {
            log::error!("Invalid longitude from API: {}", first_result.longitude);
            tr("Invalid response from location service.")
        })?;

        let mut display_name = first_result.display_name.clone();

        if let Some(ref addr) = first_result.address
            && addr
                .country_code
                .as_deref()
                .is_some_and(|country| country.eq_ignore_ascii_case("il"))
        {
            let city = addr
                .city
                .as_deref()
                .or(addr.town.as_deref())
                .or(addr.village.as_deref())
                .or(addr.suburb.as_deref())
                .unwrap_or("City");

            if let Some(country) = country_name_from_code("PS", language) {
                display_name = format!("{}, {}", city, country);
            }
        }

        log::info!(
            "City found: {} ({}, {}) timezone: {:?}",
            display_name,
            latitude,
            longitude,
            first_result.timezone
        );
        Ok((
            latitude,
            longitude,
            display_name,
            first_result.timezone.clone(),
        ))
    } else {
        log::warn!("City not found for query: {}", query);
        Err(tr("City not found. Please check the spelling."))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::MawaqitCache;

    fn empty_cache() -> MawaqitCache {
        MawaqitCache {
            url: String::new(),
            mosque_name: None,
            timezone: None,
            latitude: None,
            longitude: None,
            country_code: None,
            resolved_city: None,
            year: 2026,
            months: Vec::new(),
            fetched_on: String::new(),
        }
    }

    #[test]
    fn localizes_city_from_icu_time_zone_data() {
        assert_eq!(
            city_name_from_time_zone("Europe/Vienna", "de").as_deref(),
            Some("Wien")
        );
    }

    #[test]
    fn localizes_time_zone_name_from_icu_data() {
        let label = time_zone_location_name("Africa/Algiers", "ar").expect("localized timezone");
        assert!(!label.trim().is_empty());
        assert_ne!(label, "Africa/Algiers");
    }

    #[test]
    fn localizes_zone_path_components() {
        assert!(localized_zone("Europe/Oslo", "en").starts_with("Europe/"));
        assert!(localized_zone("Europe/Oslo", "ar").starts_with("أوروبا/"));
    }

    #[test]
    fn ocean_roots_localized_while_etc_stays_raw() {
        let localized = localized_zone("Atlantic/Canary", "en");
        assert_ne!(localized, "Atlantic/Canary");
        assert!(localized.ends_with("Canaries"));
        assert_eq!(localized_zone("Etc/UTC", "en"), "Etc/UTC");
    }

    #[test]
    fn localizes_time_with_weekday_without_seconds() {
        use chrono::TimeZone;
        let timezone: chrono_tz::Tz = "Europe/Oslo".parse().unwrap();
        let naive = chrono::NaiveDate::from_ymd_opt(2026, 1, 30)
            .unwrap()
            .and_hms_opt(15, 47, 42)
            .unwrap();
        let datetime: chrono::DateTime<chrono_tz::Tz> =
            timezone.from_local_datetime(&naive).unwrap();

        let en = localized_time(datetime, "en");
        assert!(en.contains("Fri"), "weekday should be localized: {en}");
        assert!(en.contains("3:47"), "hour:minute should be shown: {en}");
        assert!(!en.contains("42"), "seconds must not be rendered: {en}");

        let ar = localized_time(datetime, "ar");
        assert!(!ar.trim().is_empty());
        assert!(!ar.contains("Fri"), "weekday must be localized, got: {ar}");
    }

    #[test]
    fn accepts_valid_named_time_zone_ids() {
        assert_eq!(
            validated_time_zone_id(" Africa/Algiers ").as_deref(),
            Some("Africa/Algiers")
        );
    }

    #[test]
    fn canonicalizes_case_insensitive_named_time_zone_ids() {
        assert_eq!(
            validated_time_zone_id("europe/paris").as_deref(),
            Some("Europe/Paris")
        );
    }

    #[test]
    fn rejects_invalid_named_time_zone_ids() {
        assert!(validated_time_zone_id("Europe/NotARealCity").is_none());
    }

    #[test]
    fn display_location_label_prefers_mosque_name_for_mawaqit() {
        let config = crate::config::AppConfig::default();
        config.set_prayer_times_source(crate::config::PrayerTimesSource::Mawaqit);
        let mut cache = empty_cache();
        cache.mosque_name = Some("Masjid Al-Noor - Vienna".to_string());
        cache.country_code = Some("AT".to_string());
        config.set_mawaqit_cache(Some(cache));

        let label = display_location_label(&config, "en");
        assert_eq!(
            label, "Masjid Al-Noor - Vienna, Austria",
            "mosque name should lead, got: {label}"
        );
    }

    #[test]
    fn display_location_label_uses_resolved_city_when_mosque_name_absent() {
        let config = crate::config::AppConfig::default();
        config.set_prayer_times_source(crate::config::PrayerTimesSource::Mawaqit);
        let mut cache = empty_cache();
        cache.mosque_name = None;
        cache.resolved_city = Some("Vienna".to_string());
        cache.country_code = Some("AT".to_string());
        config.set_mawaqit_cache(Some(cache));

        assert!(display_location_label(&config, "en").contains("Vienna, Austria"));
    }

    #[test]
    fn display_location_label_falls_back_to_coordinates_for_mawaqit_without_data() {
        let config = crate::config::AppConfig::default();
        config.set_prayer_times_source(crate::config::PrayerTimesSource::Mawaqit);
        config.set_latitude(48.2);
        config.set_longitude(16.3);

        assert_eq!(display_location_label(&config, "en"), "48.20°N, 16.30°E");
    }

    #[test]
    fn display_location_label_uses_coordinates_in_manual_mode() {
        let config = crate::config::AppConfig::default();
        config.set_location_mode(crate::config::LocationMode::Manual);
        config.set_latitude(-33.86);
        config.set_longitude(151.2);

        assert_eq!(display_location_label(&config, "en"), "33.86°S, 151.20°E");
    }

    #[test]
    fn display_location_label_uses_stored_city_in_city_mode() {
        let config = crate::config::AppConfig::default();
        config.set_location_mode(crate::config::LocationMode::City);
        config.set_city_name(Some("Sydney, Australia".to_string()));

        assert_eq!(display_location_label(&config, "en"), "Sydney, Australia");
    }

    #[test]
    fn display_city_label_returns_stored_city() {
        assert_eq!(
            display_city_label(Some("Sydney, Australia"), "en").as_deref(),
            Some("Sydney, Australia")
        );
    }

    #[test]
    fn format_published_time_formats_both_clock_styles() {
        assert_eq!(format_published_time("03:03", TimeFormat::Hours24), "03:03");
        assert_eq!(
            format_published_time("03:03", TimeFormat::Hours12),
            "03:03 AM"
        );
        assert_eq!(
            format_published_time("17:00", TimeFormat::Hours12),
            "05:00 PM"
        );
        assert_eq!(
            format_published_time("12:15", TimeFormat::Hours12),
            "12:15 PM"
        );
        assert_eq!(
            format_published_time("00:05", TimeFormat::Hours12),
            "12:05 AM"
        );
        assert_eq!(
            format_published_time("malformed", TimeFormat::Hours24),
            "malformed"
        );
    }
}
