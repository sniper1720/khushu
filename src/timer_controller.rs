use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use chrono::{Duration, Local, NaiveDate};

use crate::adkar;

use crate::config::AppConfig;
use crate::i18n::tr;
use crate::location;
use crate::notifications::show_notification;
use crate::time::{
    PrayerEngine, PrayerSchedule, apply_timezone_override, next_prayer_from_schedule,
};

pub struct PrayerState {
    pub hero_text: String,
    pub hijri_text: String,
    pub location_text: String,
    pub next_prayer_name: String,
    pub adhan_playing: bool,
    pub adhan_prayer_name: Option<String>,
    pub is_iqamah: bool,
}

type IqamahCountdown = Rc<RefCell<Option<(String, chrono::DateTime<chrono::Local>)>>>;

struct DailyState {
    today_schedule: Option<PrayerSchedule>,
    tomorrow_schedule: Option<PrayerSchedule>,
    hijri_text: String,
    location_text: String,
    cache_date: NaiveDate,
}

fn compute_daily_state(config: &AppConfig, engine: &PrayerEngine, today: NaiveDate) -> DailyState {
    let tomorrow = today.succ_opt().unwrap_or(today);
    let language = config.language();
    let now = crate::time::effective_now(config);

    let use_mawaqit = config.prayer_times_source() == crate::config::PrayerTimesSource::Mawaqit;
    let today_schedule = if use_mawaqit {
        crate::time::schedule_for_config(config, today)
            .ok()
            .map(|result| result.schedule)
    } else {
        engine
            .get_prayer_times(today)
            .ok()
            .map(|result| apply_timezone_override(config, result.schedule))
    };
    let tomorrow_schedule = if use_mawaqit {
        crate::time::schedule_for_config(config, tomorrow)
            .ok()
            .map(|result| result.schedule)
    } else {
        engine
            .get_prayer_times(tomorrow)
            .ok()
            .map(|result| apply_timezone_override(config, result.schedule))
    };

    let hijri_text = crate::time::format_hijri_date(now, config.hijri_offset());

    let mawaqit_cache = if use_mawaqit {
        config.mawaqit_cache()
    } else {
        None
    };
    let location_text = location::display_city_label(
        config.city_name().as_deref(),
        mawaqit_cache.as_ref(),
        &language,
    )
    .unwrap_or_else(|| format!("{:.2}, {:.2}", config.latitude(), config.longitude()));

    DailyState {
        today_schedule,
        tomorrow_schedule,
        hijri_text,
        location_text,
        cache_date: today,
    }
}

fn find_next_prayer(
    today_schedule: Option<&PrayerSchedule>,
    tomorrow_schedule: Option<&PrayerSchedule>,
    now: chrono::DateTime<Local>,
) -> Option<(String, chrono::DateTime<Local>)> {
    today_schedule
        .and_then(|schedule| next_prayer_from_schedule(schedule, now))
        .or_else(|| tomorrow_schedule.map(|schedule| ("Fajr".to_string(), schedule.fajr)))
}

const MAWAQIT_REFRESH_INTERVAL_SECONDS: u32 = 3600;

const SECONDS_PER_MINUTE: i64 = 60;
const SECONDS_PER_HOUR: i64 = SECONDS_PER_MINUTE * 60;
const ADHAN_END_QUIET_PERIOD: Duration = Duration::seconds(60);
const MIN_ADHAN_WINDOW: Duration = Duration::seconds(60);
const MORNING_DIKR_1_WINDOW: std::ops::Range<Duration> = Duration::minutes(1)..Duration::minutes(6);
const MORNING_DIKR_2_WINDOW: std::ops::Range<Duration> =
    Duration::minutes(30)..Duration::minutes(31);
const EVENING_DIKR_1_WINDOW: std::ops::Range<Duration> =
    Duration::minutes(15)..Duration::minutes(16);
const EVENING_DIKR_2_WINDOW: std::ops::Range<Duration> =
    Duration::minutes(45)..Duration::minutes(46);
const NIGHT_DIKR_1_WINDOW: std::ops::Range<Duration> = Duration::minutes(30)..Duration::minutes(31);
const NIGHT_DIKR_2_WINDOW: std::ops::Range<Duration> = Duration::minutes(60)..Duration::minutes(61);

fn mawaqit_cache_fetched_today(
    cache: Option<&crate::config::MawaqitCache>,
    today: NaiveDate,
) -> bool {
    cache.is_some_and(|cache| cache.fetched_on == today.to_string())
}

fn assemble_prayer_state(
    next: Option<(String, chrono::DateTime<Local>)>,
    now: chrono::DateTime<Local>,
    hijri_text: String,
    location_text: String,
    adhan_playing: bool,
    adhan_prayer_name: Option<String>,
    iqamah_hero: Option<String>,
) -> PrayerState {
    match next {
        Some((name, time)) => {
            let duration = time.signed_duration_since(now);
            let hero_text = if duration.num_seconds() > 0 {
                let total_seconds = duration.num_seconds();
                format!(
                    "{} {} {:02}:{:02}:{:02}",
                    tr(&name),
                    tr("in"),
                    total_seconds / SECONDS_PER_HOUR,
                    (total_seconds % SECONDS_PER_HOUR) / SECONDS_PER_MINUTE,
                    total_seconds % SECONDS_PER_MINUTE,
                )
            } else {
                format!("{} {}", tr("It's time for"), tr(&name))
            };
            let is_iqamah = iqamah_hero.is_some();
            PrayerState {
                hero_text: iqamah_hero.unwrap_or(hero_text),
                hijri_text,
                location_text,
                next_prayer_name: name,
                adhan_playing,
                adhan_prayer_name,
                is_iqamah,
            }
        }
        None => PrayerState {
            hero_text: tr("Prayer times unavailable — retrying"),
            hijri_text,
            location_text,
            next_prayer_name: String::new(),
            adhan_playing,
            adhan_prayer_name,
            is_iqamah: false,
        },
    }
}

fn adkar_due<'a>(
    dikrs: &'a [adkar::Dikr],
    index: usize,
    elapsed: Duration,
    threshold: std::ops::Range<Duration>,
    sent: &mut Option<NaiveDate>,
    today: NaiveDate,
) -> Option<&'a adkar::Dikr> {
    if threshold.contains(&elapsed) && *sent != Some(today) {
        *sent = Some(today);
        dikrs.get(index)
    } else {
        None
    }
}

pub fn start_prayer_timer(config: AppConfig, on_state: impl Fn(PrayerState) + 'static) {
    use std::sync::atomic::{AtomicBool, Ordering};
    static HAS_CORE_TIMER: AtomicBool = AtomicBool::new(false);
    let is_core_timer = !HAS_CORE_TIMER.swap(true, Ordering::SeqCst);

    let prayers_handled: Rc<RefCell<HashSet<String>>> = Rc::new(RefCell::new(HashSet::new()));
    let upcoming_notified_at: Rc<RefCell<Option<chrono::DateTime<Local>>>> =
        Rc::new(RefCell::new(None));
    let adhan_for_prayer: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
    let iqamah_countdown: IqamahCountdown = Rc::new(RefCell::new(None));
    let iqamah_notified_at: Rc<RefCell<Option<chrono::DateTime<Local>>>> =
        Rc::new(RefCell::new(None));
    let adhan_ended_at: Rc<RefCell<Option<chrono::DateTime<Local>>>> = Rc::new(RefCell::new(None));

    struct TodayAdkar {
        date: NaiveDate,
        morning: Vec<crate::adkar::Dikr>,
        evening: Vec<crate::adkar::Dikr>,
        night: Vec<crate::adkar::Dikr>,
    }

    let default_date = Local::now().naive_local().date() - Duration::days(1);
    let today_adkar = Rc::new(RefCell::new(TodayAdkar {
        date: default_date,
        morning: vec![],
        evening: vec![],
        night: vec![],
    }));

    let morning_dikr_1_sent: Rc<RefCell<Option<NaiveDate>>> = Rc::new(RefCell::new(None));
    let morning_dikr_2_sent: Rc<RefCell<Option<NaiveDate>>> = Rc::new(RefCell::new(None));
    let evening_dikr_1_sent: Rc<RefCell<Option<NaiveDate>>> = Rc::new(RefCell::new(None));
    let evening_dikr_2_sent: Rc<RefCell<Option<NaiveDate>>> = Rc::new(RefCell::new(None));
    let night_dikr_1_sent: Rc<RefCell<Option<NaiveDate>>> = Rc::new(RefCell::new(None));
    let night_dikr_2_sent: Rc<RefCell<Option<NaiveDate>>> = Rc::new(RefCell::new(None));

    let engine_cache: Rc<RefCell<Option<PrayerEngine>>> = Rc::new(RefCell::new(None));
    let daily_state: Rc<RefCell<Option<DailyState>>> = Rc::new(RefCell::new(None));

    let engine_stale: Rc<RefCell<bool>> = Rc::new(RefCell::new(true));
    let schedule_stale: Rc<RefCell<bool>> = Rc::new(RefCell::new(true));

    {
        let engine_stale_c = engine_stale.clone();
        let schedule_stale_c = schedule_stale.clone();
        crate::connect_notify_blocked(&config, Some("latitude"), move |_, _| {
            *engine_stale_c.borrow_mut() = true;
            *schedule_stale_c.borrow_mut() = true;
        });
    }
    {
        let engine_stale_c = engine_stale.clone();
        let schedule_stale_c = schedule_stale.clone();
        crate::connect_notify_blocked(&config, Some("longitude"), move |_, _| {
            *engine_stale_c.borrow_mut() = true;
            *schedule_stale_c.borrow_mut() = true;
        });
    }
    {
        let engine_stale_c = engine_stale.clone();
        let schedule_stale_c = schedule_stale.clone();
        crate::connect_notify_blocked(&config, Some("method"), move |_, _| {
            *engine_stale_c.borrow_mut() = true;
            *schedule_stale_c.borrow_mut() = true;
        });
    }
    {
        let engine_stale_c = engine_stale.clone();
        let schedule_stale_c = schedule_stale.clone();
        crate::connect_notify_blocked(&config, Some("madhab"), move |_, _| {
            *engine_stale_c.borrow_mut() = true;
            *schedule_stale_c.borrow_mut() = true;
        });
    }
    {
        let schedule_stale_c = schedule_stale.clone();
        crate::connect_notify_blocked(&config, Some("language"), move |_, _| {
            *schedule_stale_c.borrow_mut() = true;
        });
    }
    {
        let schedule_stale_c = schedule_stale.clone();
        crate::connect_notify_blocked(&config, Some("city-name"), move |_, _| {
            *schedule_stale_c.borrow_mut() = true;
        });
    }
    {
        let schedule_stale_c = schedule_stale.clone();
        crate::connect_notify_blocked(&config, Some("prayer-times-source"), move |_, _| {
            *schedule_stale_c.borrow_mut() = true;
        });
    }
    {
        let schedule_stale_c = schedule_stale.clone();
        crate::connect_notify_blocked(&config, Some("timezone-mode"), move |_, _| {
            *schedule_stale_c.borrow_mut() = true;
        });
    }

    let mawaqit_config = config.clone();
    let mawaqit_state = daily_state.clone();
    let attempt_mawaqit_refresh = move || {
        if mawaqit_config.prayer_times_source() != crate::config::PrayerTimesSource::Mawaqit
            || !mawaqit_config.mawaqit_auto_refresh_daily()
        {
            return;
        }
        let Some(url) = mawaqit_config.mawaqit_url() else {
            return;
        };
        let today = crate::time::effective_today(&mawaqit_config);
        if mawaqit_cache_fetched_today(mawaqit_config.mawaqit_cache().as_ref(), today) {
            return;
        }
        let config_for_mawaqit = mawaqit_config.clone();
        let daily_state_rc = mawaqit_state.clone();
        gtk4::glib::spawn_future_local(async move {
            if let Ok(cache) = crate::mawaqit::fetch_mawaqit_cache(&url).await {
                config_for_mawaqit.set_mawaqit_cache(Some(cache.clone()));
                config_for_mawaqit.set_mawaqit_url(Some(cache.url.clone()));
                config_for_mawaqit.save();
                *daily_state_rc.borrow_mut() = None;
            }
        });
    };
    attempt_mawaqit_refresh();
    gtk4::glib::timeout_add_seconds_local(MAWAQIT_REFRESH_INTERVAL_SECONDS, move || {
        attempt_mawaqit_refresh();
        gtk4::glib::ControlFlow::Continue
    });

    gtk4::glib::timeout_add_seconds_local(1, move || {
        if *engine_stale.borrow() {
            let engine = PrayerEngine::new(
                config.latitude(),
                config.longitude(),
                &config.method(),
                &config.madhab(),
                &config.high_latitude_rule(),
                &config.polar_estimation_method(),
            );
            *engine_cache.borrow_mut() = Some(engine);
            *engine_stale.borrow_mut() = false;
        }

        let engine_guard = engine_cache.borrow();
        let Some(engine) = engine_guard.as_ref() else {
            drop(engine_guard);
            return gtk4::glib::ControlFlow::Continue;
        };
        let today = crate::time::effective_today(&config);
        let language = crate::i18n::supported_language_code(&config.language());

        let mut state_guard = daily_state.borrow_mut();
        let schedule_changed = *schedule_stale.borrow()
            || state_guard
                .as_ref()
                .map(|state| state.cache_date != today)
                .unwrap_or(true);
        if schedule_changed {
            let fresh = compute_daily_state(&config, engine, today);
            *state_guard = Some(fresh);
            *schedule_stale.borrow_mut() = false;
        }
        let hijri_text = state_guard
            .as_ref()
            .map(|state| state.hijri_text.clone())
            .unwrap_or_default();
        let location_text = state_guard
            .as_ref()
            .map(|state| state.location_text.clone())
            .unwrap_or_default();
        let today_schedule = state_guard
            .as_ref()
            .and_then(|state| state.today_schedule.clone());
        let tomorrow_schedule = state_guard
            .as_ref()
            .and_then(|state| state.tomorrow_schedule.clone());
        drop(state_guard);
        drop(engine_guard);

        let now = crate::time::effective_now(&config);

        if schedule_changed {
            prayers_handled.borrow_mut().clear();
        }

        let next = find_next_prayer(today_schedule.as_ref(), tomorrow_schedule.as_ref(), now);

        let adhan_playing = crate::audio::is_adhan();
        let adhan_was_playing = adhan_for_prayer.borrow().is_some();
        if !adhan_playing && adhan_was_playing {
            *adhan_ended_at.borrow_mut() = Some(now);
        }
        if !adhan_playing {
            *adhan_for_prayer.borrow_mut() = None;
        }
        let adhan_ended = adhan_ended_at
            .borrow()
            .is_none_or(|ended_at| now.signed_duration_since(ended_at) >= ADHAN_END_QUIET_PERIOD);

        if let Some((name, time)) = next.as_ref() {
            let total_seconds = time.signed_duration_since(now).num_seconds();

            if is_core_timer
                && config.pre_prayer_notify()
                && !config.adhan_only_mode()
                && total_seconds > 0
                && total_seconds <= config.pre_prayer_minutes() as i64 * SECONDS_PER_MINUTE
                && name != "Sunrise"
                && upcoming_notified_at
                    .borrow()
                    .is_none_or(|notified_time| notified_time < *time)
            {
                show_notification(
                    &format!("{} {}", tr("Upcoming Prayer:"), tr(name)),
                    &format!(
                        "{} {} {} {}",
                        tr(name),
                        tr("is in"),
                        config.pre_prayer_minutes(),
                        tr("minutes")
                    ),
                    false,
                    &tr("Open Khushu"),
                    &tr("Stop Adhan"),
                );
                *upcoming_notified_at.borrow_mut() = Some(*time);
            }

            if is_core_timer {
                let mut handled = prayers_handled.borrow_mut();
                if let Some(ref scan_schedule) = today_schedule {
                    for (scan_name, scan_time) in [
                        ("Fajr", scan_schedule.fajr),
                        ("Dhuhr", scan_schedule.dhuhr),
                        ("Asr", scan_schedule.asr),
                        ("Maghrib", scan_schedule.maghrib),
                        ("Isha", scan_schedule.isha),
                    ] {
                        if now >= scan_time && !handled.contains(scan_name) {
                            let iqamah_mins = config
                                .iqamah_minutes()
                                .get(scan_name)
                                .copied()
                                .unwrap_or(10) as i64;
                            let iqamah_end = scan_time + Duration::minutes(iqamah_mins);
                            if now < iqamah_end {
                                *iqamah_countdown.borrow_mut() =
                                    Some((scan_name.to_string(), iqamah_end));
                                *iqamah_notified_at.borrow_mut() = None;
                            }

                            let adhan_window = Duration::minutes(iqamah_mins).max(MIN_ADHAN_WINDOW);
                            if now.signed_duration_since(scan_time) < adhan_window {
                                show_notification(
                                    &format!("{} {}", tr("It's time for"), tr(scan_name)),
                                    &format!("{} {}.", tr("It is now time for"), tr(scan_name)),
                                    true,
                                    &tr("Open Khushu"),
                                    &tr("Stop Adhan"),
                                );

                                let adhan_path = config
                                    .adhan_sound_path()
                                    .unwrap_or_else(|| "assets/audio/Madinah.mp3".to_string());
                                if !config.adhan_muted() {
                                    crate::audio::play_adhan(&adhan_path, config.adhan_volume());
                                    *adhan_for_prayer.borrow_mut() = Some(scan_name.to_string());
                                }
                            }
                            handled.insert(scan_name.to_string());
                        }
                    }
                }
            }

            if is_core_timer && config.adkar_notification_enabled() && !config.adhan_only_mode() {
                let mut lists = today_adkar.borrow_mut();
                if lists.date != today {
                    let adkar_set = adkar::get_adkar(&language);
                    let favorites = config.favorites();
                    lists.morning =
                        adkar_set.daily_picks(adkar::DikrCategory::Morning, 2, &favorites);
                    lists.evening =
                        adkar_set.daily_picks(adkar::DikrCategory::Evening, 2, &favorites);
                    lists.night = adkar_set.daily_picks(adkar::DikrCategory::Night, 2, &favorites);
                    lists.date = today;
                }
                drop(lists);

                if adhan_ended && let Some(schedule) = today_schedule.as_ref() {
                    let fajr_elapsed = now.signed_duration_since(schedule.fajr);
                    let asr_elapsed = now.signed_duration_since(schedule.asr);
                    let isha_elapsed = now.signed_duration_since(schedule.isha);

                    let adkar_lists = today_adkar.borrow();

                    if let Some(dikr) = adkar_due(
                        &adkar_lists.morning,
                        0,
                        fajr_elapsed,
                        MORNING_DIKR_1_WINDOW,
                        &mut morning_dikr_1_sent.borrow_mut(),
                        today,
                    ) {
                        let body = if language == "ar" {
                            &dikr.arabic
                        } else {
                            &dikr.translation
                        };
                        show_notification(
                            &tr("Morning Adkar"),
                            body,
                            false,
                            &tr("Open Khushu"),
                            &tr("Stop Adhan"),
                        );
                    }
                    if let Some(dikr) = adkar_due(
                        &adkar_lists.morning,
                        1,
                        fajr_elapsed,
                        MORNING_DIKR_2_WINDOW,
                        &mut morning_dikr_2_sent.borrow_mut(),
                        today,
                    ) {
                        let body = if language == "ar" {
                            &dikr.arabic
                        } else {
                            &dikr.translation
                        };
                        show_notification(
                            &tr("Morning Adkar"),
                            body,
                            false,
                            &tr("Open Khushu"),
                            &tr("Stop Adhan"),
                        );
                    }

                    if let Some(dikr) = adkar_due(
                        &adkar_lists.evening,
                        0,
                        asr_elapsed,
                        EVENING_DIKR_1_WINDOW,
                        &mut evening_dikr_1_sent.borrow_mut(),
                        today,
                    ) {
                        let body = if language == "ar" {
                            &dikr.arabic
                        } else {
                            &dikr.translation
                        };
                        show_notification(
                            &tr("Evening Adkar"),
                            body,
                            false,
                            &tr("Open Khushu"),
                            &tr("Stop Adhan"),
                        );
                    }
                    if let Some(dikr) = adkar_due(
                        &adkar_lists.evening,
                        1,
                        asr_elapsed,
                        EVENING_DIKR_2_WINDOW,
                        &mut evening_dikr_2_sent.borrow_mut(),
                        today,
                    ) {
                        let body = if language == "ar" {
                            &dikr.arabic
                        } else {
                            &dikr.translation
                        };
                        show_notification(
                            &tr("Evening Adkar"),
                            body,
                            false,
                            &tr("Open Khushu"),
                            &tr("Stop Adhan"),
                        );
                    }

                    if let Some(dikr) = adkar_due(
                        &adkar_lists.night,
                        0,
                        isha_elapsed,
                        NIGHT_DIKR_1_WINDOW,
                        &mut night_dikr_1_sent.borrow_mut(),
                        today,
                    ) {
                        let body = if language == "ar" {
                            &dikr.arabic
                        } else {
                            &dikr.translation
                        };
                        show_notification(
                            &tr("Night Adkar"),
                            body,
                            false,
                            &tr("Open Khushu"),
                            &tr("Stop Adhan"),
                        );
                    }
                    if let Some(dikr) = adkar_due(
                        &adkar_lists.night,
                        1,
                        isha_elapsed,
                        NIGHT_DIKR_2_WINDOW,
                        &mut night_dikr_2_sent.borrow_mut(),
                        today,
                    ) {
                        let body = if language == "ar" {
                            &dikr.arabic
                        } else {
                            &dikr.translation
                        };
                        show_notification(
                            &tr("Night Adkar"),
                            body,
                            false,
                            &tr("Open Khushu"),
                            &tr("Stop Adhan"),
                        );
                    }
                }
            }

            if is_core_timer && adhan_ended {
                let should_notify = {
                    let state = iqamah_countdown.borrow();
                    state.as_ref().is_some_and(|(_iq_name, iq_end)| {
                        let remaining = iq_end.signed_duration_since(now).num_seconds();
                        remaining <= 0
                            && iqamah_notified_at
                                .borrow()
                                .is_none_or(|notified_time| notified_time < *iq_end)
                    })
                };
                if should_notify
                    && config.iqamah_notify()
                    && !config.adhan_only_mode()
                    && let Some((iq_name, iq_end)) = iqamah_countdown.borrow().as_ref().cloned()
                {
                    show_notification(
                        &format!("{} {}", tr("Iqamah"), tr(&iq_name)),
                        &format!("{} {}.", tr("It is time for Iqamah of"), tr(&iq_name)),
                        false,
                        &tr("Open Khushu"),
                        &tr("Stop Adhan"),
                    );
                    *iqamah_notified_at.borrow_mut() = Some(iq_end);
                }
            }
        }

        let iqamah_hero = {
            let state = iqamah_countdown.borrow();
            state.as_ref().and_then(|(iq_name, iq_end)| {
                let remaining = iq_end.signed_duration_since(now);
                if remaining.num_seconds() > 0 {
                    let remaining_minutes = remaining.num_minutes();
                    let remaining_seconds = remaining.num_seconds() % SECONDS_PER_MINUTE;
                    Some(format!(
                        "{} {} {:02}:{:02}",
                        tr("Iqamah"),
                        tr(iq_name),
                        remaining_minutes,
                        remaining_seconds
                    ))
                } else {
                    None
                }
            })
        };

        on_state(assemble_prayer_state(
            next,
            now,
            hijri_text,
            location_text,
            adhan_playing,
            adhan_for_prayer.borrow().clone(),
            iqamah_hero,
        ));

        gtk4::glib::ControlFlow::Continue
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use adkar::Dikr;

    fn make_dikr(arabic: &str, translation: &str) -> Dikr {
        Dikr {
            id: String::new(),
            category: adkar::DikrCategory::Morning,
            count: 0,
            count_display: None,
            arabic: arabic.to_string(),
            translation: translation.to_string(),
            reference: String::new(),
        }
    }

    #[test]
    fn adkar_due_returns_none_when_not_in_threshold() {
        let dikrs = vec![make_dikr("صباح الخير", "Good morning")];
        let mut sent = None;

        let result = adkar_due(
            &dikrs,
            0,
            Duration::seconds(5),
            Duration::seconds(10)..Duration::seconds(20),
            &mut sent,
            NaiveDate::from_ymd_opt(2026, 7, 29).unwrap(),
        );

        assert!(result.is_none());
        assert_eq!(sent, None);
    }

    #[test]
    fn adkar_due_returns_dikr_when_in_threshold() {
        let dikrs = vec![make_dikr("صباح الخير", "Good morning")];
        let mut sent = None;
        let today = NaiveDate::from_ymd_opt(2026, 7, 29).unwrap();

        let result = adkar_due(
            &dikrs,
            0,
            Duration::seconds(15),
            Duration::seconds(10)..Duration::seconds(20),
            &mut sent,
            today,
        );

        assert!(result.is_some());
        assert_eq!(result.unwrap().arabic, "صباح الخير");
        assert_eq!(sent, Some(today));
    }

    #[test]
    fn adkar_due_returns_none_after_already_sent_today() {
        let dikrs = vec![make_dikr("صباح الخير", "Good morning")];
        let today = NaiveDate::from_ymd_opt(2026, 7, 29).unwrap();
        let mut sent = Some(today);

        let result = adkar_due(
            &dikrs,
            0,
            Duration::seconds(15),
            Duration::seconds(10)..Duration::seconds(20),
            &mut sent,
            today,
        );

        assert!(result.is_none());
        assert_eq!(sent, Some(today));
    }

    #[test]
    fn adkar_due_returns_none_for_out_of_bounds_index() {
        let dikrs = vec![make_dikr("صباح الخير", "Good morning")];
        let mut sent = None;
        let today = NaiveDate::from_ymd_opt(2026, 7, 29).unwrap();

        let result = adkar_due(
            &dikrs,
            5,
            Duration::seconds(15),
            Duration::seconds(10)..Duration::seconds(20),
            &mut sent,
            today,
        );

        assert!(result.is_none());
    }

    #[test]
    fn adkar_due_returns_second_dikr_for_index_1() {
        let dikrs = vec![make_dikr("الأول", "First"), make_dikr("الثاني", "Second")];
        let mut sent = None;
        let today = NaiveDate::from_ymd_opt(2026, 7, 29).unwrap();

        let result = adkar_due(
            &dikrs,
            1,
            Duration::seconds(15),
            Duration::seconds(10)..Duration::seconds(20),
            &mut sent,
            today,
        );

        assert!(result.is_some());
        assert_eq!(result.unwrap().arabic, "الثاني");
    }

    #[test]
    fn adkar_due_resets_next_day() {
        let dikrs = vec![make_dikr("صباح الخير", "Good morning")];
        let yesterday = NaiveDate::from_ymd_opt(2026, 7, 28).unwrap();
        let today = NaiveDate::from_ymd_opt(2026, 7, 29).unwrap();
        let mut sent = Some(yesterday);

        let result = adkar_due(
            &dikrs,
            0,
            Duration::seconds(15),
            Duration::seconds(10)..Duration::seconds(20),
            &mut sent,
            today,
        );

        assert!(result.is_some());
        assert_eq!(sent, Some(today));
    }

    #[test]
    fn find_next_prayer_returns_none_when_both_schedules_missing() {
        let result = find_next_prayer(None, None, Local::now());
        assert!(result.is_none());
    }

    #[test]
    fn find_next_prayer_falls_back_to_tomorrow_fajr_when_today_exhausted() {
        let now = Local::now();

        let today = PrayerSchedule {
            fajr: now - Duration::hours(12),
            shurooq: now - Duration::hours(11),
            dhuhr: now - Duration::hours(6),
            asr: now - Duration::hours(3),
            maghrib: now - Duration::hours(1),
            isha: now - Duration::minutes(30),
        };

        let tomorrow = PrayerSchedule {
            fajr: now + Duration::hours(5),
            shurooq: now + Duration::hours(6),
            dhuhr: now + Duration::hours(10),
            asr: now + Duration::hours(13),
            maghrib: now + Duration::hours(15),
            isha: now + Duration::hours(16),
        };

        let result = find_next_prayer(Some(&today), Some(&tomorrow), now);

        assert!(result.is_some());
        assert_eq!(result.unwrap().0, "Fajr");
    }

    #[test]
    fn find_next_prayer_returns_none_when_today_exhausted_and_no_tomorrow() {
        let now = Local::now();
        let today = PrayerSchedule {
            fajr: now - Duration::hours(12),
            shurooq: now - Duration::hours(11),
            dhuhr: now - Duration::hours(6),
            asr: now - Duration::hours(3),
            maghrib: now - Duration::hours(1),
            isha: now - Duration::minutes(30),
        };

        let result = find_next_prayer(Some(&today), None, now);

        assert!(result.is_none());
    }

    fn make_mawaqit_cache(fetched_on: &str) -> crate::config::MawaqitCache {
        crate::config::MawaqitCache {
            url: String::new(),
            mosque_name: None,
            timezone: None,
            latitude: None,
            longitude: None,
            country_code: None,
            year: 2026,
            months: vec![],
            fetched_on: fetched_on.to_string(),
        }
    }

    #[test]
    fn mawaqit_cache_fetched_today_matches_cache_date() {
        let today = NaiveDate::from_ymd_opt(2026, 8, 11).unwrap();
        assert!(mawaqit_cache_fetched_today(
            Some(&make_mawaqit_cache("2026-08-11")),
            today
        ));
        assert!(!mawaqit_cache_fetched_today(
            Some(&make_mawaqit_cache("2026-08-10")),
            today
        ));
        assert!(!mawaqit_cache_fetched_today(None, today));
    }

    #[test]
    fn assemble_prayer_state_returns_fallback_when_next_is_none() {
        let state = assemble_prayer_state(
            None,
            Local::now(),
            "hijri".to_string(),
            "location".to_string(),
            false,
            None,
            None,
        );

        assert_eq!(state.hero_text, tr("Prayer times unavailable — retrying"));
        assert_eq!(state.next_prayer_name, "");
        assert!(!state.is_iqamah);
    }

    #[test]
    fn assemble_prayer_state_fallback_preserves_adhan_state() {
        let state = assemble_prayer_state(
            None,
            Local::now(),
            String::new(),
            String::new(),
            true,
            Some("Fajr".to_string()),
            None,
        );

        assert!(state.adhan_playing);
        assert_eq!(state.adhan_prayer_name.as_deref(), Some("Fajr"));
    }

    #[test]
    fn assemble_prayer_state_renders_countdown_when_next_exists() {
        let now = Local::now();
        let next = (String::from("Dhuhr"), now + Duration::minutes(90));

        let state = assemble_prayer_state(
            Some(next),
            now,
            String::new(),
            String::new(),
            false,
            None,
            None,
        );

        assert_eq!(state.next_prayer_name, "Dhuhr");
        assert_eq!(
            state.hero_text,
            format!("{} {} {:02}:{:02}:{:02}", tr("Dhuhr"), tr("in"), 1, 30, 0)
        );
        assert!(!state.is_iqamah);
    }

    #[test]
    fn assemble_prayer_state_prefers_iqamah_hero_when_active() {
        let now = Local::now();
        let next = (String::from("Dhuhr"), now + Duration::minutes(90));
        let iqamah_hero = String::from("Iqamah Dhuhr 00:05");

        let state = assemble_prayer_state(
            Some(next),
            now,
            String::new(),
            String::new(),
            false,
            None,
            Some(iqamah_hero.clone()),
        );

        assert_eq!(state.hero_text, iqamah_hero);
        assert!(state.is_iqamah);
    }
}
