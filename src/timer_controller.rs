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
    let lang = config.language();
    let now = crate::time::effective_now(config);

    let use_mawaqit = config.prayer_times_source() == crate::config::PrayerTimesSource::Mawaqit;
    let today_schedule = if use_mawaqit {
        crate::time::schedule_for_config(config, today)
            .ok()
            .map(|r| r.schedule)
    } else {
        engine
            .get_prayer_times(today)
            .ok()
            .map(|r| apply_timezone_override(config, r.schedule))
    };
    let tomorrow_schedule = if use_mawaqit {
        crate::time::schedule_for_config(config, tomorrow)
            .ok()
            .map(|r| r.schedule)
    } else {
        engine
            .get_prayer_times(tomorrow)
            .ok()
            .map(|r| apply_timezone_override(config, r.schedule))
    };

    let hijri_text = crate::time::format_hijri_date(now, config.hijri_offset());

    let mawaqit_cache = if use_mawaqit {
        config.mawaqit_cache()
    } else {
        None
    };
    let location_text =
        location::display_city_label(config.city_name().as_deref(), mawaqit_cache.as_ref(), &lang)
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
        .and_then(|s| next_prayer_from_schedule(s, now))
        .or_else(|| tomorrow_schedule.map(|s| ("Fajr".to_string(), s.fajr)))
}

fn adkar_due<'a>(
    dikrs: &'a [adkar::Dikr],
    index: usize,
    elapsed: i64,
    threshold: std::ops::Range<i64>,
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
    let last_mawaqit_attempt_day: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
    let daily_state: Rc<RefCell<Option<DailyState>>> = Rc::new(RefCell::new(None));

    let engine_stale: Rc<RefCell<bool>> = Rc::new(RefCell::new(true));
    let schedule_stale: Rc<RefCell<bool>> = Rc::new(RefCell::new(true));

    {
        let engine_stale = engine_stale.clone();
        let schedule_stale = schedule_stale.clone();
        crate::connect_notify_blocked(&config, Some("latitude"), move |_, _| {
            *engine_stale.borrow_mut() = true;
            *schedule_stale.borrow_mut() = true;
        });
    }
    {
        let engine_stale = engine_stale.clone();
        let schedule_stale = schedule_stale.clone();
        crate::connect_notify_blocked(&config, Some("longitude"), move |_, _| {
            *engine_stale.borrow_mut() = true;
            *schedule_stale.borrow_mut() = true;
        });
    }
    {
        let engine_stale = engine_stale.clone();
        let schedule_stale = schedule_stale.clone();
        crate::connect_notify_blocked(&config, Some("method"), move |_, _| {
            *engine_stale.borrow_mut() = true;
            *schedule_stale.borrow_mut() = true;
        });
    }
    {
        let engine_stale = engine_stale.clone();
        let schedule_stale = schedule_stale.clone();
        crate::connect_notify_blocked(&config, Some("madhab"), move |_, _| {
            *engine_stale.borrow_mut() = true;
            *schedule_stale.borrow_mut() = true;
        });
    }
    {
        let schedule_stale = schedule_stale.clone();
        crate::connect_notify_blocked(&config, Some("language"), move |_, _| {
            *schedule_stale.borrow_mut() = true;
        });
    }
    {
        let schedule_stale = schedule_stale.clone();
        crate::connect_notify_blocked(&config, Some("city-name"), move |_, _| {
            *schedule_stale.borrow_mut() = true;
        });
    }
    {
        let schedule_stale = schedule_stale.clone();
        crate::connect_notify_blocked(&config, Some("prayer-times-source"), move |_, _| {
            *schedule_stale.borrow_mut() = true;
        });
    }
    {
        let schedule_stale = schedule_stale.clone();
        crate::connect_notify_blocked(&config, Some("timezone-mode"), move |_, _| {
            *schedule_stale.borrow_mut() = true;
        });
    }
    {
        let schedule_stale = schedule_stale.clone();
        crate::connect_notify_blocked(&config, Some("timezone-override-minutes"), move |_, _| {
            *schedule_stale.borrow_mut() = true;
        });
    }

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
        let lang = config.language();

        let mut state_guard = daily_state.borrow_mut();
        let schedule_changed = *schedule_stale.borrow()
            || state_guard
                .as_ref()
                .map(|s| s.cache_date != today)
                .unwrap_or(true);
        if schedule_changed {
            let fresh = compute_daily_state(&config, engine, today);
            *state_guard = Some(fresh);
            *schedule_stale.borrow_mut() = false;
        }
        let hijri_text = state_guard
            .as_ref()
            .map(|s| s.hijri_text.clone())
            .unwrap_or_default();
        let location_text = state_guard
            .as_ref()
            .map(|s| s.location_text.clone())
            .unwrap_or_default();
        let today_schedule = state_guard.as_ref().and_then(|s| s.today_schedule.clone());
        let tomorrow_schedule = state_guard
            .as_ref()
            .and_then(|s| s.tomorrow_schedule.clone());
        drop(state_guard);
        drop(engine_guard);

        let now = crate::time::effective_now(&config);

        if schedule_changed {
            prayers_handled.borrow_mut().clear();
        }

        if config.prayer_times_source() == crate::config::PrayerTimesSource::Mawaqit
            && config.mawaqit_auto_refresh_daily()
            && let Some(url) = config.mawaqit_url()
        {
            let today_s = today.to_string();
            let fetched_today = config
                .mawaqit_cache()
                .as_ref()
                .map(|c| c.fetched_on.as_str() == today_s.as_str())
                .unwrap_or(false);
            let already_tried_today = last_mawaqit_attempt_day
                .borrow()
                .as_deref()
                .is_some_and(|d| d == today_s.as_str());
            if !fetched_today && !already_tried_today {
                *last_mawaqit_attempt_day.borrow_mut() = Some(today_s.clone());
                let cfg = config.clone();
                let state_rc = daily_state.clone();
                gtk4::glib::spawn_future_local(async move {
                    if let Ok(cache) = crate::mawaqit::fetch_mawaqit_cache(&url).await {
                        cfg.set_mawaqit_cache(Some(cache.clone()));
                        cfg.set_mawaqit_url(Some(cache.url.clone()));
                        cfg.save();
                        *state_rc.borrow_mut() = None;
                    }
                });
            }
        }

        let next = find_next_prayer(today_schedule.as_ref(), tomorrow_schedule.as_ref(), now);

        let adhan_playing = crate::audio::is_playing();
        let adhan_was_playing = adhan_for_prayer.borrow().is_some();
        if !adhan_playing && adhan_was_playing {
            *adhan_ended_at.borrow_mut() = Some(now);
        }
        if !adhan_playing {
            *adhan_for_prayer.borrow_mut() = None;
        }
        let adhan_ended = adhan_ended_at
            .borrow()
            .is_none_or(|t| now.signed_duration_since(t) >= Duration::seconds(60));

        if let Some((name, time)) = next {
            let duration = time.signed_duration_since(now);
            let total_seconds = duration.num_seconds();
            let hours = duration.num_hours();
            let minutes = (duration.num_minutes() % 60).abs();
            let seconds = (duration.num_seconds() % 60).abs();

            let hero_text = if total_seconds > 0 {
                format!(
                    "{} {} {:02}:{:02}:{:02}",
                    tr(&name),
                    tr("in"),
                    hours,
                    minutes,
                    seconds
                )
            } else {
                format!("{} {}", tr("It's time for"), tr(&name))
            };

            if is_core_timer
                && config.pre_prayer_notify()
                && !config.adhan_only_mode()
                && total_seconds > 0
                && total_seconds <= (config.pre_prayer_minutes() as i64 * 60)
                && name != "Sunrise"
                && upcoming_notified_at.borrow().is_none_or(|t| t < time)
            {
                show_notification(
                    &format!("{} {}", tr("Upcoming Prayer:"), tr(&name)),
                    &format!(
                        "{} {} {} {}",
                        tr(&name),
                        tr("is in"),
                        config.pre_prayer_minutes(),
                        tr("minutes")
                    ),
                    false,
                    &tr("Open Khushu"),
                    &tr("Stop Adhan"),
                );
                *upcoming_notified_at.borrow_mut() = Some(time);
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
                            let iqamah_end = scan_time + chrono::Duration::minutes(iqamah_mins);
                            if now < iqamah_end {
                                *iqamah_countdown.borrow_mut() =
                                    Some((scan_name.to_string(), iqamah_end));
                                *iqamah_notified_at.borrow_mut() = None;
                            }

                            let adhan_window = (iqamah_mins * 60).max(60);
                            if (now - scan_time).num_seconds() < adhan_window {
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
                    lists.morning = adkar::get_n_random_dikrs("morning", 2);
                    lists.evening = adkar::get_n_random_dikrs("evening", 2);
                    lists.night = adkar::get_n_random_dikrs("night", 2);
                    lists.date = today;
                }
                drop(lists);

                if adhan_ended && let Some(schedule) = today_schedule.as_ref() {
                    let fajr_elapsed = now.signed_duration_since(schedule.fajr).num_seconds();
                    let asr_elapsed = now.signed_duration_since(schedule.asr).num_seconds();
                    let isha_elapsed = now.signed_duration_since(schedule.isha).num_seconds();

                    let lists = today_adkar.borrow();

                    if let Some(dikr) = adkar_due(
                        &lists.morning,
                        0,
                        fajr_elapsed,
                        60..360,
                        &mut morning_dikr_1_sent.borrow_mut(),
                        today,
                    ) {
                        let body = if lang == "ar" {
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
                        &lists.morning,
                        1,
                        fajr_elapsed,
                        1800..1860,
                        &mut morning_dikr_2_sent.borrow_mut(),
                        today,
                    ) {
                        let body = if lang == "ar" {
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
                        &lists.evening,
                        0,
                        asr_elapsed,
                        900..960,
                        &mut evening_dikr_1_sent.borrow_mut(),
                        today,
                    ) {
                        let body = if lang == "ar" {
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
                        &lists.evening,
                        1,
                        asr_elapsed,
                        2700..2760,
                        &mut evening_dikr_2_sent.borrow_mut(),
                        today,
                    ) {
                        let body = if lang == "ar" {
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
                        &lists.night,
                        0,
                        isha_elapsed,
                        1800..1860,
                        &mut night_dikr_1_sent.borrow_mut(),
                        today,
                    ) {
                        let body = if lang == "ar" {
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
                        &lists.night,
                        1,
                        isha_elapsed,
                        3600..3660,
                        &mut night_dikr_2_sent.borrow_mut(),
                        today,
                    ) {
                        let body = if lang == "ar" {
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

            let iqamah_hero = {
                let state = iqamah_countdown.borrow();
                state.as_ref().and_then(|(iq_name, iq_end)| {
                    let remaining = iq_end.signed_duration_since(now).num_seconds();
                    if remaining > 0 {
                        let m = remaining / 60;
                        let s = remaining % 60;
                        Some(format!(
                            "{} {} {:02}:{:02}",
                            tr("Iqamah"),
                            tr(iq_name),
                            m,
                            s
                        ))
                    } else {
                        None
                    }
                })
            };

            if is_core_timer && adhan_ended {
                let should_notify = {
                    let state = iqamah_countdown.borrow();
                    state.as_ref().is_some_and(|(_iq_name, iq_end)| {
                        let remaining = iq_end.signed_duration_since(now).num_seconds();
                        remaining <= 0 && iqamah_notified_at.borrow().is_none_or(|t| t < *iq_end)
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

            let is_iqamah = iqamah_hero.is_some();
            let final_hero = iqamah_hero.unwrap_or(hero_text);

            on_state(PrayerState {
                hero_text: final_hero,
                hijri_text,
                location_text,
                next_prayer_name: name,
                adhan_playing,
                adhan_prayer_name: adhan_for_prayer.borrow().clone(),
                is_iqamah,
            });
        }

        gtk4::glib::ControlFlow::Continue
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use adkar::Dikr;

    fn make_dikr(arabic: &str, translation: &str) -> Dikr {
        Dikr {
            category: String::new(),
            count: 0,
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
            5,
            10..20,
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

        let result = adkar_due(&dikrs, 0, 15, 10..20, &mut sent, today);

        assert!(result.is_some());
        assert_eq!(result.unwrap().arabic, "صباح الخير");
        assert_eq!(sent, Some(today));
    }

    #[test]
    fn adkar_due_returns_none_after_already_sent_today() {
        let dikrs = vec![make_dikr("صباح الخير", "Good morning")];
        let today = NaiveDate::from_ymd_opt(2026, 7, 29).unwrap();
        let mut sent = Some(today);

        let result = adkar_due(&dikrs, 0, 15, 10..20, &mut sent, today);

        assert!(result.is_none());
        assert_eq!(sent, Some(today));
    }

    #[test]
    fn adkar_due_returns_none_for_out_of_bounds_index() {
        let dikrs = vec![make_dikr("صباح الخير", "Good morning")];
        let mut sent = None;
        let today = NaiveDate::from_ymd_opt(2026, 7, 29).unwrap();

        let result = adkar_due(&dikrs, 5, 15, 10..20, &mut sent, today);

        assert!(result.is_none());
    }

    #[test]
    fn adkar_due_returns_second_dikr_for_index_1() {
        let dikrs = vec![make_dikr("الأول", "First"), make_dikr("الثاني", "Second")];
        let mut sent = None;
        let today = NaiveDate::from_ymd_opt(2026, 7, 29).unwrap();

        let result = adkar_due(&dikrs, 1, 15, 10..20, &mut sent, today);

        assert!(result.is_some());
        assert_eq!(result.unwrap().arabic, "الثاني");
    }

    #[test]
    fn adkar_due_resets_next_day() {
        let dikrs = vec![make_dikr("صباح الخير", "Good morning")];
        let yesterday = NaiveDate::from_ymd_opt(2026, 7, 28).unwrap();
        let today = NaiveDate::from_ymd_opt(2026, 7, 29).unwrap();
        let mut sent = Some(yesterday);

        let result = adkar_due(&dikrs, 0, 15, 10..20, &mut sent, today);

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
}
