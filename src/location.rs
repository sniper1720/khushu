use crate::config::MawaqitCache;
use crate::i18n::tr;
use chrono::{Datelike, Timelike};
use icu::calendar::{Date, Gregorian};
use icu::datetime::options::TimePrecision;
use icu::datetime::{
    FixedCalendarDateTimeFormatter, NoCalendarFormatter,
    fieldsets::{
        ET,
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
    pub lat: String,
    pub lon: String,
    pub display_name: String,
    pub address: Option<GeocodeAddress>,
    pub timezone: Option<String>,
}

fn non_empty_text(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

pub fn validated_time_zone_id(timezone: &str) -> Option<String> {
    let trimmed = non_empty_text(timezone)?;
    if let Ok(tz) = trimmed.parse::<chrono_tz::Tz>() {
        return Some(tz.to_string());
    }

    TIME_ZONE_LOOKUP
        .get_or_init(|| {
            chrono_tz::TZ_VARIANTS
                .iter()
                .map(|tz| {
                    let canonical = tz.to_string();
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
        .and_then(|tz| validated_time_zone_id(&tz))
        .or_else(|| {
            std::fs::read_to_string("/etc/timezone")
                .ok()
                .and_then(|tz| validated_time_zone_id(&tz))
        })
        .or_else(|| {
            std::fs::read_link("/etc/localtime")
                .ok()
                .and_then(|path| path.to_str().map(str::to_string))
                .and_then(|path| {
                    path.split_once("/zoneinfo/")
                        .map(|(_, timezone)| timezone.to_string())
                })
                .and_then(|tz| validated_time_zone_id(&tz))
        })
}

fn effective_lang(lang: &str) -> String {
    if lang == "auto" || lang.is_empty() {
        crate::i18n::detect_system_locale()
    } else {
        lang.to_string()
    }
}

fn icu_locale_key(lang: &str) -> String {
    let lang = effective_lang(lang);
    let normalized = lang
        .trim()
        .split('@')
        .next()
        .unwrap_or_default()
        .split('.')
        .next()
        .unwrap_or_default()
        .replace('_', "-");

    normalized
        .parse::<Locale>()
        .map(|locale| locale.to_string())
        .or_else(|_| {
            normalized
                .split('-')
                .next()
                .unwrap_or("en")
                .parse::<Locale>()
                .map(|locale| locale.to_string())
        })
        .unwrap_or_else(|_| "en".to_string())
}

pub fn short_city_with_country(display_name: &str) -> String {
    let parts: Vec<&str> = display_name
        .split(',')
        .map(|s| s.trim())
        .filter(|s: &&str| !s.is_empty())
        .collect();
    if parts.len() >= 2 {
        format!("{}, {}", parts[0], parts[parts.len() - 1])
    } else if let Some(first) = parts.first() {
        first.to_string()
    } else {
        display_name.to_string()
    }
}

pub fn country_name_from_code(code: &str, lang: &str) -> Option<String> {
    use std::cell::RefCell;
    use std::collections::HashMap;

    thread_local! {
        static CACHE: RefCell<HashMap<String, RegionDisplayNames>> = RefCell::new(HashMap::new());
    }

    let locale_str = icu_locale_key(lang);

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
            .map(|s: &str| s.to_string())
    })
}

pub fn city_name_from_time_zone(timezone: &str, lang: &str) -> Option<String> {
    use std::cell::RefCell;
    use std::collections::HashMap;

    thread_local! {
        static CACHE: RefCell<HashMap<String, NoCalendarFormatter<ExemplarCity>>> =
            RefCell::new(HashMap::new());
    }

    let locale_str = icu_locale_key(lang);
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

pub fn time_zone_location_name(timezone: &str, lang: &str) -> Option<String> {
    use std::cell::RefCell;
    use std::collections::HashMap;

    thread_local! {
        static CACHE: RefCell<HashMap<String, NoCalendarFormatter<TimeZoneLocation>>> =
            RefCell::new(HashMap::new());
    }

    let locale_str = icu_locale_key(lang);
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

pub fn localized_time_zone_label(timezone: &str, lang: &str) -> String {
    time_zone_location_name(timezone, lang)
        .or_else(|| city_name_from_time_zone(timezone, lang))
        .or_else(|| non_empty_text(timezone))
        .unwrap_or_else(|| timezone.to_string())
}

fn localize_iana_root(root: &str, lang: &str) -> Option<String> {
    // IANA roots without UN M.49 region codes — expose to xgettext
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
    country_name_from_code(m49, lang)
}

pub fn localized_zone(zone: &str, lang: &str) -> String {
    let mut parts = zone.splitn(2, '/');
    let root = parts.next().unwrap_or(zone);
    let Some(city) = parts.next() else {
        return zone.to_string();
    };

    let localized_root = localize_iana_root(root, lang).unwrap_or_else(|| root.to_string());
    let localized_city = if root == "Etc" {
        city.to_string()
    } else {
        city_name_from_time_zone(zone, lang).unwrap_or_else(|| city.to_string())
    };
    format!("{}/{}", localized_root, localized_city)
}

fn fallback_offset(offset_secs: i32) -> String {
    let sign = if offset_secs >= 0 { '+' } else { '-' };
    let abs = offset_secs.unsigned_abs();
    let hours = abs / 3600;
    let mins = (abs % 3600) / 60;
    if mins == 0 {
        format!("UTC{}{:02}", sign, hours)
    } else {
        format!("UTC{}{:02}:{:02}", sign, hours, mins)
    }
}

pub fn localized_offset(offset_secs: i32, lang: &str) -> String {
    use std::cell::RefCell;
    use std::collections::HashMap;

    thread_local! {
        static CACHE: RefCell<HashMap<String, NoCalendarFormatter<LocalizedOffsetLong>>> =
            RefCell::new(HashMap::new());
    }

    let locale_str = icu_locale_key(lang);
    let offset = match UtcOffset::try_from_seconds(offset_secs) {
        Ok(o) => o,
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

fn fallback_time(dt: chrono::DateTime<chrono_tz::Tz>) -> String {
    dt.format("%a %H:%M").to_string()
}

pub fn localized_time(dt: chrono::DateTime<chrono_tz::Tz>, lang: &str) -> String {
    use std::cell::RefCell;
    use std::collections::HashMap;

    thread_local! {
        static CACHE: RefCell<HashMap<String, FixedCalendarDateTimeFormatter<Gregorian, ET>>> =
            RefCell::new(HashMap::new());
    }

    let locale_str = icu_locale_key(lang);
    let date = match Date::try_new_gregorian(dt.year(), dt.month() as u8, dt.day() as u8) {
        Ok(d) => d,
        Err(_) => return fallback_time(dt),
    };
    let time = match Time::try_new(dt.hour() as u8, dt.minute() as u8, dt.second() as u8, 0) {
        Ok(t) => t,
        Err(_) => return fallback_time(dt),
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
                    return fallback_time(dt);
                }
            }
        }
        map.get(&locale_str)
            .map(|fmt| fmt.format(&input).to_string())
            .unwrap_or_else(|| fallback_time(dt))
    })
}

pub fn mawaqit_fallback_city_name(
    mosque_name: Option<&str>,
    timezone: Option<&str>,
) -> Option<String> {
    mosque_name
        .and_then(|name| name.rsplit_once(" - ").map(|(_, city)| city))
        .and_then(non_empty_text)
        .or_else(|| {
            timezone
                .and_then(|tz| tz.rsplit_once('/').map(|(_, city)| city.replace('_', " ")))
                .and_then(|city| non_empty_text(&city))
        })
}

pub fn localized_mawaqit_city_name(
    current_city_name: Option<&str>,
    timezone: Option<&str>,
    mosque_name: Option<&str>,
    lang: &str,
) -> Option<String> {
    timezone
        .and_then(|tz| city_name_from_time_zone(tz, lang))
        .or_else(|| current_city_name.and_then(non_empty_text))
        .or_else(|| mawaqit_fallback_city_name(mosque_name, timezone))
}

pub fn display_city_label(
    city_name: Option<&str>,
    mawaqit_cache: Option<&MawaqitCache>,
    lang: &str,
) -> Option<String> {
    let city = if let Some(cache) = mawaqit_cache {
        localized_mawaqit_city_name(
            city_name,
            cache.timezone.as_deref(),
            cache.mosque_name.as_deref(),
            lang,
        )
    } else {
        city_name.and_then(non_empty_text)
    }?;

    let mut text = short_city_with_country(&city);
    if let Some(cache) = mawaqit_cache
        && let Some(code) = cache.country_code.as_deref()
        && !text.contains(',')
        && let Some(country) = country_name_from_code(code, lang)
        && !country.is_empty()
    {
        text = format!("{}, {}", text, country);
    }

    Some(text)
}

use ashpd::desktop::location::{Accuracy, LocationProxy};
use futures_util::StreamExt;

pub async fn fetch_auto_location(lang: &str) -> Result<(f64, f64, String), String> {
    fetch_portal_location(lang).await
}

pub async fn resolve_city_name(lat: f64, lon: f64, lang: &str) -> Result<String, String> {
    reverse_geocode(lat, lon, lang)
        .await
        .map(|name| short_city_with_country(&name))
}

async fn fetch_portal_location(lang: &str) -> Result<(f64, f64, String), String> {
    log::info!("Attempting to fetch location via ASHPD Portal...");

    let proxy = LocationProxy::new().await.map_err(|e| {
        log::error!("Failed to create Location proxy: {}", e);
        tr("Location service unavailable. Please check system settings.")
    })?;

    let session = proxy
        .create_session(None, None, Some(Accuracy::City))
        .await
        .map_err(|e| {
            log::error!("Failed to create location session: {}", e);
            tr("Location access denied or unavailable.")
        })?;

    let mut stream = proxy.receive_location_updated().await.map_err(|e| {
        log::error!("Failed to receive location updates: {}", e);
        tr("Failed to receive location updates.")
    })?;

    proxy.start(&session, None).await.map_err(|e| {
        log::error!("Failed to start location session: {}", e);
        tr("Location access denied or unavailable.")
    })?;

    use futures_util::future::Either;

    let timeout = gtk4::glib::timeout_future_seconds(10);
    let location = match futures_util::future::select(timeout, stream.next()).await {
        Either::Right((Some(loc), _)) => loc,
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

    let lat = location.latitude();
    let lon = location.longitude();

    let _ = session.close().await;

    log::info!("Portal location fetched: {}, {}", lat, lon);

    let city = match reverse_geocode(lat, lon, lang).await {
        Ok(name) => {
            log::info!("Reverse geocoded to: {}", name);
            short_city_with_country(&name)
        }
        Err(e) => {
            log::warn!("Reverse geocode failed, using coordinates: {}", e);
            format_coordinates(lat, lon)
        }
    };

    Ok((lat, lon, city))
}

async fn reverse_geocode(lat: f64, lon: f64, lang: &str) -> Result<String, String> {
    let http = client();
    let normalized_lang = icu_locale_key(lang);

    let url = format!(
        "https://nominatim.openstreetmap.org/reverse?lat={}&lon={}&format=json&zoom=10&accept-language={}&addressdetails=1",
        lat, lon, normalized_lang
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
            .is_some_and(|c| c.eq_ignore_ascii_case("il"))
    {
        let city = addr
            .city
            .as_deref()
            .or(addr.town.as_deref())
            .or(addr.village.as_deref())
            .or(addr.suburb.as_deref())
            .unwrap_or("City");

        if let Some(country) = country_name_from_code("PS", lang) {
            return Ok(format!("{}, {}", city, country));
        }
    }

    Ok(result.display_name)
}

fn format_coordinates(lat: f64, lon: f64) -> String {
    let lat_dir = if lat >= 0.0 { "N" } else { "S" };
    let lon_dir = if lon >= 0.0 { "E" } else { "W" };
    format!("{:.2}°{}, {:.2}°{}", lat.abs(), lat_dir, lon.abs(), lon_dir)
}

pub async fn search_city(
    query: &str,
    lang: &str,
) -> Result<(f64, f64, String, Option<String>), String> {
    log::info!("Searching for city: {}", query);
    let http = client();
    let normalized_lang = icu_locale_key(lang);

    let url = format!(
        "https://nominatim.openstreetmap.org/search?q={}&format=json&limit=1&accept-language={}&addressdetails=1&timezone=true",
        urlencoding::encode(query),
        normalized_lang
    );

    let resp = http.get(url).send().await.map_err(|e| {
        log::error!("Geocoding request failed: {}", e);
        tr("Network error. Please check your connection.")
    })?;

    let results: Vec<GeocodeResult> = resp.json().await.map_err(|e| {
        log::error!("Geocoding JSON parsing failed: {}", e);
        tr("Invalid response from location service.")
    })?;

    if let Some(res) = results.first() {
        let lat = res.lat.parse::<f64>().map_err(|_| {
            log::error!("Invalid latitude from API: {}", res.lat);
            tr("Invalid response from location service.")
        })?;
        let lon = res.lon.parse::<f64>().map_err(|_| {
            log::error!("Invalid longitude from API: {}", res.lon);
            tr("Invalid response from location service.")
        })?;

        let mut display_name = res.display_name.clone();

        if let Some(ref addr) = res.address
            && addr
                .country_code
                .as_deref()
                .is_some_and(|c| c.eq_ignore_ascii_case("il"))
        {
            let city = addr
                .city
                .as_deref()
                .or(addr.town.as_deref())
                .or(addr.village.as_deref())
                .or(addr.suburb.as_deref())
                .unwrap_or("City");

            if let Some(country) = country_name_from_code("PS", lang) {
                display_name = format!("{}, {}", city, country);
            }
        }

        log::info!(
            "City found: {} ({}, {}) timezone: {:?}",
            display_name,
            lat,
            lon,
            res.timezone
        );
        Ok((lat, lon, display_name, res.timezone.clone()))
    } else {
        log::warn!("City not found for query: {}", query);
        Err(tr("City not found. Please check the spelling."))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_cache() -> MawaqitCache {
        MawaqitCache {
            url: String::new(),
            mosque_name: None,
            timezone: None,
            latitude: None,
            longitude: None,
            country_code: None,
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
    fn localized_mawaqit_city_prefers_icu_over_stored_city() {
        assert_eq!(
            localized_mawaqit_city_name(Some("Vienna"), Some("Europe/Vienna"), None, "de")
                .as_deref(),
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
        let tz: chrono_tz::Tz = "Europe/Oslo".parse().unwrap();
        let naive = chrono::NaiveDate::from_ymd_opt(2026, 1, 30)
            .unwrap()
            .and_hms_opt(15, 47, 42)
            .unwrap();
        let dt: chrono::DateTime<chrono_tz::Tz> = tz.from_local_datetime(&naive).unwrap();

        let en = localized_time(dt, "en");
        assert!(en.contains("Fri"), "weekday should be localized: {en}");
        assert!(en.contains("3:47"), "hour:minute should be shown: {en}");
        assert!(!en.contains("42"), "seconds must not be rendered: {en}");

        let ar = localized_time(dt, "ar");
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
    fn display_city_label_appends_country_for_mawaqit() {
        let mut cache = empty_cache();
        cache.timezone = Some("Europe/Vienna".to_string());
        cache.country_code = Some("AT".to_string());

        assert_eq!(
            display_city_label(None, Some(&cache), "en").as_deref(),
            Some("Vienna, Austria")
        );
    }
}
