use crate::config::AppConfig;
use crate::i18n::tr;
use adw::prelude::*;
use chrono::{Datelike, Duration, Local, NaiveDate};
use gtk::{Box, Button, Frame, Grid, Label, Orientation};
use gtk4 as gtk;
use hijri_date::HijriDate;
use libadwaita as adw;
use std::cell::RefCell;
use std::rc::Rc;

struct CalendarState {
    current_hijri_month: usize,
    current_hijri_year: usize,
}

pub fn create_calendar_page(
    language: Rc<RefCell<String>>,
    config: Rc<RefCell<AppConfig>>,
) -> (Box, Rc<dyn Fn()>) {
    let now = Local::now().date_naive();
    let offset_days = config.borrow().hijri_offset;
    let adjusted_now = now + Duration::days(offset_days);
    let initial_hijri = HijriDate::from_gr(
        adjusted_now.year() as usize,
        adjusted_now.month() as usize,
        adjusted_now.day() as usize,
    )
    .expect("Failed to calculate initial Hijri date from current time");

    let state = Rc::new(RefCell::new(CalendarState {
        current_hijri_month: initial_hijri.month(),
        current_hijri_year: initial_hijri.year(),
    }));

    let container = Box::new(Orientation::Vertical, 8);
    container.set_margin_top(8);
    container.set_margin_bottom(8);
    container.set_margin_start(6);
    container.set_margin_end(6);
    container.set_overflow(gtk::Overflow::Hidden);

    let nav_box = Box::new(Orientation::Horizontal, 6);
    nav_box.set_halign(gtk::Align::Center);

    let prev_btn = Button::from_icon_name("go-previous-symbolic");
    let next_btn = Button::from_icon_name("go-next-symbolic");
    let month_label = Label::builder()
        .css_classes(["title-2"])
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .wrap(true)
        .build();

    nav_box.append(&prev_btn);
    nav_box.append(&month_label);
    nav_box.append(&next_btn);
    container.append(&nav_box);

    let grid = Grid::new();
    grid.set_column_spacing(2);
    grid.set_row_spacing(2);
    grid.set_column_homogeneous(true);
    grid.set_hexpand(true);
    container.append(&grid);

    let details_frame = Frame::new(Some(&tr("Date Details", &language.borrow())));
    let details_box = Box::new(Orientation::Vertical, 6);
    details_box.set_margin_top(12);
    details_box.set_margin_bottom(12);
    details_box.set_margin_start(12);
    details_box.set_margin_end(12);

    let hijri_details_label = Label::new(None);
    hijri_details_label.set_css_classes(&["title-3"]);
    hijri_details_label.set_wrap(true);
    details_box.append(&hijri_details_label);

    let gregorian_label = Label::new(None);
    gregorian_label.set_css_classes(&["dim-label"]);
    gregorian_label.set_wrap(true);
    details_box.append(&gregorian_label);

    let event_label = Label::new(None);
    event_label.set_css_classes(&["accent"]);
    event_label.set_wrap(true);
    details_box.append(&event_label);

    details_frame.set_child(Some(&details_box));
    container.append(&details_frame);

    let selected_date = Rc::new(RefCell::new(adjusted_now));

    let state_clone = state.clone();
    let grid_clone = grid.clone();
    let month_label_clone = month_label.clone();
    let hijri_details_clone = hijri_details_label.clone();
    let greg_details_clone = gregorian_label.clone();
    let event_details_clone = event_label.clone();
    let details_frame_clone = details_frame.clone();
    let selected_date_clone = selected_date.clone();
    let lang_refresh = language.clone();
    let config_for_calendar = config.clone();

    let refresh_inner: Rc<dyn Fn(bool)> = Rc::new(move |recenter_on_today: bool| {
        let lang = lang_refresh.borrow();
        let hijri_offset = config_for_calendar.borrow().hijri_offset;
        let today_phys = Local::now().date_naive();
        let corrected_today = today_phys + Duration::days(hijri_offset);
        let today_hijri = HijriDate::from_gr(
            corrected_today.year() as usize,
            corrected_today.month() as usize,
            corrected_today.day() as usize,
        )
        .ok();

        if recenter_on_today && let Some(ref h) = today_hijri {
            *selected_date_clone.borrow_mut() = corrected_today;
            let mut s_mut = state_clone.borrow_mut();
            s_mut.current_hijri_month = h.month();
            s_mut.current_hijri_year = h.year();
        }

        let s = state_clone.borrow();
        details_frame_clone.set_label(Some(&tr("Date Details", &lang)));

        let dummy_hijri = HijriDate::from_hijri(s.current_hijri_year, s.current_hijri_month, 1)
            .expect("Valid Hijri date");

        let month_name = get_hijri_month_name(s.current_hijri_month, &lang);
        month_label_clone.set_label(&format!("{} {}", month_name, dummy_hijri.year()));

        while let Some(child) = grid_clone.first_child() {
            grid_clone.remove(&child);
        }

        let days = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

        for (i, day) in days.iter().enumerate() {
            let label = Label::new(Some(&tr(day, &lang)));
            label.set_css_classes(&["dim-label"]);
            label.set_halign(gtk::Align::Center);
            grid_clone.attach(&label, i as i32, 0, 1, 1);
        }

        let first_day_h = HijriDate::from_hijri(s.current_hijri_year, s.current_hijri_month, 1)
            .expect("Valid Hijri date");
        let gr_first = NaiveDate::from_ymd_opt(
            first_day_h.year_gr() as i32,
            first_day_h.month_gr() as u32,
            first_day_h.day_gr() as u32,
        )
        .expect("Invalid Gregorian date for start of Hijri month");
        let start_weekday = gr_first.weekday().num_days_from_sunday();

        let mut row = 1;
        let mut col = start_weekday as i32;
        let m_len = first_day_h.month_len();

        for d in 1..=m_len {
            let current_h = HijriDate::from_hijri(s.current_hijri_year, s.current_hijri_month, d)
                .expect("Valid Hijri date");

            let day_btn = Button::with_label(&format!("{}", d));
            day_btn.set_height_request(32);

            if let Some(ref today_h) = today_hijri
                && current_h.day() == today_h.day()
                && current_h.month() == today_h.month()
                && current_h.year() == today_h.year()
            {
                day_btn.add_css_class("suggested-action");
            }

            let state_inner = state_clone.clone();
            let hijri_inner = hijri_details_clone.clone();
            let greg_inner = greg_details_clone.clone();
            let event_inner = event_details_clone.clone();
            let lang_inner = lang_refresh.clone();
            let selected_date_inner = selected_date_clone.clone();

            day_btn.connect_clicked(move |_| {
                let curr = HijriDate::from_hijri(
                    state_inner.borrow().current_hijri_year,
                    state_inner.borrow().current_hijri_month,
                    d,
                )
                .expect("Valid Hijri date");
                let naive = NaiveDate::from_ymd_opt(
                    curr.year_gr() as i32,
                    curr.month_gr() as u32,
                    curr.day_gr() as u32,
                )
                .expect("Invalid Gregorian date for Hijri day conversion");
                *selected_date_inner.borrow_mut() = naive;
                update_details(
                    naive,
                    &hijri_inner,
                    &greg_inner,
                    &event_inner,
                    &lang_inner.borrow(),
                );
            });

            grid_clone.attach(&day_btn, col, row, 1, 1);

            col += 1;
            if col > 6 {
                col = 0;
                row += 1;
            }
        }
        let selected = *selected_date_clone.borrow();
        update_details(
            selected,
            &hijri_details_clone,
            &greg_details_clone,
            &event_details_clone,
            &lang,
        );
    });

    let refresh_ui: Rc<dyn Fn()> = {
        let refresh_inner_clone = refresh_inner.clone();
        Rc::new(move || refresh_inner_clone(true))
    };

    refresh_ui();

    let state_prev = state.clone();
    let refresh_prev_inner = refresh_inner.clone();
    prev_btn.connect_clicked(move |_| {
        {
            let mut s = state_prev.borrow_mut();
            if s.current_hijri_month == 1 {
                s.current_hijri_month = 12;
                s.current_hijri_year -= 1;
            } else {
                s.current_hijri_month -= 1;
            }
        }
        refresh_prev_inner(false);
    });

    let state_next = state.clone();
    let refresh_next_inner = refresh_inner.clone();
    next_btn.connect_clicked(move |_| {
        {
            let mut s = state_next.borrow_mut();
            if s.current_hijri_month == 12 {
                s.current_hijri_month = 1;
                s.current_hijri_year += 1;
            } else {
                s.current_hijri_month += 1;
            }
        }
        refresh_next_inner(false);
    });

    (container, refresh_ui)
}

fn get_hijri_month_name(month: usize, lang: &str) -> String {
    let en_names = [
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

    let name = en_names.get(month - 1).unwrap_or(&"").to_string();
    tr(&name, lang)
}

fn update_details(
    date: NaiveDate,
    hijri_label: &Label,
    greg_label: &Label,
    event_label: &Label,
    lang: &str,
) {
    if let Ok(hijri) = HijriDate::from_gr(
        date.year() as usize,
        date.month() as usize,
        date.day() as usize,
    ) {
        let m_name = get_hijri_month_name(hijri.month(), lang);
        hijri_label.set_label(&format!("{} {} {}", hijri.day(), m_name, hijri.year()));

        let weekday = get_gregorian_weekday_name(date.weekday(), lang);
        let greg_month = get_gregorian_month_name(date.month(), lang);
        greg_label.set_label(&format!(
            "{}, {:02} {} {}",
            weekday,
            date.day(),
            greg_month,
            date.year()
        ));

        let event_key = match (hijri.month(), hijri.day()) {
            (9, 1) => Some("First Day of Ramadan"),
            (10, 1) => Some("Eid al-Fitr"),
            (12, 10) => Some("Eid al-Adha"),
            (12, 9) => Some("Day of Arafah"),
            (1, 1) => Some("Islamic New Year"),
            (1, 10) => Some("Ashura"),
            (3, 12) => Some("Mawlid al-Nabi"),
            _ => None,
        };

        if let Some(key) = event_key {
            event_label.set_label(&tr(key, lang));
            event_label.set_visible(true);
        } else {
            event_label.set_visible(false);
        }
    }
}

fn get_gregorian_month_name(month: u32, lang: &str) -> String {
    let en_names = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    let name = en_names
        .get((month - 1) as usize)
        .unwrap_or(&"")
        .to_string();
    tr(&name, lang)
}

fn get_gregorian_weekday_name(day: chrono::Weekday, lang: &str) -> String {
    let en_names = [
        "Sunday",
        "Monday",
        "Tuesday",
        "Wednesday",
        "Thursday",
        "Friday",
        "Saturday",
    ];
    let name = en_names
        .get(day.num_days_from_sunday() as usize)
        .unwrap_or(&"")
        .to_string();
    tr(&name, lang)
}
