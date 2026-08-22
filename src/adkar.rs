use crate::config::{AppConfig, AppConfigData};
use crate::i18n::tr;
use gtk::prelude::*;
use gtk4 as gtk;
use libadwaita as adw;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::{Arc, OnceLock};

use rand::prelude::IndexedRandom;
use serde::Deserialize;

#[derive(Clone, Copy, Debug, Deserialize, Hash, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DikrCategory {
    Morning,
    Evening,
    Night,
}

#[derive(Clone, Deserialize)]
pub struct Dikr {
    pub id: String,
    pub category: DikrCategory,
    pub count: u32,
    #[serde(default)]
    pub count_display: Option<String>,
    pub arabic: String,
    #[serde(default)]
    pub translation: String,
    pub reference: String,
}

/// A translated adkar collection, split once per language; views borrow slices,
/// so page builds and reminders never copy the underlying texts.
pub struct AdkarSet {
    morning: Vec<Dikr>,
    evening: Vec<Dikr>,
    night: Vec<Dikr>,
}

impl AdkarSet {
    pub fn category(&self, category: DikrCategory) -> &[Dikr] {
        match category {
            DikrCategory::Morning => &self.morning,
            DikrCategory::Evening => &self.evening,
            DikrCategory::Night => &self.night,
        }
    }

    pub fn iter_all(&self) -> impl Iterator<Item = &Dikr> {
        self.morning.iter().chain(&self.evening).chain(&self.night)
    }

    /// Picks a session's daily reminder dhikr, favorites first.
    pub fn daily_picks(
        &self,
        category: DikrCategory,
        amount: usize,
        favorites: &[String],
    ) -> Vec<Dikr> {
        let mut rng = rand::rng();
        let pool = self.category(category);

        let favorited: Vec<Dikr> = pool
            .iter()
            .filter(|dikr| favorites.contains(&dikr.id))
            .cloned()
            .collect();

        if favorited.is_empty() {
            return pool.sample(&mut rng, amount).cloned().collect();
        }
        if favorited.len() >= amount {
            return favorited
                .as_slice()
                .sample(&mut rng, amount)
                .cloned()
                .collect();
        }

        let favorite_ids: HashSet<String> = favorited.iter().map(|dikr| dikr.id.clone()).collect();
        let mut picks = favorited;
        let remaining: Vec<Dikr> = pool
            .iter()
            .filter(|dikr| !favorite_ids.contains(dikr.id.as_str()))
            .cloned()
            .collect();
        picks.extend(
            remaining
                .as_slice()
                .sample(&mut rng, amount - picks.len())
                .cloned(),
        );
        picks
    }
}

/// Per-language adkar caches; each language is parsed only when first used.
struct AdkarCache {
    slots: [OnceLock<Arc<AdkarSet>>; 6],
}

impl AdkarCache {
    const fn new() -> Self {
        Self {
            slots: [const { OnceLock::new() }; 6],
        }
    }

    fn get(&self, language: &str) -> &Arc<AdkarSet> {
        if let Some(idx) = crate::i18n::SUPPORTED_LANGUAGES
            .iter()
            .position(|code| *code == language)
        {
            let code = crate::i18n::SUPPORTED_LANGUAGES[idx];
            self.slots[idx].get_or_init(|| Arc::new(load_adkar_set(code)))
        } else {
            // "en" (index 1) is the fallback for unsupported codes.
            const EN_INDEX: usize = 1;
            self.slots[EN_INDEX].get_or_init(|| Arc::new(load_adkar_set("en")))
        }
    }
}

static ADKAR_CACHE: AdkarCache = AdkarCache::new();

pub fn get_adkar(language: &str) -> Arc<AdkarSet> {
    Arc::clone(ADKAR_CACHE.get(language))
}

fn load_adkar_set(language: &str) -> AdkarSet {
    let all_adkar = load_adkar(language);
    let all_adkar = if all_adkar.is_empty() && language != "en" {
        load_adkar("en")
    } else {
        all_adkar
    };

    let mut morning = Vec::new();
    let mut evening = Vec::new();
    let mut night = Vec::new();
    for dikr in all_adkar {
        match dikr.category {
            DikrCategory::Morning => morning.push(dikr),
            DikrCategory::Evening => evening.push(dikr),
            DikrCategory::Night => night.push(dikr),
        }
    }
    AdkarSet {
        morning,
        evening,
        night,
    }
}

fn load_adkar(language: &str) -> Vec<Dikr> {
    let resource_path = if language == "ar" {
        String::from("/io/github/sniper1720/khushu/adkar/ar.json")
    } else {
        format!(
            "/io/github/sniper1720/khushu/adkar/translations/{}.json",
            language
        )
    };

    if let Ok(bytes) =
        gtk::gio::resources_lookup_data(&resource_path, gtk::gio::ResourceLookupFlags::NONE)
    {
        if let Ok(content) = std::str::from_utf8(&bytes) {
            if let Ok(adkar) = serde_json::from_str::<Vec<Dikr>>(content) {
                return adkar;
            } else {
                log::error!("Failed to deserialize Adkar JSON for lang: {}", language);
            }
        } else {
            log::error!("Adkar GResource was not valid UTF-8 for lang: {}", language);
        }
    } else {
        log::error!(
            "Failed to locate Adkar data for lang: {} in GResource",
            language
        );
    }
    vec![]
}

// A dhikr shared across session rows expands a pre-id favorite to every row
// carrying its Arabic text; returns `changed` so callers decide on persisting.
fn expand_favorites(
    favorites: &[String],
    arabic_to_ids: &HashMap<&str, Vec<&str>>,
) -> (Vec<String>, bool) {
    let mut changed = false;
    let mut new_favs = Vec::with_capacity(favorites.len());
    for fav in favorites {
        if let Some(ids) = arabic_to_ids.get(fav.as_str()) {
            changed = true;
            new_favs.extend(ids.iter().map(|id| id.to_string()));
        } else {
            new_favs.push(fav.clone());
        }
    }
    (new_favs, changed)
}

/// One-time v0 → v1 step: expands Arabic-text favorites to row ids.
pub fn migrate_favorites(config_data: &mut AppConfigData) {
    if config_data.favorites.is_empty() {
        return;
    }
    let source = get_adkar("ar");
    let mut arabic_to_ids: HashMap<&str, Vec<&str>> = HashMap::new();
    for dikr in source.iter_all() {
        arabic_to_ids
            .entry(dikr.arabic.trim())
            .or_default()
            .push(dikr.id.as_str());
    }

    let (new_favs, changed) = expand_favorites(&config_data.favorites, &arabic_to_ids);
    if changed {
        config_data.favorites = new_favs;
    }
}

pub fn create_adkar_page(config: AppConfig) -> (gtk::Box, Rc<dyn Fn()>) {
    let container = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let stack = adw::ViewStack::new();
    stack.set_hhomogeneous(false);
    stack.set_vhomogeneous(false);

    let switcher = adw::ViewSwitcher::new();
    switcher.set_stack(Some(&stack));
    switcher.set_halign(gtk::Align::Center);
    switcher.set_margin_top(6);
    switcher.set_margin_bottom(6);

    let switcher_clamp = adw::Clamp::builder()
        .child(&switcher)
        .maximum_size(340)
        .build();
    container.append(&switcher_clamp);

    let morning_scroll = gtk::ScrolledWindow::builder()
        .vexpand(true)
        .propagate_natural_width(false)
        .propagate_natural_height(false)
        .build();
    let morning_box = gtk::ListBox::new();
    morning_box.add_css_class("boxed-list");
    morning_box.set_selection_mode(gtk::SelectionMode::None);
    morning_box.set_margin_top(12);
    morning_box.set_margin_bottom(12);
    morning_box.set_margin_start(12);
    morning_box.set_margin_end(12);

    let morning_clamp = adw::Clamp::builder()
        .maximum_size(800)
        .tightening_threshold(600)
        .child(&morning_box)
        .build();
    morning_scroll.set_child(Some(&morning_clamp));

    let evening_scroll = gtk::ScrolledWindow::builder()
        .vexpand(true)
        .propagate_natural_width(false)
        .propagate_natural_height(false)
        .build();
    let evening_box = gtk::ListBox::new();
    evening_box.add_css_class("boxed-list");
    evening_box.set_selection_mode(gtk::SelectionMode::None);
    evening_box.set_margin_top(12);
    evening_box.set_margin_bottom(12);
    evening_box.set_margin_start(12);
    evening_box.set_margin_end(12);

    let evening_clamp = adw::Clamp::builder()
        .maximum_size(800)
        .tightening_threshold(600)
        .child(&evening_box)
        .build();
    evening_scroll.set_child(Some(&evening_clamp));

    let night_scroll = gtk::ScrolledWindow::builder()
        .vexpand(true)
        .propagate_natural_width(false)
        .propagate_natural_height(false)
        .build();
    let night_box = gtk::ListBox::new();
    night_box.add_css_class("boxed-list");
    night_box.set_selection_mode(gtk::SelectionMode::None);
    night_box.set_margin_top(12);
    night_box.set_margin_bottom(12);
    night_box.set_margin_start(12);
    night_box.set_margin_end(12);

    let night_clamp = adw::Clamp::builder()
        .maximum_size(800)
        .tightening_threshold(600)
        .child(&night_box)
        .build();
    night_scroll.set_child(Some(&night_clamp));

    let morning_box_rc = Rc::new(morning_box);
    let evening_box_rc = Rc::new(evening_box);
    let night_box_rc = Rc::new(night_box);

    let morning_box_clone = morning_box_rc.clone();
    let evening_box_clone = evening_box_rc.clone();
    let night_box_clone = night_box_rc.clone();

    let rebuild_lists = Rc::new(move |config_for_adkar_lists: AppConfig| {
        let morning_box_ref = &*morning_box_clone;
        let evening_box_ref = &*evening_box_clone;
        let night_box_ref = &*night_box_clone;

        while let Some(child) = morning_box_ref.first_child() {
            morning_box_ref.remove(&child);
        }
        while let Some(child) = evening_box_ref.first_child() {
            evening_box_ref.remove(&child);
        }
        while let Some(child) = night_box_ref.first_child() {
            night_box_ref.remove(&child);
        }

        let favorites = config_for_adkar_lists.favorites();
        let language = crate::i18n::supported_language_code(&config_for_adkar_lists.language());
        let adkar_set = get_adkar(&language);
        let show_translation = language != "ar";

        for dikr in adkar_set.category(DikrCategory::Morning) {
            morning_box_ref.append(&create_dikr_row(
                dikr,
                favorites.contains(&dikr.id),
                &config_for_adkar_lists,
                show_translation,
            ));
        }
        for dikr in adkar_set.category(DikrCategory::Evening) {
            evening_box_ref.append(&create_dikr_row(
                dikr,
                favorites.contains(&dikr.id),
                &config_for_adkar_lists,
                show_translation,
            ));
        }
        for dikr in adkar_set.category(DikrCategory::Night) {
            night_box_ref.append(&create_dikr_row(
                dikr,
                favorites.contains(&dikr.id),
                &config_for_adkar_lists,
                show_translation,
            ));
        }
    });

    rebuild_lists(config.clone());

    let morning_page = stack.add_titled(&morning_scroll, Some("morning"), &tr("Morning"));
    morning_page.set_icon_name(Some("weather-clear-symbolic"));

    let evening_page = stack.add_titled(&evening_scroll, Some("evening"), &tr("Evening"));
    evening_page.set_icon_name(Some("weather-few-clouds-night-symbolic"));

    let night_page = stack.add_titled(&night_scroll, Some("night"), &tr("Night"));
    night_page.set_icon_name(Some("weather-clear-night-symbolic"));

    container.append(&stack);

    let rebuild_lists_refresh = rebuild_lists.clone();
    let config_for_adkar_refresh = config.clone();
    let refresh_ui = Rc::new(move || {
        morning_page.set_title(Some(&tr("Morning")));
        evening_page.set_title(Some(&tr("Evening")));
        night_page.set_title(Some(&tr("Night")));
        rebuild_lists_refresh(config_for_adkar_refresh.clone());
    });

    (container, refresh_ui)
}

fn create_dikr_row(
    dikr: &Dikr,
    is_favorite: bool,
    config: &AppConfig,
    show_translation: bool,
) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    row.add_css_class("activatable");

    let vbox = gtk::Box::new(gtk::Orientation::Vertical, 6);
    vbox.set_margin_top(12);
    vbox.set_margin_bottom(12);
    vbox.set_margin_start(12);
    vbox.set_margin_end(12);

    let top_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    spacer.set_hexpand(true);

    let fav_btn = gtk::Button::builder()
        .icon_name("user-bookmarks-symbolic")
        .has_frame(false)
        .build();
    if is_favorite {
        fav_btn.add_css_class("accent");
    }

    let config_for_favorites = config.clone();
    let dikr_id_signal = dikr.id.clone();

    fav_btn.connect_clicked(move |button| {
        let currently_fav = config_for_favorites.favorites().contains(&dikr_id_signal);
        if currently_fav {
            let mut favorites = config_for_favorites.favorites();
            favorites.retain(|fav_id| fav_id != &dikr_id_signal);
            config_for_favorites.set_favorites(favorites);
        } else {
            let mut favorites = config_for_favorites.favorites();
            favorites.push(dikr_id_signal.clone());
            config_for_favorites.set_favorites(favorites);
        }
        config_for_favorites.save();

        if currently_fav {
            button.remove_css_class("accent");
        } else {
            button.add_css_class("accent");
        }
    });

    top_box.append(&spacer);
    top_box.append(&fav_btn);
    vbox.append(&top_box);

    let lbl_arabic = gtk::Label::builder()
        .label(&dikr.arabic)
        .wrap(true)
        .justify(gtk::Justification::Center)
        .css_classes(["arabic-text"])
        .build();
    let attrs = gtk::pango::AttrList::new();
    let mut font_desc = gtk::pango::FontDescription::new();
    font_desc.set_size(18 * gtk::pango::SCALE);
    attrs.insert(gtk::pango::AttrFontDesc::new(&font_desc));
    lbl_arabic.set_attributes(Some(&attrs));

    let lbl_trans = gtk::Label::builder()
        .label(&dikr.translation)
        .wrap(true)
        .justify(gtk::Justification::Center)
        .css_classes(["caption"])
        .build();

    let count_text = dikr
        .count_display
        .clone()
        .unwrap_or_else(|| format!("{}x", dikr.count));
    let count_label = gtk::Label::builder()
        .label(&count_text)
        .halign(gtk::Align::Center)
        .css_classes(["numeric", "badge"])
        .build();
    let lbl_ref = gtk::Label::builder()
        .label(&dikr.reference)
        .wrap(true)
        .justify(gtk::Justification::Center)
        .css_classes(["dim-label", "caption-heading"])
        .build();

    vbox.append(&lbl_arabic);
    if show_translation {
        vbox.append(&lbl_trans);
    }
    vbox.append(&count_label);
    vbox.append(&lbl_ref);

    row.set_child(Some(&vbox));
    row
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Once;

    static RESOURCES: Once = Once::new();

    fn ensure_resources() {
        RESOURCES.call_once(|| {
            gtk::gio::resources_register_include!("khushu-resources.gresource")
                .expect("failed to register gresource");
        });
    }

    #[test]
    fn morning_adkar_have_correct_category() {
        ensure_resources();
        for language in ["en", "ar"] {
            let adkar_set = get_adkar(language);
            let dhikr_list = adkar_set.category(DikrCategory::Morning);
            if dhikr_list.is_empty() {
                continue;
            }
            for dikr in dhikr_list {
                assert_eq!(
                    dikr.category,
                    DikrCategory::Morning,
                    "Expected Morning, got {:?}",
                    dikr.category
                );
            }
        }
    }

    #[test]
    fn evening_adkar_have_correct_category() {
        ensure_resources();
        for language in ["en", "ar"] {
            let adkar_set = get_adkar(language);
            let dhikr_list = adkar_set.category(DikrCategory::Evening);
            if dhikr_list.is_empty() {
                continue;
            }
            for dikr in dhikr_list {
                assert_eq!(dikr.category, DikrCategory::Evening);
            }
        }
    }

    #[test]
    fn night_adkar_have_correct_category() {
        ensure_resources();
        for language in ["en", "ar"] {
            let adkar_set = get_adkar(language);
            let dhikr_list = adkar_set.category(DikrCategory::Night);
            if dhikr_list.is_empty() {
                continue;
            }
            for dikr in dhikr_list {
                assert_eq!(dikr.category, DikrCategory::Night);
            }
        }
    }

    #[test]
    fn random_morning_dikr_returns_some_when_data_exists() {
        ensure_resources();
        let adkar_set = get_adkar("en");
        let dhikr_list = adkar_set.category(DikrCategory::Morning);
        if dhikr_list.is_empty() {
            return;
        }
        let pick = adkar_set.daily_picks(DikrCategory::Morning, 1, &[]);
        assert!(
            !pick.is_empty(),
            "Random pick should return elements when data exists"
        );
    }

    #[test]
    fn daily_guarantees_slot_for_single_favorite() {
        ensure_resources();
        let adkar_set = get_adkar("en");
        let morning: Vec<&Dikr> = adkar_set.category(DikrCategory::Morning).iter().collect();
        if morning.is_empty() {
            return;
        }
        let fav_id = morning[0].id.clone();
        let favorites = vec![fav_id.clone()];
        for _ in 0..20 {
            let picks = adkar_set.daily_picks(DikrCategory::Morning, 2, &favorites);
            assert_eq!(picks.len(), 2, "daily must return exactly two picks");
            assert_ne!(picks[0].id, picks[1].id, "picks must be distinct");
            assert!(
                picks.iter().any(|dikr| dikr.id == fav_id),
                "single favorite must always occupy a daily slot"
            );
        }
    }

    #[test]
    fn daily_restricts_to_favorites_when_enough_exist() {
        ensure_resources();
        let adkar_set = get_adkar("en");
        let morning: Vec<&Dikr> = adkar_set.category(DikrCategory::Morning).iter().collect();
        if morning.len() < 2 {
            return;
        }
        let fav_ids: Vec<String> = morning.iter().map(|dikr| dikr.id.clone()).collect();
        for _ in 0..20 {
            let picks = adkar_set.daily_picks(DikrCategory::Morning, 2, &fav_ids);
            assert_eq!(picks.len(), 2);
            assert!(
                picks.iter().all(|dikr| fav_ids.contains(&dikr.id)),
                "picks must come only from the favorited set"
            );
        }
    }

    #[test]
    fn daily_keeps_both_favorites_when_exactly_two_exist() {
        ensure_resources();
        let adkar_set = get_adkar("en");
        let morning: Vec<&Dikr> = adkar_set.category(DikrCategory::Morning).iter().collect();
        if morning.len() < 2 {
            return;
        }
        let favorites = vec![morning[0].id.clone(), morning[1].id.clone()];
        for _ in 0..20 {
            let picks = adkar_set.daily_picks(DikrCategory::Morning, 2, &favorites);
            assert_eq!(picks.len(), 2);
            assert!(
                picks.iter().any(|dikr| dikr.id == favorites[0]),
                "first favorited dhikr must appear daily"
            );
            assert!(
                picks.iter().any(|dikr| dikr.id == favorites[1]),
                "second favorited dhikr must appear daily"
            );
        }
    }

    #[test]
    fn migrate_favorites_expands_shared_dhikr_and_keeps_ids() {
        ensure_resources();
        let source = get_adkar("ar");
        let mut arabic_to_ids: HashMap<&str, Vec<&str>> = HashMap::new();
        for dikr in source.iter_all() {
            arabic_to_ids
                .entry(dikr.arabic.trim())
                .or_default()
                .push(dikr.id.as_str());
        }

        let shared_ids: Vec<String> = arabic_to_ids
            .values()
            .find(|ids| ids.len() > 1)
            .expect("expected a dhikr shared across sessions")
            .iter()
            .map(|id| id.to_string())
            .collect();
        let shared_arabic = source
            .iter_all()
            .find(|dikr| shared_ids.contains(&dikr.id))
            .expect("shared ids must exist in the source")
            .arabic
            .trim()
            .to_string();

        let (out, changed) =
            expand_favorites(&[shared_arabic, "night-01".to_string()], &arabic_to_ids);
        assert!(changed, "legacy favorites must report a change");
        assert_eq!(
            out.len(),
            3,
            "shared dhikr expands to both ids plus night-01"
        );
        assert!(
            out.contains(&"night-01".to_string()),
            "ids pass through untouched"
        );
        for id in &shared_ids {
            assert!(out.contains(id), "expected expanded id {id}");
        }

        let (again, changed_again) = expand_favorites(&out, &arabic_to_ids);
        assert!(!changed_again, "already-migrated favorites are a no-op");
        assert_eq!(again, out);
    }

    #[test]
    fn adkar_entries_have_non_empty_fields() {
        ensure_resources();
        for language in ["ar", "en"] {
            let all_adkar = load_adkar(language);
            if all_adkar.is_empty() {
                continue;
            }
            for dikr in &all_adkar {
                assert!(!dikr.id.trim().is_empty(), "Id must not be empty");
                assert!(
                    !dikr.arabic.trim().is_empty(),
                    "Arabic text must not be empty"
                );
                assert!(
                    !dikr.reference.trim().is_empty(),
                    "Reference must not be empty"
                );
                assert!(dikr.count > 0, "Count must be positive");
                if language == "ar" {
                    assert!(
                        dikr.translation.trim().is_empty(),
                        "Arabic source must not carry a translation"
                    );
                } else {
                    assert!(
                        !dikr.translation.trim().is_empty(),
                        "Translation must not be empty"
                    );
                }
            }
        }
    }

    #[test]
    fn ids_are_unique_per_language() {
        ensure_resources();
        for language in ["ar", "en"] {
            let all_adkar = load_adkar(language);
            let mut seen = std::collections::HashSet::new();
            for dikr in &all_adkar {
                assert!(
                    seen.insert(dikr.id.as_str()),
                    "duplicate id '{}' in {}",
                    dikr.id,
                    language
                );
            }
        }
    }

    #[test]
    fn localized_files_match_source() {
        ensure_resources();
        let source = load_adkar("ar");
        if source.is_empty() {
            return;
        }
        let source_multiset: HashMap<(&str, DikrCategory), usize> =
            source.iter().fold(HashMap::new(), |mut acc, dikr| {
                *acc.entry((dikr.id.as_str(), dikr.category)).or_insert(0) += 1;
                acc
            });

        for language in ["en", "fr", "es", "tr", "id"] {
            let translations = load_adkar(language);
            if translations.is_empty() {
                continue;
            }
            let translation_multiset = translations.iter().fold(HashMap::new(), |mut acc, dikr| {
                *acc.entry((dikr.id.as_str(), dikr.category)).or_insert(0) += 1;
                acc
            });
            assert_eq!(
                translation_multiset, source_multiset,
                "lang {} must have the exact same (id, category) multiset as ar.json (no missing or duplicate entries)",
                language
            );
            for dikr in &translations {
                let source_dikr = source
                    .iter()
                    .find(|candidate| {
                        candidate.id == dikr.id && candidate.category == dikr.category
                    })
                    .unwrap();
                assert_eq!(
                    dikr.count, source_dikr.count,
                    "lang {} id '{}' count",
                    language, dikr.id
                );
                assert_eq!(
                    dikr.arabic, source_dikr.arabic,
                    "lang {} id '{}' arabic",
                    language, dikr.id
                );
                assert!(
                    !dikr.reference.trim().is_empty(),
                    "lang {} id '{}' reference must not be empty",
                    language,
                    dikr.id
                );
                assert!(
                    !dikr.translation.trim().is_empty(),
                    "lang {} id '{}' translation must not be empty",
                    language,
                    dikr.id
                );
            }
        }
    }

    #[test]
    fn fallback_to_english_for_unsupported_locale() {
        ensure_resources();
        let unsupported = get_adkar("de");
        let english = get_adkar("en");
        let unsupported_morning = unsupported.category(DikrCategory::Morning);
        let english_morning = english.category(DikrCategory::Morning);
        if unsupported_morning.is_empty() {
            return;
        }
        assert_eq!(unsupported_morning.len(), english_morning.len());
        assert_eq!(unsupported_morning[0].id, english_morning[0].id);
        assert_eq!(
            unsupported_morning[0].translation,
            english_morning[0].translation
        );
    }
}
