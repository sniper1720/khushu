use chrono::{DateTime, Datelike, Local, NaiveDate, TimeZone, Timelike, Utc};
use chrono_tz::Tz;
use mawaqit::{Configuration, Coordinates, Madhab, Method, Parameters, Prayer, PrayerTimes};

use crate::config::{AppConfig, CalculationMethod, MadhabChoice, PrayerTimesSource, TimezoneMode};

pub const HIJRI_MONTH_NAMES: [&str; 12] = [
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

        let mawaqit_method = match method {
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
            CalculationMethod::Kemenag => Method::Singapore,
            CalculationMethod::France => Method::France,
            CalculationMethod::Algeria => Method::Algeria,
        };

        let mawaqit_madhab = match madhab {
            MadhabChoice::Hanafi => Madhab::Hanafi,
            MadhabChoice::Shafi => Madhab::Shafi,
        };

        let mut params = Configuration::with(mawaqit_method, mawaqit_madhab);

        if method == &CalculationMethod::Kemenag {
            params.fajr_angle = 20.0;
            params.isha_angle = 18.0;
        }

        params.polar_fallback = mawaqit::PolarFallback::recommended(location);
        params.high_latitude_rule = mawaqit::HighLatitudeRule::Recommended;
        Self { params, location }
    }

    pub fn get_prayer_times(&self, date: NaiveDate) -> Option<PrayerSchedule> {
        let times = PrayerTimes::try_new(date, self.location, self.params)
            .expect("prayer times calculation failed");

        Some(PrayerSchedule {
            fajr: self.convert_to_local(times.time(Prayer::Fajr)),
            shurooq: self.convert_to_local(times.time(Prayer::Sunrise)),
            dhuhr: self.convert_to_local(times.time(Prayer::Dhuhr)),
            asr: self.convert_to_local(times.time(Prayer::Asr)),
            maghrib: self.convert_to_local(times.time(Prayer::Maghrib)),
            isha: self.convert_to_local(times.time(Prayer::Isha)),
        })
    }

    fn convert_to_local(&self, time: DateTime<Utc>) -> DateTime<Local> {
        DateTime::from(time)
    }
}

fn parse_hm(s: &str) -> Option<(u32, u32)> {
    let mut it = s.split(':');
    let h = it.next()?.parse::<u32>().ok()?;
    let m = it.next()?.parse::<u32>().ok()?;
    if h > 23 || m > 59 {
        return None;
    }
    Some((h, m))
}

pub fn schedule_from_hm(
    date: NaiveDate,
    fajr: &str,
    shurooq: &str,
    dhuhr: &str,
    asr: &str,
    maghrib: &str,
    isha: &str,
) -> Option<PrayerSchedule> {
    let (fh, fm) = parse_hm(fajr)?;
    let (sh, sm) = parse_hm(shurooq)?;
    let (dh, dm) = parse_hm(dhuhr)?;
    let (ah, am) = parse_hm(asr)?;
    let (mh, mm) = parse_hm(maghrib)?;
    let (ih, im) = parse_hm(isha)?;

    let fajr = Local
        .with_ymd_and_hms(date.year(), date.month(), date.day(), fh, fm, 0)
        .single()?;
    let shurooq = Local
        .with_ymd_and_hms(date.year(), date.month(), date.day(), sh, sm, 0)
        .single()?;
    let dhuhr = Local
        .with_ymd_and_hms(date.year(), date.month(), date.day(), dh, dm, 0)
        .single()?;
    let asr = Local
        .with_ymd_and_hms(date.year(), date.month(), date.day(), ah, am, 0)
        .single()?;
    let maghrib = Local
        .with_ymd_and_hms(date.year(), date.month(), date.day(), mh, mm, 0)
        .single()?;
    let isha = Local
        .with_ymd_and_hms(date.year(), date.month(), date.day(), ih, im, 0)
        .single()?;

    Some(PrayerSchedule {
        fajr,
        shurooq,
        dhuhr,
        asr,
        maghrib,
        isha,
    })
}

pub fn schedule_for_config(config: &AppConfig, date: NaiveDate) -> Option<PrayerSchedule> {
    if config.prayer_times_source() == PrayerTimesSource::Mawaqit
        && let Some(cache) = config.mawaqit_cache().as_ref()
        && cache.year == date.year()
    {
        let month_idx = date.month0() as usize;
        if let Some(month) = cache.months.get(month_idx)
            && let Some(arr) = month.get(&date.day())
        {
            return schedule_from_hm(date, &arr[0], &arr[1], &arr[2], &arr[3], &arr[4], &arr[5])
                .map(|s| apply_timezone_override(config, s));
        }
    }

    PrayerEngine::new(
        config.latitude(),
        config.longitude(),
        &config.method(),
        &config.madhab(),
    )
    .get_prayer_times(date)
    .map(|s| apply_timezone_override(config, s))
}

pub fn next_prayer_from_schedule(
    schedule: &PrayerSchedule,
    now: DateTime<Local>,
) -> Option<(String, DateTime<Local>)> {
    let prayers = [
        ("Fajr".to_string(), schedule.fajr),
        ("Sunrise".to_string(), schedule.shurooq),
        ("Dhuhr".to_string(), schedule.dhuhr),
        ("Asr".to_string(), schedule.asr),
        ("Maghrib".to_string(), schedule.maghrib),
        ("Isha".to_string(), schedule.isha),
    ];
    for (name, time) in prayers {
        if time > now {
            return Some((name, time));
        }
    }
    None
}

pub fn effective_now(config: &AppConfig) -> DateTime<Local> {
    match config.timezone_mode() {
        TimezoneMode::Auto => Local::now(),
        TimezoneMode::Named(tz_str) => {
            if let Ok(tz) = tz_str.parse::<Tz>() {
                let utc_now = Utc::now();
                let in_tz = utc_now.with_timezone(&tz);
                Local
                    .with_ymd_and_hms(
                        in_tz.year(),
                        in_tz.month(),
                        in_tz.day(),
                        in_tz.hour(),
                        in_tz.minute(),
                        in_tz.second(),
                    )
                    .single()
                    .unwrap_or_else(Local::now)
            } else {
                Local::now()
            }
        }
        TimezoneMode::UtcOffset(mins) => {
            let now = Local::now();
            let local_off = now.offset().local_minus_utc() / 60;
            let delta = mins - local_off;
            now + chrono::Duration::minutes(delta as i64)
        }
    }
}

pub fn effective_today(config: &AppConfig) -> NaiveDate {
    effective_now(config).date_naive()
}

pub fn format_hijri_date(dt: DateTime<Local>, hijri_offset: i64) -> String {
    use chrono::Duration;
    use hijri_date::HijriDate;

    let adjusted = dt + Duration::days(hijri_offset);
    match HijriDate::from_gr(
        adjusted.year() as usize,
        adjusted.month() as usize,
        adjusted.day() as usize,
    ) {
        Ok(hijri) => {
            let m_name = crate::i18n::tr(HIJRI_MONTH_NAMES.get(hijri.month() - 1).unwrap_or(&""));
            format!("{} {} {}", hijri.day(), m_name, hijri.year())
        }
        Err(e) => {
            log::error!("Failed to calculate Hijri date: {e}");
            "—".to_string()
        }
    }
}

pub fn apply_timezone_override(config: &AppConfig, schedule: PrayerSchedule) -> PrayerSchedule {
    match config.timezone_mode() {
        TimezoneMode::Auto => schedule,
        TimezoneMode::Named(tz_str) => {
            if let Ok(tz) = tz_str.parse::<Tz>() {
                let shift_time = |dt: DateTime<Local>| -> DateTime<Local> {
                    let in_target = dt.with_timezone(&tz);
                    Local
                        .with_ymd_and_hms(
                            in_target.year(),
                            in_target.month(),
                            in_target.day(),
                            in_target.hour(),
                            in_target.minute(),
                            in_target.second(),
                        )
                        .single()
                        .unwrap_or(dt)
                };
                PrayerSchedule {
                    fajr: shift_time(schedule.fajr),
                    shurooq: shift_time(schedule.shurooq),
                    dhuhr: shift_time(schedule.dhuhr),
                    asr: shift_time(schedule.asr),
                    maghrib: shift_time(schedule.maghrib),
                    isha: shift_time(schedule.isha),
                }
            } else {
                schedule
            }
        }
        TimezoneMode::UtcOffset(target) => {
            let local_off = Local::now().offset().local_minus_utc() / 60;
            let delta = target - local_off;
            if delta == 0 {
                return schedule;
            }
            let shift = chrono::Duration::minutes(delta as i64);
            PrayerSchedule {
                fajr: schedule.fajr + shift,
                shurooq: schedule.shurooq + shift,
                dhuhr: schedule.dhuhr + shift,
                asr: schedule.asr + shift,
                maghrib: schedule.maghrib + shift,
                isha: schedule.isha + shift,
            }
        }
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
        let now = today.isha + chrono::Duration::minutes(1);
        let result = next_prayer_from_schedule(&today, now);
        assert!(result.is_none());

        let next_day = date.succ_opt().unwrap();
        let tomorrow = engine.get_prayer_times(next_day).unwrap();
        assert_eq!(
            next_prayer_from_schedule(&tomorrow, tomorrow.fajr - chrono::Duration::minutes(1))
                .unwrap()
                .0,
            "Fajr"
        );
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
