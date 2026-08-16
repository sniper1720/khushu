use adw::prelude::*;
use chrono::{Local, NaiveDate};
use gtk4 as gtk;
use libadwaita as adw;

use crate::config::{AppConfig, ReminderPresentation};
use crate::i18n::{is_rtl, tr};
use crate::islamic_content::{ContentCategory, ContentService};
use crate::notifications::show_notification;
use crate::prayer_tracker::{PrayerStatus, set_prayer_status};

#[derive(Debug, Clone)]
pub enum ReminderEvent {
    PrePrayer {
        prayer_name: String,
        minutes_left: u32,
        prev_check: Option<(&'static str, NaiveDate)>,
    },
    PrayerTime {
        prayer_name: String,
    },
    Iqamah {
        prayer_name: String,
        delay_mins: u32,
    },
    QuranWird {
        start_page: u32,
        end_page: u32,
        total_pages: u32,
        completed_pages: u32,
        remaining_pages: u32,
        target_page: u32,
        is_startup: bool,
    },
}

pub fn present_reminder(config: &AppConfig, event: ReminderEvent) {
    match config.prayer_reminder_presentation() {
        ReminderPresentation::Notification => present_notification(config, &event),
        ReminderPresentation::Popup => present_popup(config, event),
    }
}

fn present_notification(_config: &AppConfig, event: &ReminderEvent) {
    match event {
        ReminderEvent::PrePrayer {
            prayer_name,
            minutes_left,
            prev_check,
        } => {
            let mut body = format!(
                "{} {} {} {}",
                tr(prayer_name),
                tr("is in"),
                minutes_left,
                tr("minutes")
            );
            if let Some((prev_name, _)) = prev_check {
                body.push_str(&format!(" ({}: {})", tr("Previous prayer"), tr(prev_name)));
            }
            show_notification(
                &format!("{} {}", tr("Upcoming Prayer:"), tr(prayer_name)),
                &body,
                false,
                &tr("Open Khushu"),
                &tr("Stop Adhan"),
            );
        }
        ReminderEvent::PrayerTime { prayer_name } => {
            show_notification(
                &format!("{} {}", tr("It's time for"), tr(prayer_name)),
                &format!("{} {}.", tr("It is now time for"), tr(prayer_name)),
                true,
                &tr("Open Khushu"),
                &tr("Stop Adhan"),
            );
        }
        ReminderEvent::Iqamah {
            prayer_name,
            delay_mins,
        } => {
            show_notification(
                &format!("{} {}", tr("Iqamah"), tr(prayer_name)),
                &format!(
                    "{} {} ({} {}).",
                    tr("It is time for Iqamah of"),
                    tr(prayer_name),
                    delay_mins,
                    tr("minutes")
                ),
                false,
                &tr("Open Khushu"),
                &tr("Stop Adhan"),
            );
        }
        ReminderEvent::QuranWird {
            start_page,
            end_page,
            remaining_pages,
            is_startup,
            ..
        } => {
            let title = tr("Today's Quran Wird");
            let body = if *is_startup {
                format!(
                    "{} ({} {}–{})",
                    tr("Don't forget your Quran reading for today."),
                    tr("Pages"),
                    start_page,
                    end_page
                )
            } else {
                format!(
                    "{} {} {}.",
                    tr("You still have"),
                    remaining_pages,
                    tr("pages remaining")
                )
            };
            show_notification(&title, &body, false, &tr("Start Reading"), &tr("Later"));
        }
    }
}

fn present_popup(config: &AppConfig, event: ReminderEvent) {
    let app = match gtk::gio::Application::default()
        .and_then(|a| a.downcast::<adw::Application>().ok())
    {
        Some(app) => app,
        None => {
            present_notification(config, &event);
            return;
        }
    };

    if let ReminderEvent::QuranWird {
        start_page,
        end_page,
        total_pages,
        completed_pages,
        remaining_pages,
        target_page,
        is_startup,
    } = event
    {
        static QURAN_POPUP_ACTIVE: std::sync::atomic::AtomicBool =
            std::sync::atomic::AtomicBool::new(false);

        if QURAN_POPUP_ACTIVE.swap(true, std::sync::atomic::Ordering::SeqCst) {
            log::info!("Quran Wird popup already active, ignoring duplicate event.");
            return;
        }

        let title = tr("Today's Quran Wird");
        let window = adw::Window::builder()
            .application(&app)
            .title(&title)
            .default_width(460)
            .default_height(340)
            .resizable(false)
            .modal(false)
            .build();

        window.connect_destroy(move |_| {
            QURAN_POPUP_ACTIVE.store(false, std::sync::atomic::Ordering::SeqCst);
        });

        if is_rtl(&config.language()) {
            window.set_direction(gtk::TextDirection::Rtl);
        } else {
            window.set_direction(gtk::TextDirection::Ltr);
        }

        let header_bar = adw::HeaderBar::new();
        header_bar.set_show_end_title_buttons(true);

        let content_box = gtk::Box::new(gtk::Orientation::Vertical, 16);
        content_box.set_margin_start(24);
        content_box.set_margin_end(24);
        content_box.set_margin_top(16);
        content_box.set_margin_bottom(24);

        let hero_label = gtk::Label::builder()
            .label(&title)
            .css_classes(["title-1", "accent"])
            .wrap(true)
            .justify(gtk::Justification::Center)
            .build();
        content_box.append(&hero_label);

        let subtitle_text = if is_startup {
            tr("Don't forget your Quran reading for today.")
        } else {
            format!(
                "{} {} {}.",
                tr("You still have"),
                remaining_pages,
                tr("pages remaining")
            )
        };
        let subtitle_label = gtk::Label::builder()
            .label(&subtitle_text)
            .css_classes(["body", "dim-label"])
            .wrap(true)
            .justify(gtk::Justification::Center)
            .build();
        content_box.append(&subtitle_label);

        let info_card = adw::Clamp::new();
        info_card.set_maximum_size(420);
        let info_box = gtk::Box::new(gtk::Orientation::Vertical, 8);
        info_box.add_css_class("card");
        info_box.set_margin_top(4);
        info_box.set_margin_bottom(4);

        let row1_text = if is_startup {
            format!("{}: {} {}", tr("Today's Goal"), total_pages, tr("pages"))
        } else {
            format!(
                "{}: {} / {} {}",
                tr("Completed"),
                completed_pages,
                total_pages,
                tr("pages")
            )
        };
        let row1_label = gtk::Label::builder()
            .label(&row1_text)
            .css_classes(["heading"])
            .justify(gtk::Justification::Center)
            .build();
        info_box.append(&row1_label);

        let row2_text = format!("{}: {}–{}", tr("Today's Range"), start_page, end_page);
        let row2_label = gtk::Label::builder()
            .label(&row2_text)
            .css_classes(["body", "dim-label"])
            .justify(gtk::Justification::Center)
            .build();
        info_box.append(&row2_label);

        info_card.set_child(Some(&info_box));
        content_box.append(&info_card);

        let actions_box = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        actions_box.set_halign(gtk::Align::Center);
        actions_box.set_margin_top(8);

        let btn_label = if is_startup {
            tr("Start Reading")
        } else {
            tr("Continue Reading")
        };
        let start_btn = gtk::Button::with_label(&btn_label);
        start_btn.add_css_class("suggested-action");

        let win_start = window.clone();
        start_btn.connect_clicked(move |_| {
            win_start.close();
            crate::quran::request_quran_page_navigation(target_page);
        });
        actions_box.append(&start_btn);

        let win_later = window.clone();
        let later_btn = gtk::Button::with_label(&tr("Later"));
        later_btn.connect_clicked(move |_| {
            win_later.close();
        });
        actions_box.append(&later_btn);

        content_box.append(&actions_box);

        let toolbar_view = adw::ToolbarView::new();
        toolbar_view.add_top_bar(&header_bar);
        toolbar_view.set_content(Some(&content_box));

        window.set_content(Some(&toolbar_view));
        window.present();
        return;
    }

    let today = Local::now().naive_local().date();

    let (title, hero_text, category, prayer_name_str) = match &event {
        ReminderEvent::PrePrayer {
            prayer_name,
            minutes_left,
            ..
        } => (
            tr("Prayer Reminder"),
            format!(
                "{} {} {} {}",
                tr(prayer_name),
                tr("in"),
                minutes_left,
                tr("minutes")
            ),
            ContentCategory::PrePrayer,
            prayer_name.clone(),
        ),
        ReminderEvent::PrayerTime { prayer_name } => (
            tr("Prayer Time"),
            format!("{} {}", tr("It's time for"), tr(prayer_name)),
            ContentCategory::PrayerTime,
            prayer_name.clone(),
        ),
        ReminderEvent::Iqamah { prayer_name, .. } => (
            tr("Iqamah Reminder"),
            format!("{} {}", tr("Approaching Iqamah for"), tr(prayer_name)),
            ContentCategory::Iqamah,
            prayer_name.clone(),
        ),
        ReminderEvent::QuranWird { .. } => unreachable!(),
    };

    let reminder_keys = ContentService::get_reminder_keys(category, today, &prayer_name_str);
    let (text, source) = reminder_keys.resolve();

    let window = adw::Window::builder()
        .application(&app)
        .title(&title)
        .default_width(460)
        .default_height(340)
        .resizable(false)
        .modal(false)
        .build();

    if is_rtl(&config.language()) {
        window.set_direction(gtk::TextDirection::Rtl);
    } else {
        window.set_direction(gtk::TextDirection::Ltr);
    }

    let header_bar = adw::HeaderBar::new();
    header_bar.set_show_end_title_buttons(true);

    let content_box = gtk::Box::new(gtk::Orientation::Vertical, 16);
    content_box.set_margin_start(24);
    content_box.set_margin_end(24);
    content_box.set_margin_top(16);
    content_box.set_margin_bottom(24);

    let hero_label = gtk::Label::builder()
        .label(&hero_text)
        .css_classes(["title-1", "accent"])
        .wrap(true)
        .justify(gtk::Justification::Center)
        .build();
    content_box.append(&hero_label);

    let quote_card = adw::Clamp::new();
    quote_card.set_maximum_size(420);
    let quote_box = gtk::Box::new(gtk::Orientation::Vertical, 6);
    quote_box.add_css_class("card");
    quote_box.set_margin_top(4);
    quote_box.set_margin_bottom(4);

    let quote_label = gtk::Label::builder()
        .label(&format!("“{}”", text))
        .wrap(true)
        .justify(gtk::Justification::Center)
        .css_classes(["body"])
        .build();
    quote_box.append(&quote_label);

    if let Some(src) = source {
        let src_label = gtk::Label::builder()
            .label(&format!("— {}", src))
            .wrap(true)
            .justify(gtk::Justification::Center)
            .css_classes(["caption", "dim-label"])
            .build();
        quote_box.append(&src_label);
    }
    quote_card.set_child(Some(&quote_box));
    content_box.append(&quote_card);

    if let ReminderEvent::PrePrayer {
        prev_check: Some((prev_name, prev_date)),
        ..
    } = event.clone()
    {
        if config.check_previous_prayer() {
            let prev_card = gtk::Box::new(gtk::Orientation::Vertical, 8);
            prev_card.add_css_class("card");

            let prompt_text = format!("{} {}?", tr("Did you pray"), tr(prev_name));
            let prompt_lbl = gtk::Label::builder()
                .label(&prompt_text)
                .css_classes(["heading"])
                .justify(gtk::Justification::Center)
                .build();
            prev_card.append(&prompt_lbl);

            let btn_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            btn_box.set_halign(gtk::Align::Center);

            let cfg_prayed = config.clone();
            let win_prayed = window.clone();
            let prayed_btn = gtk::Button::with_label(&tr("Prayed"));
            prayed_btn.add_css_class("suggested-action");
            prayed_btn.connect_clicked(move |_| {
                set_prayer_status(&cfg_prayed, prev_date, prev_name, PrayerStatus::Prayed);
                win_prayed.close();
            });
            btn_box.append(&prayed_btn);

            let cfg_missed = config.clone();
            let win_missed = window.clone();
            let missed_btn = gtk::Button::with_label(&tr("Missed"));
            missed_btn.add_css_class("destructive-action");
            missed_btn.connect_clicked(move |_| {
                set_prayer_status(&cfg_missed, prev_date, prev_name, PrayerStatus::Missed);
                win_missed.close();
            });
            btn_box.append(&missed_btn);

            let cfg_later = config.clone();
            let win_later = window.clone();
            let later_btn = gtk::Button::with_label(&tr("Later"));
            later_btn.connect_clicked(move |_| {
                set_prayer_status(&cfg_later, prev_date, prev_name, PrayerStatus::Dismissed);
                win_later.close();
            });
            btn_box.append(&later_btn);

            prev_card.append(&btn_box);
            content_box.append(&prev_card);
        }
    }

    let actions_box = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    actions_box.set_halign(gtk::Align::Center);
    actions_box.set_margin_top(8);

    if matches!(event, ReminderEvent::PrayerTime { .. }) && crate::audio::is_adhan() {
        let stop_btn = gtk::Button::with_label(&tr("Stop Adhan"));
        stop_btn.add_css_class("destructive-action");
        stop_btn.connect_clicked(move |_| {
            crate::audio::stop();
        });
        actions_box.append(&stop_btn);
    }

    let cfg_mark = config.clone();
    let p_name = prayer_name_str.clone();
    let win_mark = window.clone();
    let mark_btn = gtk::Button::with_label(&tr("Mark Prayed"));
    mark_btn.add_css_class("suggested-action");
    mark_btn.connect_clicked(move |_| {
        set_prayer_status(&cfg_mark, today, &p_name, PrayerStatus::Prayed);
        win_mark.close();
    });
    actions_box.append(&mark_btn);

    let win_close = window.clone();
    let close_btn = gtk::Button::with_label(&tr("Dismiss"));
    close_btn.connect_clicked(move |_| {
        win_close.close();
    });
    actions_box.append(&close_btn);

    content_box.append(&actions_box);

    let toolbar_view = adw::ToolbarView::new();
    toolbar_view.add_top_bar(&header_bar);
    toolbar_view.set_content(Some(&content_box));

    window.set_content(Some(&toolbar_view));
    window.present();
}
