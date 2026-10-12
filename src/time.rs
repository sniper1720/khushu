use chrono::{DateTime, Datelike, Local, NaiveDate, TimeZone, Timelike, Utc};
use chrono_tz::Tz;
use mawaqit::{Configuration, Coordinates, Madhab, Method, Prayer, PrayerTimes};

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
    pub local_relative_estimation_blocked: bool,
    pub latitude_substitution_active: bool,
    // The provenance of each computed time, copied from mawaqit's
    // PrayerTimes::status at construction. Sunrise, Dhuhr, and Asr have no
    // status arm upstream (they fall through to the crate's `_ => Actual`
    // default), so only the three covered prayers travel; the row builder
    // keys its labels on exactly these, and passes `None` for the rest so a
    // blanket read cannot invent a provenance those rows never had.
    pub fajr_time_status: mawaqit::PrayerTimeStatus,
    pub maghrib_time_status: mawaqit::PrayerTimeStatus,
    pub isha_time_status: mawaqit::PrayerTimeStatus,
}

pub struct PrayerEngine {
    configuration: Configuration,
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

        let configuration = Configuration::from_method(mawaqit_method).madhab(mawaqit_madhab);

        let configuration = if method == &CalculationMethod::Kemenag {
            configuration.fajr_angle(20.0).isha_angle(18.0)
        } else {
            configuration
        };

        let configuration = configuration.polar_estimation(match polar {
            PolarEstimationMethod::NearestLatitude => mawaqit::PolarEstimation::NearestLatitude,
            PolarEstimationMethod::Reference45 => mawaqit::PolarEstimation::Reference45,
        });

        let high_latitude_rule = match high_latitude {
            HighLatitudeChoice::Auto => mawaqit::HighLatitudeRule::Recommended,
            HighLatitudeChoice::MiddleOfTheNight => mawaqit::HighLatitudeRule::MiddleOfTheNight,
            HighLatitudeChoice::SeventhOfTheNight => mawaqit::HighLatitudeRule::SeventhOfTheNight,
            HighLatitudeChoice::TwilightAngle => mawaqit::HighLatitudeRule::TwilightAngle,
            HighLatitudeChoice::LocalRelativeEstimation => {
                mawaqit::HighLatitudeRule::LocalRelativeEstimation
            }
        };

        let configuration = configuration.high_latitude_rule(high_latitude_rule);

        Self {
            configuration,
            location,
            high_latitude: high_latitude.clone(),
        }
    }

    pub fn get_prayer_times(
        &self,
        date: NaiveDate,
    ) -> Result<PrayerResult, mawaqit::PrayerTimesError> {
        let (times, local_relative_estimation_blocked) =
            match PrayerTimes::try_new(date, self.location, self.configuration.build()) {
                Ok(prayer_times) => (prayer_times, false),
                Err(mawaqit::PrayerTimesError::LocalRelativeEstimationNotApplicable)
                    if matches!(
                        self.high_latitude,
                        HighLatitudeChoice::LocalRelativeEstimation
                    ) =>
                {
                    let retry = self
                        .configuration
                        .high_latitude_rule(mawaqit::HighLatitudeRule::Recommended)
                        .build();
                    (PrayerTimes::try_new(date, self.location, retry)?, true)
                }
                Err(err) => return Err(err),
            };

        // latitude_substitution() reports exactly when mawaqit computed away
        // from the caller's latitude, be it a polar-estimation walk, the
        // Moonsighting Committee's Zone C anchor/walk, or a nearest-latitude
        // substitute, plus any local_relative_estimation_blocked retry. We
        // trust the library's final answer rather than re-deriving a
        // substitution ourselves, so the indicator stays honest for every
        // method.
        let latitude_substitution_active = times.latitude_substitution().is_some();

        // Read the statuses only now, from whichever object actually produced
        // the displayed times: the retry, when the LRE rule was blocked. They
        // must land on PrayerResult before schedule_from_times consumes
        // `times` by value below.
        let fajr_time_status = times.status(Prayer::Fajr);
        let maghrib_time_status = times.status(Prayer::Maghrib);
        let isha_time_status = times.status(Prayer::Isha);

        Ok(PrayerResult {
            schedule: self.schedule_from_times(times),
            local_relative_estimation_blocked,
            latitude_substitution_active,
            fajr_time_status,
            maghrib_time_status,
            isha_time_status,
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

pub fn schedule_for_config(config: &AppConfig, date: NaiveDate) -> Result<PrayerResult, String> {
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
                    local_relative_estimation_blocked: false,
                    latitude_substitution_active: false,
                    // A published wall-clock has no provenance to report:
                    // no PrayerTimes object exists to ask, and the true
                    // statement khushu can make is that no rule displaced
                    // these values.
                    fajr_time_status: mawaqit::PrayerTimeStatus::Actual,
                    maghrib_time_status: mawaqit::PrayerTimeStatus::Actual,
                    isha_time_status: mawaqit::PrayerTimeStatus::Actual,
                })
                .ok_or_else(|| "failed to parse cached times".to_string());
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
    .map_err(|err| err.to_string())
    .map(|mut prayer_result| {
        prayer_result.schedule = apply_timezone_override(config, prayer_result.schedule);
        prayer_result
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

/// Translated name of a Hijri month, or `None` when `month` falls outside the
/// 1 to 12 range that the calendar conversion can produce.
///
/// Absence is returned rather than defaulted to an empty string so that no
/// caller can render a date with a silently missing month name.
pub fn hijri_month_name(month: usize) -> Option<String> {
    HIJRI_MONTH_NAMES
        .get(month.checked_sub(1)?)
        .map(|month_name| tr(month_name))
}

/// Display text for the Hijri calendar date, shifted by `hijri_offset` days
/// and shown next to the Gregorian date. Falls back to a placeholder rather
/// than a blank, unlabeled space, because a blank label is indistinguishable
/// from one that has not loaded yet.
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
        Ok(hijri) => match hijri_month_name(hijri.month()) {
            Some(hijri_month) => format!("{} {} {}", hijri.day(), hijri_month, hijri.year()),
            None => tr("Unavailable"),
        },
        Err(err) => {
            log::error!("Failed to calculate Hijri date: {err}");
            tr("Unavailable")
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

    fn default_polar_estimation() -> PolarEstimationMethod {
        PolarEstimationMethod::NearestLatitude
    }

    fn default_estimation_choices() -> (HighLatitudeChoice, PolarEstimationMethod) {
        (HighLatitudeChoice::Auto, default_polar_estimation())
    }

    #[test]
    fn test_prayer_times_calculation() {
        let (high_latitude_rule, polar_estimation_method) = default_estimation_choices();
        let engine = PrayerEngine::new(
            21.4225,
            39.8262,
            &CalculationMethod::Makkah,
            &MadhabChoice::Shafi,
            &high_latitude_rule,
            &polar_estimation_method,
        );
        let date = NaiveDate::from_ymd_opt(2023, 10, 1).unwrap();

        let prayer_result = engine.get_prayer_times(date);
        assert!(prayer_result.is_ok());

        let times = prayer_result.unwrap().schedule;

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
            let (high_latitude_rule, polar_estimation_method) = default_estimation_choices();
            let engine = PrayerEngine::new(
                36.75,
                3.05,
                method,
                &MadhabChoice::Shafi,
                &high_latitude_rule,
                &polar_estimation_method,
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
        let (high_latitude_rule, polar_estimation_method) = default_estimation_choices();
        let shafi = PrayerEngine::new(
            36.75,
            3.05,
            &CalculationMethod::MWL,
            &MadhabChoice::Shafi,
            &high_latitude_rule,
            &polar_estimation_method,
        );
        let hanafi = PrayerEngine::new(
            36.75,
            3.05,
            &CalculationMethod::MWL,
            &MadhabChoice::Hanafi,
            &high_latitude_rule,
            &polar_estimation_method,
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
        let (high_latitude_rule, polar_estimation_method) = default_estimation_choices();
        let engine = PrayerEngine::new(
            36.75,
            3.05,
            &CalculationMethod::MWL,
            &MadhabChoice::Shafi,
            &high_latitude_rule,
            &polar_estimation_method,
        );

        let today = engine.get_prayer_times(date).unwrap().schedule;
        let now = today.isha + chrono::Duration::minutes(1);
        assert!(next_prayer_from_schedule(&today, now).is_none());

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
        let (high_latitude_rule, polar_estimation_method) = default_estimation_choices();
        let mwl = PrayerEngine::new(
            36.75,
            3.05,
            &CalculationMethod::MWL,
            &MadhabChoice::Shafi,
            &high_latitude_rule,
            &polar_estimation_method,
        );
        let egypt = PrayerEngine::new(
            36.75,
            3.05,
            &CalculationMethod::Egypt,
            &MadhabChoice::Shafi,
            &high_latitude_rule,
            &polar_estimation_method,
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
    fn local_relative_estimation_blocking_falls_back_to_recommended() {
        // |lat| > 66.6 with LocalRelativeEstimation requested: mawaqit
        // rejects the rule (Zone 3), so khushu retries with Recommended ->
        // MiddleOfTheNight and the schedule is produced with the
        // local_relative_estimation_blocked flag set.
        let date = NaiveDate::from_ymd_opt(2026, 9, 21).unwrap();
        let polar_estimation_method = default_polar_estimation();
        let engine = PrayerEngine::new(
            67.0,
            25.0,
            &CalculationMethod::MWL,
            &MadhabChoice::Shafi,
            &HighLatitudeChoice::LocalRelativeEstimation,
            &polar_estimation_method,
        );

        let prayer_result = engine
            .get_prayer_times(date)
            .expect("retry must produce a schedule");
        assert!(
            prayer_result.local_relative_estimation_blocked,
            "LocalRelativeEstimation not applicable at |lat| > 66.6 must fall back to Recommended"
        );
        assert!(
            !prayer_result.latitude_substitution_active,
            "rule retry must not move the latitude"
        );
    }

    #[test]
    fn moonsighting_committee_high_latitude_reports_latitude_substitution() {
        // Hammerfest in perpetual night (|lat| > 60°): Moonsighting
        // Committee walks to the nearest working latitude, so the
        // schedule's reference latitude differs from the true one. The
        // substitution indicator must reflect that substitution via
        // latitude_substitution(), not a manual polar-estimation pre-pass
        // (which cannot see the committee's own Zone C rules).
        let date = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
        let (high_latitude_rule, polar_estimation_method) = default_estimation_choices();
        let engine = PrayerEngine::new(
            70.66,
            23.68,
            &CalculationMethod::MoonsightingCommittee,
            &MadhabChoice::Shafi,
            &high_latitude_rule,
            &polar_estimation_method,
        );

        let prayer_result = engine.get_prayer_times(date).unwrap();
        assert!(
            prayer_result.latitude_substitution_active,
            "MC high-latitude substitution must be reported as a substitution"
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

    #[test]
    fn prayer_result_carries_statuses_that_match_the_crate() {
        // Latitude 65 on the summer solstice: the sun never reaches the Fajr
        // or Isha angles, so the crate reports Estimated for both and Actual
        // for Maghrib, with no latitude substitution. prayer_result must
        // carry exactly what the configured calculation reports.
        let date = NaiveDate::from_ymd_opt(2026, 6, 21).unwrap();
        let (high_latitude_rule, polar_estimation_method) = default_estimation_choices();
        let engine = PrayerEngine::new(
            65.0,
            25.0,
            &CalculationMethod::MWL,
            &MadhabChoice::Shafi,
            &high_latitude_rule,
            &polar_estimation_method,
        );

        let prayer_result = engine.get_prayer_times(date).unwrap();

        let reference = PrayerTimes::try_new(date, engine.location, engine.configuration.build())
            .expect("the same configuration must compute times");

        assert_eq!(
            prayer_result.fajr_time_status,
            reference.status(Prayer::Fajr)
        );
        assert_eq!(
            prayer_result.maghrib_time_status,
            reference.status(Prayer::Maghrib)
        );
        assert_eq!(
            prayer_result.isha_time_status,
            reference.status(Prayer::Isha)
        );
        assert_eq!(
            prayer_result.fajr_time_status,
            mawaqit::PrayerTimeStatus::Estimated
        );
        assert_eq!(
            prayer_result.maghrib_time_status,
            mawaqit::PrayerTimeStatus::Actual
        );
    }

    #[test]
    fn lre_blocked_retry_reports_the_retry_object_statuses() {
        // |lat| > 66.6 with LocalRelativeEstimation: mawaqit rejects the
        // rule, khushu retries with Recommended, and the statuses must come
        // from the retried object (read-after-match), not the failed first
        // attempt. At latitude 75 near the equinox the sun reaches Maghrib's
        // angle but never the deeper Fajr or Isha angles.
        let date = NaiveDate::from_ymd_opt(2026, 9, 21).unwrap();
        let polar_estimation_method = default_polar_estimation();
        let engine = PrayerEngine::new(
            75.0,
            25.0,
            &CalculationMethod::MWL,
            &MadhabChoice::Shafi,
            &HighLatitudeChoice::LocalRelativeEstimation,
            &polar_estimation_method,
        );

        let prayer_result = engine
            .get_prayer_times(date)
            .expect("retry must produce a schedule");
        assert!(
            prayer_result.local_relative_estimation_blocked,
            "LocalRelativeEstimation must be blocked above 66.6 on this date"
        );
        assert!(
            !prayer_result.latitude_substitution_active,
            "rule retry must not move the latitude"
        );

        assert_eq!(
            prayer_result.fajr_time_status,
            mawaqit::PrayerTimeStatus::Estimated
        );
        assert_eq!(
            prayer_result.maghrib_time_status,
            mawaqit::PrayerTimeStatus::Actual
        );
        assert_eq!(
            prayer_result.isha_time_status,
            mawaqit::PrayerTimeStatus::Estimated
        );
    }

    #[test]
    fn cached_mosque_schedule_reports_actual_statuses() {
        let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
        let config = AppConfig::default();
        config.set_prayer_times_source(PrayerTimesSource::Mawaqit);
        let mut months: Vec<std::collections::BTreeMap<u32, [String; 6]>> =
            vec![std::collections::BTreeMap::new(); 12];
        months[0].insert(
            15,
            [
                "03:03".to_string(),
                "05:00".to_string(),
                "12:15".to_string(),
                "14:30".to_string(),
                "17:00".to_string(),
                "18:30".to_string(),
            ],
        );
        let cache = crate::config::MawaqitCache {
            url: "https://mawaqit.net".to_string(),
            mosque_name: None,
            timezone: Some("Europe/Moscow".to_string()),
            latitude: Some(55.75),
            longitude: Some(37.62),
            country_code: None,
            resolved_city: None,
            year: 2026,
            months,
            fetched_on: String::new(),
        };
        config.set_mawaqit_cache(Some(cache));

        let prayer_result = schedule_for_config(&config, date).expect("cached schedule");

        assert_eq!(
            prayer_result.fajr_time_status,
            mawaqit::PrayerTimeStatus::Actual
        );
        assert_eq!(
            prayer_result.maghrib_time_status,
            mawaqit::PrayerTimeStatus::Actual
        );
        assert_eq!(
            prayer_result.isha_time_status,
            mawaqit::PrayerTimeStatus::Actual
        );
    }

    #[test]
    fn timezone_override_remaps_schedule_but_not_statuses() {
        let date = NaiveDate::from_ymd_opt(2026, 6, 21).unwrap();
        let plain = AppConfig::default();
        plain.set_latitude(65.0);
        plain.set_longitude(25.0);
        let plain_result = schedule_for_config(&plain, date).expect("computed schedule");

        let shifted = AppConfig::default();
        shifted.set_latitude(65.0);
        shifted.set_longitude(25.0);
        shifted.set_timezone_mode(TimezoneMode::Named("Europe/Moscow".to_string()));
        let shifted_result = schedule_for_config(&shifted, date).expect("computed schedule");

        assert_eq!(
            shifted_result.fajr_time_status,
            plain_result.fajr_time_status
        );
        assert_eq!(
            shifted_result.maghrib_time_status,
            plain_result.maghrib_time_status
        );
        assert_eq!(
            shifted_result.isha_time_status,
            plain_result.isha_time_status
        );
    }
}
