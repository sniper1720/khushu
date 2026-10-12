use crate::config::AppConfig;
use crate::i18n::tr;
use adw::prelude::*;
use chrono::{Datelike, Duration, NaiveDate};
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

// The Hijri month one step away from `year` and `month`, or `None` when the
// step would leave the range the calendar crate supports. The boundary is
// discovered by asking the crate rather than duplicated here.
fn adjacent_hijri_month(
    year: usize,
    month: usize,
    step_towards_earlier_month: bool,
) -> Option<(usize, usize)> {
    let (adjacent_year, adjacent_month) = match (month, step_towards_earlier_month) {
        (1, true) => (year.checked_sub(1)?, 12),
        (12, false) => (year.checked_add(1)?, 1),
        (_, true) => (year, month - 1),
        (_, false) => (year, month + 1),
    };
    HijriDate::from_hijri(adjacent_year, adjacent_month, 1)
        .is_ok()
        .then_some((adjacent_year, adjacent_month))
}

fn previous_hijri_month(year: usize, month: usize) -> Option<(usize, usize)> {
    adjacent_hijri_month(year, month, true)
}

fn next_hijri_month(year: usize, month: usize) -> Option<(usize, usize)> {
    adjacent_hijri_month(year, month, false)
}

// Grays out whichever arrow would step outside the supported range, so the
// limit is visible rather than silently enforced. The step functions stay
// the actual guard, which keeps the displayed year representable however the
// arrow is activated, including by keyboard.
fn update_navigation_buttons(
    state: &CalendarState,
    previous_button: &Button,
    next_button: &Button,
) {
    let (year, month) = (state.current_hijri_year, state.current_hijri_month);
    previous_button.set_sensitive(previous_hijri_month(year, month).is_some());
    next_button.set_sensitive(next_hijri_month(year, month).is_some());
}

pub fn create_calendar_page(config: AppConfig) -> (Box, Rc<dyn Fn()>) {
    let now = crate::time::effective_today(&config);
    let offset_days = config.hijri_offset();
    let adjusted_now = now + Duration::days(offset_days);
    // Without a valid Hijri date there is no month to render, so the page
    // cannot show its content at all. A status page is the libadwaita widget
    // for that case, and naming the likely cause helps more than crashing.
    let initial_hijri = match HijriDate::from_gr(
        adjusted_now.year() as usize,
        adjusted_now.month() as usize,
        adjusted_now.day() as usize,
    ) {
        Ok(initial_hijri) => initial_hijri,
        Err(err) => {
            log::error!("Failed to calculate initial Hijri date from current time: {err}");
            let unavailable_page = adw::StatusPage::builder()
                .title(tr("Calendar Unavailable"))
                .description(tr(
                    "Khushu could not determine the Hijri date. Check that your system date and time are set correctly.",
                ))
                .icon_name("dialog-warning-symbolic")
                .vexpand(true)
                .build();
            let unavailable_container = Box::new(Orientation::Vertical, 0);
            unavailable_container.append(&unavailable_page);
            return (unavailable_container, Rc::new(|| {}));
        }
    };

    let calendar_state = Rc::new(RefCell::new(CalendarState {
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
    grid.add_css_class("calendar-grid");
    container.append(&grid);

    let details_frame = Frame::new(Some(&tr("Date Details")));
    let details_box = Box::new(Orientation::Vertical, 6);
    details_box.set_margin_top(12);
    details_box.set_margin_bottom(12);
    details_box.set_margin_start(12);
    details_box.set_margin_end(12);

    let hijri_details_label = Label::new(None);
    hijri_details_label.set_css_classes(&["title-3"]);
    hijri_details_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    hijri_details_label.set_max_width_chars(50);
    details_box.append(&hijri_details_label);

    let gregorian_label = Label::new(None);
    gregorian_label.set_css_classes(&["dim-label"]);
    gregorian_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    gregorian_label.set_max_width_chars(50);
    details_box.append(&gregorian_label);

    let event_label = Label::new(None);
    event_label.set_css_classes(&["accent"]);
    event_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    event_label.set_max_width_chars(50);
    details_box.append(&event_label);

    details_frame.set_child(Some(&details_box));
    container.append(&details_frame);

    let selected_date = Rc::new(RefCell::new(adjusted_now));

    let state_clone = calendar_state.clone();
    let grid_clone = grid.clone();
    let month_label_clone = month_label.clone();
    let hijri_details_clone = hijri_details_label.clone();
    let greg_details_clone = gregorian_label.clone();
    let event_details_clone = event_label.clone();
    let details_frame_clone = details_frame.clone();
    let selected_date_clone = selected_date.clone();
    let config_for_calendar = config.clone();
    let prev_btn_clone = prev_btn.clone();
    let next_btn_clone = next_btn.clone();

    let refresh_inner: Rc<dyn Fn(bool)> = Rc::new(move |recenter_on_today: bool| {
        let hijri_offset = config_for_calendar.hijri_offset();
        let today_phys = crate::time::effective_today(&config_for_calendar);
        let corrected_today = today_phys + Duration::days(hijri_offset);
        let today_hijri = HijriDate::from_gr(
            corrected_today.year() as usize,
            corrected_today.month() as usize,
            corrected_today.day() as usize,
        )
        .ok();

        if recenter_on_today && let Some(ref hijri_today) = today_hijri {
            *selected_date_clone.borrow_mut() = corrected_today;
            let mut state_guard_mut = state_clone.borrow_mut();
            state_guard_mut.current_hijri_month = hijri_today.month();
            state_guard_mut.current_hijri_year = hijri_today.year();
        }

        let state_guard = state_clone.borrow();
        details_frame_clone.set_label(Some(&tr("Date Details")));
        update_navigation_buttons(&state_guard, &prev_btn_clone, &next_btn_clone);

        // The year is already in the state, so the month label needs no second conversion.
        let month_name = crate::time::hijri_month_name(state_guard.current_hijri_month)
            .unwrap_or_else(|| tr("Unavailable"));
        month_label_clone.set_label(&format!(
            "{} {}",
            month_name, state_guard.current_hijri_year
        ));

        while let Some(child) = grid_clone.first_child() {
            grid_clone.remove(&child);
        }

        let days = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
        // DAYS: Short weekday names (expose to xgettext)
        if false {
            tr("Sun");
            tr("Mon");
            tr("Tue");
            tr("Wed");
            tr("Thu");
            tr("Fri");
            tr("Sat");
        }

        for (day_index, day) in days.iter().enumerate() {
            let label = Label::new(Some(&tr(day)));
            label.set_css_classes(&["dim-label"]);
            label.set_halign(gtk::Align::Center);
            grid_clone.attach(&label, day_index as i32, 0, 1, 1);
        }

        let Ok(month_start) = HijriDate::from_hijri(
            state_guard.current_hijri_year,
            state_guard.current_hijri_month,
            1,
        ) else {
            log::error!(
                "Cannot render Hijri month {} {}: outside the supported conversion range",
                state_guard.current_hijri_year,
                state_guard.current_hijri_month
            );
            return;
        };
        let Some(gr_first) = NaiveDate::from_ymd_opt(
            month_start.year_gr() as i32,
            month_start.month_gr() as u32,
            month_start.day_gr() as u32,
        ) else {
            log::error!("Cannot place the first day of the displayed Hijri month on the grid");
            return;
        };
        let start_weekday = gr_first.weekday().num_days_from_sunday();

        let mut row = 1;
        let mut col = start_weekday as i32;
        let month_length = month_start.month_len();

        for day_num in 1..=month_length {
            let Ok(current_hijri) = HijriDate::from_hijri(
                state_guard.current_hijri_year,
                state_guard.current_hijri_month,
                day_num,
            ) else {
                continue;
            };

            let day_btn = Button::with_label(&format!("{}", day_num));
            day_btn.set_height_request(32);

            if let Some(ref today_h) = today_hijri
                && current_hijri.day() == today_h.day()
                && current_hijri.month() == today_h.month()
                && current_hijri.year() == today_h.year()
            {
                day_btn.add_css_class("suggested-action");
            }

            let state_inner = state_clone.clone();
            let hijri_inner = hijri_details_clone.clone();
            let greg_inner = greg_details_clone.clone();
            let event_inner = event_details_clone.clone();
            let selected_date_inner = selected_date_clone.clone();

            day_btn.connect_clicked(move |_| {
                let (clicked_year, clicked_month) = {
                    let state_guard = state_inner.borrow();
                    (
                        state_guard.current_hijri_year,
                        state_guard.current_hijri_month,
                    )
                };
                let Ok(clicked_hijri) = HijriDate::from_hijri(clicked_year, clicked_month, day_num)
                else {
                    return;
                };
                let Some(clicked_date) = NaiveDate::from_ymd_opt(
                    clicked_hijri.year_gr() as i32,
                    clicked_hijri.month_gr() as u32,
                    clicked_hijri.day_gr() as u32,
                ) else {
                    return;
                };
                *selected_date_inner.borrow_mut() = clicked_date;
                update_details(clicked_date, &hijri_inner, &greg_inner, &event_inner);
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
        );
    });

    let refresh_ui: Rc<dyn Fn()> = {
        let refresh_inner_clone = refresh_inner.clone();
        Rc::new(move || refresh_inner_clone(true))
    };

    refresh_ui();

    let state_prev = calendar_state.clone();
    let refresh_prev_inner = refresh_inner.clone();
    prev_btn.connect_clicked(move |_| {
        {
            let mut state_guard_mut = state_prev.borrow_mut();
            let (year, month) = (
                state_guard_mut.current_hijri_year,
                state_guard_mut.current_hijri_month,
            );
            if let Some((stepped_year, stepped_month)) = previous_hijri_month(year, month) {
                state_guard_mut.current_hijri_year = stepped_year;
                state_guard_mut.current_hijri_month = stepped_month;
            }
        }
        refresh_prev_inner(false);
    });

    let state_next = calendar_state.clone();
    let refresh_next_inner = refresh_inner.clone();
    next_btn.connect_clicked(move |_| {
        {
            let mut state_guard_mut = state_next.borrow_mut();
            let (year, month) = (
                state_guard_mut.current_hijri_year,
                state_guard_mut.current_hijri_month,
            );
            if let Some((stepped_year, stepped_month)) = next_hijri_month(year, month) {
                state_guard_mut.current_hijri_year = stepped_year;
                state_guard_mut.current_hijri_month = stepped_month;
            }
        }
        refresh_next_inner(false);
    });

    (container, refresh_ui)
}

fn update_details(date: NaiveDate, hijri_label: &Label, greg_label: &Label, event_label: &Label) {
    // The Gregorian date does not depend on the Hijri conversion, so it must
    // render even when that conversion fails.
    let weekday = get_gregorian_weekday_name(date.weekday());
    let greg_month = get_gregorian_month_name(date.month());
    greg_label.set_label(&format!(
        "{}, {:02} {} {}",
        weekday,
        date.day(),
        greg_month,
        date.year()
    ));

    let (hijri_text, event_text) = match HijriDate::from_gr(
        date.year() as usize,
        date.month() as usize,
        date.day() as usize,
    ) {
        Ok(hijri) => {
            let event_text = match (hijri.month(), hijri.day()) {
                (9, 1) => Some(tr("First Day of Ramadan")),
                (10, 1) => Some(tr("Eid al-Fitr")),
                (12, 10) => Some(tr("Eid al-Adha")),
                (12, 9) => Some(tr("Day of Arafah")),
                (1, 1) => Some(tr("Islamic New Year")),
                (1, 10) => Some(tr("Ashura")),
                (3, 12) => Some(tr("Mawlid al-Nabi")),
                _ => None,
            };
            // A month the calendar crate reports is always nameable, but keep
            // the placeholder so a date can never render as a blank label.
            let hijri_text = match crate::time::hijri_month_name(hijri.month()) {
                Some(hijri_month) => format!("{} {} {}", hijri.day(), hijri_month, hijri.year()),
                None => tr("Unavailable"),
            };
            (hijri_text, event_text)
        }
        Err(err) => {
            log::error!("Failed to calculate Hijri date for selected day: {err}");
            (tr("Unavailable"), None)
        }
    };

    hijri_label.set_label(&hijri_text);

    match event_text {
        Some(text) => {
            event_label.set_label(&text);
            event_label.set_visible(true);
        }
        None => event_label.set_visible(false),
    }
}

fn get_gregorian_month_name(month: u32) -> String {
    let en_months = [
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
    // EN_MONTHS: Gregorian month names (expose to xgettext)
    if false {
        tr("January");
        tr("February");
        tr("March");
        tr("April");
        tr("May");
        tr("June");
        tr("July");
        tr("August");
        tr("September");
        tr("October");
        tr("November");
        tr("December");
    }
    // `chrono` guarantees 1 to 12, so the index is in bounds by contract and
    // needs no fallback branch of its own.
    tr(en_months[month as usize - 1])
}

fn get_gregorian_weekday_name(day: chrono::Weekday) -> String {
    let en_weekdays = [
        "Sunday",
        "Monday",
        "Tuesday",
        "Wednesday",
        "Thursday",
        "Friday",
        "Saturday",
    ];
    // EN_WEEKDAYS: Weekday names (expose to xgettext)
    if false {
        tr("Sunday");
        tr("Monday");
        tr("Tuesday");
        tr("Wednesday");
        tr("Thursday");
        tr("Friday");
        tr("Saturday");
    }
    tr(en_weekdays[day.num_days_from_sunday() as usize])
}
