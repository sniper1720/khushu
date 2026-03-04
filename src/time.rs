use chrono::{DateTime, Local, NaiveDate, Utc};
use salah::{Configuration, Coordinates, Madhab, Method, Parameters, Prayer, PrayerTimes};

use crate::config::{CalculationMethod, MadhabChoice};

#[derive(Clone, Debug)]
pub struct PrayerSchedule {
    pub fajr: DateTime<Local>,
    pub shurooq: DateTime<Local>,
    pub dhuhr: DateTime<Local>,
    pub asr: DateTime<Local>,
    pub maghrib: DateTime<Local>,
    pub isha: DateTime<Local>,
}

pub struct PrayerEngine {
    params: Parameters,
    location: Coordinates,
}

impl PrayerEngine {
    pub fn new(
        latitude: f64,
        longitude: f64,
        method: &CalculationMethod,
        madhab: &MadhabChoice,
    ) -> Self {
        let location = Coordinates::new(latitude, longitude);

        let salah_method = match method {
            CalculationMethod::MWL => Method::MuslimWorldLeague,
            CalculationMethod::ISNA => Method::NorthAmerica,
            CalculationMethod::Egypt => Method::Egyptian,
            CalculationMethod::Makkah => Method::UmmAlQura,
            CalculationMethod::Karachi => Method::Karachi,
            CalculationMethod::Dubai => Method::Dubai,
            CalculationMethod::MoonsightingCommittee => Method::MoonsightingCommittee,
            CalculationMethod::Kuwait => Method::Kuwait,
            CalculationMethod::Qatar => Method::Qatar,
            CalculationMethod::Singapore => Method::Singapore,
            CalculationMethod::Turkey => Method::Turkey,
        };

        let salah_madhab = match madhab {
            MadhabChoice::Hanafi => Madhab::Hanafi,
            MadhabChoice::Shafi => Madhab::Shafi,
        };

        let params = Configuration::with(salah_method, salah_madhab);

        Self { params, location }
    }

    pub fn get_prayer_times(&self, date: NaiveDate) -> Option<PrayerSchedule> {
        let times = PrayerTimes::new(date, self.location, self.params);

        Some(PrayerSchedule {
            fajr: self.convert_to_local(times.time(Prayer::Fajr)),
            shurooq: self.convert_to_local(times.time(Prayer::Sunrise)),
            dhuhr: self.convert_to_local(times.time(Prayer::Dhuhr)),
            asr: self.convert_to_local(times.time(Prayer::Asr)),
            maghrib: self.convert_to_local(times.time(Prayer::Maghrib)),
            isha: self.convert_to_local(times.time(Prayer::Isha)),
        })
    }

    pub fn next_prayer(&self, date: NaiveDate) -> Option<(String, DateTime<Local>)> {
        let times = PrayerTimes::new(date, self.location, self.params);
        let now = Local::now();

        let prayers = [
            ("Fajr", self.convert_to_local(times.time(Prayer::Fajr))),
            (
                "Sunrise",
                self.convert_to_local(times.time(Prayer::Sunrise)),
            ),
            ("Dhuhr", self.convert_to_local(times.time(Prayer::Dhuhr))),
            ("Asr", self.convert_to_local(times.time(Prayer::Asr))),
            (
                "Maghrib",
                self.convert_to_local(times.time(Prayer::Maghrib)),
            ),
            ("Isha", self.convert_to_local(times.time(Prayer::Isha))),
        ];

        for (name, time) in prayers {
            if time > now {
                return Some((name.to_string(), time));
            }
        }

        let next_day = date.succ_opt().unwrap();
        let next_times = PrayerTimes::new(next_day, self.location, self.params);
        Some((
            "Fajr".to_string(),
            self.convert_to_local(next_times.time(Prayer::Fajr)),
        ))
    }

    fn convert_to_local(&self, time: DateTime<Utc>) -> DateTime<Local> {
        DateTime::from(time)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    #[test]
    fn test_prayer_times_calculation() {
        let engine = PrayerEngine::new(
            21.4225,
            39.8262,
            &CalculationMethod::Makkah,
            &MadhabChoice::Shafi,
        );
        let date = NaiveDate::from_ymd_opt(2023, 10, 1).unwrap();

        let schedule = engine.get_prayer_times(date);
        assert!(schedule.is_some());

        let times = schedule.unwrap();

        assert!(times.fajr < times.dhuhr);
    }

    #[test]
    fn prayer_order_all_methods() {
        let methods = [
            CalculationMethod::MWL,
            CalculationMethod::ISNA,
            CalculationMethod::Egypt,
            CalculationMethod::Makkah,
            CalculationMethod::Karachi,
        ];
        let date = NaiveDate::from_ymd_opt(2024, 6, 15).unwrap();

        for method in &methods {
            let engine = PrayerEngine::new(36.75, 3.05, method, &MadhabChoice::Shafi);
            let t = engine.get_prayer_times(date).expect("schedule must exist");
            assert!(t.fajr < t.shurooq, "Fajr < Sunrise failed for {:?}", method);
            assert!(
                t.shurooq < t.dhuhr,
                "Sunrise < Dhuhr failed for {:?}",
                method
            );
            assert!(t.dhuhr < t.asr, "Dhuhr < Asr failed for {:?}", method);
            assert!(t.asr < t.maghrib, "Asr < Maghrib failed for {:?}", method);
            assert!(t.maghrib < t.isha, "Maghrib < Isha failed for {:?}", method);
        }
    }

    #[test]
    fn hanafi_asr_later_than_shafi() {
        let date = NaiveDate::from_ymd_opt(2024, 3, 20).unwrap();
        let shafi = PrayerEngine::new(36.75, 3.05, &CalculationMethod::MWL, &MadhabChoice::Shafi);
        let hanafi = PrayerEngine::new(36.75, 3.05, &CalculationMethod::MWL, &MadhabChoice::Hanafi);

        let shafi_asr = shafi.get_prayer_times(date).unwrap().asr;
        let hanafi_asr = hanafi.get_prayer_times(date).unwrap().asr;

        assert!(
            hanafi_asr > shafi_asr,
            "Hanafi Asr ({}) should be later than Shafi Asr ({})",
            hanafi_asr,
            shafi_asr
        );
    }

    #[test]
    fn next_prayer_wraps_to_tomorrow_fajr() {
        let date = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();
        let engine = PrayerEngine::new(36.75, 3.05, &CalculationMethod::MWL, &MadhabChoice::Shafi);

        let today = engine.get_prayer_times(date).unwrap();
        let result = engine.next_prayer(date);
        assert!(result.is_some());

        let (name, time) = result.unwrap();
        let valid_names = ["Fajr", "Sunrise", "Dhuhr", "Asr", "Maghrib", "Isha"];
        assert!(
            valid_names.contains(&name.as_str()),
            "Unexpected prayer name: {}",
            name
        );
        assert!(time >= today.fajr);
    }

    #[test]
    fn different_methods_produce_different_times() {
        let date = NaiveDate::from_ymd_opt(2024, 6, 15).unwrap();
        let mwl = PrayerEngine::new(36.75, 3.05, &CalculationMethod::MWL, &MadhabChoice::Shafi);
        let egypt = PrayerEngine::new(36.75, 3.05, &CalculationMethod::Egypt, &MadhabChoice::Shafi);

        let mwl_t = mwl.get_prayer_times(date).unwrap();
        let egypt_t = egypt.get_prayer_times(date).unwrap();

        assert_ne!(
            mwl_t.fajr.format("%H:%M").to_string(),
            egypt_t.fajr.format("%H:%M").to_string(),
            "MWL and Egypt Fajr should differ"
        );
    }
}
