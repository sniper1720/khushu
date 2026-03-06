mod adkar;
mod audio;
mod autostart;
mod background;
mod background_tasks;
mod calendar;
mod config;
mod home_ui;
mod location;
mod nav_ui;
mod notifications;
mod pages;
mod qibla;
mod qibla_ui;
mod security;
mod settings_ui;
mod time;
mod timer_controller;
mod welcome;

use qibla::CompassManager;

mod i18n;
use crate::i18n::tr;
use adw::prelude::*;
use adw::{Application, ApplicationWindow, HeaderBar};
use background_tasks::start_background_tasks;
use config::{AppConfig, LocationMode};

use gtk4 as gtk;
use libadwaita as adw;
use std::cell::RefCell;
use std::rc::Rc;
use timer_controller::start_prayer_timer;

use gtk::{Button, Label};

const APP_ID: &str = "io.github.sniper1720.khushu";

#[tokio::main]
async fn main() {
    env_logger::init();

    gtk::gio::resources_register_include!("khushu.gresource")
        .expect("Failed to register embedded resources");

    gtk::glib::set_prgname(Some(APP_ID));
    gtk::glib::set_application_name("Khushu");

    let config = Rc::new(RefCell::new(AppConfig::load()));

    crate::autostart::sync(config.borrow().autostart);

    crate::i18n::update_locale(&config.borrow().language);

    adw::init().expect("Failed to initialize Libadwaita");

    let app = Application::builder()
        .application_id(APP_ID)
        .flags(gtk::gio::ApplicationFlags::HANDLES_COMMAND_LINE)
        .build();

    let app_hold = Rc::new(RefCell::new(None));

    let config_startup = config.clone();
    let app_startup_clone = app.clone();
    app.connect_startup(move |_| {
        let style_manager = adw::StyleManager::default();
        match config_startup.borrow().theme {
            config::ThemeMode::Light => {
                style_manager.set_color_scheme(adw::ColorScheme::ForceLight)
            }
            config::ThemeMode::Dark => style_manager.set_color_scheme(adw::ColorScheme::PreferDark),
            config::ThemeMode::System => style_manager.set_color_scheme(adw::ColorScheme::Default),
        }

        if let Some(display) = gtk::gdk::Display::default() {
            let theme = gtk::IconTheme::for_display(&display);
            theme.add_resource_path("/io/github/sniper1720/khushu/icons/hicolor");
        }

        let quit_action = gtk::gio::SimpleAction::new("quit", None);
        let app_clone = app_startup_clone.clone();
        quit_action.connect_activate(move |_, _| {
            app_clone.quit();
        });
        app_startup_clone.add_action(&quit_action);
        app_startup_clone.set_accels_for_action("app.quit", &["<Ctrl>Q"]);

        let open_action = gtk::gio::SimpleAction::new("open-main", None);
        let app_clone_open = app_startup_clone.clone();
        open_action.connect_activate(move |_, _| {
            app_clone_open.activate();
        });
        app_startup_clone.add_action(&open_action);

        let stop_adhan_action = gtk::gio::SimpleAction::new("stop-adhan", None);
        stop_adhan_action.connect_activate(move |_, _| {
            crate::audio::AudioManager::stop_global();
            log::info!("Adhan stopped via notification action.");
        });
        app_startup_clone.add_action(&stop_adhan_action);
    });

    let app_hold_cmd = app_hold.clone();
    let _config_clone = config.clone();
    app.connect_command_line(move |app, cli| {
        let args = cli.arguments();
        let mut is_background = false;

        for arg in args.iter().skip(1) {
            if let Some(arg_str) = arg.to_str()
                && arg_str == "--background"
            {
                is_background = true;
            }
        }

        if !is_background {
            app.activate();
        } else {
            *app_hold_cmd.borrow_mut() = Some(app.hold()); // CRITICAL: Stop GTK from auto-exiting since
            let config_bg = _config_clone.clone();
            crate::timer_controller::start_prayer_timer(config_bg, |_| {});
            crate::background::setup_background();
        }

        0
    });

    let config_activate = config.clone();
    let app_hold_activate = app_hold.clone();
    app.connect_activate(move |app| {
        if config_activate.borrow().language == "ar" {
            gtk::Widget::set_default_direction(gtk::TextDirection::Rtl);
        } else {
            gtk::Widget::set_default_direction(gtk::TextDirection::Ltr);
        }

        apply_font_css(&config_activate.borrow().language);

        if !config_activate.borrow().is_configured {
            let app_clone = app.clone();
            let config_welcome = config_activate.clone();
            let config_main = config_activate.clone();
            let app_hold_welcome = app_hold_activate.clone();

            welcome::build_welcome_window(app, config_welcome, move || {
                let _ = app_hold_welcome.borrow_mut().take();
                build_main_ui(&app_clone, config_main.clone());
            });
        } else if let Some(win) = app
            .active_window()
            .or_else(|| app.windows().first().cloned())
        {
            win.present();
        } else {
            let config_main = config_activate.clone();
            let _ = app_hold_activate.borrow_mut().take();
            build_main_ui(app, config_main);
            if let Some(win) = app
                .active_window()
                .or_else(|| app.windows().first().cloned())
            {
                win.present();
            }
        }
    });
    app.run();
}

fn build_main_ui(app: &Application, config: Rc<RefCell<AppConfig>>) {
    let (loc_tx, loc_rx) = std::sync::mpsc::channel::<(f64, f64, Option<String>)>();

    if config.borrow().location_mode == LocationMode::Auto {
        let tx = loc_tx.clone();
        tokio::spawn(async move {
            if let Ok((lat, lon, name)) = location::fetch_auto_location().await {
                let _ = tx.send((lat, lon, Some(name)));
            }
        });
    }

    let initial_lang = config.borrow().language.clone();
    let current_lang = Rc::new(RefCell::new(initial_lang));

    let split_view = adw::OverlaySplitView::new();
    split_view.set_overflow(gtk::Overflow::Hidden);

    let header_bar = HeaderBar::new();
    let initial_title = tr("Prayer Times", &current_lang.borrow());
    let window_title = Label::new(Some(&initial_title));
    header_bar.set_title_widget(Some(&window_title));

    let menu_btn = Button::from_icon_name("open-menu-symbolic");
    menu_btn.set_tooltip_text(Some(&tr("Toggle Sidebar", &current_lang.borrow())));
    menu_btn.update_property(&[gtk::accessible::Property::Label(&tr(
        "Toggle Sidebar",
        &current_lang.borrow(),
    ))]);
    header_bar.pack_start(&menu_btn);

    let split_view_clone = split_view.clone();
    menu_btn.connect_clicked(move |_| {
        let is_shown = split_view_clone.shows_sidebar();
        split_view_clone.set_show_sidebar(!is_shown);
    });

    let toolbar_view = adw::ToolbarView::new();
    toolbar_view.add_top_bar(&header_bar);
    toolbar_view.set_content(Some(&split_view));
    toolbar_view.set_overflow(gtk::Overflow::Hidden);

    let compass_manager = Rc::new(CompassManager::new());
    compass_manager.start_monitoring();

    let window = ApplicationWindow::builder()
        .application(app)
        .title(tr("Khushu", &current_lang.borrow()))
        .icon_name("io.github.sniper1720.khushu")
        .default_width(1000)
        .default_height(700)
        .content(&toolbar_view)
        .build();

    let breakpoint = adw::Breakpoint::new(adw::BreakpointCondition::new_length(
        adw::BreakpointConditionLengthType::MaxWidth,
        600.0,
        adw::LengthUnit::Sp,
    ));
    breakpoint.add_setter(&split_view, "collapsed", Some(&true.to_value()));
    window.add_breakpoint(breakpoint);

    window.set_size_request(360, 360);

    let compass_manager_close = compass_manager.clone();
    window.connect_close_request(move |win| {
        compass_manager_close.stop();
        win.set_visible(false);
        crate::background::setup_background();
        gtk::glib::Propagation::Stop
    });

    let view_stack = adw::ViewStack::new();
    view_stack.set_hhomogeneous(false);
    view_stack.set_vhomogeneous(false);
    view_stack.set_vexpand(true);
    view_stack.set_hexpand(true);
    let view_stack_rc = Rc::new(view_stack);

    let sidebar_list = nav_ui::build_sidebar(&split_view, &current_lang);

    let pages_context = pages::build_pages(pages::PagesParams {
        view_stack: view_stack_rc.clone(),
        split_view: split_view.clone(),
        current_lang: current_lang.clone(),
        config: config.clone(),
        loc_tx: loc_tx.clone(),
        loc_rx,
        compass_manager: compass_manager.clone(),
        window: window.clone(),
        sidebar_list: sidebar_list.clone(),
        window_title: window_title.clone(),
    });

    nav_ui::connect_sidebar_navigation(
        &sidebar_list,
        view_stack_rc.clone(),
        &window_title,
        current_lang.clone(),
        &split_view,
        &window,
    );

    let hero = pages_context.hero_label.clone();
    let hijri = pages_context.hijri_label.clone();
    let loc = pages_context.location_label.clone();
    let lb = pages_context.list_box.clone();

    start_prayer_timer(config.clone(), move |state| {
        use timer_controller::PrayerState;
        let PrayerState {
            hero_text,
            hijri_text,
            location_text,
            next_prayer_name,
        } = state;

        hero.set_label(&hero_text);
        hijri.set_label(&hijri_text);
        loc.set_label(&location_text);

        let mut child = lb.first_child();
        while let Some(row) = child {
            if row.widget_name() == next_prayer_name {
                row.add_css_class("accent");
            } else {
                row.remove_css_class("accent");
            }
            child = row.next_sibling();
        }
    });

    start_background_tasks(
        app,
        &window,
        view_stack_rc.clone(),
        pages_context.refresh_qibla.clone(),
    );

    window.present();
}

fn show_about_window(parent: &impl IsA<gtk::Widget>, lang: &str) {
    let about = adw::AboutDialog::builder()
        .application_name(tr("Khushu", lang))
        .application_icon("io.github.sniper1720.khushu")
        .developer_name(tr("Djalel Oukid (sniper1720)", lang))
        .version("1.0.0")
        .comments(tr("An all-in-one Muslim app for Linux.", lang))
        .website("https://github.com/sniper1720/khushu")
        .issue_url("https://github.com/sniper1720/khushu/issues")
        .copyright(tr("© 2026 Djalel Oukid", lang))
        .license_type(gtk::License::Gpl30)
        .developers(vec![tr("Djalel Oukid (sniper1720)", lang)])
        .translator_credits(tr("translator-credits", lang))
        .build();

    about.add_legal_section(
            &tr("Location Policy", lang),
            None,
            gtk::License::Custom,
            Some(&tr("Auto mode: GeoClue (system). City search: Nominatim (OpenStreetMap). Manual mode: zero network traffic.", lang)),
        );
    about.add_legal_section(
            &tr("Privacy Policy", lang),
            None,
            gtk::License::Custom,
            Some(&tr("Coordinates stay on this device and are not sent to any external servers. No analytics, no telemetry, no accounts.", lang)),
        );

    about.present(Some(parent));
}

pub fn apply_font_css(lang: &str) {
    use std::cell::RefCell;

    thread_local! {
        static FONT_PROVIDER: RefCell<Option<gtk::CssProvider>> = const { RefCell::new(None) };
    }

    FONT_PROVIDER.with(|cell| {
        let mut provider_opt = cell.borrow_mut();

        if provider_opt.is_none() {
            let provider = gtk::CssProvider::new();
            gtk::style_context_add_provider_for_display(
                &gtk::gdk::Display::default().expect("Could not get default display"),
                &provider,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
            *provider_opt = Some(provider);
        }

        if let Some(provider) = provider_opt.as_ref() {
            if lang == "ar" {
                provider.load_from_data("* { font-family: 'Amiri', 'Amiri-Regular', sans-serif; }");
            } else {
                provider.load_from_data("");
            }
        }
    });
}
