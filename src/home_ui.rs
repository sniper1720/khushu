use chrono::{Datelike, Duration, Local};
use gtk::Label;
use gtk4 as gtk;
use hijri_date::HijriDate;

use crate::config::AppConfig;
use crate::i18n::tr;
use crate::location;
use crate::time::PrayerEngine;

pub fn refresh_home_ui(
    hero_label: &Label,
    hijri_label: &Label,
    location_label: &Label,
    lang: &str,
    config: &AppConfig,
) {
    let now = Local::now();
    let adjusted_now = now + Duration::days(config.hijri_offset);
    let hijri = HijriDate::from_gr(
        adjusted_now.year() as usize,
        adjusted_now.month() as usize,
        adjusted_now.day() as usize,
    )
    .expect("Hijri error");

    let en_months = [
        "Muharram",
        "Safar",
        "Rabi' al-Awwal",
        "Rabi' al-Thani",
        "Jumada al-Ula",
        "Jumada al-Akhirah",
        "Rajab",
        "Sha'ban",
        "Ramadan",
        "Shawwal",
        "Dhu al-Qi'dah",
        "Dhu al-Hijjah",
    ];
    let m_name = tr(en_months.get(hijri.month() - 1).unwrap_or(&""), lang);
    hijri_label.set_label(&format!("{} {} {}", hijri.day(), m_name, hijri.year()));

    if let Some(city) = &config.city_name {
        location_label.set_label(&location::short_city_with_country(city));
    } else {
        location_label.set_label(&format!("{:.2}, {:.2}", config.latitude, config.longitude));
    }

    let engine = PrayerEngine::new(
        config.latitude,
        config.longitude,
        &config.method,
        &config.madhab,
    );
    if let Some(next) = engine.next_prayer(now.date_naive()) {
        let name = tr(&next.0, lang);
        let time_str = next.1.format("%H:%M").to_string();

        let diff = next.1.signed_duration_since(Local::now());
        let hours = diff.num_hours();
        let minutes = diff.num_minutes() % 60;

        let label_text = if hours > 0 {
            format!("{} {} ({}h {}m)", name, time_str, hours, minutes)
        } else {
            format!("{} {} ({}m)", name, time_str, minutes)
        };
        hero_label.set_label(&label_text);
    } else {
        hero_label.set_label("");
    }
}
