use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use gtk::{Box, Label, ListBox, Orientation, SelectionMode};
use gtk4 as gtk;
use libadwaita as adw;

use crate::adkar;
use crate::calendar;
use crate::config::AppConfig;
use crate::home_ui::refresh_home_ui;
use crate::i18n::tr;
use crate::nav_ui;
use crate::qibla_ui;
use crate::settings_ui;

pub struct PagesParams {
    pub view_stack: Rc<adw::ViewStack>,
    pub split_view: adw::OverlaySplitView,
    pub current_language: Rc<RefCell<String>>,
    pub config: AppConfig,
    pub loc_tx: std::sync::mpsc::Sender<(f64, f64, Option<String>)>,
    pub loc_rx: std::sync::mpsc::Receiver<(f64, f64, Option<String>)>,
    pub compass_manager: Rc<crate::qibla::CompassManager>,
    pub window: adw::ApplicationWindow,
    pub sidebar_list: gtk::ListBox,
    pub window_title: adw::WindowTitle,
}

pub struct PagesContext {
    pub hero_label: Label,
    pub hijri_label: Label,
    pub location_label: Label,
    pub list_box: Rc<ListBox>,
}

#[allow(clippy::too_many_arguments)]
fn handle_lang_change(
    row: &adw::ComboRow,
    current_language: &Rc<RefCell<String>>,
    config: &AppConfig,
    refresh_calendar: &Rc<dyn Fn()>,
    refresh_adkar: &Rc<dyn Fn()>,
    refresh_qibla: &Rc<dyn Fn()>,
    qibla_page: &Rc<crate::qibla_ui::QiblaPage>,
    sidebar: &gtk::ListBox,
    view_stack: &adw::ViewStack,
    window_title: &adw::WindowTitle,
    window_app: &adw::ApplicationWindow,
    refresh_home: &Rc<dyn Fn()>,
    loc_tx: &std::sync::mpsc::Sender<(f64, f64, Option<String>)>,
    list_box: &Rc<gtk::ListBox>,
    settings_ctx: &Rc<RefCell<crate::settings_ui::SettingsUiContext>>,
) {
    let selected_language;
    let mut language_changed = false;
    {
        let mut language = current_language.borrow_mut();
        let next_language = crate::i18n::language_code_from_index(row.selected()).to_string();
        if *language != next_language {
            *language = next_language;
            language_changed = true;
        }
        selected_language = language.clone();
    }
    if !language_changed {
        return;
    }

    let detected_language = crate::i18n::resolved_locale(&selected_language);

    crate::i18n::update_locale(&detected_language);

    config.set_language(&selected_language);
    config.save();

    if crate::i18n::is_rtl(&detected_language) {
        gtk::Widget::set_default_direction(gtk::TextDirection::Rtl);
        window_app.set_direction(gtk::TextDirection::Rtl);
    } else {
        gtk::Widget::set_default_direction(gtk::TextDirection::Ltr);
        window_app.set_direction(gtk::TextDirection::Ltr);
    }

    crate::apply_font_css(config);

    let style_manager = adw::StyleManager::default();
    match config.theme() {
        crate::config::ThemeMode::Light => {
            style_manager.set_color_scheme(adw::ColorScheme::ForceLight)
        }
        crate::config::ThemeMode::Dark => {
            style_manager.set_color_scheme(adw::ColorScheme::PreferDark)
        }
        crate::config::ThemeMode::System => {
            style_manager.set_color_scheme(adw::ColorScheme::Default)
        }
    }

    crate::settings_ui::update_settings_ui_lang(&settings_ctx.borrow(), &detected_language);

    let sidebar_for_update = sidebar.clone();
    let labels_deferred = [
        tr("Home"),
        tr("Calendar"),
        tr("Qibla"),
        tr("Adkar"),
        tr("Noble Quran"),
        tr("Settings"),
        tr("About"),
    ];
    gtk::glib::idle_add_local(move || {
        let mut curr = sidebar_for_update.first_child();
        let mut idx = 0;
        while let Some(child) = curr {
            if let Some(row_container) = child.downcast_ref::<gtk::ListBoxRow>()
                && let Some(action_row) = row_container
                    .child()
                    .and_then(|child_widget| child_widget.downcast::<adw::ActionRow>().ok())
                && idx < labels_deferred.len()
            {
                action_row.set_title(&labels_deferred[idx]);
                idx += 1;
            }
            curr = child.next_sibling();
        }
        gtk::glib::ControlFlow::Break
    });

    if let Some(name) = view_stack.visible_child_name() {
        window_title.set_title(&nav_ui::page_title(&name));
    }

    window_app.set_title(Some(&tr("Khushu")));

    refresh_calendar();
    refresh_adkar();
    refresh_qibla();
    qibla_page.rebuild_cardinals();
    crate::quran::refresh_quran_ui(view_stack, &detected_language, config.clone());

    let ctx_for_geo = settings_ctx.clone();
    let config_for_geo = config.clone();
    let loc_tx_for_geo = loc_tx.clone();
    let refresh_home_for_geo = refresh_home.clone();
    let list_box_for_geo = list_box.clone();
    let needs_geocode = config.prayer_times_source() != crate::config::PrayerTimesSource::Mawaqit;
    if needs_geocode {
        let (latitude, longitude) = (config.latitude(), config.longitude());
        let language_geo = detected_language.clone();
        gtk::glib::spawn_future_local(async move {
            if let Ok(name) =
                crate::location::resolve_city_name(latitude, longitude, &language_geo).await
            {
                let _ = loc_tx_for_geo.send((latitude, longitude, Some(name.clone())));
                gtk::glib::idle_add_local(move || {
                    config_for_geo.set_city_name(Some(name.clone()));
                    refresh_home_for_geo();
                    crate::settings_ui::refresh_prayers(&config_for_geo, &list_box_for_geo);
                    {
                        let ctx = ctx_for_geo.borrow();
                        if let Some(cache) = config_for_geo.mawaqit_cache().as_ref() {
                            ctx.mawaqit_status_row.set_subtitle(
                                &crate::settings_ui::mawaqit_status_subtitle(cache, &language_geo),
                            );
                        }
                    }
                    gtk::glib::ControlFlow::Break
                });
            }
        });
    }
}

pub fn build_pages(params: PagesParams) -> PagesContext {
    let PagesParams {
        view_stack,
        split_view,
        current_language,
        config,
        loc_tx,
        loc_rx,
        compass_manager,
        window,
        sidebar_list,
        window_title,
    } = params;
    let home_scroll = gtk::ScrolledWindow::builder()
        .vexpand(true)
        .propagate_natural_width(false)
        .propagate_natural_height(false)
        .build();

    let home_content_box = Box::new(Orientation::Vertical, 0);
    home_content_box.set_margin_top(24);
    home_content_box.set_margin_bottom(24);
    home_content_box.set_margin_start(12);
    home_content_box.set_margin_end(12);

    let home_clamp = adw::Clamp::builder()
        .maximum_size(800)
        .tightening_threshold(600)
        .child(&home_content_box)
        .build();

    home_scroll.set_child(Some(&home_clamp));

    let hero_box = Box::new(Orientation::Vertical, 8);
    hero_box.set_halign(gtk::Align::Center);
    hero_box.set_margin_top(12);
    hero_box.set_margin_bottom(12);

    let hero_label = Label::builder().label("").css_classes(["title-1"]).build();

    let hijri_label = Label::builder()
        .label("...")
        .css_classes(["title-3", "dim-label"])
        .build();

    let location_label = Label::builder()
        .label("...")
        .css_classes(["title-4", "dim-label"])
        .build();

    hero_box.append(&hero_label);
    hero_box.append(&hijri_label);
    hero_box.append(&location_label);
    home_content_box.append(&hero_box);

    let list_box = ListBox::builder()
        .selection_mode(SelectionMode::None)
        .css_classes(["boxed-list"])
        .margin_start(8)
        .margin_end(8)
        .margin_bottom(8)
        .build();
    let list_box_rc = Rc::new(list_box);
    home_content_box.append(list_box_rc.as_ref());

    let hijri_label_ref = hijri_label.clone();
    let location_label_ref = location_label.clone();
    let list_box_home = list_box_rc.clone();
    let config_for_home = config.clone();
    let refresh_home: Rc<dyn Fn()> = Rc::new(move || {
        let language = config_for_home.language();
        refresh_home_ui(
            &hijri_label_ref,
            &location_label_ref,
            &language,
            &config_for_home,
        );
        settings_ui::refresh_prayers(&config_for_home, &list_box_home);
    });
    let refresh_home_initial = refresh_home.clone();
    refresh_home_initial();

    {
        let refresh_home_rt = refresh_home.clone();
        crate::connect_to_properties(&config, crate::CONFIG_REFRESH_PROPERTIES, move || {
            refresh_home_rt()
        });
    }

    let config_for_location = config.clone();
    let list_box_loc = list_box_rc.clone();
    let hijri_label_loc = hijri_label.clone();
    let location_label_loc = location_label.clone();
    let current_language_loc = current_language.clone();

    gtk::glib::timeout_add_local(std::time::Duration::from_millis(200), move || {
        while let Ok((latitude, longitude, city)) = loc_rx.try_recv() {
            config_for_location.set_latitude(latitude);
            config_for_location.set_longitude(longitude);
            if let Some(name) = city {
                config_for_location.set_city_name(Some(name));
            }
            config_for_location.save();

            let language = current_language_loc.borrow();
            refresh_home_ui(
                &hijri_label_loc,
                &location_label_loc,
                &language,
                &config_for_location,
            );
            settings_ui::refresh_prayers(&config_for_location, &list_box_loc);
        }
        gtk::glib::ControlFlow::Continue
    });

    view_stack.add_named(&home_scroll, Some("home"));

    let (calendar_page, refresh_calendar) = calendar::create_calendar_page(config.clone());

    let calendar_clamp = adw::Clamp::builder()
        .maximum_size(800)
        .tightening_threshold(600)
        .child(&calendar_page)
        .build();

    let calendar_scroll = gtk::ScrolledWindow::builder()
        .child(&calendar_clamp)
        .vexpand(true)
        .propagate_natural_width(false)
        .propagate_natural_height(false)
        .build();

    view_stack.add_named(&calendar_scroll, Some("calendar"));

    let calendar_grid = calendar_page
        .first_child()
        .and_then(|child| child.next_sibling())
        .and_then(|child| child.downcast::<gtk::Grid>().ok())
        .expect("Could not find calendar grid");

    let mut classes = calendar_grid.css_classes();
    if !classes.contains(&"compact-calendar".into()) {
        classes.push("compact-calendar".into());
    }

    let breakpoint = adw::Breakpoint::new(adw::BreakpointCondition::new_length(
        adw::BreakpointConditionLengthType::MaxWidth,
        370.0,
        adw::LengthUnit::Px,
    ));
    breakpoint.add_setter(&split_view, "collapsed", Some(&true.to_value()));
    breakpoint.add_setter(&calendar_grid, "css-classes", Some(&classes.to_value()));

    window.add_breakpoint(breakpoint);

    let qibla_page = Rc::new(qibla_ui::create_qibla_page(
        config.clone(),
        compass_manager.clone(),
    ));

    let qibla_clamp = adw::Clamp::builder()
        .maximum_size(600)
        .tightening_threshold(400)
        .child(&qibla_page.container)
        .build();

    let qibla_scroll = gtk::ScrolledWindow::builder()
        .child(&qibla_clamp)
        .vexpand(true)
        .propagate_natural_width(false)
        .propagate_natural_height(false)
        .build();

    view_stack.add_named(&qibla_scroll, Some("qibla"));
    let refresh_qibla = qibla_page.refresh.clone();

    let view_stack_for_notify = view_stack.clone();
    let qibla_page_for_notify = qibla_page.clone();
    let compass_for_notify = compass_manager.clone();
    view_stack.connect_visible_child_name_notify(move |_| {
        let name = view_stack_for_notify
            .visible_child_name()
            .map(|child_name| child_name.to_string())
            .unwrap_or_default();
        if name == "qibla" {
            compass_for_notify.start_monitoring();
            qibla_page_for_notify.start_listening();
        } else {
            qibla_page_for_notify.stop_listening();
        }
    });

    let (adkar_box, refresh_adkar) = adkar::create_adkar_page(config.clone());
    view_stack.add_named(&adkar_box, Some("adkar"));

    let quran_page =
        crate::quran::create_quran_page(&current_language.borrow(), &view_stack, config.clone());
    view_stack.add_named(&quran_page, Some("quran"));

    let settings_box = Box::new(Orientation::Vertical, 0);
    settings_box.set_margin_top(24);
    settings_box.set_margin_bottom(24);
    settings_box.set_margin_start(12);
    settings_box.set_margin_end(12);

    let settings_clamp = adw::Clamp::builder()
        .maximum_size(800)
        .tightening_threshold(600)
        .child(&settings_box)
        .build();

    let dynamic_settings_box = Box::new(Orientation::Vertical, 0);
    settings_box.append(&dynamic_settings_box);

    let (language_row, settings_ctx) =
        settings_ui::setup_settings_ui(settings_ui::SettingsUiParams {
            settings_box: &dynamic_settings_box,
            config: config.clone(),
            list_box_rc: list_box_rc.clone(),
            window: &window,
            current_language: current_language.clone(),
            loc_tx: loc_tx.clone(),
            refresh_calendar: refresh_calendar.clone(),
        });

    let current_language_signal = current_language.clone();
    let config_signal = config.clone();
    let ref_cal = refresh_calendar.clone();
    let ref_adkar = refresh_adkar.clone();
    let ref_qibla = refresh_qibla.clone();
    let ref_qibla_page = qibla_page.clone();
    let ref_sidebar = sidebar_list.clone();
    let ref_view = view_stack.clone();
    let ref_title = window_title.clone();
    let ref_window = window.clone();
    let ref_home = refresh_home.clone();
    let ref_tx = loc_tx.clone();
    let ref_list = list_box_rc.clone();
    let ref_ctx = settings_ctx.clone();

    language_row.connect_selected_notify(move |row| {
        log::info!(
            "selected-notify (settings): selected={}, current_language={}",
            row.selected(),
            current_language_signal.borrow(),
        );
        handle_lang_change(
            row,
            &current_language_signal,
            &config_signal,
            &ref_cal,
            &ref_adkar,
            &ref_qibla,
            &ref_qibla_page,
            &ref_sidebar,
            &ref_view,
            &ref_title,
            &ref_window,
            &ref_home,
            &ref_tx,
            &ref_list,
            &ref_ctx,
        );
    });

    let settings_scroll = gtk::ScrolledWindow::builder()
        .vexpand(true)
        .propagate_natural_width(false)
        .propagate_natural_height(false)
        .child(&settings_clamp)
        .build();

    view_stack.add_named(&settings_scroll, Some("settings"));

    let settings_scroll_for_fonts = settings_scroll.clone();
    let fonts_heading = settings_ctx.borrow().fonts_heading.clone();
    let fonts_view_stack = view_stack.clone();
    let fonts_sidebar_list = sidebar_list.clone();
    let fonts_window_title = window_title.clone();
    let fonts_current_language = current_language.clone();
    let fonts_split_view = split_view.clone();
    let fonts_config = config.clone();
    settings_ui::register_open_fonts_settings(Rc::new(move || {
        nav_ui::navigate_to(
            "settings",
            &fonts_sidebar_list,
            &fonts_view_stack,
            &fonts_window_title,
            &fonts_current_language.borrow(),
            &fonts_split_view,
            &fonts_config,
        );
        scroll_to_widget(&settings_scroll_for_fonts, fonts_heading.upcast_ref());
    }));

    let toast_overlay = adw::ToastOverlay::new();
    toast_overlay.set_child(Some(view_stack.as_ref()));
    split_view.set_content(Some(&toast_overlay));

    let config_for_map = config.clone();
    window.connect_map(move |win| {
        let today = crate::time::effective_today(&config_for_map);
        if let Ok(result) = crate::time::schedule_for_config(&config_for_map, today) {
            crate::settings_ui::update_lre_toast(&config_for_map, &result, win);
            crate::settings_ui::update_fallback_toast(&config_for_map, &result, win);
        }
    });

    PagesContext {
        hero_label,
        hijri_label,
        location_label,
        list_box: list_box_rc,
    }
}

fn scroll_to_widget(scrolled: &gtk::ScrolledWindow, target: &gtk::Widget) {
    let Some(content) = scrolled.child() else {
        return;
    };
    let tick_scrolled = scrolled.clone();
    let target_c = target.clone();
    scrolled.add_tick_callback(move |_, _| {
        if target_c.allocated_width() > 0 {
            if let Some((_, y)) = target_c.translate_coordinates(&content, 0.0, 0.0) {
                let adjustment = tick_scrolled.vadjustment();
                let max = (adjustment.upper() - adjustment.page_size()).max(0.0);
                adjustment.set_value((y - 24.0).clamp(0.0, max));
            }
            gtk::glib::ControlFlow::Break
        } else {
            gtk::glib::ControlFlow::Continue
        }
    });
}
