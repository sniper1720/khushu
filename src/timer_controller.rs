use std::cell::RefCell;
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
}

type IqamahState = Rc<RefCell<Option<(String, chrono::DateTime<chrono::Local>)>>>;

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
    } else {
        engine
            .get_prayer_times(today)
            .map(|s| apply_timezone_override(config, s))
    };
    let tomorrow_schedule = if use_mawaqit {
        crate::time::schedule_for_config(config, tomorrow)
    } else {
        engine
            .get_prayer_times(tomorrow)
            .map(|s| apply_timezone_override(config, s))
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

fn get_or_compute_schedule<'a>(
    today_schedule: &'a Option<PrayerSchedule>,
    tomorrow_schedule: &'a Option<PrayerSchedule>,
    now: chrono::DateTime<Local>,
) -> Option<(String, chrono::DateTime<Local>)> {
    today_schedule
        .as_ref()
        .and_then(|s| next_prayer_from_schedule(s, now))
        .or_else(|| {
            tomorrow_schedule
                .as_ref()
                .map(|s| ("Fajr".to_string(), s.fajr))
        })
}

pub fn start_prayer_timer(config: AppConfig, on_state: impl Fn(PrayerState) + 'static) {
    use std::sync::atomic::{AtomicBool, Ordering};
    static HAS_CORE_TIMER: AtomicBool = AtomicBool::new(false);
    let is_core_timer = !HAS_CORE_TIMER.swap(true, Ordering::SeqCst);

    let last_notified_prayer: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
    let last_pre_notified: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
    let current_adhan_prayer: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
    let iqamah_state: IqamahState = Rc::new(RefCell::new(None));
    let iqamah_notified: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));

    struct DailyAdkarLists {
        date: NaiveDate,
        morning: Vec<crate::adkar::Dikr>,
        evening: Vec<crate::adkar::Dikr>,
        night: Vec<crate::adkar::Dikr>,
    }

    let default_date = Local::now().naive_local().date() - Duration::days(1);
    let daily_adkar_lists = Rc::new(RefCell::new(DailyAdkarLists {
        date: default_date,
        morning: vec![],
        evening: vec![],
        night: vec![],
    }));

    let last_morning_adkar_1: Rc<RefCell<Option<NaiveDate>>> = Rc::new(RefCell::new(None));
    let last_morning_adkar_2: Rc<RefCell<Option<NaiveDate>>> = Rc::new(RefCell::new(None));
    let last_evening_adkar_1: Rc<RefCell<Option<NaiveDate>>> = Rc::new(RefCell::new(None));
    let last_evening_adkar_2: Rc<RefCell<Option<NaiveDate>>> = Rc::new(RefCell::new(None));
    let last_night_adkar_1: Rc<RefCell<Option<NaiveDate>>> = Rc::new(RefCell::new(None));
    let last_night_adkar_2: Rc<RefCell<Option<NaiveDate>>> = Rc::new(RefCell::new(None));

    let engine_cache: Rc<RefCell<Option<PrayerEngine>>> = Rc::new(RefCell::new(None));
    let last_mawaqit_attempt_day: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
    let daily_state: Rc<RefCell<Option<DailyState>>> = Rc::new(RefCell::new(None));

    let engine_dirty: Rc<RefCell<bool>> = Rc::new(RefCell::new(true));
    let schedule_dirty: Rc<RefCell<bool>> = Rc::new(RefCell::new(true));

    {
        let ed = engine_dirty.clone();
        let sd = schedule_dirty.clone();
        crate::connect_notify_blocked(&config, Some("latitude"), move |_, _| {
            *ed.borrow_mut() = true;
            *sd.borrow_mut() = true;
        });
    }
    {
        let ed = engine_dirty.clone();
        let sd = schedule_dirty.clone();
        crate::connect_notify_blocked(&config, Some("longitude"), move |_, _| {
            *ed.borrow_mut() = true;
            *sd.borrow_mut() = true;
        });
    }
    {
        let ed = engine_dirty.clone();
        let sd = schedule_dirty.clone();
        crate::connect_notify_blocked(&config, Some("method"), move |_, _| {
            *ed.borrow_mut() = true;
            *sd.borrow_mut() = true;
        });
    }
    {
        let ed = engine_dirty.clone();
        let sd = schedule_dirty.clone();
        crate::connect_notify_blocked(&config, Some("madhab"), move |_, _| {
            *ed.borrow_mut() = true;
            *sd.borrow_mut() = true;
        });
    }
    {
        let sd = schedule_dirty.clone();
        crate::connect_notify_blocked(&config, Some("language"), move |_, _| {
            *sd.borrow_mut() = true;
        });
    }
    {
        let sd = schedule_dirty.clone();
        crate::connect_notify_blocked(&config, Some("city-name"), move |_, _| {
            *sd.borrow_mut() = true;
        });
    }
    {
        let sd = schedule_dirty.clone();
        crate::connect_notify_blocked(&config, Some("prayer-times-source"), move |_, _| {
            *sd.borrow_mut() = true;
        });
    }
    {
        let sd = schedule_dirty.clone();
        crate::connect_notify_blocked(&config, Some("timezone-mode"), move |_, _| {
            *sd.borrow_mut() = true;
        });
    }
    {
        let sd = schedule_dirty.clone();
        crate::connect_notify_blocked(&config, Some("timezone-override-minutes"), move |_, _| {
            *sd.borrow_mut() = true;
        });
    }

    gtk4::glib::timeout_add_seconds_local(1, move || {
        if *engine_dirty.borrow() {
            let engine = PrayerEngine::new(
                config.latitude(),
                config.longitude(),
                &config.method(),
                &config.madhab(),
            );
            *engine_cache.borrow_mut() = Some(engine);
            *engine_dirty.borrow_mut() = false;
        }

        let engine_guard = engine_cache.borrow();
        let engine = engine_guard
            .as_ref()
            .expect("prayer engine should be cached");
        let today = crate::time::effective_today(&config);
        let lang = config.language();

        let mut state_guard = daily_state.borrow_mut();
        let need_recompute = *schedule_dirty.borrow()
            || state_guard
                .as_ref()
                .map(|s| s.cache_date != today)
                .unwrap_or(true);
        if need_recompute {
            let fresh = compute_daily_state(&config, engine, today);
            *state_guard = Some(fresh);
            *schedule_dirty.borrow_mut() = false;
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

        let next = get_or_compute_schedule(&today_schedule, &tomorrow_schedule, now);

        let adhan_playing = crate::audio::is_playing();
        if !adhan_playing {
            *current_adhan_prayer.borrow_mut() = None;
        }

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
            {
                let mut last_pre = last_pre_notified.borrow_mut();
                if last_pre.as_deref() != Some(name.as_str()) {
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
                    *last_pre = Some(name.clone());
                }
            }

            if is_core_timer && total_seconds <= 0 && total_seconds > -60 {
                let mut last_pray = last_notified_prayer.borrow_mut();
                if last_pray.as_deref() != Some(name.as_str()) {
                    let is_prayer = name != "Sunrise";
                    show_notification(
                        &format!("{} {}", tr("It's time for"), tr(&name)),
                        &format!("{} {}.", tr("It is now time for"), tr(&name)),
                        is_prayer,
                        &tr("Open Khushu"),
                        &tr("Stop Adhan"),
                    );

                    if name != "Sunrise" {
                        let path = config
                            .adhan_sound_path()
                            .unwrap_or_else(|| "assets/audio/Madinah.mp3".to_string());
                        if !config.adhan_muted() {
                            crate::audio::play_adhan(&path, config.adhan_volume());
                            *current_adhan_prayer.borrow_mut() = Some(name.clone());
                        }
                        let iqamah_mins =
                            config.iqamah_minutes().get(&name).copied().unwrap_or(10) as i64;
                        let iqamah_end = time + chrono::Duration::minutes(iqamah_mins);
                        *iqamah_state.borrow_mut() = Some((name.clone(), iqamah_end));
                        *iqamah_notified.borrow_mut() = None;
                    }

                    *last_pray = Some(name.clone());
                    *last_pre_notified.borrow_mut() = None;
                }
            }

            if is_core_timer && config.adkar_notification_enabled() && !config.adhan_only_mode() {
                let mut d_lists = daily_adkar_lists.borrow_mut();
                if d_lists.date != today {
                    d_lists.morning = adkar::get_n_random_dikrs("morning", 2);
                    d_lists.evening = adkar::get_n_random_dikrs("evening", 2);
                    d_lists.night = adkar::get_n_random_dikrs("night", 2);
                    d_lists.date = today;
                }
                drop(d_lists);

                if let Some(schedule) = today_schedule.as_ref() {
                    let fajr_elapsed = now.signed_duration_since(schedule.fajr).num_seconds();
                    let asr_elapsed = now.signed_duration_since(schedule.asr).num_seconds();
                    let isha_elapsed = now.signed_duration_since(schedule.isha).num_seconds();

                    if (60..120).contains(&fajr_elapsed) {
                        let mut state = last_morning_adkar_1.borrow_mut();
                        if *state != Some(today) {
                            let lists = daily_adkar_lists.borrow();
                            if let Some(dikr) = lists.morning.first() {
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
                            *state = Some(today);
                        }
                    }
                    if (1800..1860).contains(&fajr_elapsed) {
                        let mut state = last_morning_adkar_2.borrow_mut();
                        if *state != Some(today) {
                            let lists = daily_adkar_lists.borrow();
                            if let Some(dikr) = lists.morning.get(1) {
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
                            *state = Some(today);
                        }
                    }

                    if (900..960).contains(&asr_elapsed) {
                        let mut state = last_evening_adkar_1.borrow_mut();
                        if *state != Some(today) {
                            let lists = daily_adkar_lists.borrow();
                            if let Some(dikr) = lists.evening.first() {
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
                            *state = Some(today);
                        }
                    }
                    if (2700..2760).contains(&asr_elapsed) {
                        let mut state = last_evening_adkar_2.borrow_mut();
                        if *state != Some(today) {
                            let lists = daily_adkar_lists.borrow();
                            if let Some(dikr) = lists.evening.get(1) {
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
                            *state = Some(today);
                        }
                    }

                    if (1800..1860).contains(&isha_elapsed) {
                        let mut state = last_night_adkar_1.borrow_mut();
                        if *state != Some(today) {
                            let lists = daily_adkar_lists.borrow();
                            if let Some(dikr) = lists.night.first() {
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
                            *state = Some(today);
                        }
                    }
                    if (3600..3660).contains(&isha_elapsed) {
                        let mut state = last_night_adkar_2.borrow_mut();
                        if *state != Some(today) {
                            let lists = daily_adkar_lists.borrow();
                            if let Some(dikr) = lists.night.get(1) {
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
                            *state = Some(today);
                        }
                    }
                }
            }

            if total_seconds < -1000 {
                *last_notified_prayer.borrow_mut() = None;
                *iqamah_state.borrow_mut() = None;
            }

            let iqamah_hero = {
                let state = iqamah_state.borrow();
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

            if is_core_timer {
                let should_notify = {
                    let state = iqamah_state.borrow();
                    let notified = iqamah_notified.borrow();
                    state.as_ref().is_some_and(|(iq_name, iq_end)| {
                        let remaining = iq_end.signed_duration_since(now).num_seconds();
                        remaining <= 0 && remaining > -60 && notified.as_deref() != Some(iq_name)
                    })
                };
                if should_notify
                    && config.iqamah_notify()
                    && !config.adhan_only_mode()
                    && let Some((iq_name, _)) = iqamah_state.borrow().as_ref()
                {
                    show_notification(
                        &format!("{} {}", tr("Iqamah"), tr(iq_name)),
                        &format!("{} {}.", tr("It is time for Iqamah of"), tr(iq_name)),
                        false,
                        &tr("Open Khushu"),
                        &tr("Stop Adhan"),
                    );
                    *iqamah_notified.borrow_mut() = Some(iq_name.clone());
                }
            }

            let final_hero = iqamah_hero.unwrap_or(hero_text);

            on_state(PrayerState {
                hero_text: final_hero,
                hijri_text,
                location_text,
                next_prayer_name: name,
                adhan_playing,
                adhan_prayer_name: current_adhan_prayer.borrow().clone(),
            });
        }

        gtk4::glib::ControlFlow::Continue
    });
}
