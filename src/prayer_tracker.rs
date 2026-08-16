use chrono::{Duration, NaiveDate};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::config::AppConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum PrayerStatus {
    #[default]
    Pending,
    Prayed,
    Missed,
    Dismissed,
}

impl PrayerStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            PrayerStatus::Pending => "pending",
            PrayerStatus::Prayed => "prayed",
            PrayerStatus::Missed => "missed",
            PrayerStatus::Dismissed => "dismissed",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "prayed" => PrayerStatus::Prayed,
            "missed" => PrayerStatus::Missed,
            "dismissed" => PrayerStatus::Dismissed,
            _ => PrayerStatus::Pending,
        }
    }
}

#[allow(dead_code)]
pub const OBLIGATORY_PRAYERS: [&str; 5] = ["Fajr", "Dhuhr", "Asr", "Maghrib", "Isha"];

pub fn get_previous_prayer(
    date: NaiveDate,
    current_prayer: &str,
) -> Option<(&'static str, NaiveDate)> {
    match current_prayer {
        "Dhuhr" => Some(("Fajr", date)),
        "Asr" => Some(("Dhuhr", date)),
        "Maghrib" => Some(("Asr", date)),
        "Isha" => Some(("Maghrib", date)),
        "Fajr" => date.pred_opt().map(|prev_date| ("Isha", prev_date)),
        _ => None,
    }
}

pub fn prune_old_entries(
    logs: &mut HashMap<String, HashMap<String, String>>,
    today: NaiveDate,
    max_days: i64,
) {
    let cutoff = today - Duration::days(max_days);
    logs.retain(|date_str, _| {
        if let Ok(parsed_date) = NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
            parsed_date >= cutoff
        } else {
            false
        }
    });
}

pub trait PrayerStore {
    fn get_status(&self, date: NaiveDate, prayer_name: &str) -> PrayerStatus;
    fn set_status(&self, date: NaiveDate, prayer_name: &str, status: PrayerStatus);
}

pub struct ConfigPrayerTracker<'a> {
    pub config: &'a AppConfig,
}

impl<'a> PrayerStore for ConfigPrayerTracker<'a> {
    fn get_status(&self, date: NaiveDate, prayer_name: &str) -> PrayerStatus {
        let date_str = date.to_string();
        let logs = self.config.prayer_logs();
        logs.get(&date_str)
            .and_then(|m| m.get(prayer_name))
            .map(|s| PrayerStatus::from_str(s))
            .unwrap_or(PrayerStatus::Pending)
    }

    fn set_status(&self, date: NaiveDate, prayer_name: &str, status: PrayerStatus) {
        let date_str = date.to_string();
        let mut logs = self.config.prayer_logs();
        let day_map = logs.entry(date_str).or_default();
        day_map.insert(prayer_name.to_string(), status.as_str().to_string());

        prune_old_entries(&mut logs, date, 14);
        self.config.set_prayer_logs(logs);
        self.config.save();
    }
}

pub fn get_prayer_status(config: &AppConfig, date: NaiveDate, prayer_name: &str) -> PrayerStatus {
    let tracker = ConfigPrayerTracker { config };
    tracker.get_status(date, prayer_name)
}

pub fn set_prayer_status(
    config: &AppConfig,
    date: NaiveDate,
    prayer_name: &str,
    status: PrayerStatus,
) {
    let tracker = ConfigPrayerTracker { config };
    tracker.set_status(date, prayer_name, status);
}

pub fn is_previous_prayer_completed_or_dismissed(
    config: &AppConfig,
    date: NaiveDate,
    current_prayer: &str,
) -> bool {
    if let Some((prev_name, prev_date)) = get_previous_prayer(date, current_prayer) {
        let status = get_prayer_status(config, prev_date, prev_name);
        matches!(
            status,
            PrayerStatus::Prayed | PrayerStatus::Missed | PrayerStatus::Dismissed
        )
    } else {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_previous_prayer_lookup() {
        let today = NaiveDate::from_ymd_opt(2026, 8, 16).unwrap();
        assert_eq!(get_previous_prayer(today, "Dhuhr"), Some(("Fajr", today)));
        assert_eq!(get_previous_prayer(today, "Asr"), Some(("Dhuhr", today)));
        assert_eq!(get_previous_prayer(today, "Maghrib"), Some(("Asr", today)));
        assert_eq!(get_previous_prayer(today, "Isha"), Some(("Maghrib", today)));

        let yesterday = NaiveDate::from_ymd_opt(2026, 8, 15).unwrap();
        assert_eq!(
            get_previous_prayer(today, "Fajr"),
            Some(("Isha", yesterday))
        );
    }

    #[test]
    fn test_prune_old_entries() {
        let mut logs = HashMap::new();
        let today = NaiveDate::from_ymd_opt(2026, 8, 16).unwrap();

        logs.insert("2026-08-16".to_string(), HashMap::new());
        logs.insert("2026-08-05".to_string(), HashMap::new());
        logs.insert("2026-07-01".to_string(), HashMap::new());

        prune_old_entries(&mut logs, today, 14);

        assert!(logs.contains_key("2026-08-16"));
        assert!(logs.contains_key("2026-08-05"));
        assert!(!logs.contains_key("2026-07-01"));
    }
}
