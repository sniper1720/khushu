use chrono::{DateTime, Datelike, Local, NaiveDate, TimeZone, Timelike, Utc};
use chrono_tz::Tz;
use mawaqit::{Configuration, Coordinates, Madhab, Method, Parameters, Prayer, PrayerTimes};

use crate::config::{
    AppConfig, CalculationMethod, HighLatitudeChoice, MadhabChoice, PolarEstimationMethod,
    PrayerTimesSource, TimezoneMode,
};
use crate::i18n::tr;

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

#[derive(Clone, Debug)]
pub struct PrayerResult {
    pub schedule: PrayerSchedule,
    pub lre_blocked: bool,
    pub fallback_active: bool,
}

pub struct PrayerEngine {
    params: Parameters,
    location: Coordinates,
    high_latitude: HighLatitudeChoice,
}

impl PrayerEngine {
    pub fn new(
        latitude: f64,
        longitude: f64,
        method: &CalculationMethod,
        madhab: &MadhabChoice,
        high_latitude: &HighLatitudeChoice,
        polar: &PolarEstimationMethod,
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

        params.polar_estimation = Some(match polar {
            PolarEstimationMethod::NearestLatitude => mawaqit::PolarEstimation::NearestLatitude,
            PolarEstimationMethod::Reference45 => mawaqit::PolarEstimation::Reference45,
        });

        params.high_latitude_rule = match high_latitude {
            HighLatitudeChoice::Auto => mawaqit::HighLatitudeRule::Recommended,
            HighLatitudeChoice::MiddleOfTheNight => mawaqit::HighLatitudeRule::MiddleOfTheNight,
            HighLatitudeChoice::SeventhOfTheNight => mawaqit::HighLatitudeRule::SeventhOfTheNight,
            HighLatitudeChoice::TwilightAngle => mawaqit::HighLatitudeRule::TwilightAngle,
            HighLatitudeChoice::LocalRelativeEstimation => {
                mawaqit::HighLatitudeRule::LocalRelativeEstimation
            }
        };

        Self {
            params,
            location,
            high_latitude: high_latitude.clone(),
        }
    }

    pub fn get_prayer_times(&self, date: NaiveDate) -> Result<PrayerResult, &'static str> {
        let (times, lre_blocked) = match PrayerTimes::try_new(date, self.location, self.params) {
            Ok(prayer_times) => (prayer_times, false),
            Err(_)
                if matches!(
                    self.high_latitude,
                    HighLatitudeChoice::LocalRelativeEstimation
                ) =>
            {
                let mut params = self.params;
                params.high_latitude_rule = mawaqit::HighLatitudeRule::Recommended;
                (PrayerTimes::try_new(date, self.location, params)?, true)
            }
            Err(err) => return Err(err),
        };

        // reference_latitude() already folds in any substitution mawaqit used,
        // be it a polar-estimation walk or Moonsighting Committee's Zone C
        // anchor/walk, plus any lre_blocked retry. We trust the library's final
        // answer rather than re-deriving a substitution ourselves, so the
        // indicator stays honest for every method.
        let fallback_active = (times.reference_latitude() - self.location.latitude).abs() > 0.001;

        Ok(PrayerResult {
            schedule: self.schedule_from_times(times),
            lre_blocked,
            fallback_active,
        })
    }

    fn schedule_from_times(&self, times: PrayerTimes) -> PrayerSchedule {
        PrayerSchedule {
            fajr: self.convert_to_local(times.time(Prayer::Fajr)),
            shurooq: self.convert_to_local(times.time(Prayer::Sunrise)),
            dhuhr: self.convert_to_local(times.time(Prayer::Dhuhr)),
            asr: self.convert_to_local(times.time(Prayer::Asr)),
            maghrib: self.convert_to_local(times.time(Prayer::Maghrib)),
            isha: self.convert_to_local(times.time(Prayer::Isha)),
        }
    }

    fn convert_to_local(&self, time: DateTime<Utc>) -> DateTime<Local> {
        DateTime::from(time)
    }
}

fn parse_hm(text: &str) -> Option<(u32, u32)> {
    let mut parts = text.split(':');
    let hours = parts.next()?.parse::<u32>().ok()?;
    let minutes = parts.next()?.parse::<u32>().ok()?;
    if hours > 23 || minutes > 59 {
        return None;
    }
    Some((hours, minutes))
}

pub fn schedule_from_hm(
    date: NaiveDate,
    times: &[String; 6],
    timezone: Option<&Tz>,
) -> Option<PrayerSchedule> {
    let fajr_time = wall_clock_from_hm(date, &times[0], timezone)?;
    let shurooq_time = wall_clock_from_hm(date, &times[1], timezone)?;
    let dhuhr_time = wall_clock_from_hm(date, &times[2], timezone)?;
    let asr_time = wall_clock_from_hm(date, &times[3], timezone)?;
    let maghrib_time = wall_clock_from_hm(date, &times[4], timezone)?;
    let isha_time = wall_clock_from_hm(date, &times[5], timezone)?;

    Some(PrayerSchedule {
        fajr: fajr_time,
        shurooq: shurooq_time,
        dhuhr: dhuhr_time,
        asr: asr_time,
        maghrib: maghrib_time,
        isha: isha_time,
    })
}

fn wall_clock_from_hm(
    date: NaiveDate,
    time: &str,
    timezone: Option<&Tz>,
) -> Option<DateTime<Local>> {
    let (hours, minutes) = parse_hm(time)?;
    wall_clock_to_local(date, hours, minutes, timezone)
}

/// Builds a real instant from a wall-clock time.
///
/// With a target timezone the wall-clock is interpreted in that zone (the
/// Connected Mosque schedule is the mosque's local wall-clock); otherwise it
/// is taken as the system-local wall-clock.
fn wall_clock_to_local(
    date: NaiveDate,
    hours: u32,
    minutes: u32,
    timezone: Option<&Tz>,
) -> Option<DateTime<Local>> {
    match timezone {
        Some(zone) => {
            let naive = date.and_hms_opt(hours, minutes, 0)?;
            let zoned = zone.from_local_datetime(&naive).single()?;
            Some(zoned.with_timezone(&Local))
        }
        None => Local
            .with_ymd_and_hms(date.year(), date.month(), date.day(), hours, minutes, 0)
            .single(),
    }
}

pub fn schedule_for_config(
    config: &AppConfig,
    date: NaiveDate,
) -> Result<PrayerResult, &'static str> {
    if config.prayer_times_source() == PrayerTimesSource::Mawaqit
        && let Some(cache) = config.mawaqit_cache().as_ref()
        && cache.year == date.year()
    {
        let month_idx = date.month0() as usize;
        if let Some(month) = cache.months.get(month_idx)
            && let Some(arr) = month.get(&date.day())
        {
            // The cached strings are the mosque's wall-clock times; build them
            // as real instants in the mosque timezone and do not apply the
            // timezone override, which would reinterpret the wall-clock and
            // shift the schedule.
            let mosque_timezone = cache
                .timezone
                .as_deref()
                .and_then(|timezone| timezone.parse::<Tz>().ok());
            return schedule_from_hm(date, arr, mosque_timezone.as_ref())
                .map(|schedule| PrayerResult {
                    schedule,
                    lre_blocked: false,
                    fallback_active: false,
                })
                .ok_or("failed to parse cached times");
        }
    }

    PrayerEngine::new(
        config.latitude(),
        config.longitude(),
        &config.method(),
        &config.madhab(),
        &config.high_latitude_rule(),
        &config.polar_estimation_method(),
    )
    .get_prayer_times(date)
    .map(|mut result| {
        result.schedule = apply_timezone_override(config, result.schedule);
        result
    })
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

/// The timezone that actually governs prayer times.
///
/// For the Connected Mosque source the mosque timezone is forced: it reflects
/// the cached wall-clock schedule. Otherwise the user's stored timezone mode
/// applies. The stored mode is never mutated.
pub fn effective_timezone(config: &AppConfig) -> TimezoneMode {
    if config.prayer_times_source() == PrayerTimesSource::Mawaqit
        && let Some(cache) = config.mawaqit_cache().as_ref()
        && let Some(timezone) = cache.timezone.as_deref()
    {
        return TimezoneMode::Named(timezone.to_string());
    }
    config.timezone_mode()
}

pub fn effective_now(config: &AppConfig) -> DateTime<Local> {
    // For the Connected Mosque source the schedule is real wall-clock in the
    // mosque timezone.
    if config.prayer_times_source() == PrayerTimesSource::Mawaqit {
        return Local::now();
    }
    match config.timezone_mode() {
        TimezoneMode::Auto => Local::now(),
        TimezoneMode::Named(timezone_str) => {
            if let Ok(timezone) = timezone_str.parse::<Tz>() {
                let utc_now = Utc::now();
                let in_timezone = utc_now.with_timezone(&timezone);
                Local
                    .with_ymd_and_hms(
                        in_timezone.year(),
                        in_timezone.month(),
                        in_timezone.day(),
                        in_timezone.hour(),
                        in_timezone.minute(),
                        in_timezone.second(),
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

pub fn format_hijri_date(datetime: DateTime<Local>, hijri_offset: i64) -> String {
    // HIJRI_MONTH_NAMES: Islamic month names (expose to xgettext)
    if false {
        tr("Muharram");
        tr("Safar");
        tr("Rabi' al-Awwal");
        tr("Rabi' al-Thani");
        tr("Jumada al-Ula");
        tr("Jumada al-Akhirah");
        tr("Rajab");
        tr("Sha'ban");
        tr("Ramadan");
        tr("Shawwal");
        tr("Dhu al-Qi'dah");
        tr("Dhu al-Hijjah");
    }

    use chrono::Duration;
    use hijri_date::HijriDate;

    let adjusted = datetime + Duration::days(hijri_offset);
    match HijriDate::from_gr(
        adjusted.year() as usize,
        adjusted.month() as usize,
        adjusted.day() as usize,
    ) {
        Ok(hijri) => {
            let hijri_month = tr(HIJRI_MONTH_NAMES.get(hijri.month() - 1).unwrap_or(&""));
            format!("{} {} {}", hijri.day(), hijri_month, hijri.year())
        }
        Err(err) => {
            log::error!("Failed to calculate Hijri date: {err}");
            "—".to_string()
        }
    }
}

pub fn apply_timezone_override(config: &AppConfig, schedule: PrayerSchedule) -> PrayerSchedule {
    match config.timezone_mode() {
        TimezoneMode::Auto => schedule,
        TimezoneMode::Named(timezone_str) => {
            if let Ok(timezone) = timezone_str.parse::<Tz>() {
                let shift_time = |datetime: DateTime<Local>| -> DateTime<Local> {
                    let in_target = datetime.with_timezone(&timezone);
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
                        .unwrap_or(datetime)
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

    fn default_params() -> (HighLatitudeChoice, PolarEstimationMethod) {
        (
            HighLatitudeChoice::Auto,
            PolarEstimationMethod::NearestLatitude,
        )
    }

    #[test]
    fn test_prayer_times_calculation() {
        let (high_latitude_rule, prayer_params) = default_params();
        let engine = PrayerEngine::new(
            21.4225,
            39.8262,
            &CalculationMethod::Makkah,
            &MadhabChoice::Shafi,
            &high_latitude_rule,
            &prayer_params,
        );
        let date = NaiveDate::from_ymd_opt(2023, 10, 1).unwrap();

        let result = engine.get_prayer_times(date);
        assert!(result.is_ok());

        let times = result.unwrap().schedule;

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
            let (high_latitude_rule, prayer_params) = default_params();
            let engine = PrayerEngine::new(
                36.75,
                3.05,
                method,
                &MadhabChoice::Shafi,
                &high_latitude_rule,
                &prayer_params,
            );
            let schedule = engine
                .get_prayer_times(date)
                .expect("schedule must exist")
                .schedule;
            assert!(
                schedule.fajr < schedule.shurooq,
                "Fajr < Sunrise failed for {:?}",
                method
            );
            assert!(
                schedule.shurooq < schedule.dhuhr,
                "Sunrise < Dhuhr failed for {:?}",
                method
            );
            assert!(
                schedule.dhuhr < schedule.asr,
                "Dhuhr < Asr failed for {:?}",
                method
            );
            assert!(
                schedule.asr < schedule.maghrib,
                "Asr < Maghrib failed for {:?}",
                method
            );
            assert!(
                schedule.maghrib < schedule.isha,
                "Maghrib < Isha failed for {:?}",
                method
            );
        }
    }

    #[test]
    fn hanafi_asr_later_than_shafi() {
        let date = NaiveDate::from_ymd_opt(2024, 3, 20).unwrap();
        let (high_latitude_rule, prayer_params) = default_params();
        let shafi = PrayerEngine::new(
            36.75,
            3.05,
            &CalculationMethod::MWL,
            &MadhabChoice::Shafi,
            &high_latitude_rule,
            &prayer_params,
        );
        let hanafi = PrayerEngine::new(
            36.75,
            3.05,
            &CalculationMethod::MWL,
            &MadhabChoice::Hanafi,
            &high_latitude_rule,
            &prayer_params,
        );

        let shafi_asr = shafi.get_prayer_times(date).unwrap().schedule.asr;
        let hanafi_asr = hanafi.get_prayer_times(date).unwrap().schedule.asr;

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
        let (high_latitude_rule, prayer_params) = default_params();
        let engine = PrayerEngine::new(
            36.75,
            3.05,
            &CalculationMethod::MWL,
            &MadhabChoice::Shafi,
            &high_latitude_rule,
            &prayer_params,
        );

        let today = engine.get_prayer_times(date).unwrap().schedule;
        let now = today.isha + chrono::Duration::minutes(1);
        let result = next_prayer_from_schedule(&today, now);
        assert!(result.is_none());

        let next_day = date.succ_opt().unwrap();
        let tomorrow = engine.get_prayer_times(next_day).unwrap().schedule;
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
        let (high_latitude_rule, prayer_params) = default_params();
        let mwl = PrayerEngine::new(
            36.75,
            3.05,
            &CalculationMethod::MWL,
            &MadhabChoice::Shafi,
            &high_latitude_rule,
            &prayer_params,
        );
        let egypt = PrayerEngine::new(
            36.75,
            3.05,
            &CalculationMethod::Egypt,
            &MadhabChoice::Shafi,
            &high_latitude_rule,
            &prayer_params,
        );

        let mwl_t = mwl.get_prayer_times(date).unwrap().schedule;
        let egypt_t = egypt.get_prayer_times(date).unwrap().schedule;

        assert_ne!(
            mwl_t.fajr.format("%H:%M").to_string(),
            egypt_t.fajr.format("%H:%M").to_string(),
            "MWL and Egypt Fajr should differ"
        );
    }

    #[test]
    fn moonsighting_committee_high_latitude_reports_fallback_active() {
        // Hammerfest in perpetual night (|lat| > 60°): Moonsighting
        // Committee walks to the nearest working latitude, so the
        // schedule's reference latitude differs from the true one. The
        // fallback indicator must reflect that substitution via
        // reference_latitude(), not a manual polar-estimation pre-pass
        // (which cannot see the committee's own Zone C rules).
        let date = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
        let (high_latitude_rule, prayer_params) = default_params();
        let engine = PrayerEngine::new(
            70.66,
            23.68,
            &CalculationMethod::MoonsightingCommittee,
            &MadhabChoice::Shafi,
            &high_latitude_rule,
            &prayer_params,
        );

        let result = engine.get_prayer_times(date).unwrap();
        assert!(
            result.fallback_active,
            "MC high-latitude substitution must be reported as a fallback"
        );
    }

    #[test]
    fn mawaqit_schedule_builds_real_instant_in_mosque_timezone() {
        let moscow: Tz = "Europe/Moscow".parse().unwrap();
        let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
        let schedule = schedule_from_hm(
            date,
            &[
                "03:03".to_string(),
                "05:00".to_string(),
                "12:15".to_string(),
                "14:30".to_string(),
                "17:00".to_string(),
                "18:30".to_string(),
            ],
            Some(&moscow),
        )
        .expect("schedule");

        // The published wall-clock is preserved in the mosque timezone...
        let fajr_wall_clock = schedule
            .fajr
            .with_timezone(&moscow)
            .format("%H:%M")
            .to_string();
        assert_eq!(fajr_wall_clock, "03:03");
        // ...and the underlying instant is real (Moscow is UTC+3), never
        // shifted by the timezone override regardless of the system zone.
        let fajr_utc = schedule
            .fajr
            .with_timezone(&Utc)
            .format("%H:%M")
            .to_string();
        assert_eq!(fajr_utc, "00:03");
    }

    #[test]
    fn effective_timezone_forces_mosque_zone_under_mawaqit() {
        let config = AppConfig::default();
        config.set_prayer_times_source(PrayerTimesSource::Mawaqit);
        let mut cache = crate::config::MawaqitCache {
            url: "https://mawaqit.net".to_string(),
            mosque_name: None,
            timezone: Some("Europe/Moscow".to_string()),
            latitude: Some(55.75),
            longitude: Some(37.62),
            country_code: None,
            resolved_city: None,
            year: 2026,
            months: Vec::new(),
            fetched_on: String::new(),
        };
        config.set_mawaqit_cache(Some(cache.clone()));

        assert_eq!(
            effective_timezone(&config),
            TimezoneMode::Named("Europe/Moscow".to_string())
        );

        // Without a cache timezone, the user's stored mode is returned.
        cache.timezone = None;
        config.set_mawaqit_cache(Some(cache));
        assert_eq!(effective_timezone(&config), TimezoneMode::Auto);
    }
}
