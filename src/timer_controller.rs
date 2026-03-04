use std::cell::RefCell;
use std::rc::Rc;

use chrono::{Datelike, Duration, Local, NaiveDate};
use hijri_date::HijriDate;

use crate::adkar;
use crate::audio::AudioManager;
use crate::config::AppConfig;
use crate::i18n::tr;
use crate::location;
use crate::notifications::show_notification;
use crate::time::PrayerEngine;

pub struct PrayerState {
    pub hero_text: String,
    pub hijri_text: String,
    pub location_text: String,
    pub next_prayer_name: String,
}

pub fn start_prayer_timer(
    config: Rc<RefCell<AppConfig>>,
    on_state: impl Fn(PrayerState) + 'static,
) {
    use std::sync::atomic::{AtomicBool, Ordering};
    static HAS_CORE_TIMER: AtomicBool = AtomicBool::new(false);
    let is_core_timer = !HAS_CORE_TIMER.swap(true, Ordering::SeqCst);

    let audio_manager = Rc::new(RefCell::new(AudioManager::new()));

    let last_notified_prayer: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
    let last_pre_notified: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));

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

    let engine_cache: Rc<RefCell<Option<(PrayerEngine, String)>>> = Rc::new(RefCell::new(None));

    gtk4::glib::timeout_add_seconds_local(1, move || {
        let config = config.borrow();

        let fingerprint = format!(
            "{}:{}:{:?}:{:?}",
            config.latitude, config.longitude, config.method, config.madhab
        );
        {
            let mut cache = engine_cache.borrow_mut();
            if cache
                .as_ref()
                .map(|(_, f)| f != &fingerprint)
                .unwrap_or(true)
            {
                let engine = PrayerEngine::new(
                    config.latitude,
                    config.longitude,
                    &config.method,
                    &config.madhab,
                );
                *cache = Some((engine, fingerprint));
            }
        }

        let cache = engine_cache.borrow();
        let (engine, _) = cache.as_ref().unwrap();
        let today = Local::now().date_naive();
        let lang = config.language.clone();

        let now = Local::now();
        let adjusted_now = now + Duration::days(config.hijri_offset);
        let hijri = HijriDate::from_gr(
            adjusted_now.year() as usize,
            adjusted_now.month() as usize,
            adjusted_now.day() as usize,
        )
        .expect("Failed to calculate Hijri date");

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
        let m_name = tr(en_months.get(hijri.month() - 1).unwrap_or(&""), &lang);
        let hijri_text = format!("{} {} {}", hijri.day(), m_name, hijri.year());

        let location_text = if let Some(city) = &config.city_name {
            location::short_city_with_country(city)
        } else {
            format!("{:.2}, {:.2}", config.latitude, config.longitude)
        };

        if let Some((name, time)) = engine.next_prayer(today) {
            let duration = time.signed_duration_since(now);
            let total_seconds = duration.num_seconds();
            let hours = duration.num_hours();
            let minutes = (duration.num_minutes() % 60).abs();
            let seconds = (duration.num_seconds() % 60).abs();

            let hero_text = if total_seconds > 0 {
                format!(
                    "{} {} {:02}:{:02}:{:02}",
                    tr(&name, &lang),
                    tr("in", &lang),
                    hours,
                    minutes,
                    seconds
                )
            } else {
                format!("{} {}", tr("It's time for", &lang), tr(&name, &lang))
            };

            if is_core_timer
                && config.pre_prayer_notify
                && total_seconds > 0
                && total_seconds <= (config.pre_prayer_minutes as i64 * 60)
            {
                let mut last_pre = last_pre_notified.borrow_mut();
                if last_pre.as_deref() != Some(name.as_str()) {
                    show_notification(
                        &format!("{} {}", tr("Upcoming Prayer:", &lang), tr(&name, &lang)),
                        &format!(
                            "{} {} {} {}",
                            tr(&name, &lang),
                            tr("is in", &lang),
                            config.pre_prayer_minutes,
                            tr("minutes", &lang)
                        ),
                        false,
                        &tr("Open Khushu", &lang),
                        &tr("Stop Adhan", &lang),
                    );
                    *last_pre = Some(name.clone());
                }
            }

            if is_core_timer && total_seconds <= 0 && total_seconds > -60 {
                let mut last_pray = last_notified_prayer.borrow_mut();
                if last_pray.as_deref() != Some(name.as_str()) {
                    if name != "Sunrise" {
                        show_notification(
                            &format!("{} {}", tr("It's time for", &lang), tr(&name, &lang)),
                            &format!("{} {}.", tr("It is now time for", &lang), tr(&name, &lang)),
                            true,
                            &tr("Open Khushu", &lang),
                            &tr("Stop Adhan", &lang),
                        );

                        let path = config
                            .adhan_sound_path
                            .clone()
                            .unwrap_or_else(|| "assets/audio/Madinah.mp3".to_string());
                        if !config.adhan_muted {
                            audio_manager
                                .borrow()
                                .play_adhan(&path, config.adhan_volume);
                        }
                    }

                    *last_pray = Some(name.clone());
                    *last_pre_notified.borrow_mut() = None;
                }
            }

            if is_core_timer && config.adkar_notification_enabled {
                let mut d_lists = daily_adkar_lists.borrow_mut();
                if d_lists.date != today {
                    d_lists.morning = adkar::get_n_random_dikrs("morning", 2);
                    d_lists.evening = adkar::get_n_random_dikrs("evening", 2);
                    d_lists.night = adkar::get_n_random_dikrs("night", 2);
                    d_lists.date = today;
                }

                if name == "Sunrise" {
                    if total_seconds <= 0 && total_seconds > -60 {
                        let mut state = last_morning_adkar_1.borrow_mut();
                        if *state != Some(today) {
                            if let Some(dikr) = d_lists.morning.first() {
                                let body = if lang == "ar" {
                                    &dikr.arabic
                                } else {
                                    &dikr.translation
                                };
                                show_notification(
                                    &tr("Morning Adkar", &lang),
                                    body,
                                    false,
                                    &tr("Open Khushu", &lang),
                                    &tr("Stop Adhan", &lang),
                                );
                            }
                            *state = Some(today);
                        }
                    } else if total_seconds < -1800 && total_seconds > -1860 {
                        let mut state = last_morning_adkar_2.borrow_mut();
                        if *state != Some(today) {
                            if let Some(dikr) = d_lists.morning.get(1) {
                                let body = if lang == "ar" {
                                    &dikr.arabic
                                } else {
                                    &dikr.translation
                                };
                                show_notification(
                                    &tr("Morning Adkar", &lang),
                                    body,
                                    false,
                                    &tr("Open Khushu", &lang),
                                    &tr("Stop Adhan", &lang),
                                );
                            }
                            *state = Some(today);
                        }
                    }
                }

                if name == "Asr" {
                    if total_seconds < -900 && total_seconds > -960 {
                        let mut state = last_evening_adkar_1.borrow_mut();
                        if *state != Some(today) {
                            if let Some(dikr) = d_lists.evening.first() {
                                let body = if lang == "ar" {
                                    &dikr.arabic
                                } else {
                                    &dikr.translation
                                };
                                show_notification(
                                    &tr("Evening Adkar", &lang),
                                    body,
                                    false,
                                    &tr("Open Khushu", &lang),
                                    &tr("Stop Adhan", &lang),
                                );
                            }
                            *state = Some(today);
                        }
                    } else if total_seconds < -2700 && total_seconds > -2760 {
                        let mut state = last_evening_adkar_2.borrow_mut();
                        if *state != Some(today) {
                            if let Some(dikr) = d_lists.evening.get(1) {
                                let body = if lang == "ar" {
                                    &dikr.arabic
                                } else {
                                    &dikr.translation
                                };
                                show_notification(
                                    &tr("Evening Adkar", &lang),
                                    body,
                                    false,
                                    &tr("Open Khushu", &lang),
                                    &tr("Stop Adhan", &lang),
                                );
                            }
                            *state = Some(today);
                        }
                    }
                }

                if name == "Isha" {
                    if total_seconds < -1800 && total_seconds > -1860 {
                        let mut state = last_night_adkar_1.borrow_mut();
                        if *state != Some(today) {
                            if let Some(dikr) = d_lists.night.first() {
                                let body = if lang == "ar" {
                                    &dikr.arabic
                                } else {
                                    &dikr.translation
                                };
                                show_notification(
                                    &tr("Night Adkar", &lang),
                                    body,
                                    false,
                                    &tr("Open Khushu", &lang),
                                    &tr("Stop Adhan", &lang),
                                );
                            }
                            *state = Some(today);
                        }
                    } else if total_seconds < -3600 && total_seconds > -3660 {
                        let mut state = last_night_adkar_2.borrow_mut();
                        if *state != Some(today) {
                            if let Some(dikr) = d_lists.night.get(1) {
                                let body = if lang == "ar" {
                                    &dikr.arabic
                                } else {
                                    &dikr.translation
                                };
                                show_notification(
                                    &tr("Night Adkar", &lang),
                                    body,
                                    false,
                                    &tr("Open Khushu", &lang),
                                    &tr("Stop Adhan", &lang),
                                );
                            }
                            *state = Some(today);
                        }
                    }
                }
            }

            if total_seconds < -1000 {
                *last_notified_prayer.borrow_mut() = None;
            }

            on_state(PrayerState {
                hero_text,
                hijri_text,
                location_text,
                next_prayer_name: name,
            });
        }

        gtk4::glib::ControlFlow::Continue
    });
}
