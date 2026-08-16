use adw::prelude::*;
use gtk4 as gtk;
use libadwaita as adw;
use std::cell::RefCell;
use std::rc::Rc;

use crate::config::AppConfig;
use crate::i18n::tr;
use crate::quran_planner::{
    ConfigPlanStore, DailyStatus, HifzGoalType, HifzPlanData, HifzUnit, PlanStore, ReadingPlanData,
    ReadingUnit, SetupMode, calculate_hifz_workload, calculate_reading_workload,
    create_new_hifz_plan, create_new_reading_plan, format_hifz_goal_title, get_active_daily_record,
    get_active_hifz_record, map_hifz_goal_scope,
};

#[derive(Clone, Debug)]
enum KhatmaWizardStep {
    Dashboard,
    EmptyState,
    SetupModeChoice,
    ConfigureParameters {
        setup_mode: SetupMode,
    },
    ReviewSummary {
        setup_mode: SetupMode,
        target_days: u32,
        daily_amount: u32,
        unit: ReadingUnit,
        start_page: u32,
        end_page: u32,
    },
}

#[derive(Clone, Debug)]
enum HifzWizardStep {
    Dashboard,
    EmptyState,
    ChooseGoal,
    ConfigureParameters {
        goal_type: HifzGoalType,
    },
    ReviewSummary {
        goal_type: HifzGoalType,
        setup_mode: SetupMode,
        target_days: u32,
        sabaq_unit: HifzUnit,
        sabqi_window: u32,
        manzil_cycle: u32,
    },
}

pub fn create_planner_page(
    view_stack: &adw::ViewStack,
    config: AppConfig,
    current_lang: &str,
) -> gtk::Widget {
    let main_box = gtk::Box::new(gtk::Orientation::Vertical, 0);

    let switcher_bar = adw::ViewSwitcher::new();
    let sub_stack = adw::ViewStack::new();

    let khatma_widget = create_khatma_section(view_stack, config.clone(), current_lang);
    let hifz_widget = create_hifz_section(view_stack, config.clone(), current_lang);

    let khatma_page = sub_stack.add_named(&khatma_widget, Some("khatma"));
    khatma_page.set_title(Some(&tr("Reading Khatma")));
    khatma_page.set_icon_name(Some("emblem-documents-symbolic"));

    let hifz_page = sub_stack.add_named(&hifz_widget, Some("hifz"));
    hifz_page.set_title(Some(&tr("Hifz Planner")));
    hifz_page.set_icon_name(Some("starred-symbolic"));

    switcher_bar.set_stack(Some(&sub_stack));
    switcher_bar.set_margin_top(10);
    switcher_bar.set_margin_bottom(10);
    switcher_bar.set_margin_start(16);
    switcher_bar.set_margin_end(16);

    main_box.append(&switcher_bar);
    main_box.append(&sub_stack);

    main_box.upcast::<gtk::Widget>()
}

fn create_khatma_section(
    view_stack: &adw::ViewStack,
    config: AppConfig,
    lang: &str,
) -> gtk::Widget {
    let clamp = adw::Clamp::builder().maximum_size(720).build();
    let scroll = gtk::ScrolledWindow::builder().vexpand(true).build();

    let content_box = gtk::Box::new(gtk::Orientation::Vertical, 16);
    content_box.set_margin_start(16);
    content_box.set_margin_end(16);
    content_box.set_margin_top(16);
    content_box.set_margin_bottom(24);

    let step_state = Rc::new(RefCell::new(KhatmaWizardStep::Dashboard));

    refresh_khatma_view(&content_box, view_stack, &config, lang, step_state);

    clamp.set_child(Some(&content_box));
    scroll.set_child(Some(&clamp));
    scroll.upcast::<gtk::Widget>()
}

fn refresh_khatma_view(
    container: &gtk::Box,
    view_stack: &adw::ViewStack,
    config: &AppConfig,
    lang: &str,
    step_state: Rc<RefCell<KhatmaWizardStep>>,
) {
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }

    let store = ConfigPlanStore::new(config);
    let plans = store.load_reading_plans();
    let active_plan = plans.into_iter().find(|p| p.is_active && !p.is_archived);

    let current_step = step_state.borrow().clone();

    match current_step {
        KhatmaWizardStep::Dashboard => {
            if let Some(plan) = active_plan {
                render_active_khatma_dashboard(
                    container, view_stack, config, lang, plan, step_state,
                );
            } else {
                render_empty_khatma_state(container, view_stack, config, lang, step_state);
            }
        }
        KhatmaWizardStep::EmptyState => {
            render_empty_khatma_state(container, view_stack, config, lang, step_state);
        }
        KhatmaWizardStep::SetupModeChoice => {
            render_khatma_mode_choice(container, view_stack, config, lang, step_state);
        }
        KhatmaWizardStep::ConfigureParameters { setup_mode } => {
            render_khatma_configure_params(
                container, view_stack, config, lang, setup_mode, step_state,
            );
        }
        KhatmaWizardStep::ReviewSummary {
            setup_mode,
            target_days,
            daily_amount,
            unit,
            start_page,
            end_page,
        } => {
            render_khatma_review_summary(
                container,
                view_stack,
                config,
                lang,
                setup_mode,
                target_days,
                daily_amount,
                unit,
                start_page,
                end_page,
                step_state,
            );
        }
    }
}

fn render_empty_khatma_state(
    container: &gtk::Box,
    view_stack: &adw::ViewStack,
    config: &AppConfig,
    lang: &str,
    step_state: Rc<RefCell<KhatmaWizardStep>>,
) {
    let empty_card = gtk::Box::new(gtk::Orientation::Vertical, 0);
    empty_card.add_css_class("card");
    empty_card.set_margin_start(16);
    empty_card.set_margin_end(16);
    empty_card.set_margin_top(16);
    empty_card.set_margin_bottom(16);

    let inner_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
    inner_box.set_margin_start(28);
    inner_box.set_margin_end(28);
    inner_box.set_margin_top(32);
    inner_box.set_margin_bottom(32);

    let icon = gtk::Image::from_icon_name("emblem-documents-symbolic");
    icon.set_pixel_size(48);
    icon.set_margin_bottom(16);
    inner_box.append(&icon);

    let heading = gtk::Label::builder()
        .label(&tr("No active Reading Khatma"))
        .css_classes(["title-2"])
        .margin_bottom(8)
        .build();

    let subtitle = gtk::Label::builder()
        .label(&tr(
            "Start your Quran reading journey by creating a customized daily reading plan.",
        ))
        .css_classes(["body", "dim-label"])
        .wrap(true)
        .justify(gtk::Justification::Center)
        .margin_bottom(28)
        .build();

    inner_box.append(&heading);
    inner_box.append(&subtitle);

    let create_btn = gtk::Button::builder()
        .label(&tr("Create a Reading Khatma"))
        .css_classes(["suggested-action"])
        .halign(gtk::Align::Center)
        .build();

    let container_clone = container.clone();
    let view_stack_clone = view_stack.clone();
    let config_clone = config.clone();
    let lang_owned = lang.to_string();
    let state_clone = step_state;

    create_btn.connect_clicked(move |_| {
        *state_clone.borrow_mut() = KhatmaWizardStep::SetupModeChoice;
        refresh_khatma_view(
            &container_clone,
            &view_stack_clone,
            &config_clone,
            &lang_owned,
            state_clone.clone(),
        );
    });

    inner_box.append(&create_btn);
    empty_card.append(&inner_box);
    container.append(&empty_card);
}

fn render_khatma_mode_choice(
    container: &gtk::Box,
    view_stack: &adw::ViewStack,
    config: &AppConfig,
    lang: &str,
    step_state: Rc<RefCell<KhatmaWizardStep>>,
) {
    let card = gtk::Box::new(gtk::Orientation::Vertical, 0);
    card.add_css_class("card");
    card.set_margin_start(16);
    card.set_margin_end(16);
    card.set_margin_top(16);
    card.set_margin_bottom(16);

    let inner_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
    inner_box.set_margin_start(24);
    inner_box.set_margin_end(24);
    inner_box.set_margin_top(24);
    inner_box.set_margin_bottom(24);

    let heading = gtk::Label::builder()
        .label(&tr("Reading Khatma Goal"))
        .css_classes(["title-2"])
        .halign(gtk::Align::Start)
        .margin_bottom(16)
        .build();
    inner_box.append(&heading);

    let group = adw::PreferencesGroup::builder()
        .title(tr("How would you like to set up your plan?"))
        .build();

    let row_target_date = adw::ActionRow::builder()
        .title(tr("Finish by target date"))
        .subtitle(tr(
            "Calculate daily reading required to finish by a specific date.",
        ))
        .activatable(true)
        .build();

    let row_daily_amount = adw::ActionRow::builder()
        .title(tr("Read a fixed amount each day"))
        .subtitle(tr(
            "Set a fixed daily target (Pages, Hizb, or Juz) and calculate end date.",
        ))
        .activatable(true)
        .build();

    group.add(&row_target_date);
    group.add(&row_daily_amount);
    inner_box.append(&group);

    let container_clone = container.clone();
    let view_stack_clone = view_stack.clone();
    let config_clone = config.clone();
    let lang_owned = lang.to_string();
    let state_clone1 = step_state.clone();

    row_target_date.connect_activated(move |_| {
        *state_clone1.borrow_mut() = KhatmaWizardStep::ConfigureParameters {
            setup_mode: SetupMode::ByTargetDate,
        };
        refresh_khatma_view(
            &container_clone,
            &view_stack_clone,
            &config_clone,
            &lang_owned,
            state_clone1.clone(),
        );
    });

    let container_clone2 = container.clone();
    let view_stack_clone2 = view_stack.clone();
    let config_clone2 = config.clone();
    let lang_owned2 = lang.to_string();
    let state_clone2 = step_state;

    row_daily_amount.connect_activated(move |_| {
        *state_clone2.borrow_mut() = KhatmaWizardStep::ConfigureParameters {
            setup_mode: SetupMode::ByDailyAmount,
        };
        refresh_khatma_view(
            &container_clone2,
            &view_stack_clone2,
            &config_clone2,
            &lang_owned2,
            state_clone2.clone(),
        );
    });

    card.append(&inner_box);
    container.append(&card);
}

fn render_khatma_configure_params(
    container: &gtk::Box,
    view_stack: &adw::ViewStack,
    config: &AppConfig,
    lang: &str,
    setup_mode: SetupMode,
    step_state: Rc<RefCell<KhatmaWizardStep>>,
) {
    let card = gtk::Box::new(gtk::Orientation::Vertical, 0);
    card.add_css_class("card");
    card.set_margin_start(16);
    card.set_margin_end(16);
    card.set_margin_top(16);
    card.set_margin_bottom(16);

    let inner_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
    inner_box.set_margin_start(24);
    inner_box.set_margin_end(24);
    inner_box.set_margin_top(24);
    inner_box.set_margin_bottom(24);

    let heading = gtk::Label::builder()
        .label(&tr("Configure Plan Parameters"))
        .css_classes(["title-2"])
        .halign(gtk::Align::Start)
        .margin_bottom(16)
        .build();
    inner_box.append(&heading);

    let group = adw::PreferencesGroup::new();

    let duration_spin = gtk::SpinButton::with_range(1.0, 365.0, 1.0);
    duration_spin.set_value(30.0);
    let duration_row = adw::ActionRow::builder()
        .title(tr("Target Duration (Days)"))
        .build();
    duration_row.add_suffix(&duration_spin);

    let unit_combo = adw::ComboRow::builder()
        .title(tr("Daily Reading Unit"))
        .model(&gtk::StringList::new(&[
            &tr("Pages"),
            &tr("Hizb"),
            &tr("Juz"),
        ]))
        .build();

    let daily_amount_spin = gtk::SpinButton::with_range(1.0, 60.0, 1.0);
    daily_amount_spin.set_value(20.0);
    let daily_amount_row = adw::ActionRow::builder()
        .title(tr("Daily Target Amount"))
        .build();
    daily_amount_row.add_suffix(&daily_amount_spin);

    match setup_mode {
        SetupMode::ByTargetDate => {
            group.add(&duration_row);
        }
        SetupMode::ByDailyAmount => {
            group.add(&unit_combo);
            group.add(&daily_amount_row);
        }
    }

    let start_pos_combo = adw::ComboRow::builder()
        .title(tr("Start Position"))
        .model(&gtk::StringList::new(&[
            &tr("Beginning of Quran (Page 1)"),
            &tr("Current Reading Position"),
        ]))
        .build();
    group.add(&start_pos_combo);

    inner_box.append(&group);

    let next_btn = gtk::Button::builder()
        .label(&tr("Review Plan"))
        .css_classes(["suggested-action"])
        .halign(gtk::Align::End)
        .margin_top(20)
        .build();

    let container_clone = container.clone();
    let view_stack_clone = view_stack.clone();
    let config_clone = config.clone();
    let lang_owned = lang.to_string();
    let state_clone = step_state;

    next_btn.connect_clicked(move |_| {
        let start_page = if start_pos_combo.selected() == 1 {
            config_clone.quran_last_page().unwrap_or(1)
        } else {
            1
        };

        let target_days = duration_spin.value() as u32;
        let daily_amount = daily_amount_spin.value() as u32;
        let unit = match unit_combo.selected() {
            1 => ReadingUnit::Hizb,
            2 => ReadingUnit::Juz,
            _ => ReadingUnit::Pages,
        };

        *state_clone.borrow_mut() = KhatmaWizardStep::ReviewSummary {
            setup_mode: setup_mode.clone(),
            target_days,
            daily_amount,
            unit,
            start_page,
            end_page: 604,
        };
        refresh_khatma_view(
            &container_clone,
            &view_stack_clone,
            &config_clone,
            &lang_owned,
            state_clone.clone(),
        );
    });

    inner_box.append(&next_btn);
    card.append(&inner_box);
    container.append(&card);
}

fn render_khatma_review_summary(
    container: &gtk::Box,
    view_stack: &adw::ViewStack,
    config: &AppConfig,
    lang: &str,
    setup_mode: SetupMode,
    target_days: u32,
    daily_amount: u32,
    unit: ReadingUnit,
    start_page: u32,
    end_page: u32,
    step_state: Rc<RefCell<KhatmaWizardStep>>,
) {
    let today = chrono::Local::now().date_naive();
    let (calc_days, calc_daily, expected_end, history) = calculate_reading_workload(
        start_page,
        end_page,
        &setup_mode,
        target_days,
        daily_amount,
        &unit,
        today,
    );

    let card = gtk::Box::new(gtk::Orientation::Vertical, 0);
    card.add_css_class("card");
    card.set_margin_start(16);
    card.set_margin_end(16);
    card.set_margin_top(16);
    card.set_margin_bottom(16);

    let inner_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
    inner_box.set_margin_start(24);
    inner_box.set_margin_end(24);
    inner_box.set_margin_top(24);
    inner_box.set_margin_bottom(24);

    let heading = gtk::Label::builder()
        .label(&tr("Review Your Reading Plan"))
        .css_classes(["title-2"])
        .halign(gtk::Align::Start)
        .margin_bottom(16)
        .build();
    inner_box.append(&heading);

    let group = adw::PreferencesGroup::new();
    let row_duration = adw::ActionRow::builder()
        .title(tr("Planned Duration"))
        .subtitle(&format!("{} {}", calc_days, tr("days")))
        .build();
    let row_daily = adw::ActionRow::builder()
        .title(tr("Daily Target"))
        .subtitle(&format!("~{} {}", calc_daily, tr("pages/day")))
        .build();
    let row_start = adw::ActionRow::builder()
        .title(tr("Start Page"))
        .subtitle(&format!("{} {}", tr("Page"), start_page))
        .build();
    let row_end = adw::ActionRow::builder()
        .title(tr("Expected Completion"))
        .subtitle(&expected_end.format("%Y-%m-%d").to_string())
        .build();

    group.add(&row_duration);
    group.add(&row_daily);
    group.add(&row_start);
    group.add(&row_end);
    inner_box.append(&group);

    let btn_box = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    btn_box.set_halign(gtk::Align::End);
    btn_box.set_margin_top(20);

    let back_btn = gtk::Button::builder().label(&tr("Back")).build();
    let create_btn = gtk::Button::builder()
        .label(&tr("Create Plan"))
        .css_classes(["suggested-action"])
        .build();

    btn_box.append(&back_btn);
    btn_box.append(&create_btn);

    let container_clone = container.clone();
    let view_stack_clone = view_stack.clone();
    let config_clone = config.clone();
    let lang_owned = lang.to_string();
    let state_clone1 = step_state.clone();

    back_btn.connect_clicked(move |_| {
        *state_clone1.borrow_mut() = KhatmaWizardStep::SetupModeChoice;
        refresh_khatma_view(
            &container_clone,
            &view_stack_clone,
            &config_clone,
            &lang_owned,
            state_clone1.clone(),
        );
    });

    let container_clone2 = container.clone();
    let view_stack_clone2 = view_stack.clone();
    let config_clone2 = config.clone();
    let lang_owned2 = lang.to_string();
    let state_clone2 = step_state;

    let setup_mode_clone = setup_mode.clone();
    let unit_clone = unit.clone();
    let history_clone = history.clone();

    create_btn.connect_clicked(move |_| {
        let store = ConfigPlanStore::new(&config_clone2);
        let plan = create_new_reading_plan(
            tr("Reading Khatma Plan"),
            setup_mode_clone.clone(),
            unit_clone.clone(),
            start_page,
            end_page,
            calc_days,
            calc_daily,
            today,
            expected_end,
            history_clone.clone(),
        );

        let mut existing = store.load_reading_plans();
        for p in &mut existing {
            p.is_active = false;
        }
        existing.push(plan);
        store.save_reading_plans(&existing);

        *state_clone2.borrow_mut() = KhatmaWizardStep::Dashboard;
        refresh_khatma_view(
            &container_clone2,
            &view_stack_clone2,
            &config_clone2,
            &lang_owned2,
            state_clone2.clone(),
        );
    });

    inner_box.append(&btn_box);
    card.append(&inner_box);
    container.append(&card);
}

fn render_active_khatma_dashboard(
    container: &gtk::Box,
    view_stack: &adw::ViewStack,
    config: &AppConfig,
    lang: &str,
    plan: ReadingPlanData,
    step_state: Rc<RefCell<KhatmaWizardStep>>,
) {
    let card = gtk::Box::new(gtk::Orientation::Vertical, 0);
    card.add_css_class("card");
    card.set_margin_start(16);
    card.set_margin_end(16);
    card.set_margin_top(16);
    card.set_margin_bottom(16);

    let inner_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
    inner_box.set_margin_start(24);
    inner_box.set_margin_end(24);
    inner_box.set_margin_top(24);
    inner_box.set_margin_bottom(24);

    let current_record = get_active_daily_record(&plan);

    let title_label = gtk::Label::builder()
        .label(&plan.title)
        .css_classes(["title-2", "accent"])
        .halign(gtk::Align::Start)
        .margin_bottom(12)
        .build();
    inner_box.append(&title_label);

    let is_all_completed = plan
        .history
        .iter()
        .all(|r| r.status == DailyStatus::Completed);
    let target_range_text = if is_all_completed {
        tr("Plan Completed! Alhamdulillah.")
    } else {
        format!("{} {}", tr("Today's Range:"), current_record.range_label)
    };
    let range_label = gtk::Label::builder()
        .label(&target_range_text)
        .css_classes(["title-3"])
        .halign(gtk::Align::Start)
        .margin_bottom(16)
        .build();
    inner_box.append(&range_label);

    let progress_bar = gtk::ProgressBar::new();
    let completed_count = plan
        .history
        .iter()
        .filter(|r| r.status == DailyStatus::Completed)
        .count();
    let fraction = if !plan.history.is_empty() {
        completed_count as f64 / plan.history.len() as f64
    } else {
        0.0
    };
    progress_bar.set_fraction(fraction);
    progress_bar.set_show_text(true);
    progress_bar.set_text(Some(&format!(
        "{} {} / {} {}",
        tr("Day"),
        current_record.day_index,
        plan.history.len(),
        tr("days")
    )));
    progress_bar.set_margin_bottom(20);
    inner_box.append(&progress_bar);

    let btn_box = gtk::Box::new(gtk::Orientation::Horizontal, 12);

    let continue_btn = gtk::Button::builder()
        .label(&tr("Start Today's Reading"))
        .css_classes(["suggested-action"])
        .build();

    let target_page = current_record.start_page;
    let view_stack_clone = view_stack.clone();
    let config_clone = config.clone();
    let lang_owned = lang.to_string();
    continue_btn.connect_clicked(move |_| {
        crate::quran::open_surah_at_page(
            &view_stack_clone,
            &lang_owned,
            config_clone.clone(),
            target_page,
        );
    });
    btn_box.append(&continue_btn);

    let complete_btn_label = if current_record.status == DailyStatus::Completed {
        tr("Completed ✓")
    } else {
        tr("Mark Completed")
    };
    let complete_btn = gtk::Button::builder().label(&complete_btn_label).build();
    let config_mark = config.clone();
    let plan_id = plan.id.clone();
    let record_day = current_record.day_index;
    let container_clone = container.clone();
    let view_stack_mark = view_stack.clone();
    let lang_mark = lang.to_string();
    let state_mark = step_state.clone();

    complete_btn.connect_clicked(move |_| {
        let store = ConfigPlanStore::new(&config_mark);
        let mut plans = store.load_reading_plans();
        if let Some(p) = plans.iter_mut().find(|p| p.id == plan_id) {
            if let Some(r) = p.history.iter_mut().find(|r| r.day_index == record_day) {
                r.status = DailyStatus::Completed;
                r.completed_at = Some(chrono::Local::now().to_rfc3339());
            }
            store.save_reading_plans(&plans);
        }
        refresh_khatma_view(
            &container_clone,
            &view_stack_mark,
            &config_mark,
            &lang_mark,
            state_mark.clone(),
        );
    });
    btn_box.append(&complete_btn);

    let archive_btn = gtk::Button::builder().label(&tr("Archive Plan")).build();
    let config_arch = config.clone();
    let plan_id_arch = plan.id;
    let container_arch = container.clone();
    let view_stack_arch = view_stack.clone();
    let lang_arch = lang.to_string();
    let state_arch = step_state;

    archive_btn.connect_clicked(move |_| {
        let store = ConfigPlanStore::new(&config_arch);
        let mut plans = store.load_reading_plans();
        if let Some(p) = plans.iter_mut().find(|p| p.id == plan_id_arch) {
            p.is_active = false;
            p.is_archived = true;
        }
        store.save_reading_plans(&plans);
        *state_arch.borrow_mut() = KhatmaWizardStep::EmptyState;
        refresh_khatma_view(
            &container_arch,
            &view_stack_arch,
            &config_arch,
            &lang_arch,
            state_arch.clone(),
        );
    });
    btn_box.append(&archive_btn);

    inner_box.append(&btn_box);
    card.append(&inner_box);
    container.append(&card);
}

fn create_hifz_section(view_stack: &adw::ViewStack, config: AppConfig, lang: &str) -> gtk::Widget {
    let clamp = adw::Clamp::builder().maximum_size(720).build();
    let scroll = gtk::ScrolledWindow::builder().vexpand(true).build();

    let content_box = gtk::Box::new(gtk::Orientation::Vertical, 16);
    content_box.set_margin_start(16);
    content_box.set_margin_end(16);
    content_box.set_margin_top(16);
    content_box.set_margin_bottom(24);

    let step_state = Rc::new(RefCell::new(HifzWizardStep::Dashboard));

    refresh_hifz_view(&content_box, view_stack, &config, lang, step_state);

    clamp.set_child(Some(&content_box));
    scroll.set_child(Some(&clamp));
    scroll.upcast::<gtk::Widget>()
}

fn refresh_hifz_view(
    container: &gtk::Box,
    view_stack: &adw::ViewStack,
    config: &AppConfig,
    lang: &str,
    step_state: Rc<RefCell<HifzWizardStep>>,
) {
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }

    let store = ConfigPlanStore::new(config);
    let plans = store.load_hifz_plans();
    let active_plan = plans.into_iter().find(|p| p.is_active && !p.is_archived);

    let current_step = step_state.borrow().clone();

    match current_step {
        HifzWizardStep::Dashboard => {
            if let Some(plan) = active_plan {
                render_active_hifz_dashboard(container, view_stack, config, lang, plan, step_state);
            } else {
                render_empty_hifz_state(container, view_stack, config, lang, step_state);
            }
        }
        HifzWizardStep::EmptyState => {
            render_empty_hifz_state(container, view_stack, config, lang, step_state);
        }
        HifzWizardStep::ChooseGoal => {
            render_hifz_choose_goal(container, view_stack, config, lang, step_state);
        }
        HifzWizardStep::ConfigureParameters { goal_type } => {
            render_hifz_configure_params(
                container, view_stack, config, lang, goal_type, step_state,
            );
        }
        HifzWizardStep::ReviewSummary {
            goal_type,
            setup_mode,
            target_days,
            sabaq_unit,
            sabqi_window,
            manzil_cycle,
        } => {
            render_hifz_review_summary(
                container,
                view_stack,
                config,
                lang,
                goal_type,
                setup_mode,
                target_days,
                sabaq_unit,
                sabqi_window,
                manzil_cycle,
                step_state,
            );
        }
    }
}

fn render_empty_hifz_state(
    container: &gtk::Box,
    view_stack: &adw::ViewStack,
    config: &AppConfig,
    lang: &str,
    step_state: Rc<RefCell<HifzWizardStep>>,
) {
    let empty_card = gtk::Box::new(gtk::Orientation::Vertical, 0);
    empty_card.add_css_class("card");
    empty_card.set_margin_start(16);
    empty_card.set_margin_end(16);
    empty_card.set_margin_top(16);
    empty_card.set_margin_bottom(16);

    let inner_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
    inner_box.set_margin_start(28);
    inner_box.set_margin_end(28);
    inner_box.set_margin_top(32);
    inner_box.set_margin_bottom(32);

    let icon = gtk::Image::from_icon_name("starred-symbolic");
    icon.set_pixel_size(48);
    icon.set_margin_bottom(16);
    inner_box.append(&icon);

    let heading = gtk::Label::builder()
        .label(&tr("No active Hifz Plan"))
        .css_classes(["title-2"])
        .margin_bottom(8)
        .build();

    let subtitle = gtk::Label::builder()
        .label(&tr(
            "Create a structured memorization and revision plan incorporating Sabaq, Sabqi, and Manzil.",
        ))
        .css_classes(["body", "dim-label"])
        .wrap(true)
        .justify(gtk::Justification::Center)
        .margin_bottom(28)
        .build();

    inner_box.append(&heading);
    inner_box.append(&subtitle);

    let create_btn = gtk::Button::builder()
        .label(&tr("Create a Hifz Plan"))
        .css_classes(["suggested-action"])
        .halign(gtk::Align::Center)
        .build();

    let container_clone = container.clone();
    let view_stack_clone = view_stack.clone();
    let config_clone = config.clone();
    let lang_owned = lang.to_string();
    let state_clone = step_state;

    create_btn.connect_clicked(move |_| {
        *state_clone.borrow_mut() = HifzWizardStep::ChooseGoal;
        refresh_hifz_view(
            &container_clone,
            &view_stack_clone,
            &config_clone,
            &lang_owned,
            state_clone.clone(),
        );
    });

    inner_box.append(&create_btn);
    empty_card.append(&inner_box);
    container.append(&empty_card);
}

fn render_hifz_choose_goal(
    container: &gtk::Box,
    view_stack: &adw::ViewStack,
    config: &AppConfig,
    lang: &str,
    step_state: Rc<RefCell<HifzWizardStep>>,
) {
    let card = gtk::Box::new(gtk::Orientation::Vertical, 0);
    card.add_css_class("card");
    card.set_margin_start(16);
    card.set_margin_end(16);
    card.set_margin_top(16);
    card.set_margin_bottom(16);

    let inner_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
    inner_box.set_margin_start(24);
    inner_box.set_margin_end(24);
    inner_box.set_margin_top(24);
    inner_box.set_margin_bottom(24);

    let heading = gtk::Label::builder()
        .label(&tr("Hifz Memorization Goal"))
        .css_classes(["title-2"])
        .halign(gtk::Align::Start)
        .margin_bottom(4)
        .build();
    inner_box.append(&heading);

    let subtitle = gtk::Label::builder()
        .label(&tr(
            "Select what portion of the Quran you want to memorize.",
        ))
        .css_classes(["body", "dim-label"])
        .halign(gtk::Align::Start)
        .margin_bottom(16)
        .build();
    inner_box.append(&subtitle);

    let group = adw::PreferencesGroup::builder()
        .title(tr("Memorization Scope"))
        .build();

    let scope_combo = adw::ComboRow::builder()
        .title(tr("Select Scope"))
        .model(&gtk::StringList::new(&[
            &tr("Full Quran"),
            &tr("Single Juz"),
            &tr("Juz Range"),
            &tr("Single Surah"),
            &tr("Surah Range"),
            &tr("Custom Page Range"),
        ]))
        .build();
    group.add(&scope_combo);
    inner_box.append(&group);

    let dynamic_controls_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
    inner_box.append(&dynamic_controls_box);

    let validation_label = gtk::Label::builder()
        .css_classes(["error", "caption"])
        .halign(gtk::Align::Start)
        .margin_top(8)
        .visible(false)
        .build();
    inner_box.append(&validation_label);

    let preview_group = adw::PreferencesGroup::builder()
        .title(tr("Live Scope Summary"))
        .margin_top(16)
        .build();

    let preview_row = adw::ActionRow::builder()
        .title(tr("Selected Goal Scope"))
        .subtitle(tr("Pages 1–604 (604 pages)"))
        .build();
    preview_group.add(&preview_row);

    inner_box.append(&preview_group);

    let next_btn = gtk::Button::builder()
        .label(&tr("Next"))
        .css_classes(["suggested-action"])
        .halign(gtk::Align::End)
        .margin_top(20)
        .build();

    let current_goal_type = Rc::new(RefCell::new(HifzGoalType::FullQuran));

    let update_preview = {
        let preview_row = preview_row.clone();
        let goal_type_ref = current_goal_type.clone();
        let lang_owned = lang.to_string();
        move || {
            let goal = goal_type_ref.borrow().clone();
            let (start_p, end_p) = map_hifz_goal_scope(&goal);
            let count = (end_p + 1).saturating_sub(start_p);
            let title_str = format_hifz_goal_title(&goal, &lang_owned);
            preview_row.set_title(&title_str);
            preview_row.set_subtitle(&format!(
                "{}: {}–{} ({} {})",
                tr("Pages"),
                start_p,
                end_p,
                count,
                tr("pages")
            ));
        }
    };

    let surah_list = crate::quran::get_surah_display_list(lang);

    let render_dynamic_controls = {
        let dynamic_controls_box = dynamic_controls_box.clone();
        let goal_type_ref = current_goal_type.clone();
        let scope_combo = scope_combo.clone();
        let update_preview = update_preview.clone();
        let surah_list = surah_list.clone();
        let validation_label = validation_label.clone();
        let next_btn = next_btn.clone();

        move || {
            while let Some(child) = dynamic_controls_box.first_child() {
                dynamic_controls_box.remove(&child);
            }

            validation_label.set_visible(false);
            next_btn.set_sensitive(true);

            let dyn_group = adw::PreferencesGroup::builder().margin_top(12).build();

            match scope_combo.selected() {
                0 => {
                    *goal_type_ref.borrow_mut() = HifzGoalType::FullQuran;
                    update_preview();
                }
                1 => {
                    let juz_spin = gtk::SpinButton::with_range(1.0, 30.0, 1.0);
                    juz_spin.set_value(1.0);
                    let juz_row = adw::ActionRow::builder().title(tr("Juz Number")).build();
                    juz_row.add_suffix(&juz_spin);

                    let goal_ref = goal_type_ref.clone();
                    let update_prev = update_preview.clone();
                    juz_spin.connect_value_changed(move |spin| {
                        let j = spin.value() as u32;
                        *goal_ref.borrow_mut() = HifzGoalType::SelectedJuz(j);
                        update_prev();
                    });

                    *goal_type_ref.borrow_mut() = HifzGoalType::SelectedJuz(1);
                    dyn_group.add(&juz_row);
                    dynamic_controls_box.append(&dyn_group);
                    update_preview();
                }
                2 => {
                    let start_spin = gtk::SpinButton::with_range(1.0, 30.0, 1.0);
                    start_spin.set_value(1.0);
                    let end_spin = gtk::SpinButton::with_range(1.0, 30.0, 1.0);
                    end_spin.set_value(5.0);

                    let start_row = adw::ActionRow::builder().title(tr("Start Juz")).build();
                    start_row.add_suffix(&start_spin);
                    let end_row = adw::ActionRow::builder().title(tr("End Juz")).build();
                    end_row.add_suffix(&end_spin);

                    let goal_ref = goal_type_ref.clone();
                    let update_prev = update_preview.clone();
                    let start_s = start_spin.clone();
                    let end_s = end_spin.clone();
                    let val_label = validation_label.clone();
                    let nxt_btn = next_btn.clone();

                    let sync_juz_range = move || {
                        let sj = start_s.value() as u32;
                        let ej = end_s.value() as u32;
                        if sj > ej {
                            val_label
                                .set_text(&tr("Start Juz must be less than or equal to End Juz."));
                            val_label.set_visible(true);
                            nxt_btn.set_sensitive(false);
                        } else {
                            val_label.set_visible(false);
                            nxt_btn.set_sensitive(true);
                            *goal_ref.borrow_mut() = HifzGoalType::SelectedJuzRange {
                                start_juz: sj,
                                end_juz: ej,
                            };
                            update_prev();
                        }
                    };

                    let sync1 = sync_juz_range.clone();
                    start_spin.connect_value_changed(move |_| sync1());
                    let sync2 = sync_juz_range.clone();
                    end_spin.connect_value_changed(move |_| sync2());

                    *goal_type_ref.borrow_mut() = HifzGoalType::SelectedJuzRange {
                        start_juz: 1,
                        end_juz: 5,
                    };
                    dyn_group.add(&start_row);
                    dyn_group.add(&end_row);
                    dynamic_controls_box.append(&dyn_group);
                    update_preview();
                }
                3 => {
                    let surah_strings: Vec<&str> = surah_list.iter().map(|s| s.as_str()).collect();
                    let surah_combo = adw::ComboRow::builder()
                        .title(tr("Surah"))
                        .model(&gtk::StringList::new(&surah_strings))
                        .build();
                    surah_combo.set_selected(1);

                    let goal_ref = goal_type_ref.clone();
                    let update_prev = update_preview.clone();
                    surah_combo.connect_selected_notify(move |row| {
                        let s = (row.selected() + 1) as u32;
                        *goal_ref.borrow_mut() = HifzGoalType::SelectedSurah(s);
                        update_prev();
                    });

                    *goal_type_ref.borrow_mut() = HifzGoalType::SelectedSurah(2);
                    dyn_group.add(&surah_combo);
                    dynamic_controls_box.append(&dyn_group);
                    update_preview();
                }
                4 => {
                    let surah_strings: Vec<&str> = surah_list.iter().map(|s| s.as_str()).collect();
                    let start_combo = adw::ComboRow::builder()
                        .title(tr("Start Surah"))
                        .model(&gtk::StringList::new(&surah_strings))
                        .build();
                    start_combo.set_selected(1);

                    let end_combo = adw::ComboRow::builder()
                        .title(tr("End Surah"))
                        .model(&gtk::StringList::new(&surah_strings))
                        .build();
                    end_combo.set_selected(2);

                    let goal_ref = goal_type_ref.clone();
                    let update_prev = update_preview.clone();
                    let sc = start_combo.clone();
                    let ec = end_combo.clone();
                    let val_label = validation_label.clone();
                    let nxt_btn = next_btn.clone();

                    let sync_surah_range = move || {
                        let ss = (sc.selected() + 1) as u32;
                        let es = (ec.selected() + 1) as u32;
                        if ss > es {
                            val_label.set_text(&tr(
                                "Start Surah must be less than or equal to End Surah.",
                            ));
                            val_label.set_visible(true);
                            nxt_btn.set_sensitive(false);
                        } else {
                            val_label.set_visible(false);
                            nxt_btn.set_sensitive(true);
                            *goal_ref.borrow_mut() = HifzGoalType::SelectedSurahRange {
                                start_surah: ss,
                                end_surah: es,
                            };
                            update_prev();
                        }
                    };

                    let sync1 = sync_surah_range.clone();
                    start_combo.connect_selected_notify(move |_| sync1());
                    let sync2 = sync_surah_range.clone();
                    end_combo.connect_selected_notify(move |_| sync2());

                    *goal_type_ref.borrow_mut() = HifzGoalType::SelectedSurahRange {
                        start_surah: 2,
                        end_surah: 3,
                    };
                    dyn_group.add(&start_combo);
                    dyn_group.add(&end_combo);
                    dynamic_controls_box.append(&dyn_group);
                    update_preview();
                }
                5 => {
                    let start_spin = gtk::SpinButton::with_range(1.0, 604.0, 1.0);
                    start_spin.set_value(120.0);
                    let end_spin = gtk::SpinButton::with_range(1.0, 604.0, 1.0);
                    end_spin.set_value(180.0);

                    let start_row = adw::ActionRow::builder().title(tr("Start Page")).build();
                    start_row.add_suffix(&start_spin);
                    let end_row = adw::ActionRow::builder().title(tr("End Page")).build();
                    end_row.add_suffix(&end_spin);

                    let goal_ref = goal_type_ref.clone();
                    let update_prev = update_preview.clone();
                    let start_s = start_spin.clone();
                    let end_s = end_spin.clone();
                    let val_label = validation_label.clone();
                    let nxt_btn = next_btn.clone();

                    let sync_page_range = move || {
                        let sp = start_s.value() as u32;
                        let ep = end_s.value() as u32;
                        if sp > ep {
                            val_label.set_text(&tr(
                                "Start Page must be less than or equal to End Page.",
                            ));
                            val_label.set_visible(true);
                            nxt_btn.set_sensitive(false);
                        } else {
                            val_label.set_visible(false);
                            nxt_btn.set_sensitive(true);
                            *goal_ref.borrow_mut() = HifzGoalType::CustomPageRange {
                                start_page: sp,
                                end_page: ep,
                            };
                            update_prev();
                        }
                    };

                    let sync1 = sync_page_range.clone();
                    start_spin.connect_value_changed(move |_| sync1());
                    let sync2 = sync_page_range.clone();
                    end_spin.connect_value_changed(move |_| sync2());

                    *goal_type_ref.borrow_mut() = HifzGoalType::CustomPageRange {
                        start_page: 120,
                        end_page: 180,
                    };
                    dyn_group.add(&start_row);
                    dyn_group.add(&end_row);
                    dynamic_controls_box.append(&dyn_group);
                    update_preview();
                }
                _ => {}
            }
        }
    };

    render_dynamic_controls();

    let render_dyn = render_dynamic_controls.clone();
    scope_combo.connect_selected_notify(move |_| {
        render_dyn();
    });

    let container_clone = container.clone();
    let view_stack_clone = view_stack.clone();
    let config_clone = config.clone();
    let lang_owned = lang.to_string();
    let state_clone = step_state;

    next_btn.connect_clicked(move |_| {
        let final_goal_type = current_goal_type.borrow().clone();
        *state_clone.borrow_mut() = HifzWizardStep::ConfigureParameters {
            goal_type: final_goal_type,
        };
        refresh_hifz_view(
            &container_clone,
            &view_stack_clone,
            &config_clone,
            &lang_owned,
            state_clone.clone(),
        );
    });

    inner_box.append(&next_btn);
    card.append(&inner_box);
    container.append(&card);
}

fn render_hifz_configure_params(
    container: &gtk::Box,
    view_stack: &adw::ViewStack,
    config: &AppConfig,
    lang: &str,
    goal_type: HifzGoalType,
    step_state: Rc<RefCell<HifzWizardStep>>,
) {
    let card = gtk::Box::new(gtk::Orientation::Vertical, 0);
    card.add_css_class("card");
    card.set_margin_start(16);
    card.set_margin_end(16);
    card.set_margin_top(16);
    card.set_margin_bottom(16);

    let inner_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
    inner_box.set_margin_start(24);
    inner_box.set_margin_end(24);
    inner_box.set_margin_top(24);
    inner_box.set_margin_bottom(24);

    let heading = gtk::Label::builder()
        .label(&tr("Configure Hifz Framework"))
        .css_classes(["title-2"])
        .halign(gtk::Align::Start)
        .margin_bottom(16)
        .build();
    inner_box.append(&heading);

    let group = adw::PreferencesGroup::new();

    let sabaq_combo = adw::ComboRow::builder()
        .title(tr("Sabaq — New Lesson Target"))
        .model(&gtk::StringList::new(&[
            &tr("3 Lines"),
            &tr("5 Lines"),
            &tr("Half Page (1/2 page)"),
            &tr("1 Page"),
        ]))
        .build();

    let sabqi_combo = adw::ComboRow::builder()
        .title(tr("Sabqi — Recent Review Window"))
        .model(&gtk::StringList::new(&[
            &tr("3 Days"),
            &tr("5 Days"),
            &tr("7 Days"),
            &tr("10 Days"),
            &tr("14 Days"),
        ]))
        .build();

    let manzil_combo = adw::ComboRow::builder()
        .title(tr("Manzil — Older Revision Cycle"))
        .model(&gtk::StringList::new(&[
            &tr("7 Days Cycle"),
            &tr("10 Days Cycle"),
            &tr("14 Days Cycle"),
        ]))
        .build();

    group.add(&sabaq_combo);
    group.add(&sabqi_combo);
    group.add(&manzil_combo);
    inner_box.append(&group);

    let next_btn = gtk::Button::builder()
        .label(&tr("Review Hifz Plan"))
        .css_classes(["suggested-action"])
        .halign(gtk::Align::End)
        .margin_top(20)
        .build();

    let container_clone = container.clone();
    let view_stack_clone = view_stack.clone();
    let config_clone = config.clone();
    let lang_owned = lang.to_string();
    let state_clone = step_state;

    next_btn.connect_clicked(move |_| {
        let sabaq_unit = match sabaq_combo.selected() {
            0 => HifzUnit::Lines(3),
            1 => HifzUnit::Lines(5),
            2 => HifzUnit::HalfPage,
            _ => HifzUnit::Page,
        };

        let sabqi_window = match sabqi_combo.selected() {
            0 => 3,
            1 => 5,
            2 => 7,
            3 => 10,
            _ => 14,
        };

        let manzil_cycle = match manzil_combo.selected() {
            0 => 7,
            1 => 10,
            _ => 14,
        };

        *state_clone.borrow_mut() = HifzWizardStep::ReviewSummary {
            goal_type: goal_type.clone(),
            setup_mode: SetupMode::ByDailyAmount,
            target_days: 365,
            sabaq_unit,
            sabqi_window,
            manzil_cycle,
        };
        refresh_hifz_view(
            &container_clone,
            &view_stack_clone,
            &config_clone,
            &lang_owned,
            state_clone.clone(),
        );
    });

    inner_box.append(&next_btn);
    card.append(&inner_box);
    container.append(&card);
}

fn render_hifz_review_summary(
    container: &gtk::Box,
    view_stack: &adw::ViewStack,
    config: &AppConfig,
    lang: &str,
    goal_type: HifzGoalType,
    setup_mode: SetupMode,
    target_days: u32,
    sabaq_unit: HifzUnit,
    sabqi_window: u32,
    manzil_cycle: u32,
    step_state: Rc<RefCell<HifzWizardStep>>,
) {
    let today = chrono::Local::now().date_naive();
    let (calc_days, expected_end, history) = calculate_hifz_workload(
        &goal_type,
        &setup_mode,
        target_days,
        &sabaq_unit,
        sabqi_window,
        manzil_cycle,
        today,
    );

    let card = gtk::Box::new(gtk::Orientation::Vertical, 0);
    card.add_css_class("card");
    card.set_margin_start(16);
    card.set_margin_end(16);
    card.set_margin_top(16);
    card.set_margin_bottom(16);

    let inner_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
    inner_box.set_margin_start(24);
    inner_box.set_margin_end(24);
    inner_box.set_margin_top(24);
    inner_box.set_margin_bottom(24);

    let heading = gtk::Label::builder()
        .label(&tr("Review Your Hifz Plan"))
        .css_classes(["title-2"])
        .halign(gtk::Align::Start)
        .margin_bottom(16)
        .build();
    inner_box.append(&heading);

    let (start_p, end_p) = map_hifz_goal_scope(&goal_type);

    let group = adw::PreferencesGroup::new();
    let row_goal = adw::ActionRow::builder()
        .title(tr("Goal Scope"))
        .subtitle(&format!("Pages {}–{}", start_p, end_p))
        .build();
    let row_sabaq = adw::ActionRow::builder()
        .title(tr("Sabaq — New Lesson"))
        .subtitle(tr("Configured daily portion"))
        .build();
    let row_sabqi = adw::ActionRow::builder()
        .title(tr("Sabqi — Recent Review"))
        .subtitle(&format!("{} {}", sabqi_window, tr("days window")))
        .build();
    let row_manzil = adw::ActionRow::builder()
        .title(tr("Manzil — Long-Term Cycle"))
        .subtitle(&format!("{} {}", manzil_cycle, tr("days cycle")))
        .build();
    let row_end = adw::ActionRow::builder()
        .title(tr("Estimated Completion"))
        .subtitle(&expected_end.format("%Y-%m-%d").to_string())
        .build();

    group.add(&row_goal);
    group.add(&row_sabaq);
    group.add(&row_sabqi);
    group.add(&row_manzil);
    group.add(&row_end);
    inner_box.append(&group);

    let btn_box = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    btn_box.set_halign(gtk::Align::End);
    btn_box.set_margin_top(20);

    let back_btn = gtk::Button::builder().label(&tr("Back")).build();
    let create_btn = gtk::Button::builder()
        .label(&tr("Create Hifz Plan"))
        .css_classes(["suggested-action"])
        .build();

    btn_box.append(&back_btn);
    btn_box.append(&create_btn);

    let container_clone = container.clone();
    let view_stack_clone = view_stack.clone();
    let config_clone = config.clone();
    let lang_owned = lang.to_string();
    let state_clone1 = step_state.clone();

    back_btn.connect_clicked(move |_| {
        *state_clone1.borrow_mut() = HifzWizardStep::ChooseGoal;
        refresh_hifz_view(
            &container_clone,
            &view_stack_clone,
            &config_clone,
            &lang_owned,
            state_clone1.clone(),
        );
    });

    let container_clone2 = container.clone();
    let view_stack_clone2 = view_stack.clone();
    let config_clone2 = config.clone();
    let lang_owned2 = lang.to_string();
    let state_clone2 = step_state;

    let goal_type_clone = goal_type.clone();
    let setup_mode_clone = setup_mode.clone();
    let sabaq_unit_clone = sabaq_unit.clone();
    let history_clone = history.clone();

    create_btn.connect_clicked(move |_| {
        let store = ConfigPlanStore::new(&config_clone2);
        let plan = create_new_hifz_plan(
            tr("Hifz Plan"),
            goal_type_clone.clone(),
            setup_mode_clone.clone(),
            start_p,
            end_p,
            sabaq_unit_clone.clone(),
            sabqi_window,
            manzil_cycle,
            calc_days,
            today,
            expected_end,
            history_clone.clone(),
        );

        let mut existing = store.load_hifz_plans();
        for p in &mut existing {
            p.is_active = false;
        }
        existing.push(plan);
        store.save_hifz_plans(&existing);

        *state_clone2.borrow_mut() = HifzWizardStep::Dashboard;
        refresh_hifz_view(
            &container_clone2,
            &view_stack_clone2,
            &config_clone2,
            &lang_owned2,
            state_clone2.clone(),
        );
    });

    inner_box.append(&btn_box);
    card.append(&inner_box);
    container.append(&card);
}

fn render_active_hifz_dashboard(
    container: &gtk::Box,
    view_stack: &adw::ViewStack,
    config: &AppConfig,
    lang: &str,
    plan: HifzPlanData,
    step_state: Rc<RefCell<HifzWizardStep>>,
) {
    let heading = gtk::Label::builder()
        .label(&plan.title)
        .css_classes(["title-2", "accent"])
        .halign(gtk::Align::Start)
        .margin_start(16)
        .margin_end(16)
        .margin_bottom(4)
        .build();
    container.append(&heading);

    let scope_title = format_hifz_goal_title(&plan.goal_type, lang);
    let scope_subtitle = gtk::Label::builder()
        .label(&format!(
            "{}: {} | {}: {}–{}",
            tr("Scope"),
            scope_title,
            tr("Pages"),
            plan.start_page,
            plan.end_page
        ))
        .css_classes(["body", "dim-label"])
        .halign(gtk::Align::Start)
        .margin_start(16)
        .margin_end(16)
        .margin_bottom(16)
        .build();
    container.append(&scope_subtitle);

    let current_record = get_active_hifz_record(&plan);
    let day_idx = current_record.day_index;

    // ── 1. Overall Progress Card ─────────────────────────────────────────────
    let total_scope_pages = (plan.end_page + 1).saturating_sub(plan.start_page).max(1);
    let memorized_pages = (current_record.sabaq_end_page + 1)
        .saturating_sub(plan.start_page)
        .min(total_scope_pages);
    let overall_fraction = (memorized_pages as f64 / total_scope_pages as f64).clamp(0.0, 1.0);
    let overall_percent = (overall_fraction * 100.0) as u32;

    let overall_card = gtk::Box::new(gtk::Orientation::Vertical, 8);
    overall_card.add_css_class("card");
    overall_card.set_margin_start(16);
    overall_card.set_margin_end(16);
    overall_card.set_margin_bottom(16);

    let overall_inner = gtk::Box::new(gtk::Orientation::Vertical, 8);
    overall_inner.set_margin_start(16);
    overall_inner.set_margin_end(16);
    overall_inner.set_margin_top(16);
    overall_inner.set_margin_bottom(16);

    let overall_title = gtk::Label::builder()
        .label(&format!(
            "{} — {}%",
            tr("Overall Progress"),
            overall_percent
        ))
        .css_classes(["heading"])
        .halign(gtk::Align::Start)
        .build();
    overall_inner.append(&overall_title);

    let overall_bar = gtk::ProgressBar::new();
    overall_bar.set_fraction(overall_fraction);
    overall_inner.append(&overall_bar);

    let overall_sub = gtk::Label::builder()
        .label(&format!(
            "{} / {} {} | {}: {}",
            memorized_pages,
            total_scope_pages,
            tr("pages memorized"),
            tr("Estimated Completion"),
            plan.expected_end_date
        ))
        .css_classes(["body", "dim-label"])
        .halign(gtk::Align::Start)
        .build();
    overall_inner.append(&overall_sub);

    overall_card.append(&overall_inner);
    container.append(&overall_card);

    // ── 2. Today's Progress & Continue Action Card ─────────────────────────────
    let sabaq_done = current_record.sabaq_status == DailyStatus::Completed;
    let sabqi_done = current_record.sabqi_status == DailyStatus::Completed;
    let manzil_done = current_record.manzil_status == DailyStatus::Completed;

    let completed_today_count = (if sabaq_done { 1 } else { 0 })
        + (if sabqi_done { 1 } else { 0 })
        + (if manzil_done { 1 } else { 0 });
    let today_fraction = completed_today_count as f64 / 3.0;
    let today_percent = (today_fraction * 100.0) as u32;

    let today_card = gtk::Box::new(gtk::Orientation::Vertical, 8);
    today_card.add_css_class("card");
    today_card.set_margin_start(16);
    today_card.set_margin_end(16);
    today_card.set_margin_bottom(16);

    let today_inner = gtk::Box::new(gtk::Orientation::Vertical, 8);
    today_inner.set_margin_start(16);
    today_inner.set_margin_end(16);
    today_inner.set_margin_top(16);
    today_inner.set_margin_bottom(16);

    let today_title = gtk::Label::builder()
        .label(&format!("{} — {}%", tr("Today's Progress"), today_percent))
        .css_classes(["heading"])
        .halign(gtk::Align::Start)
        .build();
    today_inner.append(&today_title);

    let today_bar = gtk::ProgressBar::new();
    today_bar.set_fraction(today_fraction);
    today_inner.append(&today_bar);

    let next_page_target = if !sabaq_done {
        current_record.sabaq_start_page
    } else if !sabqi_done {
        current_record.sabqi_start_page
    } else if !manzil_done {
        current_record.manzil_start_page
    } else {
        current_record.sabaq_start_page
    };

    let continue_btn = gtk::Button::builder()
        .label(&if completed_today_count == 3 {
            tr("Today's Hifz Completed ✓")
        } else {
            tr("Continue Today's Hifz")
        })
        .css_classes(if completed_today_count == 3 {
            vec!["flat"]
        } else {
            vec!["suggested-action"]
        })
        .sensitive(completed_today_count < 3)
        .halign(gtk::Align::Start)
        .margin_top(8)
        .build();

    let view_stack_cont = view_stack.clone();
    let config_cont = config.clone();
    let lang_cont = lang.to_string();
    continue_btn.connect_clicked(move |_| {
        crate::quran::open_surah_at_page(
            &view_stack_cont,
            &lang_cont,
            config_cont.clone(),
            next_page_target,
        );
    });
    today_inner.append(&continue_btn);

    today_card.append(&today_inner);
    container.append(&today_card);

    // ── 3. Today's Lesson Cards (Sabaq, Sabqi, Manzil) ─────────────────────────

    // Sabaq Card
    let sabaq_group = adw::PreferencesGroup::builder()
        .title(tr("Sabaq — New Memorization"))
        .description(tr("Today's new lesson"))
        .build();
    let sabaq_row = adw::ActionRow::builder()
        .title(&format!(
            "{} {}",
            tr("Pages"),
            if current_record.sabaq_start_page == current_record.sabaq_end_page {
                format!("{}", current_record.sabaq_start_page)
            } else {
                format!(
                    "{}–{}",
                    current_record.sabaq_start_page, current_record.sabaq_end_page
                )
            }
        ))
        .subtitle(tr("New memorization portion"))
        .build();

    let sabaq_btn = gtk::Button::builder()
        .label(&tr("Start Today's Sabaq"))
        .build();
    let sabaq_start_page = current_record.sabaq_start_page;
    let view_stack_sabaq = view_stack.clone();
    let config_sabaq = config.clone();
    let lang_sabaq = lang.to_string();
    sabaq_btn.connect_clicked(move |_| {
        crate::quran::open_surah_at_page(
            &view_stack_sabaq,
            &lang_sabaq,
            config_sabaq.clone(),
            sabaq_start_page,
        );
    });
    sabaq_row.add_suffix(&sabaq_btn);

    let sabaq_mark_btn = gtk::Button::builder()
        .label(if sabaq_done {
            tr("Sabaq Completed ✓")
        } else {
            tr("Mark Sabaq Completed")
        })
        .css_classes(if sabaq_done {
            vec!["suggested-action"]
        } else {
            vec![]
        })
        .build();
    let config_sabaq_mark = config.clone();
    let plan_id_sabaq = plan.id.clone();
    let container_sabaq_mark = container.clone();
    let view_stack_sabaq_mark = view_stack.clone();
    let lang_sabaq_mark = lang.to_string();
    let state_sabaq_mark = step_state.clone();

    sabaq_mark_btn.connect_clicked(move |_| {
        let store = ConfigPlanStore::new(&config_sabaq_mark);
        let mut plans = store.load_hifz_plans();
        if let Some(p) = plans.iter_mut().find(|p| p.id == plan_id_sabaq) {
            if let Some(r) = p.history.iter_mut().find(|r| r.day_index == day_idx) {
                r.sabaq_status = if r.sabaq_status == DailyStatus::Completed {
                    DailyStatus::Pending
                } else {
                    DailyStatus::Completed
                };
            }
            store.save_hifz_plans(&plans);
        }
        refresh_hifz_view(
            &container_sabaq_mark,
            &view_stack_sabaq_mark,
            &config_sabaq_mark,
            &lang_sabaq_mark,
            state_sabaq_mark.clone(),
        );
    });
    sabaq_row.add_suffix(&sabaq_mark_btn);
    sabaq_group.add(&sabaq_row);
    container.append(&sabaq_group);

    // Sabqi Card
    let sabqi_group = adw::PreferencesGroup::builder()
        .title(tr("Sabqi — Recent Revision"))
        .description(tr("Review of lessons from recent days"))
        .build();
    let sabqi_row = adw::ActionRow::builder()
        .title(&format!(
            "{} {}–{}",
            tr("Pages"),
            current_record.sabqi_start_page,
            current_record.sabqi_end_page
        ))
        .subtitle(tr("Recent review window"))
        .build();

    let sabqi_btn = gtk::Button::builder().label(&tr("Start Sabqi")).build();
    let sabqi_start_page = current_record.sabqi_start_page;
    let view_stack_sabqi = view_stack.clone();
    let config_sabqi = config.clone();
    let lang_sabqi = lang.to_string();
    sabqi_btn.connect_clicked(move |_| {
        crate::quran::open_surah_at_page(
            &view_stack_sabqi,
            &lang_sabqi,
            config_sabqi.clone(),
            sabqi_start_page,
        );
    });
    sabqi_row.add_suffix(&sabqi_btn);

    let sabqi_mark_btn = gtk::Button::builder()
        .label(if sabqi_done {
            tr("Sabqi Completed ✓")
        } else {
            tr("Mark Sabqi Completed")
        })
        .css_classes(if sabqi_done {
            vec!["suggested-action"]
        } else {
            vec![]
        })
        .build();
    let config_sabqi_mark = config.clone();
    let plan_id_sabqi = plan.id.clone();
    let container_sabqi_mark = container.clone();
    let view_stack_sabqi_mark = view_stack.clone();
    let lang_sabqi_mark = lang.to_string();
    let state_sabqi_mark = step_state.clone();

    sabqi_mark_btn.connect_clicked(move |_| {
        let store = ConfigPlanStore::new(&config_sabqi_mark);
        let mut plans = store.load_hifz_plans();
        if let Some(p) = plans.iter_mut().find(|p| p.id == plan_id_sabqi) {
            if let Some(r) = p.history.iter_mut().find(|r| r.day_index == day_idx) {
                r.sabqi_status = if r.sabqi_status == DailyStatus::Completed {
                    DailyStatus::Pending
                } else {
                    DailyStatus::Completed
                };
            }
            store.save_hifz_plans(&plans);
        }
        refresh_hifz_view(
            &container_sabqi_mark,
            &view_stack_sabqi_mark,
            &config_sabqi_mark,
            &lang_sabqi_mark,
            state_sabqi_mark.clone(),
        );
    });
    sabqi_row.add_suffix(&sabqi_mark_btn);
    sabqi_group.add(&sabqi_row);
    container.append(&sabqi_group);

    // Manzil Card
    let manzil_group = adw::PreferencesGroup::builder()
        .title(tr("Manzil — Long-Term Revision"))
        .description(tr("Cycle review of older memorization"))
        .build();
    let manzil_row = adw::ActionRow::builder()
        .title(&format!(
            "{} {}–{}",
            tr("Pages"),
            current_record.manzil_start_page,
            current_record.manzil_end_page
        ))
        .subtitle(tr("Older revision cycle"))
        .build();

    let manzil_btn = gtk::Button::builder().label(&tr("Start Manzil")).build();
    let manzil_start_page = current_record.manzil_start_page;
    let view_stack_manzil = view_stack.clone();
    let config_manzil = config.clone();
    let lang_manzil = lang.to_string();
    manzil_btn.connect_clicked(move |_| {
        crate::quran::open_surah_at_page(
            &view_stack_manzil,
            &lang_manzil,
            config_manzil.clone(),
            manzil_start_page,
        );
    });
    manzil_row.add_suffix(&manzil_btn);

    let manzil_mark_btn = gtk::Button::builder()
        .label(if manzil_done {
            tr("Manzil Completed ✓")
        } else {
            tr("Mark Manzil Completed")
        })
        .css_classes(if manzil_done {
            vec!["suggested-action"]
        } else {
            vec![]
        })
        .build();
    let config_manzil_mark = config.clone();
    let plan_id_manzil = plan.id.clone();
    let container_manzil_mark = container.clone();
    let view_stack_manzil_mark = view_stack.clone();
    let lang_manzil_mark = lang.to_string();
    let state_manzil_mark = step_state.clone();

    manzil_mark_btn.connect_clicked(move |_| {
        let store = ConfigPlanStore::new(&config_manzil_mark);
        let mut plans = store.load_hifz_plans();
        if let Some(p) = plans.iter_mut().find(|p| p.id == plan_id_manzil) {
            if let Some(r) = p.history.iter_mut().find(|r| r.day_index == day_idx) {
                r.manzil_status = if r.manzil_status == DailyStatus::Completed {
                    DailyStatus::Pending
                } else {
                    DailyStatus::Completed
                };
            }
            store.save_hifz_plans(&plans);
        }
        refresh_hifz_view(
            &container_manzil_mark,
            &view_stack_manzil_mark,
            &config_manzil_mark,
            &lang_manzil_mark,
            state_manzil_mark.clone(),
        );
    });
    manzil_row.add_suffix(&manzil_mark_btn);
    manzil_group.add(&manzil_row);
    container.append(&manzil_group);

    // ── 4. Completion Banners & Advancement ────────────────────────────────────
    let is_plan_fully_completed = current_record.sabaq_end_page >= plan.end_page && sabaq_done;

    if is_plan_fully_completed {
        let banner_card = gtk::Box::new(gtk::Orientation::Vertical, 8);
        banner_card.add_css_class("card");
        banner_card.set_margin_start(16);
        banner_card.set_margin_end(16);
        banner_card.set_margin_top(16);
        banner_card.set_margin_bottom(16);

        let banner_inner = gtk::Box::new(gtk::Orientation::Vertical, 8);
        banner_inner.set_margin_start(20);
        banner_inner.set_margin_end(20);
        banner_inner.set_margin_top(20);
        banner_inner.set_margin_bottom(20);

        let banner_title = gtk::Label::builder()
            .label(&format!("🎉 {}", tr("Hifz Plan Completed")))
            .css_classes(["title-2", "accent"])
            .halign(gtk::Align::Center)
            .build();
        banner_inner.append(&banner_title);

        let banner_sub = gtk::Label::builder()
            .label(&tr(
                "You have completed memorizing your selected Quran scope.",
            ))
            .css_classes(["body", "dim-label"])
            .halign(gtk::Align::Center)
            .build();
        banner_inner.append(&banner_sub);

        let banner_btns = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        banner_btns.set_halign(gtk::Align::Center);
        banner_btns.set_margin_top(12);

        let arch_btn = gtk::Button::builder()
            .label(&tr("Archive Plan"))
            .css_classes(["suggested-action"])
            .build();
        let new_plan_btn = gtk::Button::builder()
            .label(&tr("Create New Hifz Plan"))
            .build();

        let config_arch = config.clone();
        let plan_id_arch = plan.id.clone();
        let container_arch = container.clone();
        let view_stack_arch = view_stack.clone();
        let state_arch = step_state.clone();
        let lang_arch = lang.to_string();

        arch_btn.connect_clicked(move |_| {
            let store = ConfigPlanStore::new(&config_arch);
            let mut plans = store.load_hifz_plans();
            if let Some(p) = plans.iter_mut().find(|p| p.id == plan_id_arch) {
                p.is_active = false;
                p.is_archived = true;
            }
            store.save_hifz_plans(&plans);
            *state_arch.borrow_mut() = HifzWizardStep::EmptyState;
            refresh_hifz_view(
                &container_arch,
                &view_stack_arch,
                &config_arch,
                &lang_arch,
                state_arch.clone(),
            );
        });

        let config_new = config.clone();
        let container_new = container.clone();
        let view_stack_new = view_stack.clone();
        let state_new = step_state.clone();
        let lang_new = lang.to_string();

        new_plan_btn.connect_clicked(move |_| {
            *state_new.borrow_mut() = HifzWizardStep::ChooseGoal;
            refresh_hifz_view(
                &container_new,
                &view_stack_new,
                &config_new,
                &lang_new,
                state_new.clone(),
            );
        });

        banner_btns.append(&arch_btn);
        banner_btns.append(&new_plan_btn);
        banner_inner.append(&banner_btns);
        banner_card.append(&banner_inner);
        container.append(&banner_card);
    } else if completed_today_count == 3 {
        let banner_card = gtk::Box::new(gtk::Orientation::Vertical, 8);
        banner_card.add_css_class("card");
        banner_card.set_margin_start(16);
        banner_card.set_margin_end(16);
        banner_card.set_margin_top(16);
        banner_card.set_margin_bottom(16);

        let banner_inner = gtk::Box::new(gtk::Orientation::Vertical, 8);
        banner_inner.set_margin_start(20);
        banner_inner.set_margin_end(20);
        banner_inner.set_margin_top(20);
        banner_inner.set_margin_bottom(20);

        let banner_title = gtk::Label::builder()
            .label(&format!("✓ {}", tr("Today's Hifz Lessons Completed")))
            .css_classes(["title-3", "accent"])
            .halign(gtk::Align::Center)
            .build();
        banner_inner.append(&banner_title);

        if let Some(next_rec) = plan.history.iter().find(|r| r.day_index == day_idx + 1) {
            let next_sub = gtk::Label::builder()
                .label(&format!(
                    "{}: {} {}",
                    tr("Tomorrow's Sabaq"),
                    tr("Pages"),
                    if next_rec.sabaq_start_page == next_rec.sabaq_end_page {
                        format!("{}", next_rec.sabaq_start_page)
                    } else {
                        format!("{}–{}", next_rec.sabaq_start_page, next_rec.sabaq_end_page)
                    }
                ))
                .css_classes(["body", "dim-label"])
                .halign(gtk::Align::Center)
                .build();
            banner_inner.append(&next_sub);
        }

        banner_card.append(&banner_inner);
        container.append(&banner_card);
    }

    let btn_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    btn_box.set_margin_top(12);
    btn_box.set_margin_start(16);
    btn_box.set_margin_end(16);

    let archive_btn = gtk::Button::builder().label(&tr("Archive Plan")).build();
    let config_arch = config.clone();
    let plan_id_arch = plan.id;
    let container_arch = container.clone();
    let view_stack_arch = view_stack.clone();
    let state_arch = step_state;
    let lang_arch = lang.to_string();

    archive_btn.connect_clicked(move |_| {
        let store = ConfigPlanStore::new(&config_arch);
        let mut plans = store.load_hifz_plans();
        if let Some(p) = plans.iter_mut().find(|p| p.id == plan_id_arch) {
            p.is_active = false;
            p.is_archived = true;
        }
        store.save_hifz_plans(&plans);
        *state_arch.borrow_mut() = HifzWizardStep::EmptyState;
        refresh_hifz_view(
            &container_arch,
            &view_stack_arch,
            &config_arch,
            &lang_arch,
            state_arch.clone(),
        );
    });
    btn_box.append(&archive_btn);
    container.append(&btn_box);
}
