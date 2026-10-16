use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;
use std::sync::OnceLock;

use adw::prelude::*;
use gtk::gio;
use gtk::glib;
use gtk::glib::subclass::prelude::*;
use gtk4 as gtk;
use libadwaita as adw;

use crate::i18n::tr;
use crate::location;

mod timezone_item_imp {
    use super::*;
    use gtk::glib::Properties;

    #[derive(Properties, Default)]
    #[properties(wrapper_type = super::TimezoneItem)]
    pub struct TimezoneItem {
        #[property(get, set)]
        pub city_name: RefCell<String>,
        #[property(get, set)]
        pub location: RefCell<String>,
        #[property(get, set)]
        pub zone: RefCell<String>,
        #[property(get, set)]
        pub zone_label: RefCell<String>,
        #[property(get, set)]
        pub offset: RefCell<String>,
        #[property(get, set)]
        pub time: RefCell<String>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for TimezoneItem {
        const NAME: &'static str = "KhushuTimezoneItem";
        type Type = super::TimezoneItem;
    }

    #[glib::derived_properties]
    impl ObjectImpl for TimezoneItem {}
}

glib::wrapper! {
    pub struct TimezoneItem(ObjectSubclass<timezone_item_imp::TimezoneItem>);
}

impl TimezoneItem {
    pub fn new(
        zone: &str,
        zone_label: &str,
        city_name: &str,
        location: &str,
        offset: &str,
        time: &str,
    ) -> Self {
        glib::Object::builder()
            .property("zone", zone)
            .property("zone_label", zone_label)
            .property("city_name", city_name)
            .property("location", location)
            .property("offset", offset)
            .property("time", time)
            .build()
    }
}

fn offset_secs(zone: &jiff::tz::TimeZone) -> i32 {
    zone.to_offset(jiff::Timestamp::now()).seconds()
}

fn picker_zone_names() -> &'static HashSet<String> {
    static SET: OnceLock<HashSet<String>> = OnceLock::new();
    SET.get_or_init(|| {
        include_str!("../data/zone.tab")
            .lines()
            .filter(|line| !line.starts_with('#') && !line.trim().is_empty())
            .filter_map(|line| {
                let raw_zone = line.split('\t').nth(2)?;
                let zone = raw_zone.trim();
                if zone.is_empty() {
                    None
                } else {
                    Some(zone.to_string())
                }
            })
            .collect()
    })
}

fn load_tz_items(language: &str) -> Vec<TimezoneItem> {
    let picker_names = picker_zone_names();
    let now = chrono::Utc::now();
    let mut items: Vec<TimezoneItem> = jiff::tz::db()
        .available()
        .filter(|name| picker_names.contains(name.as_str()))
        .filter_map(|name| {
            let zone_str = name.as_str().to_string();
            let zone = jiff::tz::TimeZone::get(&zone_str).ok()?;
            let city_name =
                location::city_name_from_time_zone(&zone_str, language).unwrap_or_else(|| {
                    zone_str
                        .split('/')
                        .next_back()
                        .unwrap_or(&zone_str)
                        .replace('_', " ")
                });
            let location =
                location::time_zone_location_name(&zone_str, language).unwrap_or_default();
            let zone_offset_seconds = offset_secs(&zone);
            let offset = location::localized_offset(zone_offset_seconds, language);
            let fixed_offset = chrono::FixedOffset::east_opt(zone_offset_seconds)
                .unwrap_or_else(|| chrono::FixedOffset::east_opt(0).unwrap());
            let time = location::localized_time(
                chrono::TimeZone::from_utc_datetime(&fixed_offset, &now.naive_utc()),
                language,
            );
            let zone_label = location::localized_zone(&zone_str, language);
            Some(TimezoneItem::new(
                &zone_str,
                &zone_label,
                &city_name,
                &location,
                &offset,
                &time,
            ))
        })
        .collect();
    items.sort_by_key(|item| item.city_name().to_lowercase());
    items
}

fn create_factory() -> gtk::SignalListItemFactory {
    let factory = gtk::SignalListItemFactory::new();

    factory.connect_setup(move |_, list_item| {
        let list_item = list_item.downcast_ref::<gtk::ListItem>().unwrap();

        let grid = gtk::Grid::builder()
            .margin_start(12)
            .margin_end(12)
            .margin_top(12)
            .margin_bottom(12)
            .row_spacing(6)
            .column_spacing(6)
            .build();

        let city_label = gtk::Label::builder()
            .xalign(0.0)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .css_classes(["title"])
            .build();
        grid.attach(&city_label, 0, 0, 1, 1);

        let location_label = gtk::Label::builder()
            .xalign(0.0)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .hexpand(true)
            .css_classes(["heading"])
            .build();
        grid.attach(&location_label, 1, 0, 1, 1);

        let time_label = gtk::Label::builder()
            .xalign(0.0)
            .halign(gtk::Align::End)
            .css_classes(["dim-label", "numeric"])
            .build();
        grid.attach(&time_label, 2, 0, 1, 2);

        let sub_box = gtk::Box::builder()
            .spacing(3)
            .css_classes(["caption", "dim-label"])
            .build();

        let zone_label = gtk::Label::builder()
            .xalign(0.0)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .build();
        sub_box.append(&zone_label);

        let sep_label = gtk::Label::builder()
            .margin_start(3)
            .margin_end(3)
            .label("•")
            .build();
        sub_box.append(&sep_label);

        let offset_label = gtk::Label::builder().xalign(0.0).build();
        sub_box.append(&offset_label);

        grid.attach(&sub_box, 0, 1, 2, 1);

        list_item.set_child(Some(&grid));
    });

    factory.connect_bind(move |_, list_item| {
        let list_item = list_item.downcast_ref::<gtk::ListItem>().unwrap();
        let item = list_item
            .item()
            .and_then(|object| object.downcast::<TimezoneItem>().ok());
        let grid = list_item
            .child()
            .and_then(|widget| widget.downcast::<gtk::Grid>().ok());
        if let (Some(timezone_item), Some(timezone_grid)) = (item, grid) {
            if let Some(widget) = timezone_grid.child_at(0, 0)
                && let Ok(label) = widget.downcast::<gtk::Label>()
            {
                let escaped = glib::markup_escape_text(&timezone_item.city_name());
                label.set_label(&escaped);
            }
            if let Some(widget) = timezone_grid.child_at(1, 0)
                && let Ok(label) = widget.downcast::<gtk::Label>()
            {
                let escaped = glib::markup_escape_text(&timezone_item.location());
                label.set_label(&escaped);
            }
            if let Some(widget) = timezone_grid.child_at(2, 0)
                && let Ok(label) = widget.downcast::<gtk::Label>()
            {
                label.set_label(&timezone_item.time());
            }
            if let Some(widget) = timezone_grid.child_at(0, 1)
                && let Ok(box_) = widget.downcast::<gtk::Box>()
            {
                if let Some(first) = box_.first_child()
                    && let Ok(label) = first.downcast::<gtk::Label>()
                {
                    let escaped = glib::markup_escape_text(&timezone_item.zone_label());
                    label.set_label(&escaped);
                }
                if let Some(last) = box_.last_child()
                    && let Ok(label) = last.downcast::<gtk::Label>()
                {
                    label.set_label(&timezone_item.offset());
                }
            }
        }
    });

    factory
}

pub fn open_tz_dialog(parent: &impl IsA<gtk::Widget>, on_select: Rc<dyn Fn(&str)>, language: &str) {
    let store = gio::ListStore::new::<TimezoneItem>();
    let items = load_tz_items(language);
    for item in &items {
        store.append(item);
    }

    let sort_expression = gtk::PropertyExpression::new(
        TimezoneItem::static_type(),
        None::<&gtk::Expression>,
        "city_name",
    );
    let sorter = gtk::StringSorter::new(Some(sort_expression));
    let sorted = gtk::SortListModel::new(Some(store), Some(sorter));

    let search_bar = gtk::SearchBar::builder().search_mode_enabled(true).build();

    let search_entry = gtk::SearchEntry::builder()
        .placeholder_text(tr("Search cities"))
        .hexpand(true)
        .build();
    search_bar.set_child(Some(&search_entry));

    let filter_model = gtk::FilterListModel::new(Some(sorted), Option::<gtk::CustomFilter>::None);
    let filter_ref = filter_model.clone();
    search_entry.connect_search_changed(move |entry| {
        let text = entry.text().to_string();
        if text.is_empty() {
            filter_ref.set_filter(Option::<gtk::CustomFilter>::None.as_ref());
        } else {
            let filter = gtk::CustomFilter::new(move |obj| {
                let item = match obj.downcast_ref::<TimezoneItem>() {
                    Some(item) => item,
                    None => return false,
                };
                let search_lower = text.to_lowercase();
                let words: Vec<&str> = search_lower.split_whitespace().collect();
                words.iter().all(|word| {
                    item.city_name().to_lowercase().contains(word)
                        || item.zone().to_lowercase().contains(word)
                        || item.zone_label().to_lowercase().contains(word)
                        || item.location().to_lowercase().contains(word)
                })
            });
            filter_ref.set_filter(Some(&filter));
        }
    });

    let selection_model = gtk::NoSelection::new(Some(filter_model.clone()));
    let factory = create_factory();

    let list_view = gtk::ListView::builder()
        .model(&selection_model)
        .factory(&factory)
        .show_separators(true)
        .single_click_activate(true)
        .build();

    let on_select_clone = on_select.clone();
    let filter_for_activate = filter_model.clone();
    let dialog_close_ref: Rc<RefCell<Option<adw::Dialog>>> = Rc::new(RefCell::new(None));
    let dialog_close_ref2 = dialog_close_ref.clone();
    list_view.connect_activate(move |_, position| {
        if let Some(item) = filter_for_activate.item(position)
            && let Ok(timezone_item) = item.downcast::<TimezoneItem>()
        {
            on_select_clone(&timezone_item.zone());
            if let Some(dlg) = dialog_close_ref2.borrow().as_ref() {
                dlg.close();
            }
        }
    });

    let empty_page = adw::StatusPage::builder()
        .title(tr("No Results"))
        .icon_name("system-search-symbolic")
        .margin_top(18)
        .build();

    let scrolled = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&list_view)
        .build();

    let clamp = adw::Clamp::builder()
        .orientation(gtk::Orientation::Horizontal)
        .maximum_size(400)
        .build();
    clamp.set_child(Some(&scrolled));

    let main_stack = gtk::Stack::new();
    main_stack.add_named(&empty_page, Some("empty"));
    main_stack.add_named(&clamp, Some("list"));
    main_stack.set_visible_child_name("list");

    let main_stack_clone = main_stack.clone();
    filter_model.connect_items_changed(move |model, _, _, _| {
        if model.n_items() == 0 {
            main_stack_clone.set_visible_child_name("empty");
        } else {
            main_stack_clone.set_visible_child_name("list");
        }
    });

    let toolbar_view = adw::ToolbarView::new();
    toolbar_view.add_top_bar(&adw::HeaderBar::new());

    let search_bin = adw::Bin::builder()
        .css_classes(["toolbar"])
        .child(&search_bar)
        .build();
    toolbar_view.add_top_bar(&search_bin);
    toolbar_view.set_content(Some(&main_stack));

    let dialog = adw::Dialog::builder()
        .title(tr("Select Time Zone"))
        .content_width(400)
        .content_height(540)
        .css_classes(["view"])
        .child(&toolbar_view)
        .build();

    *dialog_close_ref.borrow_mut() = Some(dialog.clone());

    dialog.connect_map(move |_| {
        search_entry.grab_focus();
    });

    dialog.present(Some(parent));
}

#[cfg(test)]
mod tests {
    use super::picker_zone_names;

    #[test]
    fn picker_zone_names_reads_zone_tab_column_three() {
        let zones = picker_zone_names();
        assert!(zones.contains("Africa/Casablanca"));
        assert!(zones.contains("America/New_York"));
        assert!(zones.contains("Europe/Oslo"));
        assert!(!zones.contains("US/Eastern"));
    }

    #[test]
    fn every_picker_zone_exists_in_jiffs_database() {
        for zone in picker_zone_names() {
            assert!(
                jiff::tz::TimeZone::get(zone).is_ok(),
                "zone.tab zone missing from jiff: {zone}"
            );
        }
    }
}
