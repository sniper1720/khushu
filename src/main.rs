mod adkar;
mod audio;
mod autostart;
mod background;
mod calendar;
mod config;
mod home_ui;
mod location;
mod mawaqit;
mod nav_ui;
mod notifications;
mod pages;
mod platform;
mod qibla;
mod qibla_ui;
mod quran;
mod reciter_ui;
mod settings_ui;
mod time;
mod timer_controller;
mod tz_dialog;
mod welcome;

use qibla::CompassManager;

mod i18n;
use crate::i18n::tr;
use crate::platform::{is_flatpak, is_sandboxed};
use adw::prelude::*;
use adw::{Application, ApplicationWindow, HeaderBar};
use config::{AppConfig, LocationMode};

use gtk4 as gtk;
use libadwaita as adw;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use timer_controller::start_prayer_timer;

use gtk::Button;

pub(crate) const APP_ID: &str = match option_env!("APP_ID") {
    Some(id) => id,
    None => "io.github.sniper1720.khushu",
};

pub(crate) const USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));

fn connect_notify_blocked<T, F>(
    target: &T,
    property: Option<&str>,
    f: F,
) -> gtk4::glib::SignalHandlerId
where
    T: gtk4::glib::prelude::ObjectExt,
    F: Fn(&T, &gtk4::glib::ParamSpec) + 'static,
{
    target.connect_notify_local(property, move |source, pspec| {
        let _guard = source.freeze_notify();
        f(source, pspec);
    })
}

#[tokio::main]
async fn main() {
    env_logger::init();

    let is_sandboxed = is_sandboxed();

    if !is_sandboxed {
        gtk::glib::set_prgname(Some("khushu"));
    } else {
        gtk::glib::set_prgname(Some(APP_ID));
    }

    gtk::glib::set_application_name("Khushu");

    gtk::gio::resources_register_include!("khushu-resources.gresource")
        .expect("Failed to register embedded resources");

    crate::audio::preload_builtin_audio();

    let config = AppConfig::load();

    {
        if let Some(ref path) = config.adhan_sound_path()
            && !path.starts_with("assets/")
            && !std::path::Path::new(path).exists()
        {
            log::info!("Resetting stale custom audio path: {path}");
            config.set_adhan_sound_path(None);
            config.save();
        }
    }

    crate::i18n::save_original_locale();
    crate::i18n::update_locale(&config.language());

    adw::init().expect("Failed to initialize Libadwaita");

    if !is_flatpak() {
        crate::autostart::sync(config.autostart());
    }

    crate::i18n::rebind_locale_after_adw_init();

    let application = Application::builder()
        .application_id(APP_ID)
        .flags(gtk::gio::ApplicationFlags::HANDLES_COMMAND_LINE)
        .build();

    let app_hold = Rc::new(RefCell::new(None));

    let config_startup = config.clone();
    let app_startup_clone = application.clone();
    application.connect_startup(move |_| {
        let style_manager = adw::StyleManager::default();
        match config_startup.theme() {
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
            crate::audio::stop();
            log::info!("Adhan stopped via notification action.");
        });
        app_startup_clone.add_action(&stop_adhan_action);
    });

    let app_hold_cmd = app_hold.clone();
    let _config_clone = config.clone();
    application.connect_command_line(move |app, cli| {
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
            *app_hold_cmd.borrow_mut() = Some(app.hold());
            let config_bg = _config_clone.clone();
            crate::timer_controller::start_prayer_timer(config_bg, |_| {});
            crate::background::setup_background();
        }

        0.into()
    });

    let config_activate = config.clone();
    let app_hold_activate = app_hold.clone();
    application.connect_activate(move |app| {
        if crate::i18n::is_rtl(&config_activate.language()) {
            gtk::Widget::set_default_direction(gtk::TextDirection::Rtl);
        } else {
            gtk::Widget::set_default_direction(gtk::TextDirection::Ltr);
        }

        apply_font_css(&config_activate);

        if !config_activate.is_configured() {
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
    application.run();
}

fn build_main_ui(app: &Application, config: AppConfig) {
    let (loc_tx, loc_rx) = std::sync::mpsc::channel::<(f64, f64, Option<String>)>();
    static LOCATION_EPOCH: AtomicU64 = AtomicU64::new(0);

    if config.location_mode() == LocationMode::Auto {
        let epoch = LOCATION_EPOCH.fetch_add(1, Ordering::Relaxed);
        let sender = loc_tx.clone();
        let lang = config.language();
        gtk::glib::spawn_future_local(async move {
            if LOCATION_EPOCH.load(Ordering::Relaxed) != epoch {
                return;
            }
            if let Ok((latitude, longitude, name)) = location::fetch_auto_location(&lang).await {
                let _ = sender.send((latitude, longitude, Some(name)));
            }
        });
    }

    {
        let sender = loc_tx.clone();
        connect_notify_blocked(&config, Some("location-mode"), move |cfg, _| {
            if cfg.location_mode() == LocationMode::Auto {
                let epoch = LOCATION_EPOCH.fetch_add(1, Ordering::Relaxed);
                let lang = cfg.language();
                let sender_c = sender.clone();
                gtk::glib::spawn_future_local(async move {
                    if LOCATION_EPOCH.load(Ordering::Relaxed) != epoch {
                        return;
                    }
                    if let Ok((latitude, longitude, name)) =
                        location::fetch_auto_location(&lang).await
                    {
                        let _ = sender_c.send((latitude, longitude, Some(name)));
                    }
                });
            }
        });
    }

    let initial_lang = crate::i18n::supported_language_code(&config.language());
    let current_lang = Rc::new(RefCell::new(initial_lang));

    let split_view = adw::OverlaySplitView::new();
    split_view.set_overflow(gtk::Overflow::Hidden);

    let header_bar = HeaderBar::new();
    let initial_title = tr("Prayer Times");
    let window_title = adw::WindowTitle::new(&initial_title, "");
    header_bar.set_title_widget(Some(&window_title));

    let menu_btn = Button::from_icon_name("open-menu-symbolic");
    menu_btn.set_tooltip_text(Some(&tr("Toggle Sidebar")));
    menu_btn.update_property(&[gtk::accessible::Property::Label(&tr("Toggle Sidebar"))]);
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
        .title(tr("Khushu"))
        .icon_name("io.github.sniper1720.khushu")
        .default_width(1000)
        .default_height(700)
        .width_request(360)
        .height_request(294)
        .content(&toolbar_view)
        .build();

    let breakpoint = adw::Breakpoint::new(adw::BreakpointCondition::new_length(
        adw::BreakpointConditionLengthType::MaxWidth,
        600.0,
        adw::LengthUnit::Sp,
    ));
    breakpoint.add_setter(&split_view, "collapsed", Some(&true.to_value()));
    window.add_breakpoint(breakpoint);

    let compass_manager_close = compass_manager.clone();
    window.connect_close_request(move |win| {
        compass_manager_close.stop();
        win.set_visible(false);
        crate::background::setup_background();
        gtk::glib::Propagation::Stop
    });

    let compass_manager_visible = compass_manager.clone();
    window.connect_notify_local(Some("visible"), move |win, _| {
        if win.is_visible() {
            compass_manager_visible.restart();
        }
    });

    let view_stack = adw::ViewStack::new();
    view_stack.set_hhomogeneous(false);
    view_stack.set_vhomogeneous(false);
    view_stack.set_vexpand(true);
    view_stack.set_hexpand(true);
    let view_stack_rc = Rc::new(view_stack);

    let sidebar_list = nav_ui::build_sidebar(&split_view);

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
        config.clone(),
    );

    let hero = pages_context.hero_label.clone();
    let hijri = pages_context.hijri_label.clone();
    let loc = pages_context.location_label.clone();
    let list_box = pages_context.list_box.clone();
    let stop_btn = gtk::Button::from_icon_name("media-playback-stop-symbolic");
    stop_btn.add_css_class("flat");
    stop_btn.set_tooltip_text(Some(&tr("Stop Adhan")));
    let stop_btn_rc = Rc::new(stop_btn);
    let stop_btn_for_click = stop_btn_rc.clone();
    stop_btn_for_click.connect_clicked(move |_| {
        crate::audio::stop();
    });

    start_prayer_timer(config.clone(), move |state| {
        use timer_controller::PrayerState;
        let PrayerState {
            hero_text,
            hijri_text,
            location_text,
            next_prayer_name,
            adhan_playing,
            adhan_prayer_name,
            is_iqamah,
        } = state;

        if is_iqamah {
            hero.add_css_class("warning");
        } else {
            hero.remove_css_class("warning");
        }
        hero.set_label(&hero_text);
        hijri.set_label(&hijri_text);
        loc.set_label(&location_text);

        if stop_btn_rc.parent().is_some() {
            stop_btn_rc.unparent();
        }
        stop_btn_rc.set_visible(false);

        let mut child = list_box.first_child();
        while let Some(row) = child {
            if row.widget_name() == next_prayer_name {
                row.add_css_class("accent");
            } else {
                row.remove_css_class("accent");
            }

            if adhan_playing
                && adhan_prayer_name
                    .as_deref()
                    .is_some_and(|prayer_name| row.widget_name() == prayer_name)
                && let Ok(action_row) = row.clone().downcast::<adw::ActionRow>()
            {
                stop_btn_rc.set_tooltip_text(Some(&tr("Stop Adhan")));
                stop_btn_rc.set_visible(true);
                action_row.add_suffix(&*stop_btn_rc);
            }
            child = row.next_sibling();
        }
    });

    window.present();
}

fn show_about_window(parent: &impl IsA<gtk::Widget>) {
    let about = adw::AboutDialog::builder()
        .application_name(tr("Khushu"))
        .application_icon("io.github.sniper1720.khushu")
        .developer_name(tr("Djalel Oukid (sniper1720)"))
        .version(env!("CARGO_PKG_VERSION"))
        .comments(tr("An all-in-one Muslim app for Linux"))
        .website("https://github.com/sniper1720/khushu")
        .issue_url("https://github.com/sniper1720/khushu/issues")
        .copyright(tr("© 2026 Djalel Oukid"))
        .license_type(gtk::License::Gpl30)
        .developers(vec![tr("Djalel Oukid (sniper1720)")])
        .translator_credits(tr("translator-credits"))
        .build();

    about.add_legal_section(
            &tr("Location Policy"),
            None,
            gtk::License::Custom,
            Some(&tr("Auto mode: XDG Desktop Portal (GeoClue). City search: Nominatim (OpenStreetMap). Manual mode: zero network traffic.")),
        );
    about.add_legal_section(
            &tr("Privacy Policy"),
            None,
            gtk::License::Custom,
            Some(&tr("Coordinates stay on this device and are not sent to any external servers. No analytics, no telemetry, no accounts.")),
        );
    about.add_legal_section(
        &tr("Quran Text, Translations & Recitations"),
        None,
        gtk::License::Custom,
        Some(&tr("Arabic text from Tanzil.net. English, French, Spanish, and Turkish translations from Tanzil.net. Indonesian translation from QuranEnc.com (Encyclopedia of the Noble Quran). Recitation audio provided by VerseByVerse Quran.")),
    );
    about.add_legal_section(
        &tr("Quran Translations Disclaimer"),
        None,
        gtk::License::Custom,
        Some(&tr("No translation of Quran can be a hundred percent accurate, nor it can be used as a replacement of the Quran text. We got Quran translations from Tanzil.net and QuranEnc.com websites, we cannot guarantee their authenticity and/or accuracy. Please use them at your own discretion.")),
    );

    about.present(Some(parent));
}

pub fn generate_font_css(
    ui_font: Option<&str>,
    arabic_font: Option<&str>,
    quran_font: Option<&str>,
    arabic_px: f64,
    trans_px: f64,
    line_height: f64,
) -> String {
    let mut css = String::new();

    if let Some(family) = ui_font.filter(|font| !font.is_empty()) {
        css.push_str(&format!(
            "window, popover.background {{ font-family: {family}, sans-serif; }}\n"
        ));
    }

    if let Some(family) = arabic_font.filter(|font| !font.is_empty()) {
        css.push_str(&format!(
            ".arabic-text {{ font-family: {family}, sans-serif; }}\n"
        ));
    }

    let quran_family = quran_font
        .filter(|font| !font.is_empty())
        .unwrap_or("'Amiri Quran'");
    css.push_str(&format!(
        ".marker-row {{ padding: 8px 12px; }}\n\
.quran-highlight {{ background-color: alpha(@accent_bg_color, 0.25); border-radius: 12px; }}\n\
.quran-highlight.quran-search-flash {{ animation-name: khushu-search-flash; animation-duration: 2s; }}\n\
@keyframes khushu-search-flash {{ 0% {{ background-color: alpha(@accent_bg_color, 0.60); }} 100% {{ background-color: alpha(@accent_bg_color, 0.25); }} }}\n\
.quran-arabic {{ font-family: {quran_family}, sans-serif; font-size: {arabic_px}px; line-height: {line_height}; }}\n\
.quran-translation {{ font-size: {trans_px}px; line-height: {line_height}; }}\n\
.quran-arabic-caption {{ font-family: {quran_family}, sans-serif; }}\n"
    ));

    css
}

pub fn apply_font_css(config: &crate::config::AppConfig) {
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
            let arabic_px = config.quran_arabic_font_px().clamp(16.0, 40.0);
            let trans_px = config.quran_translation_font_px().clamp(10.0, 28.0);
            let line_height = config.quran_line_height().clamp(1.0, 2.6);

            let css = generate_font_css(
                config.ui_font_family().as_deref(),
                config.arabic_font_family().as_deref(),
                config.quran_font_family().as_deref(),
                arabic_px,
                trans_px,
                line_height,
            );
            provider.load_from_data(&css);
            add_calendar_compact_styles();
        }
    });
}

pub fn add_calendar_compact_styles() {
    thread_local! {
        static CALENDAR_PROVIDER: std::cell::RefCell<Option<gtk::CssProvider>> = const { std::cell::RefCell::new(None) };
    }

    CALENDAR_PROVIDER.with(|cell| {
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
            let css = "\
.calendar-grid button { min-height: 24px; min-width: 24px; font-size: 0.9em; padding: 0; }\
.calendar-grid.compact-calendar button { min-height: 20px; min-width: 20px; font-size: 0.8em; }\
.calendar-grid.compact-calendar .dim-label { font-size: 0.8em; }\
";
            provider.load_from_data(css);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_font_css_system_defaults() {
        let css = generate_font_css(None, None, None, 22.0, 14.0, 1.0);
        assert!(!css.contains("window { font-family:"));
        assert!(!css.contains(".arabic-text { font-family:"));
        assert!(css.contains(".quran-arabic { font-family: 'Amiri Quran', sans-serif;"));
        assert!(css.contains(".quran-arabic-caption { font-family: 'Amiri Quran', sans-serif;"));
    }

    #[test]
    fn test_generate_font_css_custom_ui_font() {
        let css = generate_font_css(Some("CustomFont"), None, None, 22.0, 14.0, 1.0);
        assert!(css.contains("window, popover.background { font-family: CustomFont, sans-serif;"));
        assert!(!css.contains(".arabic-text { font-family:"));
    }

    #[test]
    fn test_generate_font_css_custom_arabic_font() {
        let css = generate_font_css(None, Some("Amiri"), None, 22.0, 14.0, 1.0);
        assert!(!css.contains("window { font-family:"));
        assert!(css.contains(".arabic-text { font-family: Amiri, sans-serif;"));
    }

    #[test]
    fn test_generate_font_css_custom_quran_font() {
        let css = generate_font_css(None, None, Some("Uthmani"), 22.0, 14.0, 1.5);
        assert!(css.contains(".quran-arabic { font-family: Uthmani, sans-serif;"));
        assert!(css.contains(".quran-arabic-caption { font-family: Uthmani, sans-serif;"));
        assert!(css.contains("font-size: 22px"));
        assert!(css.contains("line-height: 1.5"));
        assert!(css.contains(".quran-translation { font-size: 14px"));
    }

    #[test]
    fn test_generate_font_css_no_direction_rules() {
        let css = generate_font_css(Some("A"), Some("B"), Some("C"), 22.0, 14.0, 1.0);
        assert!(!css.contains("direction:"));
        assert!(!css.contains("rtl"));
    }
}
