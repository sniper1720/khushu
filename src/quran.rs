use crate::config::{AppConfig, QuranBookmark, StopCondition};
use crate::i18n::tr;

use gtk::ListBox;
use gtk4 as gtk;
use gtk4::glib;
use libadwaita as adw;
use libadwaita::prelude::*;
use serde::Deserialize;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

type RebuildFn = Rc<RefCell<Option<Box<dyn Fn()>>>>;
type PlayFn = Rc<RefCell<Option<Box<dyn Fn(u32, u32)>>>>;

#[derive(Clone, Debug, Deserialize)]
pub struct Verse {
    pub id: u32,
    pub text: String,
    #[serde(default)]
    pub translation: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct TranslationSurah {
    pub id: u32,
    pub name: String,
    pub transliteration: String,
    pub translation: String,
    #[serde(rename = "type")]
    pub surah_type: String,
    #[serde(rename = "total_verses")]
    pub total_verses: u32,
    pub verses: Vec<Verse>,
}

#[derive(Clone, Debug, Deserialize)]
struct ArabicVerse {
    verse: u32,
    text: String,
}

#[derive(Clone, Debug, Deserialize)]
struct ArabicData(HashMap<String, Vec<ArabicVerse>>);

#[derive(Clone, Debug, Deserialize)]
struct SurahInfo {
    id: u32,
    name: String,
    transliteration: String,
    #[serde(rename = "type")]
    surah_type: String,
    #[serde(rename = "total_verses")]
    total_verses: u32,
}

#[derive(Clone, Debug, Deserialize)]
struct SurahsWrapper {
    data: Vec<SurahInfo>,
}

#[derive(Clone, Debug, Deserialize)]
struct DataWrapper<T> {
    data: Vec<T>,
}

#[derive(Clone, Debug, Deserialize)]
struct MarkerPos {
    id: u32,
    #[serde(rename = "surah")]
    surah_num: u32,
    verse: u32,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PageVerse {
    verse: u32,
    surah_num: u32,
    content: String,
}

#[derive(Clone, Debug, Deserialize)]
struct PageStart {
    #[serde(rename = "surah")]
    surah_num: u32,
    verse: u32,
}

#[derive(Clone, Debug, Deserialize)]
struct PageIndex {
    #[serde(rename = "page_starts")]
    page_starts: HashMap<u32, PageStart>,
    #[serde(rename = "surah_start_pages")]
    surah_start_pages: HashMap<u32, u32>,
    #[serde(rename = "surah_page_count")]
    surah_page_count: HashMap<u32, u32>,
    #[serde(rename = "surah_verse_counts")]
    surah_verse_counts: HashMap<u32, u32>,
    #[serde(rename = "total_pages")]
    total_pages: u32,
}

fn get_surah_info() -> Option<Vec<SurahInfo>> {
    if let Ok(bytes) = gtk::gio::resources_lookup_data(
        "/io/github/sniper1720/khushu/quran/chapters.json",
        gtk::gio::ResourceLookupFlags::NONE,
    ) && let Ok(content) = std::str::from_utf8(&bytes)
    {
        if let Ok(wrapper) = serde_json::from_str::<SurahsWrapper>(content) {
            return Some(wrapper.data);
        }
        if let Ok(info) = serde_json::from_str::<Vec<SurahInfo>>(content) {
            return Some(info);
        }
    }
    None
}

fn parse_arabic_data(json: &str) -> Vec<TranslationSurah> {
    if let Ok(data) = serde_json::from_str::<ArabicData>(json) {
        let surah_info = get_surah_info();
        let mut surahs: Vec<TranslationSurah> = Vec::new();
        for (key, verses) in data.0 {
            if let Ok(surah_num) = key.parse::<u32>() {
                let info = surah_info.as_ref().and_then(|surah_infos| {
                    surah_infos
                        .iter()
                        .find(|surah| surah.id == surah_num)
                        .cloned()
                });
                let surah_verses: Vec<Verse> = verses
                    .into_iter()
                    .map(|verse_data| Verse {
                        id: verse_data.verse,
                        text: verse_data.text,
                        translation: String::new(),
                    })
                    .collect();
                let surah_name = info
                    .as_ref()
                    .map(|surah| surah.name.clone())
                    .unwrap_or_default();
                let surah_translit = info
                    .as_ref()
                    .map(|surah| surah.transliteration.clone())
                    .unwrap_or_default();
                let surah_type = info
                    .as_ref()
                    .map(|surah| surah.surah_type.clone())
                    .unwrap_or_else(|| String::from("meccan"));
                let surah_total = info
                    .as_ref()
                    .map(|surah| surah.total_verses)
                    .unwrap_or(surah_verses.len() as u32);
                surahs.push(TranslationSurah {
                    id: surah_num,
                    name: surah_name,
                    transliteration: surah_translit,
                    translation: String::new(),
                    surah_type,
                    total_verses: surah_total,
                    verses: surah_verses,
                });
            }
        }
        surahs.sort_by_key(|surah| surah.id);
        surahs
    } else {
        Vec::new()
    }
}

type NormalizedIndex = HashMap<(u32, u32), String>;
type MarkerIndexU32 = HashMap<(u32, u32), u32>;

thread_local! {
    static QURAN_CACHE: std::cell::RefCell<Option<HashMap<String, Rc<Vec<TranslationSurah>>>>> = const { std::cell::RefCell::new(None) };
    static SURAH_READING_POSITIONS: std::cell::RefCell<HashMap<u32, u32>> = std::cell::RefCell::new(HashMap::new());
    static PAGE_INDEX: std::cell::RefCell<Option<PageIndex>> = const { std::cell::RefCell::new(None) };
    static NORMALIZED_CACHE: std::cell::RefCell<Option<Rc<NormalizedIndex>>> = const { std::cell::RefCell::new(None) };
    static JUZ_CACHE: std::cell::RefCell<Option<Rc<MarkerIndexU32>>> = const { std::cell::RefCell::new(None) };
    static HIZB_QUARTER_CACHE: std::cell::RefCell<Option<Rc<MarkerIndexU32>>> = const { std::cell::RefCell::new(None) };
    static JUZ_LIST_CACHE: std::cell::RefCell<Option<Rc<Vec<MarkerPos>>>> = const { std::cell::RefCell::new(None) };
    static HIZB_QUARTER_LIST_CACHE: std::cell::RefCell<Option<Rc<Vec<MarkerPos>>>> = const { std::cell::RefCell::new(None) };
}

fn get_normalized_index() -> Rc<NormalizedIndex> {
    NORMALIZED_CACHE.with(|cache| {
        let mut cache_ref = cache.borrow_mut();
        if let Some(ref index) = *cache_ref {
            return Rc::clone(index);
        }
        let arabic_quran = get_quran("ar");
        let mut index = HashMap::new();
        for surah in arabic_quran.iter() {
            for verse in &surah.verses {
                index.insert((surah.id, verse.id), normalize_arabic(&verse.text));
            }
        }
        let index_rc = Rc::new(index);
        *cache_ref = Some(Rc::clone(&index_rc));
        index_rc
    })
}

fn load_marker_index_u32(resource_path: &str) -> Option<MarkerIndexU32> {
    if let Ok(bytes) =
        gtk::gio::resources_lookup_data(resource_path, gtk::gio::ResourceLookupFlags::NONE)
        && let Ok(content) = std::str::from_utf8(&bytes)
        && let Ok(wrapper) = serde_json::from_str::<DataWrapper<MarkerPos>>(content)
    {
        let mut marker_index = HashMap::new();
        for marker in wrapper.data {
            marker_index.insert((marker.surah_num, marker.verse), marker.id);
        }
        return Some(marker_index);
    }
    None
}

fn load_marker_positions(resource_path: &str) -> Option<Vec<MarkerPos>> {
    if let Ok(bytes) =
        gtk::gio::resources_lookup_data(resource_path, gtk::gio::ResourceLookupFlags::NONE)
        && let Ok(content) = std::str::from_utf8(&bytes)
        && let Ok(wrapper) = serde_json::from_str::<DataWrapper<MarkerPos>>(content)
    {
        let mut markers = wrapper.data;
        markers.sort_by_key(|marker| (marker.surah_num, marker.verse));
        return Some(markers);
    }
    None
}

fn get_juz_index() -> Rc<MarkerIndexU32> {
    JUZ_CACHE.with(|cache| {
        let mut cache_ref = cache.borrow_mut();
        if let Some(ref idx) = *cache_ref {
            return Rc::clone(idx);
        }
        let index = load_marker_index_u32("/io/github/sniper1720/khushu/quran/juzs.json")
            .unwrap_or_default();
        let index_rc = Rc::new(index);
        *cache_ref = Some(Rc::clone(&index_rc));
        index_rc
    })
}

fn get_juz_list() -> Rc<Vec<MarkerPos>> {
    JUZ_LIST_CACHE.with(|cache| {
        let mut cache_ref = cache.borrow_mut();
        if let Some(ref idx) = *cache_ref {
            return Rc::clone(idx);
        }
        let list = load_marker_positions("/io/github/sniper1720/khushu/quran/juzs.json")
            .unwrap_or_default();
        let list_rc = Rc::new(list);
        *cache_ref = Some(Rc::clone(&list_rc));
        list_rc
    })
}

fn get_hizb_quarter_index() -> Rc<MarkerIndexU32> {
    HIZB_QUARTER_CACHE.with(|cache| {
        let mut cache_ref = cache.borrow_mut();
        if let Some(ref idx) = *cache_ref {
            return Rc::clone(idx);
        }
        let index = load_marker_index_u32("/io/github/sniper1720/khushu/quran/hizbs.json")
            .unwrap_or_default();
        let index_rc = Rc::new(index);
        *cache_ref = Some(Rc::clone(&index_rc));
        index_rc
    })
}

fn get_hizb_quarter_list() -> Rc<Vec<MarkerPos>> {
    HIZB_QUARTER_LIST_CACHE.with(|cache| {
        let mut cache_ref = cache.borrow_mut();
        if let Some(ref idx) = *cache_ref {
            return Rc::clone(idx);
        }
        let list = load_marker_positions("/io/github/sniper1720/khushu/quran/hizbs.json")
            .unwrap_or_default();
        let list_rc = Rc::new(list);
        *cache_ref = Some(Rc::clone(&list_rc));
        list_rc
    })
}

fn load_page_index() -> Option<PageIndex> {
    if let Ok(bytes) = gtk::gio::resources_lookup_data(
        "/io/github/sniper1720/khushu/quran/quran_pages_index.json",
        gtk::gio::ResourceLookupFlags::NONE,
    ) && let Ok(content) = std::str::from_utf8(&bytes)
        && let Ok(index) = serde_json::from_str::<PageIndex>(content)
    {
        return Some(index);
    }
    None
}

fn get_page_index() -> Option<PageIndex> {
    PAGE_INDEX.with(|cache| {
        let mut cache_ref = cache.borrow_mut();
        if cache_ref.is_none() {
            *cache_ref = load_page_index();
        }
        cache_ref.clone()
    })
}

pub fn get_surah_start_page(surah_num: u32) -> Option<u32> {
    get_page_index()
        .and_then(|idx| idx.surah_start_pages.get(&surah_num).copied())
        .or_else(|| get_verse_page(surah_num, 1))
}

pub fn get_surah_page_count(surah_num: u32) -> Option<u32> {
    get_page_index().and_then(|idx| idx.surah_page_count.get(&surah_num).copied())
}

pub fn get_total_pages() -> u32 {
    get_page_index().map(|idx| idx.total_pages).unwrap_or(604)
}

pub fn get_verse_page(surah_num: u32, verse: u32) -> Option<u32> {
    let page_index = get_page_index()?;
    let page_starts = &page_index.page_starts;
    let target = (surah_num, verse);
    let total_pages = page_index.total_pages;

    for page_id in 1..=total_pages {
        let Some(start) = page_starts.get(&page_id) else {
            continue;
        };
        let start_pos = (start.surah_num, start.verse);

        let end_pos = if let Some(next_start) = page_starts.get(&(page_id + 1)) {
            (next_start.surah_num, next_start.verse)
        } else {
            (115, 0)
        };

        if target >= start_pos && target < end_pos {
            return Some(page_id);
        }
    }

    None
}

pub fn get_page_verses(page: u32) -> Option<Vec<PageVerse>> {
    let page_index = get_page_index()?;
    let arabic = get_quran("ar");
    let page_start = page_index.page_starts.get(&page)?;
    let next_page = page + 1;

    let mut verses = Vec::new();
    let mut current_surah_num = page_start.surah_num;
    let mut current_verse = page_start.verse;

    let end_surah_num;
    let end_verse;
    if let Some(next_start) = page_index.page_starts.get(&next_page) {
        end_surah_num = next_start.surah_num;
        end_verse = next_start.verse;
    } else {
        end_surah_num = 115;
        end_verse = 0;
    }

    loop {
        if current_surah_num == end_surah_num && current_verse >= end_verse {
            break;
        }
        if current_surah_num > end_surah_num {
            break;
        }

        if let Some(surah) = arabic.iter().find(|surah| surah.id == current_surah_num)
            && let Some(verse) = surah
                .verses
                .iter()
                .find(|verse_data| verse_data.id == current_verse)
        {
            verses.push(PageVerse {
                verse: current_verse,
                surah_num: current_surah_num,
                content: verse.text.clone(),
            });
        }

        current_verse += 1;
        if current_verse
            > page_index
                .surah_verse_counts
                .get(&current_surah_num)
                .copied()
                .unwrap_or(0)
        {
            current_surah_num += 1;
            current_verse = 1;
            if current_surah_num > 114 {
                break;
            }
        }
    }

    Some(verses)
}

fn load_quran(lang: &str) -> Vec<TranslationSurah> {
    let resource_path = if lang == "ar" {
        String::from("/io/github/sniper1720/khushu/quran/ar.json")
    } else {
        format!(
            "/io/github/sniper1720/khushu/quran/translations/{}.json",
            lang
        )
    };

    if let Ok(bytes) =
        gtk::gio::resources_lookup_data(&resource_path, gtk::gio::ResourceLookupFlags::NONE)
    {
        if let Ok(content) = std::str::from_utf8(&bytes) {
            if lang == "ar" {
                return parse_arabic_data(content);
            } else {
                if let Ok(quran) = serde_json::from_str::<Vec<TranslationSurah>>(content) {
                    return quran;
                } else {
                    log::error!("Failed to deserialize Quran JSON for lang: {}", lang);
                }
            }
        } else {
            log::error!("Quran GResource was not valid UTF-8 for lang: {}", lang);
        }
    } else {
        log::error!(
            "Failed to locate Quran data for lang: {} in GResource",
            lang
        );
    }
    vec![]
}

fn get_quran(lang: &str) -> Rc<Vec<TranslationSurah>> {
    QURAN_CACHE.with(|cache| {
        let mut cache_ref = cache.borrow_mut();
        if cache_ref.is_none() {
            *cache_ref = Some(HashMap::new());
        }
        if let Some(ref mut map) = cache_ref.as_mut() {
            if let Some(data) = map.get(lang) {
                return Rc::clone(data);
            }
            let data = Rc::new(load_quran(lang));
            map.insert(lang.to_string(), Rc::clone(&data));
            data
        } else {
            Rc::new(load_quran(lang))
        }
    })
}

pub fn get_surah(surah_num: u32, lang: &str) -> Option<TranslationSurah> {
    let quran = get_quran(lang);
    quran.iter().find(|surah| surah.id == surah_num).cloned()
}

pub fn get_verse(surah_num: u32, verse: u32, lang: &str) -> Option<Verse> {
    let quran = get_quran(lang);
    quran
        .iter()
        .find(|surah_data| surah_data.id == surah_num)
        .and_then(|surah_data| {
            surah_data
                .verses
                .iter()
                .find(|verse_data| verse_data.id == verse)
                .cloned()
        })
}

pub fn get_arabic_text(surah_num: u32, verse: u32) -> Option<String> {
    let quran = get_quran("ar");
    quran
        .iter()
        .find(|surah_data| surah_data.id == surah_num)
        .and_then(|surah_data| {
            surah_data
                .verses
                .iter()
                .find(|verse_data| verse_data.id == verse)
                .map(|verse_data| verse_data.text.clone())
        })
}

#[derive(Clone, Debug)]
pub struct SurahListItem {
    pub id: u32,
    pub name: String,
    pub transliteration: String,
    pub translation: String,
    pub surah_type: String,
    pub total_verses: u32,
}

#[derive(Clone, Debug)]
pub struct VerseMatch {
    pub surah_id: u32,
    pub verse_id: u32,
    pub translation_text: String,
    pub surah_name: String,
    pub surah_translation: String,
}

pub fn get_surah_list(lang: &str) -> Vec<SurahListItem> {
    get_quran(lang)
        .iter()
        .map(|surah| SurahListItem {
            id: surah.id,
            name: surah.name.clone(),
            transliteration: surah.transliteration.clone(),
            translation: surah.translation.clone(),
            surah_type: surah.surah_type.clone(),
            total_verses: surah.total_verses,
        })
        .collect()
}

fn is_arabic_ignorable(character: char) -> bool {
    let code = character as u32;
    matches!(code,
        // Arabic combining marks
        0x0610..=0x061A |
        // Tatweel (elongation mark used in Uthmani script)
        0x0640 |
        // Tashkeel (Arabic diacritics / vowel marks)
        0x064B..=0x065F |
        // Quranic annotation signs
        0x06D6..=0x06DC | 0x06DD | 0x06DE | 0x06DF..=0x06E4 | 0x06E7..=0x06ED |
        // Small high/low letters used in Uthmani orthography
        0x06E5 | 0x06E6 |
        // Extended Arabic marks (found in some Uthmani fonts)
        0x08D3..=0x08FF |
        // Arabic presentation forms — combining marks
        0xFE70..=0xFE7F
    )
}

fn is_arabic_query(query: &str) -> bool {
    query.chars().all(|character| {
        let code = character as u32;
        character.is_whitespace()
            || is_arabic_ignorable(character)
            || (0x0600..=0x06FF).contains(&code)
            || (0x0750..=0x077F).contains(&code)
            || (0x08A0..=0x08FF).contains(&code)
            || (0xFB50..=0xFDFF).contains(&code)
            || (0xFE70..=0xFEFF).contains(&code)
    })
}

fn normalize_arabic_char(character: char) -> char {
    match character {
        // Alef variants → plain Alef  (أ إ آ ٱ ٲ ٳ → ا)
        // U+0670 = Superscript Alef (dagger alef) — represents a pronounced alef in Uthmani
        '\u{0622}' | '\u{0623}' | '\u{0625}' | '\u{0670}' | '\u{0671}' | '\u{0672}'
        | '\u{0673}' => '\u{0627}',
        // Alef Maqsura → Yaa  (ى → ي)
        '\u{0649}' => '\u{064A}',
        // Taa Marbuta → Haa  (ة → ه)
        '\u{0629}' => '\u{0647}',
        // Waw with hamza → plain Waw  (ؤ → و)
        '\u{0624}' => '\u{0648}',
        // Yaa with hamza → plain Yaa  (ئ → ي)
        '\u{0626}' => '\u{064A}',
        // Standalone hamza variants → drop (handled by filter below)
        _ => character,
    }
}

fn normalize_arabic(text: &str) -> String {
    text.chars()
        .filter(|character| {
            let code = *character as u32;
            if code < 0x0600 {
                return true;
            }
            if is_arabic_ignorable(*character) {
                return false;
            }
            !matches!(code, 0x0621 | 0x0674)
        })
        .map(normalize_arabic_char)
        .collect()
}

pub fn search_quran(query: &str, lang: &str) -> Vec<VerseMatch> {
    if query.is_empty() {
        return Vec::new();
    }
    let quran = get_quran(lang);
    let mut matches = Vec::new();

    let is_arabic_query = is_arabic_query(query);

    let search_query = if is_arabic_query {
        normalize_arabic(query)
    } else {
        query.to_lowercase()
    };

    let normalized_index = if is_arabic_query {
        Some(get_normalized_index())
    } else {
        None
    };

    for surah in quran.iter() {
        for verse in surah.verses.iter() {
            let matches_arabic = is_arabic_query
                && normalized_index
                    .as_ref()
                    .and_then(|idx| idx.get(&(surah.id, verse.id)))
                    .map(|norm| norm.contains(&search_query))
                    .unwrap_or(false);

            let translation_text = if lang == "ar" {
                String::new()
            } else {
                verse.translation.clone()
            };
            let matches_translation =
                !is_arabic_query && translation_text.to_lowercase().contains(&search_query);

            if matches_arabic || matches_translation {
                matches.push(VerseMatch {
                    surah_id: surah.id,
                    verse_id: verse.id,
                    translation_text: if lang == "ar" {
                        verse.text.clone()
                    } else {
                        translation_text
                    },
                    surah_name: surah.name.clone(),
                    surah_translation: surah.translation.clone(),
                });
            }
        }
    }

    matches.sort_by(|left, right| {
        let left_key = format!("{:03}{:04}", left.surah_id, left.verse_id);
        let right_key = format!("{:03}{:04}", right.surah_id, right.verse_id);
        left_key.cmp(&right_key)
    });

    matches
}

pub fn create_quran_page(
    current_lang: &str,
    view_stack: &adw::ViewStack,
    config: AppConfig,
) -> gtk::Widget {
    let top_box = gtk::Box::new(gtk::Orientation::Vertical, 0);

    let switcher = adw::ViewSwitcher::new();
    let quran_view_stack = adw::ViewStack::new();

    let container = gtk::Box::new(gtk::Orientation::Vertical, 0);

    let search_entry = gtk::SearchEntry::new();
    search_entry.set_widget_name("quran_search");
    search_entry.set_placeholder_text(Some(&tr("Search surahs")));
    search_entry.set_margin_top(12);
    search_entry.set_margin_bottom(6);
    search_entry.set_margin_start(12);
    search_entry.set_margin_end(12);
    container.append(&search_entry);

    let list_box = ListBox::new();
    list_box.set_widget_name("surah_list_box");
    list_box.add_css_class("list-box");
    list_box.set_selection_mode(gtk::SelectionMode::None);
    list_box.set_activate_on_single_click(true);

    let scrolled = gtk::ScrolledWindow::builder().vexpand(true).build();
    scrolled.set_child(Some(&list_box));
    container.append(&scrolled);

    let quran_lang_owned = crate::i18n::supported_language_code(current_lang);
    let quran_lang = quran_lang_owned.as_str();
    let surah_list = get_surah_list(quran_lang);

    let list_box_rc: Rc<RefCell<ListBox>> = Rc::new(RefCell::new(list_box));

    let bookmarks_row = adw::ExpanderRow::new();
    bookmarks_row.set_widget_name("bookmarks_expander");
    bookmarks_row.set_title(&tr("Bookmarks"));
    bookmarks_row.set_expanded(false);
    let mut bookmarks = config.quran_bookmarks();
    bookmarks.sort_by_key(|bookmark| bookmark.page);
    bookmarks.dedup_by_key(|bookmark| bookmark.page);
    let total = get_total_pages();
    for bookmark in &bookmarks {
        let row = build_bookmark_row(
            bookmark,
            quran_lang,
            total,
            view_stack,
            config.clone(),
            None,
        );
        bookmarks_row.add_row(&row);
    }
    if !bookmarks.is_empty() {
        list_box_rc.borrow().append(&bookmarks_row);
    }

    for surah in &surah_list {
        let row = build_surah_row_for_list(surah, quran_lang, view_stack, config.clone());
        list_box_rc.borrow().append(&row);
    }

    let initial_surah_list = surah_list.clone();
    let surah_list_rc: Rc<RefCell<Vec<SurahListItem>>> = Rc::new(RefCell::new(initial_surah_list));
    let view_stack_for_search = view_stack.clone();
    let quran_lang_for_search = quran_lang.to_string();
    let config_for_search = config.clone();

    let search_list_box = list_box_rc.clone();
    let search_surah_list = surah_list_rc.clone();

    fn build_verse_match_row(
        verse_match: &VerseMatch,
        current_lang: &str,
        view_stack: &adw::ViewStack,
        config: AppConfig,
    ) -> adw::ActionRow {
        let row = adw::ActionRow::new();
        row.set_activatable(true);

        let badge = gtk::Label::new(None);
        badge.set_markup(&format!(
            "<b>{}:{}</b>",
            verse_match.surah_id, verse_match.verse_id
        ));
        badge.set_xalign(0.5);
        badge.set_width_request(48);
        badge.set_height_request(36);

        let surah_title = if !verse_match.surah_translation.is_empty() {
            format!(
                "{} - {}",
                verse_match.surah_translation, verse_match.surah_name
            )
        } else {
            verse_match.surah_name.clone()
        };

        let display_text = if !verse_match.translation_text.is_empty() {
            let preview: String = verse_match.translation_text.chars().take(80).collect();
            let extra = if verse_match.translation_text.len() > 80 {
                "..."
            } else {
                ""
            };
            format!("{} - {}{}", surah_title, preview, extra)
        } else {
            surah_title
        };

        let title: &str = if verse_match.surah_translation.is_empty() {
            &verse_match.surah_name
        } else {
            &verse_match.surah_translation
        };
        row.set_title(title);
        row.set_subtitle(&display_text);

        row.add_prefix(&badge);

        let lang_owned = current_lang.to_string();
        let view_stack_clone = view_stack.clone();
        let surah_num_owned = verse_match.surah_id;
        let verse_num_owned = verse_match.verse_id;
        row.connect_activated(move |_| {
            let page_name = format!("surah_{}", surah_num_owned);
            if let Some(old) = view_stack_clone.child_by_name(&page_name) {
                view_stack_clone.remove(&old);
            }
            let surah_view = create_surah_view(
                surah_num_owned,
                &lang_owned,
                &view_stack_clone,
                None,
                Some(verse_num_owned),
                Some(verse_num_owned),
                config.clone(),
            );
            surah_view.set_vexpand(true);
            view_stack_clone.add_named(&surah_view, Some(&page_name));
            view_stack_clone.set_visible_child_name(&page_name);
        });

        row
    }

    search_entry.connect_changed(move |entry| {
        let query = gtk::prelude::EditableExt::text(entry).trim().to_string();
        let search_results = search_list_box.borrow_mut();
        while let Some(child) = search_results.first_child() {
            search_results.remove(&child);
        }

        if query.is_empty() {
            for surah in search_surah_list.borrow().iter() {
                let row = build_surah_row_for_list(
                    surah,
                    &quran_lang_for_search,
                    &view_stack_for_search,
                    config_for_search.clone(),
                );
                search_results.append(&row);
            }
        } else {
            let search_lang_owned = crate::i18n::supported_language_code(&quran_lang_for_search);
            let search_lang = search_lang_owned.as_str();
            let verse_matches = search_quran(&query, search_lang);

            let is_arabic_query = is_arabic_query(&query);

            let query_lower = if is_arabic_query {
                normalize_arabic(&query)
            } else {
                query.to_lowercase()
            };

            for verse_match in verse_matches.iter().take(50) {
                let row = build_verse_match_row(
                    verse_match,
                    search_lang,
                    &view_stack_for_search,
                    config_for_search.clone(),
                );
                search_results.append(&row);
            }

            for surah in search_surah_list.borrow().iter() {
                let name_lower = if is_arabic_query {
                    normalize_arabic(&surah.name)
                } else {
                    surah.name.to_lowercase()
                };
                let matches = name_lower.contains(&query_lower)
                    || surah.transliteration.to_lowercase().contains(&query_lower)
                    || surah.translation.to_lowercase().contains(&query_lower)
                    || surah.id.to_string().contains(&query);
                if matches {
                    let row = build_surah_row_for_list(
                        surah,
                        &quran_lang_for_search,
                        &view_stack_for_search,
                        config_for_search.clone(),
                    );
                    search_results.append(&row);
                }
            }
        }
    });

    let reader_page = quran_view_stack.add_named(&container, Some("reader"));
    reader_page.set_title(Some(&tr("Reader")));
    reader_page.set_icon_name(Some("document-open-symbolic"));

    let planner_widget =
        crate::quran_planner_ui::create_planner_page(view_stack, config.clone(), current_lang);
    let planner_page = quran_view_stack.add_named(&planner_widget, Some("planner"));
    planner_page.set_title(Some(&tr("Plans")));
    planner_page.set_icon_name(Some("bookmarks-symbolic"));

    switcher.set_stack(Some(&quran_view_stack));
    switcher.set_margin_top(10);
    switcher.set_margin_bottom(10);
    switcher.set_margin_start(16);
    switcher.set_margin_end(16);

    top_box.append(&switcher);
    top_box.append(&quran_view_stack);

    top_box.upcast()
}

fn populate_quran_list(
    list_box: &gtk::ListBox,
    quran_lang: &str,
    surah_list: &[SurahListItem],
    view_stack: &adw::ViewStack,
    config: AppConfig,
) {
    let bookmarks_row = adw::ExpanderRow::new();
    bookmarks_row.set_widget_name("bookmarks_expander");
    bookmarks_row.set_title(&tr("Bookmarks"));
    bookmarks_row.set_expanded(false);
    let mut bookmarks = config.quran_bookmarks();
    bookmarks.sort_by_key(|bookmark| bookmark.page);
    bookmarks.dedup_by_key(|bookmark| bookmark.page);
    let total = get_total_pages();
    for bookmark in &bookmarks {
        let row = build_bookmark_row(
            bookmark,
            quran_lang,
            total,
            view_stack,
            config.clone(),
            None,
        );
        bookmarks_row.add_row(&row);
    }
    if !bookmarks.is_empty() {
        list_box.append(&bookmarks_row);
    }

    for surah in surah_list {
        let row = build_surah_row_for_list(surah, quran_lang, view_stack, config.clone());
        list_box.append(&row);
    }
}

fn build_bookmark_row(
    bookmark: &QuranBookmark,
    lang: &str,
    total_pages: u32,
    view_stack: &adw::ViewStack,
    config: AppConfig,
    popover: Option<&gtk::Popover>,
) -> adw::ActionRow {
    let meta = surah_meta(bookmark.surah_num, lang);
    let name = if lang == "ar" || meta.translated.trim().is_empty() {
        meta.arabic
    } else {
        meta.translated
    };
    let row = adw::ActionRow::new();
    row.set_title(&name);
    row.set_subtitle(&page_label_text(bookmark.page, total_pages));
    row.set_activatable(true);
    row.set_selectable(false);
    let view_stack_row = view_stack.clone();
    let lang_row = lang.to_string();
    let surah_row = bookmark.surah_num;
    let verse_row = bookmark.verse;
    let config_bm = config.clone();
    let popover_opt = popover.cloned();
    row.connect_activated(move |_| {
        let page_name = format!("surah_{}", surah_row);
        if let Some(old) = view_stack_row.child_by_name(&page_name) {
            view_stack_row.remove(&old);
        }
        let surah_view = create_surah_view(
            surah_row,
            &lang_row,
            &view_stack_row,
            None,
            Some(verse_row),
            None,
            config_bm.clone(),
        );
        surah_view.set_vexpand(true);
        view_stack_row.add_named(&surah_view, Some(&page_name));
        view_stack_row.set_visible_child_name(&page_name);
        if let Some(ref popover_widget) = popover_opt {
            popover_widget.popdown();
        }
    });
    row
}

fn build_surah_row_for_list(
    surah: &SurahListItem,
    current_lang: &str,
    view_stack: &adw::ViewStack,
    config: AppConfig,
) -> adw::ActionRow {
    let row = adw::ActionRow::new();
    row.set_activatable(true);

    let badge = gtk::Label::new(None);
    badge.set_markup(&format!("<b>{}</b>", surah.id));
    badge.set_xalign(0.5);
    badge.set_width_request(36);
    badge.set_height_request(36);

    let title_str = if !surah.name.is_empty() && current_lang == "ar" {
        surah.name.clone()
    } else if !surah.transliteration.is_empty() && !surah.translation.is_empty() {
        format!("{} - {}", surah.transliteration, surah.translation)
    } else if !surah.transliteration.is_empty() {
        surah.transliteration.clone()
    } else if !surah.name.is_empty() {
        surah.name.clone()
    } else {
        format!("Surah {}", surah.id)
    };
    row.set_title(&title_str);

    let subtitle = if surah.surah_type == "meccan" {
        format!("{} • {} {}", tr("Meccan"), surah.total_verses, tr("Verses"))
    } else {
        format!(
            "{} • {} {}",
            tr("Medinan"),
            surah.total_verses,
            tr("Verses")
        )
    };
    row.set_subtitle(&subtitle);

    row.add_prefix(&badge);

    let surah_num = surah.id;
    let view_stack_clone = view_stack.clone();
    let lang_clone = current_lang.to_string();
    row.connect_activated(move |_| {
        let page_name = format!("surah_{}", surah_num);
        if view_stack_clone.child_by_name(&page_name).is_none() {
            let surah_view = create_surah_view(
                surah_num,
                &lang_clone,
                &view_stack_clone,
                None,
                None,
                None,
                config.clone(),
            );
            surah_view.set_vexpand(true);
            view_stack_clone.add_named(&surah_view, Some(&page_name));
        }
        view_stack_clone.set_visible_child_name(&page_name);
    });

    row
}

pub fn refresh_quran_ui(view_stack: &adw::ViewStack, lang: &str, config: AppConfig) {
    let visible = view_stack.visible_child_name().map(|name| name.to_string());
    let was_quran_related = visible
        .as_deref()
        .is_some_and(|name| name == "quran" || name.starts_with("surah_"));

    let quran_lang_owned = crate::i18n::supported_language_code(lang);
    let quran_lang = quran_lang_owned.as_str();

    if let Some(quran_child) = view_stack.child_by_name("quran") {
        let list_box = find_widget_by_name(&quran_child, "surah_list_box");
        let search_entry = find_widget_by_name(&quran_child, "quran_search");

        if let Some(widget) = search_entry
            && let Some(entry) = widget.downcast_ref::<gtk::SearchEntry>()
        {
            entry.set_placeholder_text(Some(&tr("Search surahs")));
        }

        if let Some(widget) = list_box
            && let Some(list) = widget.downcast_ref::<gtk::ListBox>()
        {
            while let Some(child) = list.first_child() {
                list.remove(&child);
            }
            let surah_list = get_surah_list(quran_lang);
            populate_quran_list(list, quran_lang, &surah_list, view_stack, config.clone());
        }
    } else {
        let quran_page = create_quran_page(lang, view_stack, config.clone());
        view_stack.add_named(&quran_page, Some("quran"));
    }

    {
        let pages = view_stack.pages();
        let mut to_remove: Vec<gtk::Widget> = Vec::new();
        for index in 0..pages.n_items() {
            if let Some(page_obj) = pages.item(index)
                && let Ok(page) = page_obj.downcast::<adw::ViewStackPage>()
                && let Some(name) = page.name()
                && name.starts_with("surah_")
            {
                to_remove.push(page.child());
            }
        }
        for child in to_remove {
            view_stack.remove(&child);
        }
    }

    if let Some(name) = &visible
        && name.starts_with("surah_")
        && let Some(rest) = name.strip_prefix("surah_")
        && let Ok(surah_num) = rest.parse::<u32>()
    {
        let page = SURAH_READING_POSITIONS.with(|pos| pos.borrow().get(&surah_num).copied());
        let verse = page
            .and_then(get_page_verses)
            .and_then(|page_verses| {
                page_verses
                    .into_iter()
                    .find(|page_verse| page_verse.surah_num == surah_num)
                    .map(|page_verse| page_verse.verse)
            })
            .unwrap_or(1);
        let surah_view = create_surah_view(
            surah_num,
            lang,
            view_stack,
            None,
            Some(verse),
            None,
            config.clone(),
        );
        surah_view.set_vexpand(true);
        view_stack.add_named(&surah_view, Some(name));
    }

    if was_quran_related && let Some(name) = visible {
        if view_stack.child_by_name(&name).is_some() {
            view_stack.set_visible_child_name(&name);
        } else {
            view_stack.set_visible_child_name("quran");
        }
    }
}

pub fn get_surah_for_page(page: u32) -> u32 {
    if let Some(verses) = get_page_verses(page) {
        if let Some(first) = verses.first() {
            return first.surah_num;
        }
    }
    1
}

static REQUESTED_QURAN_PAGE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

pub fn request_quran_page_navigation(target_page: u32) {
    log::info!("Quran navigation requested: page={target_page}");
    REQUESTED_QURAN_PAGE.store(target_page, std::sync::atomic::Ordering::SeqCst);

    if let Some(app) =
        gtk::gio::Application::default().and_then(|a| a.downcast::<adw::Application>().ok())
    {
        app.activate_action("open-quran-page", Some(&target_page.to_variant()));
        app.activate();
    }
}

pub fn take_requested_quran_page() -> Option<u32> {
    let val = REQUESTED_QURAN_PAGE.swap(0, std::sync::atomic::Ordering::SeqCst);
    if (1..=604).contains(&val) {
        Some(val)
    } else {
        None
    }
}

pub fn get_surah_display_list(lang: &str) -> Vec<String> {
    if let Some(info) = get_surah_info() {
        let is_ar = crate::i18n::is_arabic(lang);
        info.into_iter()
            .map(|s| {
                if is_ar {
                    format!("{}. {}", s.id, s.name)
                } else {
                    format!("{}. {}", s.id, s.transliteration)
                }
            })
            .collect()
    } else {
        (1..=114).map(|i| format!("Surah {}", i)).collect()
    }
}

pub fn open_surah_at_page(
    view_stack: &adw::ViewStack,
    lang: &str,
    config: AppConfig,
    target_page: u32,
) {
    let surah_num = get_surah_for_page(target_page);
    config.set_quran_last_surah_num(Some(surah_num));
    config.set_quran_last_page(Some(target_page));
    config.save();
    open_last_read_or_list(view_stack, lang, config);
}

pub fn open_last_read_or_list(view_stack: &adw::ViewStack, lang: &str, config: AppConfig) {
    if let (Some(surah_num), Some(page)) = (config.quran_last_surah_num(), config.quran_last_page())
    {
        SURAH_READING_POSITIONS.with(|pos| pos.borrow_mut().insert(surah_num, page));
        let page_name = format!("surah_{}", surah_num);
        if let Some(old) = view_stack.child_by_name(&page_name) {
            view_stack.remove(&old);
        }
        let surah_view = create_surah_view(
            surah_num,
            lang,
            view_stack,
            None,
            None,
            None,
            config.clone(),
        );
        surah_view.set_vexpand(true);
        view_stack.add_named(&surah_view, Some(&page_name));
        view_stack.set_visible_child_name(&page_name);
        return;
    }
    view_stack.set_visible_child_name("quran");
}

fn selected_text_for_label(label: &gtk::Label) -> String {
    let text = label.text().to_string();
    if let Some((start, end)) = label.selection_bounds() {
        let start_idx = start.max(0) as usize;
        let end_idx = end.max(0) as usize;
        if start_idx == end_idx {
            return text;
        }
        let (min_idx, max_idx) = if start_idx < end_idx {
            (start_idx, end_idx)
        } else {
            (end_idx, start_idx)
        };
        let mut out = String::new();
        for (idx, ch) in text.chars().enumerate() {
            if idx >= min_idx && idx < max_idx {
                out.push(ch);
            }
        }
        if !out.is_empty() {
            return out;
        }
    }
    text
}

fn attach_readonly_context_menu(label: &gtk::Label) {
    attach_context_menu_impl(label, None);
}

fn attach_arabic_context_menu(label: &gtk::Label, rec_state: Rc<RefCell<RecitationState>>) {
    let copy_fn: Rc<dyn Fn() -> String> = Rc::new(move || {
        rec_state
            .borrow()
            .selected_verse
            .get()
            .and_then(|(surah_num, verse)| get_arabic_text(surah_num, verse))
            .unwrap_or_default()
    });
    attach_context_menu_impl(label, Some(copy_fn));
}

fn attach_context_menu_impl(label: &gtk::Label, copy_fn: Option<Rc<dyn Fn() -> String>>) {
    label.set_can_focus(false);
    let popover = gtk::Popover::new();
    popover.set_has_arrow(false);

    let box_menu = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let copy_btn = gtk::Button::with_label(&tr("Copy"));
    copy_btn.add_css_class("flat");
    box_menu.append(&copy_btn);
    popover.set_child(Some(&box_menu));

    if let Some(copy_fn) = copy_fn {
        let copy_fn_c = copy_fn.clone();
        let popover_for_copy = popover.clone();
        copy_btn.connect_clicked(move |_| {
            let text = copy_fn_c();
            if !text.is_empty()
                && let Some(display) = gtk::gdk::Display::default()
            {
                display.clipboard().set_text(&text);
            }
            popover_for_copy.popdown();
        });
    } else {
        let label_for_copy = label.clone();
        let popover_for_copy = popover.clone();
        copy_btn.connect_clicked(move |_| {
            if let Some(display) = gtk::gdk::Display::default() {
                display
                    .clipboard()
                    .set_text(&selected_text_for_label(&label_for_copy));
            }
            popover_for_copy.popdown();
        });
    }

    let popover_for_click = popover.clone();
    let label_for_click = label.clone();
    let gesture = gtk::GestureClick::builder().button(3).build();
    gesture.set_propagation_phase(gtk::PropagationPhase::Capture);
    gesture.connect_pressed(move |gesture_click, _, x, y| {
        let parent = label_for_click
            .root()
            .and_then(|root| root.downcast::<gtk::Window>().ok())
            .map(|window| window.upcast::<gtk::Widget>())
            .unwrap_or_else(|| label_for_click.clone().upcast());

        if popover_for_click.parent().is_none() {
            popover_for_click.set_parent(&parent);
        }

        let (pointer_x, pointer_y) = label_for_click
            .translate_coordinates(&parent, x, y)
            .unwrap_or((x, y));

        popover_for_click.set_pointing_to(Some(&gtk::gdk::Rectangle::new(
            pointer_x as i32,
            pointer_y as i32,
            1,
            1,
        )));
        popover_for_click.popup();
        gesture_click.set_state(gtk::EventSequenceState::Claimed);
    });
    label.add_controller(gesture);
}

fn find_widget_by_name(root: &gtk::Widget, name: &str) -> Option<gtk::Widget> {
    if root.widget_name() == name {
        return Some(root.clone());
    }
    let mut child = root.first_child();
    while let Some(widget) = child {
        if let Some(found) = find_widget_by_name(&widget, name) {
            return Some(found);
        }
        child = widget.next_sibling();
    }
    None
}

fn attach_unified_verse_menu(
    verse_box: &gtk::Box,
    rec_state: Rc<RefCell<RecitationState>>,
    lang: &str,
) {
    let popover = gtk::Popover::new();
    popover.set_has_arrow(false);

    let box_menu = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let copy_btn = gtk::Button::with_label(&tr("Copy"));
    copy_btn.add_css_class("flat");
    box_menu.append(&copy_btn);
    popover.set_child(Some(&box_menu));

    let lang_c = lang.to_string();
    let rec_state_c = rec_state.clone();
    let popover_c = popover.clone();
    copy_btn.connect_clicked(move |_| {
        let text = rec_state_c
            .borrow()
            .selected_verse
            .get()
            .map(|(surah_num, verse)| {
                let arabic = get_arabic_text(surah_num, verse).unwrap_or_default();
                let translation = get_verse(surah_num, verse, &lang_c)
                    .map(|verse_data| verse_data.translation)
                    .unwrap_or_default();
                if translation.is_empty() {
                    arabic
                } else {
                    format!("{}\n\n{}", arabic, translation)
                }
            })
            .unwrap_or_default();
        if !text.is_empty()
            && let Some(display) = gtk::gdk::Display::default()
        {
            display.clipboard().set_text(&text);
        }
        popover_c.popdown();
    });

    let popover_for_click = popover.clone();
    let widget_for_click = verse_box.clone().upcast::<gtk::Widget>();
    let gesture = gtk::GestureClick::builder()
        .button(3)
        .propagation_phase(gtk::PropagationPhase::Capture)
        .build();
    gesture.connect_pressed(move |gesture_click, _, x, y| {
        let parent = widget_for_click
            .root()
            .and_then(|root| root.downcast::<gtk::Window>().ok())
            .map(|window| window.upcast::<gtk::Widget>())
            .unwrap_or_else(|| widget_for_click.clone());

        if popover_for_click.parent().is_none() {
            popover_for_click.set_parent(&parent);
        }

        let (pointer_x, pointer_y) = widget_for_click
            .translate_coordinates(&parent, x, y)
            .unwrap_or((x, y));

        popover_for_click.set_pointing_to(Some(&gtk::gdk::Rectangle::new(
            pointer_x as i32,
            pointer_y as i32,
            1,
            1,
        )));
        popover_for_click.popup();
        gesture_click.set_state(gtk::EventSequenceState::Claimed);
    });
    verse_box.add_controller(gesture);
}

fn to_arabic_indic(num: u32) -> String {
    num.to_string()
        .chars()
        .map(|digit| match digit {
            '0' => '٠',
            '1' => '١',
            '2' => '٢',
            '3' => '٣',
            '4' => '٤',
            '5' => '٥',
            '6' => '٦',
            '7' => '٧',
            '8' => '٨',
            '9' => '٩',
            _ => digit,
        })
        .collect()
}

const BISMILLAH: &str = "بِسْمِ ٱللَّهِ ٱلرَّحْمَـٰنِ ٱلرَّحِيمِ";

#[derive(Clone, Debug)]
struct SurahMeta {
    arabic: String,
    transliteration: String,
    translated: String,
    surah_type: String,
}

fn surah_meta(surah_num: u32, lang: &str) -> SurahMeta {
    let mut meta = SurahMeta {
        arabic: String::new(),
        transliteration: String::new(),
        translated: String::new(),
        surah_type: String::new(),
    };

    if let Some(info) = get_surah_info()
        && let Some(surah) = info.iter().find(|surah| surah.id == surah_num)
    {
        meta.arabic = surah.name.clone();
        meta.transliteration = surah.transliteration.clone();
        meta.surah_type = surah.surah_type.clone();
    }

    if lang != "ar"
        && let Some(surah) = get_surah(surah_num, lang)
    {
        meta.translated = surah.translation.clone();
        if meta.arabic.is_empty() {
            meta.arabic = surah.name.clone();
        }
    }

    meta
}

fn page_label_text(global_page: u32, total_pages: u32) -> String {
    format!("{} {} / {}", tr("page"), global_page, total_pages)
}

pub(crate) fn surah_total_verses(surah_num: u32) -> Option<u32> {
    get_surah_info().and_then(|info| {
        info.iter()
            .find(|surah| surah.id == surah_num)
            .map(|surah| surah.total_verses)
    })
}

fn marker_id_for_page(
    page: u32,
    marker_index: &MarkerIndexU32,
    marker_list: &[MarkerPos],
) -> Option<u32> {
    let page_index = get_page_index()?;
    let verses = get_page_verses(page)?;

    let mut best_in_page: Option<u32> = None;
    for page_verse in &verses {
        if let Some(id) = marker_index.get(&(page_verse.surah_num, page_verse.verse)) {
            best_in_page = Some(best_in_page.map(|prev| prev.max(*id)).unwrap_or(*id));
        }
    }
    if best_in_page.is_some() {
        return best_in_page;
    }

    let start = page_index.page_starts.get(&page)?;
    let pos = (start.surah_num, start.verse);
    let mut best: Option<u32> = None;
    for marker in marker_list {
        if (marker.surah_num, marker.verse) <= pos {
            best = Some(marker.id);
        } else {
            break;
        }
    }
    best
}

#[derive(Clone, Copy, Debug)]
struct PageMarkers {
    juz: Option<u32>,
    hizb: Option<u32>,
    quarter: Option<u32>,
}

fn page_markers_for_page(page: u32) -> PageMarkers {
    let juz_index = get_juz_index();
    let hizb_quarter_index = get_hizb_quarter_index();
    let juz_list = get_juz_list();
    let hizb_list = get_hizb_quarter_list();

    let juz = marker_id_for_page(page, &juz_index, &juz_list);
    let (hizb, quarter) =
        if let Some(qid) = marker_id_for_page(page, &hizb_quarter_index, &hizb_list) {
            let hizb = ((qid - 1) / 4) + 1;
            let quarter = ((qid - 1) % 4) + 1;
            (Some(hizb), Some(quarter))
        } else {
            (None, None)
        };

    PageMarkers { juz, hizb, quarter }
}

// Family-only scope: `.quran-arabic` would also force the verse-text font size.
fn apply_quran_meta_font(label: &gtk::Label, lang: &str) {
    if crate::i18n::is_arabic(lang) {
        label.add_css_class("quran-arabic-caption");
    }
}

fn update_marker_frame(frame: &gtk::Box, page: u32, lang: &str) {
    while let Some(child) = frame.first_child() {
        frame.remove(&child);
    }

    let markers = page_markers_for_page(page);
    if markers.juz.is_none() && markers.hizb.is_none() && markers.quarter.is_none() {
        frame.set_visible(false);
        return;
    }

    let mut parts: Vec<String> = Vec::new();
    if let Some(juz) = markers.juz {
        let label = if lang == "ar" {
            to_arabic_indic(juz)
        } else {
            juz.to_string()
        };
        parts.push(format!("{} {}", tr("Juz"), label));
    }
    if let Some(hizb) = markers.hizb {
        let label = if lang == "ar" {
            to_arabic_indic(hizb)
        } else {
            hizb.to_string()
        };
        parts.push(format!("{} {}", tr("Hizb"), label));
    }
    if let Some(quarter) = markers.quarter {
        let label = if lang == "ar" {
            to_arabic_indic(quarter)
        } else {
            quarter.to_string()
        };
        parts.push(format!("{} {}", tr("Quarter"), label));
    }

    for (idx, text) in parts.iter().enumerate() {
        if idx > 0 {
            let sep = gtk::Label::new(Some("•"));
            sep.add_css_class("dim-label");
            frame.append(&sep);
        }
        let meta_label = gtk::Label::new(Some(text));
        meta_label.set_wrap(true);
        meta_label.set_xalign(0.5);
        meta_label.add_css_class("dim-label");
        apply_quran_meta_font(&meta_label, lang);
        frame.append(&meta_label);
    }
    frame.set_visible(true);
}

#[derive(Clone)]
struct RecitationState {
    selected_verse: Cell<Option<(u32, u32)>>,
    playing: Cell<bool>,
    stop_boundary: Cell<Option<(u32, u32)>>,
    current_playing_surah_num: Cell<u32>,
}

// Keeps the audio-event callbacks of one surah view alive until the view is
// unrealized, at which point they are dropped so the audio registry's weak
// references expire and stop firing into a torn-down view. The fields are
// write-only on purpose: their sole job is to own the callbacks.
struct RecitationSubscriptions {
    _recitation_state: Option<Rc<dyn Fn(bool)>>,
    _verse_finished: Option<Rc<dyn Fn(u32, u32)>>,
}

#[derive(Clone, Debug)]
struct VerseBoundary {
    surah_num: u32,
    verse: u32,
    byte_start: usize,
    byte_end: usize,
}

fn get_verse_display_len(content: &str, verse: u32) -> usize {
    content.len() + 7 + (verse.checked_ilog10().unwrap_or(0) as usize + 1) * 2
}

fn compute_verse_boundaries(page: u32) -> Vec<VerseBoundary> {
    let mut bounds = Vec::new();
    let mut offset: usize = 0;
    if let Some(verses_data) = get_page_verses(page) {
        for page_verse in verses_data.iter() {
            let len = get_verse_display_len(&page_verse.content, page_verse.verse);
            bounds.push(VerseBoundary {
                surah_num: page_verse.surah_num,
                verse: page_verse.verse,
                byte_start: offset,
                byte_end: offset + len,
            });
            offset += len + 1;
        }
    }
    bounds
}

fn find_verse_at_offset(offset: usize, boundaries: &[VerseBoundary]) -> Option<(u32, u32)> {
    for boundary in boundaries {
        if offset >= boundary.byte_start && offset < boundary.byte_end {
            return Some((boundary.surah_num, boundary.verse));
        }
    }
    None
}

fn attach_arabic_verse_clicks(
    content: &gtk::Box,
    page: u32,
    rec_state: Rc<RefCell<RecitationState>>,
    boundaries_cache: Rc<RefCell<HashMap<u32, Vec<VerseBoundary>>>>,
    rebuild_fn: RebuildFn,
    label_ranges: Rc<RefCell<Vec<(usize, usize)>>>,
    sync_nav: Rc<dyn Fn()>,
) {
    if !boundaries_cache.borrow().contains_key(&page) {
        let boundary = compute_verse_boundaries(page);
        boundaries_cache.borrow_mut().insert(page, boundary);
    }

    let bounds_data = boundaries_cache.borrow().get(&page).cloned();
    let ranges_data = label_ranges.borrow().clone();
    let mut mushaf_idx: usize = 0;

    let mut child = content.first_child();
    while let Some(widget) = child {
        if let Some(label) = widget.downcast_ref::<gtk::Label>()
            && label.has_css_class("quran-verse-block")
        {
            let (first_verse_idx, _) = ranges_data[mushaf_idx];
            let base = bounds_data.as_ref().unwrap()[first_verse_idx].byte_start;

            let click = gtk::GestureClick::builder().button(1).build();
            let rec_state_c = rec_state.clone();
            let bounds_c = bounds_data.clone();
            let rebuild_fn_c = rebuild_fn.clone();
            let label_c = label.clone();
            let sync_nav_c = sync_nav.clone();
            click.connect_pressed(move |_, _, x, y| {
                let layout = label_c.layout();
                let (within_text, byte_offset, _) = layout.xy_to_index(
                    (x * f64::from(gtk::pango::SCALE)) as i32,
                    (y * f64::from(gtk::pango::SCALE)) as i32,
                );
                if within_text
                    && let Some(ref bounds) = bounds_c
                    && let Some(verse) = find_verse_at_offset(byte_offset as usize + base, bounds)
                {
                    rec_state_c.borrow().selected_verse.set(Some(verse));
                    if let Some(ref rebuild) = *rebuild_fn_c.borrow() {
                        rebuild();
                    }
                    sync_nav_c();
                }
            });
            label.add_controller(click);
            mushaf_idx += 1;
        }
        child = widget.next_sibling();
    }
}

fn attach_translation_verse_clicks(
    content: &gtk::Box,
    rec_state: Rc<RefCell<RecitationState>>,
    rebuild_fn: RebuildFn,
    sync_nav: Rc<dyn Fn()>,
) {
    let mut child = content.first_child();
    while let Some(widget) = child {
        if let Some(box_widget) = widget.downcast_ref::<gtk::Box>()
            && box_widget.has_css_class("quran-verse-box")
        {
            // SAFETY: these quarks are only ever written below with `u32`
            // values and never replaced, so the pointer stays valid and typed.
            let surah_num = unsafe {
                *box_widget
                    .qdata::<u32>(glib::Quark::from_str("khushu-verse-surah"))
                    .unwrap()
                    .as_ptr()
            };
            let verse = unsafe {
                *box_widget
                    .qdata::<u32>(glib::Quark::from_str("khushu-verse-verse"))
                    .unwrap()
                    .as_ptr()
            };
            let click = gtk::GestureClick::builder()
                .button(1)
                .propagation_phase(gtk::PropagationPhase::Capture)
                .build();
            let rec_state_c = rec_state.clone();
            let rebuild_fn_c = rebuild_fn.clone();
            let sync_nav_c = sync_nav.clone();
            click.connect_pressed(move |_, _, _, _| {
                rec_state_c
                    .borrow()
                    .selected_verse
                    .set(Some((surah_num, verse)));
                if let Some(ref rebuild) = *rebuild_fn_c.borrow() {
                    rebuild();
                }
                sync_nav_c();
            });
            widget.add_controller(click);
        }
        child = widget.next_sibling();
    }
}

fn compute_stop_boundary(surah_num: u32, verse: u32, stop: StopCondition) -> Option<(u32, u32)> {
    match stop {
        StopCondition::None => None,
        StopCondition::Ayah => Some((surah_num, verse)),
        StopCondition::Page => {
            let page = get_verse_page(surah_num, verse)?;
            let verses = get_page_verses(page)?;
            verses
                .last()
                .map(|last_verse| (last_verse.surah_num, last_verse.verse))
        }
        StopCondition::Juz => {
            let juz_index = get_juz_index();
            let current_juz = juz_index.get(&(surah_num, verse)).copied()?;
            let mut last = (surah_num, verse);
            for next_surah_num in surah_num..=114 {
                let total = surah_total_verses(next_surah_num).unwrap_or(u32::MAX);
                let start = if next_surah_num == surah_num {
                    verse + 1
                } else {
                    1
                };
                for verse_num in start..=total {
                    match juz_index.get(&(next_surah_num, verse_num)) {
                        Some(&j) if j == current_juz => last = (next_surah_num, verse_num),
                        _ => return Some(last),
                    }
                }
            }
            Some(last)
        }
        StopCondition::Surah => Some((surah_num, surah_total_verses(surah_num)?)),
    }
}

fn next_verse_on_page_or_next(surah_num: u32, verse: u32) -> Option<(u32, u32)> {
    let page = get_verse_page(surah_num, verse)?;
    let verses = get_page_verses(page)?;
    let pos = verses
        .iter()
        .position(|verse_data| verse_data.surah_num == surah_num && verse_data.verse == verse)?;
    if let Some(next) = verses.get(pos + 1) {
        return Some((next.surah_num, next.verse));
    }
    let next_page = page + 1;
    let next_verses = get_page_verses(next_page)?;
    next_verses
        .first()
        .map(|first_verse| (first_verse.surah_num, first_verse.verse))
}

fn build_recitation_toolbar(
    rec_state: Rc<RefCell<RecitationState>>,
    config: &AppConfig,
    play_fn: PlayFn,
    surah_num: u32,
) -> (gtk::CenterBox, gtk::Button, gtk::Button, gtk::Button) {
    let toolbar = gtk::CenterBox::new();
    toolbar.set_margin_top(4);
    toolbar.set_margin_bottom(4);
    toolbar.set_margin_start(8);
    toolbar.set_margin_end(8);
    toolbar.add_css_class("toolbar");

    let prev_btn = gtk::Button::new();
    prev_btn.set_icon_name("media-skip-backward-symbolic");
    prev_btn.add_css_class("flat");

    let play_btn = gtk::Button::new();
    play_btn.set_icon_name("media-playback-start-symbolic");
    play_btn.add_css_class("flat");

    let next_btn = gtk::Button::new();
    next_btn.set_icon_name("media-skip-forward-symbolic");
    next_btn.add_css_class("flat");

    let sync_nav = {
        let rec_state_c = rec_state.clone();
        let prev_btn_c = prev_btn.clone();
        let next_btn_c = next_btn.clone();
        move || {
            let total = surah_total_verses(surah_num).unwrap_or(u32::MAX);
            let prev_enabled = rec_state_c
                .borrow()
                .selected_verse
                .get()
                .is_some_and(|(_, verse_num)| verse_num > 1);
            let next_enabled = rec_state_c
                .borrow()
                .selected_verse
                .get()
                .is_some_and(|(_, verse_num)| verse_num < total);
            prev_btn_c.set_sensitive(prev_enabled);
            next_btn_c.set_sensitive(next_enabled);
        }
    };
    sync_nav();

    let media_box = gtk::Box::new(gtk::Orientation::Horizontal, 2);
    media_box.append(&prev_btn);
    media_box.append(&play_btn);
    media_box.append(&next_btn);

    let current_slug = config.reciter_slug();
    let current_pos = crate::reciter_ui::RECITERS
        .iter()
        .position(|reciter| reciter.slug == current_slug);
    let reciter_box = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    let reciter_label = gtk::Label::new(Some(
        current_pos
            .and_then(|pos| crate::reciter_ui::RECITERS.get(pos))
            .map(|reciter| tr(reciter.display))
            .as_deref()
            .unwrap_or(&tr("Reciter")),
    ));
    reciter_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    reciter_box.append(&reciter_label);
    let reciter_btn = gtk::Button::new();
    reciter_btn.set_child(Some(&reciter_box));
    let dialog_btn = reciter_btn.clone();
    let dialog_cfg = config.clone();
    reciter_btn.connect_clicked(move |_| {
        crate::reciter_ui::open_reciter_dialog(
            &dialog_btn,
            dialog_cfg.clone(),
            reciter_label.clone(),
        );
    });

    let stop_items = [
        tr("None"),
        tr("End of Ayah"),
        tr("End of Page"),
        tr("End of Juz"),
        tr("End of Surah"),
    ];
    let stop_refs: Vec<&str> = stop_items.iter().map(|item| item.as_str()).collect();
    let stop_model = gtk::StringList::new(&stop_refs);
    let stop_dropdown = gtk::DropDown::new(Some(stop_model), Option::<&gtk::Expression>::None);
    stop_dropdown.add_css_class("flat");
    stop_dropdown.set_selected(match config.stop_condition() {
        StopCondition::None => 0,
        StopCondition::Ayah => 1,
        StopCondition::Page => 2,
        StopCondition::Juz => 3,
        StopCondition::Surah => 4,
    });
    let stop_cfg = config.clone();
    stop_dropdown.connect_selected_notify(move |dropdown| {
        let cond = match dropdown.selected() {
            0 => StopCondition::None,
            1 => StopCondition::Ayah,
            2 => StopCondition::Page,
            3 => StopCondition::Juz,
            _ => StopCondition::Surah,
        };
        stop_cfg.set_stop_condition(cond);
        stop_cfg.save();
    });

    let left_box = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    left_box.append(&media_box);

    let right_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    right_box.append(&stop_dropdown);

    toolbar.set_start_widget(Some(&left_box));
    toolbar.set_center_widget(Some(&reciter_btn));
    toolbar.set_end_widget(Some(&right_box));

    let play_rec_state = rec_state.clone();
    let play_fn_c = play_fn.clone();
    let play_btn_return = play_btn.clone();
    let play_surah_num = surah_num;
    play_btn.connect_clicked(move |btn| {
        let was_playing = play_rec_state.borrow().playing.get();
        let (selected_surah_num, verse) = play_rec_state
            .borrow()
            .selected_verse
            .get()
            .unwrap_or((play_surah_num, 1));
        if was_playing {
            play_rec_state.borrow().playing.set(false);
            crate::audio::stop();
            btn.set_icon_name("media-playback-start-symbolic");
        } else {
            if let Some(ref play) = *play_fn_c.borrow() {
                play(selected_surah_num, verse);
            }
            btn.set_icon_name("media-playback-pause-symbolic");
        }
    });

    let prev_sync = sync_nav.clone();
    let prev_rec_state = rec_state.clone();
    let play_fn_c2 = play_fn.clone();
    let prev_play_btn = play_btn.clone();
    prev_btn.connect_clicked(move |_| {
        let selected = prev_rec_state
            .borrow()
            .selected_verse
            .get()
            .filter(|&(_, verse_num)| verse_num > 1);
        if let Some((selected_surah_num, verse)) = selected {
            prev_rec_state
                .borrow()
                .selected_verse
                .set(Some((selected_surah_num, verse - 1)));
            if let Some(ref play) = *play_fn_c2.borrow() {
                play(selected_surah_num, verse - 1);
            }
            prev_play_btn.set_icon_name("media-playback-pause-symbolic");
        }
        prev_sync();
    });

    let next_sync = sync_nav.clone();
    let next_rec_state = rec_state.clone();
    let play_fn_c3 = play_fn;
    let next_play_btn = play_btn.clone();
    next_btn.connect_clicked(move |_| {
        let selected = next_rec_state.borrow().selected_verse.get();
        if let Some((selected_surah_num, verse)) = selected {
            next_rec_state
                .borrow()
                .selected_verse
                .set(Some((selected_surah_num, verse + 1)));
            if let Some(ref play) = *play_fn_c3.borrow() {
                play(selected_surah_num, verse + 1);
            }
            next_play_btn.set_icon_name("media-playback-pause-symbolic");
        }
        next_sync();
    });

    (toolbar, play_btn_return, prev_btn, next_btn)
}

fn is_verse_on_page(surah_num: u32, verse: u32, page: u32) -> bool {
    if let Some(verses) = get_page_verses(page) {
        verses
            .iter()
            .any(|page_verse| page_verse.surah_num == surah_num && page_verse.verse == verse)
    } else {
        false
    }
}

fn find_arabic_label(content: &gtk::Box, label_idx: usize) -> Option<gtk::Label> {
    let mut child = content.first_child();
    let mut found_idx = 0;
    while let Some(widget) = child {
        if let Some(label) = widget.downcast_ref::<gtk::Label>()
            && label.has_css_class("quran-verse-block")
        {
            if found_idx == label_idx {
                return Some(label.clone());
            }
            found_idx += 1;
        }
        child = widget.next_sibling();
    }
    None
}

pub fn create_surah_view(
    surah_num: u32,
    lang: &str,
    view_stack: &adw::ViewStack,
    target_page: Option<u32>,
    scroll_to_verse: Option<u32>,
    highlight_verse: Option<u32>,
    config: AppConfig,
) -> gtk::Widget {
    let container = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let toast_overlay = adw::ToastOverlay::new();

    let quran_lang_owned = crate::i18n::supported_language_code(lang);
    let quran_lang = quran_lang_owned.as_str();

    let meta = surah_meta(surah_num, quran_lang);
    let surah_arabic_name = if meta.arabic.is_empty() {
        format!("Surah {}", surah_num)
    } else {
        meta.arabic.clone()
    };

    let header_box = gtk::Box::new(gtk::Orientation::Vertical, 4);
    header_box.set_margin_top(8);
    header_box.set_margin_bottom(4);
    header_box.set_margin_start(8);
    header_box.set_margin_end(8);

    let header_center = gtk::CenterBox::new();
    header_center.set_hexpand(true);

    let back_btn = gtk::Button::new();
    back_btn.set_icon_name("go-previous-symbolic");
    back_btn.add_css_class("flat");
    back_btn.set_tooltip_text(Some(&tr("Back")));

    let start_btn = gtk::Button::new();
    start_btn.set_icon_name("go-first-symbolic");
    start_btn.add_css_class("flat");
    start_btn.set_tooltip_text(Some(&tr("Start of Surah")));

    let header_start = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    header_start.set_valign(gtk::Align::Center);
    header_start.append(&back_btn);
    header_start.append(&start_btn);
    header_center.set_start_widget(Some(&header_start));

    let title_box = gtk::Box::new(gtk::Orientation::Vertical, 2);
    title_box.set_halign(gtk::Align::Center);
    let header_title: gtk::Label;
    let mut header_extra: Option<gtk::Label> = None;
    if quran_lang == "ar" {
        let surah_title = gtk::Label::new(Some(&surah_arabic_name));
        surah_title.add_css_class("title-2");
        surah_title.add_css_class("quran-arabic");
        header_title = surah_title.clone();

        title_box.append(&surah_title);

        if !meta.surah_type.trim().is_empty() {
            let typ = if meta.surah_type.trim().eq_ignore_ascii_case("meccan") {
                tr("Meccan")
            } else {
                tr("Medinan")
            };
            let type_label = gtk::Label::new(Some(&typ));
            type_label.add_css_class("caption");
            apply_quran_meta_font(&type_label, "ar");
            type_label.set_margin_bottom(6);

            title_box.append(&type_label);
            header_extra = Some(type_label.clone());
        }
    } else {
        let primary_name = if !meta.translated.trim().is_empty() {
            meta.translated.trim().to_string()
        } else if !meta.transliteration.trim().is_empty() {
            meta.transliteration.trim().to_string()
        } else {
            surah_arabic_name.clone()
        };

        let surah_title = gtk::Label::new(Some(&primary_name));
        surah_title.add_css_class("title-2");
        surah_title.add_css_class("quran-translation");
        surah_title.set_ellipsize(gtk::pango::EllipsizeMode::End);
        surah_title.set_max_width_chars(30);
        header_title = surah_title.clone();

        title_box.append(&surah_title);

        let mut sub_parts = Vec::new();
        if !meta.transliteration.trim().is_empty() {
            sub_parts.push(meta.transliteration.trim().to_string());
        }
        if !meta.surah_type.trim().is_empty() {
            let typ = if meta.surah_type.trim().eq_ignore_ascii_case("meccan") {
                tr("Meccan")
            } else {
                tr("Medinan")
            };
            sub_parts.push(typ);
        }

        if !sub_parts.is_empty() {
            let subtitle = gtk::Label::new(Some(&sub_parts.join(" • ")));
            subtitle.add_css_class("caption");
            subtitle.add_css_class("quran-translation");
            subtitle.set_margin_bottom(6);

            title_box.append(&subtitle);
            header_extra = Some(subtitle.clone());
        }
    }
    header_center.set_center_widget(Some(&title_box));

    let bookmark_toggle_btn = gtk::Button::builder()
        .icon_name("user-bookmarks-symbolic")
        .has_frame(false)
        .build();
    bookmark_toggle_btn.set_tooltip_text(Some(&tr("Bookmark")));

    let bookmarks_btn = gtk::Button::new();
    bookmarks_btn.add_css_class("flat");
    bookmarks_btn.set_tooltip_text(Some(&tr("Bookmarks")));
    let bookmarks_btn_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    let bookmarks_icon = gtk::Image::from_icon_name("user-bookmarks-symbolic");
    let dropdown_icon = gtk::Image::from_icon_name("pan-down-symbolic");
    bookmarks_btn_box.append(&bookmarks_icon);
    bookmarks_btn_box.append(&dropdown_icon);
    bookmarks_btn.set_child(Some(&bookmarks_btn_box));

    let typography_btn = gtk::MenuButton::new();
    typography_btn.set_icon_name("preferences-desktop-font-symbolic");
    typography_btn.add_css_class("flat");
    typography_btn.set_tooltip_text(Some(&tr("Typography Options")));

    let typo_popover = gtk::Popover::new();
    let typo_outer = gtk::Box::new(gtk::Orientation::Vertical, 0);
    typo_outer.set_margin_start(4);
    typo_outer.set_margin_end(4);
    typo_outer.set_margin_top(8);
    typo_outer.set_margin_bottom(12);

    let typo_group = adw::PreferencesGroup::builder()
        .title(tr("Reading Display"))
        .build();
    typo_outer.append(&typo_group);

    let cfg_typo = AppConfig::load();

    let arabic_adj =
        gtk::Adjustment::new(cfg_typo.quran_arabic_font_px(), 16.0, 40.0, 1.0, 0.0, 0.0);
    let arabic_spin = adw::SpinRow::builder()
        .title(tr("Arabic Font Size"))
        .subtitle(tr("Size in pixels (16–40)"))
        .adjustment(&arabic_adj)
        .digits(0)
        .build();
    typo_group.add(&arabic_spin);

    let config_for_arabic = config.clone();
    arabic_adj.connect_value_changed(move |adj| {
        config_for_arabic.set_quran_arabic_font_px(adj.value());
        config_for_arabic.save();
        crate::apply_font_css(&config_for_arabic);
    });

    let trans_adj = gtk::Adjustment::new(
        cfg_typo.quran_translation_font_px(),
        10.0,
        28.0,
        1.0,
        0.0,
        0.0,
    );
    let trans_spin = adw::SpinRow::builder()
        .title(tr("Translation Font Size"))
        .subtitle(tr("Size in pixels (10–28)"))
        .adjustment(&trans_adj)
        .digits(0)
        .build();
    typo_group.add(&trans_spin);

    let config_for_trans = config.clone();
    trans_adj.connect_value_changed(move |adj| {
        config_for_trans.set_quran_translation_font_px(adj.value());
        config_for_trans.save();
        crate::apply_font_css(&config_for_trans);
    });

    let lh_adj = gtk::Adjustment::new(cfg_typo.quran_line_height(), 1.0, 3.0, 0.1, 0.0, 0.0);
    let lh_spin = adw::SpinRow::builder()
        .title(tr("Line Spacing"))
        .subtitle(tr("Multiplier (1.0–3.0)"))
        .adjustment(&lh_adj)
        .digits(1)
        .build();
    typo_group.add(&lh_spin);

    let config_for_lh = config.clone();
    lh_adj.connect_value_changed(move |adj| {
        config_for_lh.set_quran_line_height(adj.value());
        config_for_lh.save();
        crate::apply_font_css(&config_for_lh);
    });

    let fonts_link_row = adw::ActionRow::builder()
        .title(tr("Open Fonts Settings"))
        .subtitle(tr("Choose the font family used for the Quran text."))
        .activatable(true)
        .build();
    fonts_link_row.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));
    let popover_for_fonts_link = typo_popover.clone();
    fonts_link_row.connect_activated(move |_| {
        popover_for_fonts_link.popdown();
        crate::settings_ui::open_fonts_settings();
    });
    typo_group.add(&fonts_link_row);

    let reset_btn = gtk::Button::with_label(&tr("Reset to Default"));
    reset_btn.set_margin_top(8);
    reset_btn.set_margin_start(4);
    reset_btn.set_margin_end(4);
    let arabic_adj_reset = arabic_adj.clone();
    let trans_adj_reset = trans_adj.clone();
    let lh_adj_reset = lh_adj.clone();
    let config_for_reset = config.clone();
    reset_btn.connect_clicked(move |_| {
        arabic_adj_reset.set_value(22.0);
        trans_adj_reset.set_value(14.0);
        lh_adj_reset.set_value(1.0);
        config_for_reset.set_quran_arabic_font_px(22.0);
        config_for_reset.set_quran_translation_font_px(14.0);
        config_for_reset.set_quran_line_height(1.0);
        config_for_reset.save();
        crate::apply_font_css(&config_for_reset);
    });
    typo_outer.append(&reset_btn);

    typo_popover.set_child(Some(&typo_outer));
    typography_btn.set_popover(Some(&typo_popover));

    let header_actions = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    header_actions.set_valign(gtk::Align::Center);
    header_actions.append(&typography_btn);
    header_actions.append(&bookmark_toggle_btn);
    header_actions.append(&bookmarks_btn);
    header_center.set_end_widget(Some(&header_actions));

    header_box.append(&header_center);
    container.append(&header_box);

    let marker_frame = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    marker_frame.add_css_class("card");
    marker_frame.add_css_class("marker-row");
    marker_frame.set_margin_top(4);
    marker_frame.set_margin_bottom(8);
    marker_frame.set_margin_start(12);
    marker_frame.set_margin_end(12);
    marker_frame.set_halign(gtk::Align::Center);
    marker_frame.set_visible(false);
    container.append(&marker_frame);

    let content_area = gtk::Box::new(gtk::Orientation::Vertical, 0);
    content_area.set_vexpand(true);

    let surah_translation = get_surah(surah_num, quran_lang);
    let nominal_start = get_surah_start_page(surah_num).unwrap_or(1);
    let page_count = get_surah_page_count(surah_num).unwrap_or(1);
    let total_pages = get_total_pages();
    let end_page = nominal_start.saturating_add(page_count).saturating_sub(1);
    let start_page = get_verse_page(surah_num, 1).unwrap_or(nominal_start);

    let initial_page = if let Some(page) = target_page {
        page
    } else if let Some(verse) = scroll_to_verse {
        get_verse_page(surah_num, verse).unwrap_or(start_page)
    } else {
        SURAH_READING_POSITIONS.with(|positions| {
            positions
                .borrow()
                .get(&surah_num)
                .copied()
                .filter(|&saved_page| saved_page >= start_page && saved_page <= end_page)
                .unwrap_or(start_page)
        })
    };
    SURAH_READING_POSITIONS.with(|positions| {
        positions.borrow_mut().insert(surah_num, initial_page);
    });
    config.set_quran_last_surah_num(Some(surah_num));
    config.set_quran_last_page(Some(initial_page));
    config.save();
    update_marker_frame(&marker_frame, initial_page, quran_lang);

    let current_page = Rc::new(RefCell::new(initial_page));
    let surah_translation_rc = Rc::new(surah_translation);
    let quran_lang_rc = Rc::new(quran_lang.to_string());
    let config_rc = config.clone();

    let rec_state = Rc::new(RefCell::new(RecitationState {
        selected_verse: Cell::new(None),
        playing: Cell::new(false),
        stop_boundary: Cell::new(None),
        current_playing_surah_num: Cell::new(0),
    }));
    let verse_boundaries: Rc<RefCell<HashMap<u32, Vec<VerseBoundary>>>> =
        Rc::new(RefCell::new(HashMap::new()));

    let rebuild_fn: RebuildFn = Rc::new(RefCell::new(None));
    let rebuild_follow_fn: RebuildFn = Rc::new(RefCell::new(None));
    let play_fn: PlayFn = Rc::new(RefCell::new(None));
    let page_label_ranges: Rc<RefCell<Vec<(usize, usize)>>> = Rc::new(RefCell::new(Vec::new()));
    let page_label_ranges_for_follow = page_label_ranges.clone();
    let page_content_box: Rc<RefCell<Option<gtk::Box>>> = Rc::new(RefCell::new(None));
    let page_content_box_for_follow = page_content_box.clone();

    fn build_page_content(
        page: u32,
        surah_num: u32,
        quran_lang: &str,
        surah_translation: Option<TranslationSurah>,
        highlight_verse: Option<(u32, u32)>,
        search_flash: bool,
        rec_state: Rc<RefCell<RecitationState>>,
    ) -> (gtk::Box, Vec<(usize, usize)>, Vec<gtk::Box>) {
        let box_content = gtk::Box::new(gtk::Orientation::Vertical, 8);
        box_content.set_margin_top(12);
        box_content.set_margin_bottom(12);
        box_content.set_margin_start(12);
        box_content.set_margin_end(12);

        let mut label_ranges: Vec<(usize, usize)> = Vec::new();
        let mut flash_boxes: Vec<gtk::Box> = Vec::new();

        if let Some(verses_data) = get_page_verses(page) {
            let mut header_cache: HashMap<u32, SurahMeta> = HashMap::new();

            if quran_lang == "ar" {
                let mut last_surah_num: Option<u32> = None;
                let mut chunk_surah_num: Option<u32> = None;
                let mut chunk_text = String::new();
                let mut chunk_start = 0;

                for (idx, page_verse) in verses_data.iter().enumerate() {
                    let surah_changed =
                        last_surah_num.is_some() && last_surah_num != Some(page_verse.surah_num);

                    if surah_changed {
                        if !chunk_text.is_empty() {
                            label_ranges.push((chunk_start, idx));
                            let mushaf_label = gtk::Label::new(None);
                            mushaf_label.set_markup(&chunk_text);
                            mushaf_label.set_wrap(true);
                            mushaf_label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
                            mushaf_label.set_selectable(true);
                            attach_arabic_context_menu(&mushaf_label, rec_state.clone());
                            if surah_num == 1 && chunk_surah_num == Some(1) {
                                mushaf_label.set_xalign(0.5);
                                mushaf_label.set_justify(gtk::Justification::Center);
                            } else {
                                mushaf_label.set_xalign(0.0);
                                mushaf_label.set_justify(gtk::Justification::Fill);
                            }
                            mushaf_label.add_css_class("quran-arabic");
                            mushaf_label.add_css_class("quran-verse-block");
                            box_content.append(&mushaf_label);
                            chunk_text.clear();
                        }
                        chunk_surah_num = None;
                    }

                    let is_surah_start =
                        page_verse.verse == 1 && last_surah_num != Some(page_verse.surah_num);
                    if is_surah_start {
                        let meta = header_cache
                            .entry(page_verse.surah_num)
                            .or_insert_with(|| surah_meta(page_verse.surah_num, quran_lang))
                            .clone();

                        let header_box = gtk::Box::new(gtk::Orientation::Vertical, 4);
                        header_box.add_css_class("card");
                        header_box.set_margin_top(12);
                        header_box.set_margin_bottom(6);

                        let arabic_label =
                            gtk::Label::new(Some(&format!("﴿ {} ﴾", meta.arabic.trim())));
                        arabic_label.set_wrap(true);
                        arabic_label.set_xalign(0.5);
                        arabic_label.add_css_class("quran-arabic");
                        header_box.append(&arabic_label);

                        if !meta.surah_type.trim().is_empty() {
                            let typ = if meta.surah_type.trim().eq_ignore_ascii_case("meccan") {
                                tr("Meccan")
                            } else {
                                tr("Medinan")
                            };
                            let type_label = gtk::Label::new(Some(&typ));
                            type_label.set_wrap(true);
                            type_label.set_xalign(0.5);
                            apply_quran_meta_font(&type_label, "ar");
                            type_label.set_margin_bottom(6);
                            header_box.append(&type_label);
                        }

                        box_content.append(&header_box);

                        if page_verse.surah_num != 1 && page_verse.surah_num != 9 {
                            let bismillah_label = gtk::Label::new(Some(BISMILLAH));
                            bismillah_label.set_wrap(true);
                            bismillah_label.set_xalign(0.5);
                            bismillah_label.set_justify(gtk::Justification::Center);
                            bismillah_label.add_css_class("quran-arabic");
                            bismillah_label.set_selectable(true);
                            attach_arabic_context_menu(&bismillah_label, rec_state.clone());
                            bismillah_label.set_margin_bottom(6);
                            box_content.append(&bismillah_label);
                        }
                    }

                    if page_verse.surah_num == 1 && page_verse.verse == 1 {
                        label_ranges.push((idx, idx + 1));
                        let bismillah_label = gtk::Label::new(None);
                        let content = if highlight_verse == Some((1, 1)) {
                            format!(
                                "<span underline='single' underline_color='#3584e4'>{}</span>",
                                BISMILLAH
                            )
                        } else {
                            BISMILLAH.to_string()
                        };
                        bismillah_label.set_markup(&format!(
                            "{} <span size='small' color='gray'>﴿{}﴾</span>",
                            content,
                            to_arabic_indic(1)
                        ));
                        bismillah_label.set_wrap(true);
                        bismillah_label.set_xalign(0.5);
                        bismillah_label.set_justify(gtk::Justification::Center);
                        bismillah_label.add_css_class("quran-arabic");
                        bismillah_label.add_css_class("quran-verse-block");
                        bismillah_label.set_selectable(true);
                        attach_arabic_context_menu(&bismillah_label, rec_state.clone());
                        bismillah_label.set_margin_bottom(6);
                        box_content.append(&bismillah_label);
                        last_surah_num = Some(page_verse.surah_num);
                        continue;
                    }

                    if chunk_surah_num.is_none() {
                        chunk_surah_num = Some(page_verse.surah_num);
                        chunk_start = idx;
                    }

                    if !chunk_text.is_empty() {
                        chunk_text.push(' ');
                    }
                    let escaped = gtk::glib::markup_escape_text(&page_verse.content);
                    if highlight_verse == Some((page_verse.surah_num, page_verse.verse)) {
                        chunk_text.push_str(&format!(
                            "<span underline='single' underline_color='#3584e4'>{}</span>",
                            escaped
                        ));
                    } else {
                        chunk_text.push_str(&escaped);
                    }
                    let verse_num = to_arabic_indic(page_verse.verse);
                    chunk_text.push_str(&format!(" ﴿{}﴾", verse_num));

                    last_surah_num = Some(page_verse.surah_num);
                }

                if !chunk_text.is_empty() {
                    label_ranges.push((chunk_start, verses_data.len()));
                    let mushaf_label = gtk::Label::new(None);
                    mushaf_label.set_markup(&chunk_text);
                    mushaf_label.set_wrap(true);
                    mushaf_label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
                    mushaf_label.set_selectable(true);
                    attach_arabic_context_menu(&mushaf_label, rec_state.clone());
                    if surah_num == 1 && chunk_surah_num == Some(1) {
                        mushaf_label.set_xalign(0.5);
                        mushaf_label.set_justify(gtk::Justification::Center);
                    } else {
                        mushaf_label.set_xalign(0.0);
                        mushaf_label.set_justify(gtk::Justification::Fill);
                    }
                    mushaf_label.add_css_class("quran-arabic");
                    mushaf_label.add_css_class("quran-verse-block");
                    box_content.append(&mushaf_label);
                }
            } else {
                let mut last_surah_num: Option<u32> = None;
                let mut translation_cache: HashMap<u32, Option<TranslationSurah>> = HashMap::new();

                for page_verse in verses_data.iter() {
                    let is_surah_start =
                        page_verse.verse == 1 && last_surah_num != Some(page_verse.surah_num);
                    if is_surah_start {
                        let meta = header_cache
                            .entry(page_verse.surah_num)
                            .or_insert_with(|| surah_meta(page_verse.surah_num, quran_lang))
                            .clone();

                        let header_box = gtk::Box::new(gtk::Orientation::Vertical, 4);
                        header_box.add_css_class("card");
                        header_box.set_margin_top(12);
                        header_box.set_margin_bottom(6);

                        let arabic_label = gtk::Label::new(Some(meta.arabic.trim()));
                        arabic_label.set_wrap(true);
                        arabic_label.set_xalign(0.5);
                        arabic_label.add_css_class("quran-arabic");
                        arabic_label.set_selectable(true);
                        attach_arabic_context_menu(&arabic_label, rec_state.clone());
                        header_box.append(&arabic_label);

                        let mut name_parts = Vec::new();
                        if !meta.translated.trim().is_empty() {
                            name_parts.push(meta.translated.trim().to_string());
                        }
                        if !meta.transliteration.trim().is_empty() {
                            name_parts.push(meta.transliteration.trim().to_string());
                        }
                        if !name_parts.is_empty() {
                            let trans_label = gtk::Label::new(Some(&name_parts.join(" • ")));
                            trans_label.set_wrap(true);
                            trans_label.set_xalign(0.5);
                            trans_label.add_css_class("quran-translation");
                            trans_label.set_selectable(true);
                            attach_readonly_context_menu(&trans_label);
                            header_box.append(&trans_label);
                        }

                        if !meta.surah_type.trim().is_empty() {
                            let typ = if meta.surah_type.trim().eq_ignore_ascii_case("meccan") {
                                tr("Meccan")
                            } else {
                                tr("Medinan")
                            };
                            let type_label = gtk::Label::new(Some(&typ));
                            type_label.set_wrap(true);
                            type_label.set_xalign(0.5);
                            type_label.add_css_class("caption");
                            type_label.add_css_class("quran-translation");
                            type_label.set_margin_bottom(6);
                            header_box.append(&type_label);
                        }

                        box_content.append(&header_box);

                        if page_verse.surah_num != 1 && page_verse.surah_num != 9 {
                            let bismillah_box = gtk::Box::new(gtk::Orientation::Vertical, 4);
                            bismillah_box.set_margin_bottom(6);

                            let bismillah_label = gtk::Label::new(Some(BISMILLAH));
                            bismillah_label.set_wrap(true);
                            bismillah_label.set_xalign(0.5);
                            bismillah_label.set_justify(gtk::Justification::Center);
                            bismillah_label.add_css_class("quran-arabic");
                            bismillah_label.set_selectable(true);
                            attach_arabic_context_menu(&bismillah_label, rec_state.clone());
                            bismillah_box.append(&bismillah_label);

                            if quran_lang != "ar" {
                                let bismillah_surah = translation_cache
                                    .entry(1)
                                    .or_insert_with(|| get_surah(1, quran_lang));
                                if let Some(surah) = bismillah_surah.as_ref()
                                    && let Some(verse_data) =
                                        surah.verses.iter().find(|verse_data| verse_data.id == 1)
                                    && !verse_data.translation.is_empty()
                                {
                                    let translation_label = gtk::Label::new(None);
                                    translation_label.set_markup(&format!(
                                        "<span size='small' color='gray'>{}</span>",
                                        gtk::glib::markup_escape_text(&verse_data.translation)
                                    ));
                                    translation_label.set_wrap(true);
                                    translation_label.set_xalign(0.5);
                                    translation_label.set_justify(gtk::Justification::Center);
                                    translation_label.add_css_class("quran-translation");
                                    translation_label.set_selectable(true);
                                    attach_readonly_context_menu(&translation_label);
                                    bismillah_box.append(&translation_label);
                                }
                            }

                            box_content.append(&bismillah_box);
                        }
                    }

                    let verse_box = gtk::Box::new(gtk::Orientation::Vertical, 4);
                    verse_box.add_css_class("card");
                    verse_box.add_css_class("quran-verse-box");
                    verse_box.set_margin_bottom(6);
                    verse_box.set_widget_name(&format!(
                        "verse_{}_{}",
                        page_verse.surah_num, page_verse.verse
                    ));
                    unsafe {
                        verse_box.set_qdata(
                            glib::Quark::from_str("khushu-verse-surah"),
                            page_verse.surah_num,
                        );
                        verse_box.set_qdata(
                            glib::Quark::from_str("khushu-verse-verse"),
                            page_verse.verse,
                        );
                    }

                    let arabic_label = gtk::Label::new(None);
                    let escaped = gtk::glib::markup_escape_text(&page_verse.content);
                    arabic_label.set_markup(&format!(
                        "{} <span size='small' color='gray'>﴿{}﴾</span>",
                        escaped, page_verse.verse
                    ));
                    arabic_label.set_wrap(true);
                    arabic_label.set_selectable(true);
                    arabic_label.set_xalign(1.0);
                    arabic_label.add_css_class("quran-arabic");
                    arabic_label.set_margin_top(8);
                    arabic_label.set_margin_start(12);
                    arabic_label.set_margin_end(12);
                    arabic_label.set_margin_bottom(4);
                    verse_box.append(&arabic_label);

                    if highlight_verse == Some((page_verse.surah_num, page_verse.verse)) {
                        verse_box.add_css_class("quran-highlight");
                        if search_flash {
                            verse_box.add_css_class("quran-search-flash");
                            flash_boxes.push(verse_box.clone());
                        }
                    }

                    let surah_entry = translation_cache
                        .entry(page_verse.surah_num)
                        .or_insert_with(|| get_surah(page_verse.surah_num, quran_lang));
                    if let Some(surah) = surah_entry.as_ref() {
                        if let Some(verse_data) = surah
                            .verses
                            .iter()
                            .find(|verse_data| verse_data.id == page_verse.verse)
                            && !verse_data.translation.is_empty()
                        {
                            let translation_label = gtk::Label::new(None);
                            let t_escaped = gtk::glib::markup_escape_text(&verse_data.translation);
                            translation_label.set_markup(&format!(
                                "<span size='small' color='gray'>{}:{}</span>  {}",
                                page_verse.surah_num, page_verse.verse, t_escaped
                            ));
                            translation_label.set_wrap(true);
                            translation_label.set_selectable(true);
                            translation_label.set_xalign(0.0);
                            translation_label.add_css_class("body");
                            translation_label.add_css_class("quran-translation");
                            translation_label.set_margin_top(4);
                            translation_label.set_margin_start(12);
                            translation_label.set_margin_end(12);
                            translation_label.set_margin_bottom(8);
                            verse_box.append(&translation_label);
                        }
                    } else if let Some(surah) = surah_translation.as_ref()
                        && page_verse.surah_num == surah.id
                        && let Some(verse_data) = surah
                            .verses
                            .iter()
                            .find(|verse_data| verse_data.id == page_verse.verse)
                        && !verse_data.translation.is_empty()
                    {
                        let translation_label = gtk::Label::new(None);
                        let t_escaped = gtk::glib::markup_escape_text(&verse_data.translation);
                        translation_label.set_markup(&format!(
                            "<span size='small' color='gray'>{}:{}</span>  {}",
                            page_verse.surah_num, page_verse.verse, t_escaped
                        ));
                        translation_label.set_wrap(true);
                        translation_label.set_xalign(0.0);
                        translation_label.add_css_class("body");
                        translation_label.add_css_class("quran-translation");
                        translation_label.set_margin_top(4);
                        translation_label.set_margin_start(12);
                        translation_label.set_margin_end(12);
                        translation_label.set_margin_bottom(8);
                        verse_box.append(&translation_label);
                    }

                    attach_unified_verse_menu(&verse_box, rec_state.clone(), quran_lang);
                    box_content.append(&verse_box);
                    last_surah_num = Some(page_verse.surah_num);
                }
            }
        }

        (box_content, label_ranges, flash_boxes)
    }

    let content_stack = gtk::Box::new(gtk::Orientation::Vertical, 0);
    content_stack.set_vexpand(true);

    let scrolled = gtk::ScrolledWindow::builder().vexpand(true).build();
    let (initial_content, initial_label_ranges, initial_flash_boxes) = build_page_content(
        initial_page,
        surah_num,
        quran_lang,
        (*surah_translation_rc).clone(),
        highlight_verse.map(|verse_num| (surah_num, verse_num)),
        true,
        rec_state.clone(),
    );
    *page_label_ranges.borrow_mut() = initial_label_ranges;
    *page_content_box.borrow_mut() = Some(initial_content.clone());
    scrolled.set_child(Some(&initial_content));
    content_stack.append(&scrolled);

    fn scroll_to_translation_verse(
        content: &gtk::Box,
        scrolled: &gtk::ScrolledWindow,
        surah_num: u32,
        verse: u32,
    ) {
        let target_name = format!("verse_{}_{}", surah_num, verse);
        if let Some(target) = find_widget_by_name(content.upcast_ref(), &target_name) {
            let content_for_scroll: gtk::Widget = content.clone().upcast();
            let tick_scrolled = scrolled.clone();
            scrolled.add_tick_callback(move |_, _| {
                if target.allocated_width() > 0 {
                    if let Some((_, y)) =
                        target.translate_coordinates(&content_for_scroll, 0.0, 0.0)
                    {
                        let adj = tick_scrolled.vadjustment();
                        let max = (adj.upper() - adj.page_size()).max(0.0);
                        adj.set_value((y - 24.0).clamp(0.0, max));
                    }
                    glib::ControlFlow::Break
                } else {
                    let target_c = target.clone();
                    let content_for_scroll_c = content_for_scroll.clone();
                    let tick_scrolled_c = tick_scrolled.clone();
                    glib::idle_add_local(move || {
                        if let Some((_, y)) =
                            target_c.translate_coordinates(&content_for_scroll_c, 0.0, 0.0)
                        {
                            let adj = tick_scrolled_c.vadjustment();
                            let max = (adj.upper() - adj.page_size()).max(0.0);
                            adj.set_value((y - 24.0).clamp(0.0, max));
                        }
                        glib::ControlFlow::Break
                    });
                    glib::ControlFlow::Break
                }
            });
        }
    }

    fn scroll_to_arabic_verse(
        content: &gtk::Box,
        scrolled: &gtk::ScrolledWindow,
        surah_num: u32,
        verse: u32,
        boundaries: &HashMap<u32, Vec<VerseBoundary>>,
        label_ranges: &[(usize, usize)],
        page: u32,
    ) {
        if let Some(boundaries_data) = boundaries.get(&page)
            && let Some(target_idx) = boundaries_data
                .iter()
                .position(|boundary| boundary.surah_num == surah_num && boundary.verse == verse)
            && let Some(label_idx) = label_ranges
                .iter()
                .position(|&(start, end)| target_idx >= start && target_idx < end)
        {
            let (label_start_boundary, _) = label_ranges[label_idx];
            let target_start = boundaries_data[target_idx].byte_start;
            let first_start = boundaries_data[label_start_boundary].byte_start;
            let offset = target_start - first_start;
            if let Some(label) = find_arabic_label(content, label_idx) {
                let tick_scrolled = scrolled.clone();
                let content_widget: gtk::Widget = content.clone().upcast();
                scrolled.add_tick_callback(move |_, _| {
                    if label.allocated_width() > 0 {
                        let layout = label.layout();
                        let rect = layout.index_to_pos(offset as i32);
                        if let Some((_, label_y)) =
                            label.translate_coordinates(&content_widget, 0.0, 0.0)
                        {
                            let y_pixels = label_y + rect.y() as f64 / f64::from(gtk::pango::SCALE);
                            let adj = tick_scrolled.vadjustment();
                            let max = (adj.upper() - adj.page_size()).max(0.0);
                            adj.set_value((y_pixels - 24.0).clamp(0.0, max));
                        }
                        glib::ControlFlow::Break
                    } else {
                        let label_c = label.clone();
                        let tick_scrolled_c = tick_scrolled.clone();
                        let content_widget_c = content_widget.clone();
                        glib::idle_add_local(move || {
                            let layout = label_c.layout();
                            let rect = layout.index_to_pos(offset as i32);
                            if let Some((_, label_y)) =
                                label_c.translate_coordinates(&content_widget_c, 0.0, 0.0)
                            {
                                let y_pixels =
                                    label_y + rect.y() as f64 / f64::from(gtk::pango::SCALE);
                                let adj = tick_scrolled_c.vadjustment();
                                let max = (adj.upper() - adj.page_size()).max(0.0);
                                adj.set_value((y_pixels - 24.0).clamp(0.0, max));
                            }
                            glib::ControlFlow::Break
                        });
                        glib::ControlFlow::Break
                    }
                });
            }
        }
    }

    if let Some(verse) = scroll_to_verse {
        if quran_lang == "ar" && !verse_boundaries.borrow().contains_key(&initial_page) {
            let boundaries = compute_verse_boundaries(initial_page);
            verse_boundaries
                .borrow_mut()
                .insert(initial_page, boundaries);
        }
        if quran_lang != "ar" {
            scroll_to_translation_verse(&initial_content, &scrolled, surah_num, verse);
        } else {
            scroll_to_arabic_verse(
                &initial_content,
                &scrolled,
                surah_num,
                verse,
                &verse_boundaries.borrow(),
                &page_label_ranges.borrow(),
                initial_page,
            );
        }
    }

    if quran_lang == "ar" && highlight_verse.is_some() {
        let rebuild_fn_for_hl = rebuild_fn.clone();
        gtk::glib::timeout_add_local(std::time::Duration::from_millis(5000), move || {
            if let Some(ref rebuild) = *rebuild_fn_for_hl.borrow() {
                rebuild();
            }
            gtk::glib::ControlFlow::Break
        });
    }

    // Translation-mode search highlight is CSS-based: clear it by removing the
    // classes. Detached boxes from a rebuild are a harmless no-op.
    if !initial_flash_boxes.is_empty() {
        gtk::glib::timeout_add_local(std::time::Duration::from_millis(5000), move || {
            for flash_box in &initial_flash_boxes {
                flash_box.remove_css_class("quran-search-flash");
                flash_box.remove_css_class("quran-highlight");
            }
            gtk::glib::ControlFlow::Break
        });
    }

    let page_entry = gtk::Entry::new();
    gtk::prelude::EntryExt::set_alignment(&page_entry, 0.5);
    page_entry.set_width_chars(4);
    page_entry.set_max_length(4);
    page_entry.set_input_purpose(gtk::InputPurpose::Digits);
    gtk::prelude::EditableExt::set_text(&page_entry, &initial_page.to_string());
    page_entry.set_tooltip_text(Some(&page_label_text(initial_page, total_pages)));

    let page_prefix = gtk::Label::new(Some(&tr("page")));
    page_prefix.add_css_class("dim-label");

    let page_total = gtk::Label::new(Some(&format!("/ {}", total_pages)));
    page_total.add_css_class("dim-label");

    let page_input_box = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    page_input_box.set_halign(gtk::Align::Center);
    page_input_box.append(&page_prefix);
    page_input_box.append(&page_entry);
    page_input_box.append(&page_total);

    let prev_btn = gtk::Button::new();
    prev_btn.set_icon_name("go-previous-symbolic");
    prev_btn.set_sensitive(initial_page > 1 || surah_num > 1);

    let next_btn = gtk::Button::new();
    next_btn.set_icon_name("go-next-symbolic");
    next_btn.set_sensitive(initial_page < total_pages || surah_num < 114);

    let nav_center = gtk::CenterBox::new();
    nav_center.set_hexpand(true);
    nav_center.set_start_widget(Some(&prev_btn));
    nav_center.set_center_widget(Some(&page_input_box));
    nav_center.set_end_widget(Some(&next_btn));

    let nav_container = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    nav_container.set_margin_top(6);
    nav_container.set_margin_bottom(6);
    nav_container.set_margin_start(12);
    nav_container.set_margin_end(12);
    nav_container.append(&nav_center);

    content_stack.append(&nav_container);
    let content_clamp = adw::Clamp::builder()
        .maximum_size(760)
        .tightening_threshold(640)
        .child(&content_stack)
        .build();
    content_clamp.set_vexpand(true);
    content_area.append(&content_clamp);
    container.append(&content_area);

    let (toolbar, rec_play_btn, rec_prev_btn, rec_next_btn) =
        build_recitation_toolbar(rec_state.clone(), &config_rc, play_fn.clone(), surah_num);
    content_stack.insert_child_after(&toolbar, Some(&scrolled));

    rec_play_btn.set_icon_name(if crate::audio::is_reciting() {
        "media-playback-pause-symbolic"
    } else {
        "media-playback-start-symbolic"
    });

    let sync_nav: Rc<dyn Fn()> = {
        let rec_state_nav = rec_state.clone();
        let prev_btn_nav = rec_prev_btn.clone();
        let next_btn_nav = rec_next_btn.clone();
        Rc::new(move || {
            let total = surah_total_verses(surah_num).unwrap_or(u32::MAX);
            let prev_enabled = rec_state_nav
                .borrow()
                .selected_verse
                .get()
                .is_some_and(|(_, verse_num)| verse_num > 1);
            let next_enabled = rec_state_nav
                .borrow()
                .selected_verse
                .get()
                .is_some_and(|(_, verse_num)| verse_num < total);
            prev_btn_nav.set_sensitive(prev_enabled);
            next_btn_nav.set_sensitive(next_enabled);
        })
    };

    let rebuild_scrolled = scrolled.clone();
    let rebuild_surah_num = surah_num;
    let rebuild_lang = quran_lang_rc.clone();
    let rebuild_surah_translation = surah_translation_rc.clone();
    let rebuild_total_pages = total_pages;
    let rebuild_marker = marker_frame.clone();
    let rebuild_entry = page_entry.clone();
    let rebuild_prev_btn = prev_btn.clone();
    let rebuild_next_btn = next_btn.clone();
    let rebuild_config = config_rc.clone();
    let rebuild_current_page = current_page.clone();
    let rebuild_rec_state = rec_state.clone();
    let rebuild_bounds = verse_boundaries.clone();
    let rebuild_follow_fn_c = rebuild_follow_fn.clone();
    let rebuild_rec_state_for_follow = rebuild_rec_state.clone();
    let rebuild_current_page_for_follow = rebuild_current_page.clone();
    let rebuild_fn_for_follow = rebuild_fn.clone();
    let rebuild_scrolled_for_follow = rebuild_scrolled.clone();
    let rebuild_lang_for_follow = rebuild_lang.clone();
    let rebuild_bounds_for_follow = rebuild_bounds.clone();
    let rebuild_label_ranges = page_label_ranges.clone();
    let rebuild_header_title = header_title.clone();
    let rebuild_header_extra = header_extra.clone();
    let rebuild_sync_nav = sync_nav.clone();
    *rebuild_fn.borrow_mut() = Some(Box::new(move || {
        let page = *rebuild_current_page.borrow();
        let verse = rebuild_rec_state.borrow().selected_verse.get();
        let highlight =
            verse.filter(|(surah_pos, verse_pos)| is_verse_on_page(*surah_pos, *verse_pos, page));
        let (new_content, new_label_ranges, _new_flash_boxes) = build_page_content(
            page,
            rebuild_surah_num,
            &rebuild_lang,
            (*rebuild_surah_translation).clone(),
            highlight,
            false,
            rebuild_rec_state.clone(),
        );
        *rebuild_label_ranges.borrow_mut() = new_label_ranges;
        *page_content_box.borrow_mut() = Some(new_content.clone());
        rebuild_scrolled.set_child(Some(&new_content));
        if *rebuild_lang == "ar" {
            attach_arabic_verse_clicks(
                &new_content,
                page,
                rebuild_rec_state.clone(),
                rebuild_bounds.clone(),
                rebuild_follow_fn_c.clone(),
                rebuild_label_ranges.clone(),
                rebuild_sync_nav.clone(),
            );
        } else {
            attach_translation_verse_clicks(
                &new_content,
                rebuild_rec_state.clone(),
                rebuild_follow_fn_c.clone(),
                rebuild_sync_nav.clone(),
            );
        }
        update_marker_frame(&rebuild_marker, page, &rebuild_lang);
        gtk::prelude::EditableExt::set_text(&rebuild_entry, &page.to_string());
        rebuild_prev_btn.set_sensitive(page > 1 || rebuild_surah_num > 1);
        rebuild_next_btn.set_sensitive(page < rebuild_total_pages || rebuild_surah_num < 114);
        SURAH_READING_POSITIONS.with(|pos| pos.borrow_mut().insert(rebuild_surah_num, page));
        rebuild_config.set_quran_last_surah_num(Some(rebuild_surah_num));
        rebuild_config.set_quran_last_page(Some(page));
        rebuild_config.save();

        if let Some((selected_surah_num, _)) =
            verse.filter(|(candidate_surah_num, _)| *candidate_surah_num != rebuild_surah_num)
        {
            let surah_meta_info = surah_meta(selected_surah_num, &rebuild_lang);
            if *rebuild_lang == "ar" {
                let name = if surah_meta_info.arabic.is_empty() {
                    format!("Surah {}", selected_surah_num)
                } else {
                    surah_meta_info.arabic.clone()
                };
                rebuild_header_title.set_text(&name);
                if let Some(ref extra) = rebuild_header_extra {
                    let typ = if surah_meta_info
                        .surah_type
                        .trim()
                        .eq_ignore_ascii_case("meccan")
                    {
                        tr("Meccan")
                    } else {
                        tr("Medinan")
                    };
                    extra.set_text(&typ);
                }
            } else {
                let name = if !surah_meta_info.translated.trim().is_empty() {
                    surah_meta_info.translated.trim().to_string()
                } else if !surah_meta_info.transliteration.trim().is_empty() {
                    surah_meta_info.transliteration.trim().to_string()
                } else {
                    surah_meta_info.arabic.clone()
                };
                rebuild_header_title.set_text(&name);
                if let Some(ref extra) = rebuild_header_extra {
                    let mut sub_parts = Vec::new();
                    if !surah_meta_info.transliteration.trim().is_empty() {
                        sub_parts.push(surah_meta_info.transliteration.trim().to_string());
                    }
                    if !surah_meta_info.surah_type.trim().is_empty() {
                        let typ = if surah_meta_info
                            .surah_type
                            .trim()
                            .eq_ignore_ascii_case("meccan")
                        {
                            tr("Meccan")
                        } else {
                            tr("Medinan")
                        };
                        sub_parts.push(typ);
                    }
                    extra.set_text(&sub_parts.join(" • "));
                }
            }
        }
    }));

    *rebuild_follow_fn.borrow_mut() = Some(Box::new(move || {
        let page = *rebuild_current_page_for_follow.borrow();
        let verse = rebuild_rec_state_for_follow.borrow().selected_verse.get();
        if let Some((selected_surah_num, verse_num)) = verse
            && !is_verse_on_page(selected_surah_num, verse_num, page)
            && let Some(resolved_page) = get_verse_page(selected_surah_num, verse_num)
        {
            *rebuild_current_page_for_follow.borrow_mut() = resolved_page;
        }
        if let Some(ref rebuild) = *rebuild_fn_for_follow.borrow() {
            rebuild();
        }
        if let Some((selected_surah_num, verse_num)) = verse {
            let new_page = *rebuild_current_page_for_follow.borrow();
            if let Some(content) = page_content_box_for_follow.borrow().clone() {
                let cb_scrolled = rebuild_scrolled_for_follow.clone();
                if *rebuild_lang_for_follow != "ar" {
                    scroll_to_translation_verse(
                        &content,
                        &cb_scrolled,
                        selected_surah_num,
                        verse_num,
                    );
                } else {
                    let cb_bounds = rebuild_bounds_for_follow.clone();
                    let cb_label_ranges = page_label_ranges_for_follow.clone();
                    scroll_to_arabic_verse(
                        &content,
                        &cb_scrolled,
                        selected_surah_num,
                        verse_num,
                        &cb_bounds.borrow(),
                        &cb_label_ranges.borrow(),
                        new_page,
                    );
                }
            }
        }
    }));

    let play_rec_state = rec_state.clone();
    let play_config = config_rc.clone();
    let play_rebuild = rebuild_follow_fn.clone();
    *play_fn.borrow_mut() = Some(Box::new(move |play_surah_num, verse| {
        log::info!("Play fn called: surah={}, verse={}", play_surah_num, verse);
        play_rec_state.borrow().playing.set(true);
        play_rec_state
            .borrow()
            .selected_verse
            .set(Some((play_surah_num, verse)));
        play_rec_state
            .borrow()
            .current_playing_surah_num
            .set(play_surah_num);
        if let Some(ref rebuild) = *play_rebuild.borrow() {
            rebuild();
        }
        let slug = play_config.reciter_slug();
        log::info!(
            "Playing verse: slug={}, surah={}, verse={}",
            slug,
            play_surah_num,
            verse,
        );
        let stop = play_config.stop_condition();
        let boundary = compute_stop_boundary(play_surah_num, verse, stop);
        play_rec_state.borrow().stop_boundary.set(boundary);
        match stop {
            StopCondition::Ayah => {
                crate::audio::play_verse(&slug, play_surah_num, verse);
            }
            StopCondition::None => {
                let total = surah_total_verses(play_surah_num).unwrap_or(u32::MAX);
                crate::audio::play_surah(&slug, play_surah_num, verse, total);
            }
            _ => {
                if let Some((end_surah_num, end_verse)) = boundary {
                    let end = if end_surah_num == play_surah_num {
                        end_verse
                    } else {
                        surah_total_verses(play_surah_num).unwrap_or(u32::MAX)
                    };
                    crate::audio::play_surah(&slug, play_surah_num, verse, end);
                } else {
                    let total = surah_total_verses(play_surah_num).unwrap_or(u32::MAX);
                    crate::audio::play_surah(&slug, play_surah_num, verse, total);
                }
            }
        }
    }));

    if quran_lang == "ar" {
        attach_arabic_verse_clicks(
            &initial_content,
            initial_page,
            rec_state.clone(),
            verse_boundaries.clone(),
            rebuild_fn.clone(),
            page_label_ranges.clone(),
            sync_nav.clone(),
        );
    } else {
        attach_translation_verse_clicks(
            &initial_content,
            rec_state.clone(),
            rebuild_fn.clone(),
            sync_nav.clone(),
        );
    }

    let subscriptions: Rc<RefCell<Option<RecitationSubscriptions>>> = Rc::new(RefCell::new(None));

    let sync_nav_play_state = sync_nav.clone();
    let recitation_state_callback: Rc<dyn Fn(bool)> = {
        let play_btn = rec_play_btn.clone();
        let callback_rec_state = rec_state.clone();
        Rc::new(move |is_reciting| {
            // "Stopped" is global: reset on stop, natural end, or adhan takeover.
            if !is_reciting {
                callback_rec_state.borrow().playing.set(false);
            }
            play_btn.set_icon_name(if is_reciting {
                "media-playback-pause-symbolic"
            } else {
                "media-playback-start-symbolic"
            });
            sync_nav_play_state();
        })
    };
    crate::audio::register_recitation_state_callback(&recitation_state_callback);

    let verse_finished_callback: Rc<dyn Fn(u32, u32)> = {
        let event_rec_state = rec_state.clone();
        let event_rebuild = rebuild_follow_fn.clone();
        let event_play_fn = play_fn.clone();
        let event_sync_nav = sync_nav.clone();
        Rc::new(move |finished_surah_num, verse| {
            let (playing_surah_num, currently_playing) = {
                let state = event_rec_state.borrow();
                (state.current_playing_surah_num.get(), state.playing.get())
            };
            if finished_surah_num != playing_surah_num || !currently_playing {
                return;
            }
            // Kind is updated before the notify runs, so this is post-transition:
            // true = still chaining, false = the recitation ended.
            if crate::audio::is_reciting() {
                let next_verse = verse + 1;
                event_rec_state
                    .borrow()
                    .selected_verse
                    .set(Some((finished_surah_num, next_verse)));
                if let Some(ref rebuild) = *event_rebuild.borrow() {
                    rebuild();
                }
            } else {
                let current = (finished_surah_num, verse);
                let boundary = event_rec_state.borrow().stop_boundary.get();
                if boundary.is_some_and(|target| current >= target) {
                    event_rec_state.borrow().playing.set(false);
                } else if let Some((next_surah_num, next_verse)) =
                    next_verse_on_page_or_next(finished_surah_num, verse)
                {
                    if let Some(ref playback_fn) = *event_play_fn.borrow() {
                        playback_fn(next_surah_num, next_verse);
                    }
                } else {
                    event_rec_state.borrow().playing.set(false);
                }
            }
            event_sync_nav();
        })
    };
    crate::audio::register_verse_finished_callback(&verse_finished_callback);

    *subscriptions.borrow_mut() = Some(RecitationSubscriptions {
        _recitation_state: Some(recitation_state_callback),
        _verse_finished: Some(verse_finished_callback),
    });

    let subscriptions_on_unrealize = subscriptions.clone();
    container.connect_unrealize(move |_| {
        subscriptions_on_unrealize.borrow_mut().take();
    });

    let scrolled_for_prev = scrolled.clone();
    let current_page_for_prev = current_page.clone();
    let lang_for_prev = quran_lang_rc.clone();
    let view_stack_for_prev = view_stack.clone();
    let config_for_prev = config_rc.clone();
    let rebuild_fn_for_prev = rebuild_fn.clone();

    fn navigate_to_surah(
        target_surah_num: u32,
        target_page: Option<u32>,
        scroll_verse: Option<u32>,
        view_stack: &adw::ViewStack,
        cfg: AppConfig,
        lang: &str,
    ) {
        let page_name = format!("surah_{}", target_surah_num);
        if let Some(existing) = view_stack.child_by_name(&page_name) {
            if target_page.is_some() {
                view_stack.remove(&existing);
                let surah_view = create_surah_view(
                    target_surah_num,
                    lang,
                    view_stack,
                    target_page,
                    scroll_verse,
                    None,
                    cfg,
                );
                surah_view.set_vexpand(true);
                view_stack.add_named(&surah_view, Some(&page_name));
                view_stack.set_visible_child_name(&page_name);
            } else {
                view_stack.set_visible_child(&existing);
            }
        } else {
            let surah_view = create_surah_view(
                target_surah_num,
                lang,
                view_stack,
                target_page,
                scroll_verse,
                None,
                cfg,
            );
            surah_view.set_vexpand(true);
            view_stack.add_named(&surah_view, Some(&page_name));
            view_stack.set_visible_child_name(&page_name);
        }
    }

    prev_btn.connect_clicked(move |_| {
        let mut page = current_page_for_prev.borrow_mut();
        if *page > 1 {
            *page -= 1;
            let new_page = *page;
            let in_current = new_page >= start_page;
            drop(page);
            if in_current {
                if let Some(ref rebuild) = *rebuild_fn_for_prev.borrow() {
                    rebuild();
                }
                scrolled_for_prev.vadjustment().set_value(0.0);
            } else if surah_num > 1 {
                let target_surah_num = get_page_index()
                    .and_then(|idx| {
                        idx.page_starts
                            .get(&new_page)
                            .map(|page_start| page_start.surah_num)
                    })
                    .unwrap_or(surah_num - 1);
                navigate_to_surah(
                    target_surah_num,
                    Some(new_page),
                    None,
                    &view_stack_for_prev,
                    config_for_prev.clone(),
                    &lang_for_prev,
                );
            }
        } else if surah_num > 1 {
            navigate_to_surah(
                surah_num - 1,
                None,
                None,
                &view_stack_for_prev,
                config_for_prev.clone(),
                &lang_for_prev,
            );
        }
    });

    let scrolled_for_next = scrolled.clone();
    let current_page_for_next = current_page.clone();
    let total_pages_for_next = total_pages;
    let lang_for_next = quran_lang_rc.clone();
    let view_stack_for_next = view_stack.clone();
    let config_for_next = config_rc.clone();
    let rebuild_fn_for_next = rebuild_fn.clone();

    next_btn.connect_clicked(move |_| {
        let mut page = current_page_for_next.borrow_mut();
        let new_page = *page + 1;
        if new_page <= total_pages_for_next {
            *page = new_page;
            let in_current = new_page <= end_page;
            drop(page);
            if in_current {
                if let Some(ref rebuild) = *rebuild_fn_for_next.borrow() {
                    rebuild();
                }
                scrolled_for_next.vadjustment().set_value(0.0);
            } else if surah_num < 114 {
                let target_surah_num = get_page_index()
                    .and_then(|idx| {
                        idx.page_starts
                            .get(&new_page)
                            .map(|page_start| page_start.surah_num)
                    })
                    .unwrap_or(surah_num + 1);
                navigate_to_surah(
                    target_surah_num,
                    Some(new_page),
                    None,
                    &view_stack_for_next,
                    config_for_next.clone(),
                    &lang_for_next,
                );
            }
        }
    });

    let view_stack_back = view_stack.clone();
    let lang_for_back = quran_lang.to_string();
    let config_for_back = config_rc.clone();
    back_btn.connect_clicked(move |_| {
        {
            let pages = view_stack_back.pages();
            let mut to_remove: Vec<gtk::Widget> = Vec::new();
            for index in 0..pages.n_items() {
                if let Some(page_obj) = pages.item(index)
                    && let Ok(page) = page_obj.downcast::<adw::ViewStackPage>()
                    && let Some(name) = page.name()
                    && name.starts_with("surah_")
                {
                    to_remove.push(page.child());
                }
            }
            for child in to_remove {
                view_stack_back.remove(&child);
            }
        }
        if let Some(old) = view_stack_back.child_by_name("quran") {
            view_stack_back.remove(&old);
        }
        let quran_page =
            create_quran_page(&lang_for_back, &view_stack_back, config_for_back.clone());
        view_stack_back.add_named(&quran_page, Some("quran"));
        view_stack_back.set_visible_child_name("quran");
    });

    let rebuild_fn_for_start = rebuild_fn.clone();
    let start_page_cp = current_page.clone();
    let scrolled_for_start = scrolled.clone();
    start_btn.connect_clicked(move |_| {
        *start_page_cp.borrow_mut() = start_page;
        if let Some(ref rebuild) = *rebuild_fn_for_start.borrow() {
            rebuild();
        }
        let adj = scrolled_for_start.vadjustment();
        adj.set_value(0.0);
    });

    let current_page_for_bm = current_page.clone();
    let bookmark_btn_for_toggle_in_toggle = bookmark_toggle_btn.clone();
    let bookmark_btn_for_toggle_in_popover = bookmark_toggle_btn.clone();
    let bookmark_btn_for_toggle_init = bookmark_toggle_btn.clone();
    let bookmarks_btn_for_popover = bookmarks_btn.clone();
    let view_stack_for_bookmarks = view_stack.clone();
    let lang_for_bookmarks = quran_lang_rc.clone();
    let total_pages_for_bookmarks = total_pages;
    let toast_overlay_for_toggle = toast_overlay.clone();
    let config_for_bm_toggle = config_rc.clone();

    let bookmarks_popover = gtk::Popover::builder().has_arrow(true).build();
    bookmarks_popover.set_parent(&bookmarks_btn_for_popover);
    let bookmarks_list = gtk::ListBox::new();
    bookmarks_list.add_css_class("list-box");
    bookmarks_list.set_selection_mode(gtk::SelectionMode::None);
    bookmarks_list.set_activate_on_single_click(true);
    bookmarks_popover.set_child(Some(&bookmarks_list));

    fn is_page_bookmarked(config: &AppConfig, page: u32) -> bool {
        config
            .quran_bookmarks()
            .iter()
            .any(|bookmark| bookmark.page == page)
    }

    fn bookmark_for_page(page: u32) -> QuranBookmark {
        if let Some(idx) = get_page_index()
            && let Some(start) = idx.page_starts.get(&page)
        {
            return QuranBookmark {
                page,
                surah_num: start.surah_num,
                verse: start.verse,
            };
        }
        QuranBookmark {
            page,
            surah_num: 1,
            verse: 1,
        }
    }

    fn set_bookmark_state(btn: &gtk::Button, page: u32, config: &AppConfig) {
        let active = is_page_bookmarked(config, page);
        if active {
            btn.add_css_class("accent");
        } else {
            btn.remove_css_class("accent");
        }
        btn.set_icon_name("user-bookmarks-symbolic");
    }

    set_bookmark_state(&bookmark_btn_for_toggle_init, initial_page, &config);

    let config_for_toggle = config_for_bm_toggle.clone();
    bookmark_toggle_btn.connect_clicked(move |btn| {
        let page = *current_page_for_bm.borrow();
        let is_bookmarked = is_page_bookmarked(&config_for_toggle, page);
        if is_bookmarked {
            let mut bookmarks = config_for_toggle.quran_bookmarks();
            bookmarks.retain(|bookmark| bookmark.page != page);
            config_for_toggle.set_quran_bookmarks(bookmarks);
        } else {
            let mut bookmarks = config_for_toggle.quran_bookmarks();
            bookmarks.push(bookmark_for_page(page));
            bookmarks.sort_by_key(|bookmark| bookmark.page);
            bookmarks.dedup_by_key(|bookmark| bookmark.page);
            config_for_toggle.set_quran_bookmarks(bookmarks);
        }
        config_for_toggle.save();
        set_bookmark_state(btn, page, &config_for_toggle);
        set_bookmark_state(&bookmark_btn_for_toggle_in_toggle, page, &config_for_toggle);
        let msg = if is_bookmarked {
            tr("Bookmark removed")
        } else {
            tr("Bookmark added")
        };
        toast_overlay_for_toggle.add_toast(adw::Toast::new(&msg));
    });

    let current_page_for_popover = current_page.clone();
    let config_for_popover = config_rc.clone();
    let config_for_popover_row = config_rc.clone();
    gtk::prelude::ButtonExt::connect_clicked(&bookmarks_btn, move |_| {
        while let Some(child) = bookmarks_list.first_child() {
            bookmarks_list.remove(&child);
        }

        let mut bookmarks = config_for_popover.quran_bookmarks();
        bookmarks.sort_by_key(|bookmark| bookmark.page);
        bookmarks.dedup_by_key(|bookmark| bookmark.page);

        for bookmark in &bookmarks {
            let row = build_bookmark_row(
                bookmark,
                &lang_for_bookmarks,
                total_pages_for_bookmarks,
                &view_stack_for_bookmarks,
                config_for_popover_row.clone(),
                Some(&bookmarks_popover),
            );
            bookmarks_list.append(&row);
        }

        if bookmarks.is_empty() {
            let placeholder_row = adw::ActionRow::new();
            placeholder_row.set_title(&tr("No bookmarks yet"));
            placeholder_row.set_subtitle(&tr("Bookmark pages by clicking the bookmark icon"));
            placeholder_row.set_activatable(false);
            placeholder_row.set_selectable(false);
            bookmarks_list.append(&placeholder_row);
        }

        let page = *current_page_for_popover.borrow();
        set_bookmark_state(
            &bookmark_btn_for_toggle_in_popover,
            page,
            &config_for_popover,
        );
        bookmarks_popover.popup();
    });

    let current_page_for_input = current_page.clone();
    let lang_for_input = quran_lang_rc.clone();
    let toast_overlay_for_input = toast_overlay.clone();
    let view_stack_for_input = view_stack.clone();
    let config_for_input = config_rc.clone();
    gtk::prelude::EntryExt::connect_activate(&page_entry, move |entry| {
        let text = gtk::prelude::EditableExt::text(entry).trim().to_string();
        let Ok(page) = text.parse::<u32>() else {
            gtk::prelude::EditableExt::set_text(
                entry,
                &current_page_for_input.borrow().to_string(),
            );
            toast_overlay_for_input.add_toast(adw::Toast::new(&tr("Invalid page number")));
            return;
        };
        if page < 1 || page > total_pages {
            gtk::prelude::EditableExt::set_text(
                entry,
                &current_page_for_input.borrow().to_string(),
            );
            toast_overlay_for_input.add_toast(adw::Toast::new(&tr("Invalid page number")));
            return;
        }

        let Some(idx) = get_page_index() else {
            return;
        };
        let Some(start) = idx.page_starts.get(&page) else {
            return;
        };
        let target_surah_num = start.surah_num;
        let target_verse = start.verse;

        let page_name = format!("surah_{}", target_surah_num);
        if let Some(old) = view_stack_for_input.child_by_name(&page_name) {
            view_stack_for_input.remove(&old);
        }
        let surah_view = create_surah_view(
            target_surah_num,
            &lang_for_input,
            &view_stack_for_input,
            None,
            Some(target_verse),
            None,
            config_for_input.clone(),
        );
        surah_view.set_vexpand(true);
        view_stack_for_input.add_named(&surah_view, Some(&page_name));
        view_stack_for_input.set_visible_child_name(&page_name);
    });

    toast_overlay.set_child(Some(&container));
    toast_overlay.upcast()
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

    fn shared_page_surah_starts() -> Vec<(u32, u32, u32)> {
        ensure_resources();
        let total = get_total_pages();
        let mut out = Vec::new();

        for page in 1..=total {
            let Some(verses) = get_page_verses(page) else {
                continue;
            };
            let mut last_surah_num: Option<u32> = None;
            for page_verse in verses {
                if let Some(prev) = last_surah_num
                    && prev != page_verse.surah_num
                    && page_verse.verse == 1
                {
                    out.push((page, prev, page_verse.surah_num));
                }
                last_surah_num = Some(page_verse.surah_num);
            }
        }

        out
    }

    fn page_markers(page: u32) -> Vec<(Option<u32>, Option<u32>, u32, u32)> {
        ensure_resources();
        let Some(verses) = get_page_verses(page) else {
            return Vec::new();
        };

        let mut markers = Vec::new();
        let mut last_surah_num: Option<u32> = None;

        for page_verse in verses.iter() {
            let is_surah_start =
                page_verse.verse == 1 && last_surah_num != Some(page_verse.surah_num);
            if is_surah_start {
                markers.push((Some(page_verse.surah_num), None, 0, 0));
                if page_verse.surah_num != 1 && page_verse.surah_num != 9 {
                    markers.push((None, Some(page_verse.surah_num), 0, 0));
                }
            }
            markers.push((None, None, page_verse.surah_num, page_verse.verse));
            last_surah_num = Some(page_verse.surah_num);
        }

        markers
    }

    #[test]
    fn bismillah_is_clean() {
        assert!(!BISMILLAH.contains('*'));
        assert!(!BISMILLAH.contains('<'));
        assert!(!BISMILLAH.contains('>'));
    }

    #[test]
    fn fatiha_verse_one_is_bismillah() {
        ensure_resources();
        let ar = get_quran("ar");
        let v1 = ar
            .iter()
            .find(|surah| surah.id == 1)
            .and_then(|surah| surah.verses.iter().find(|verse| verse.id == 1))
            .map(|verse| verse.text.as_str())
            .unwrap_or("");
        assert_eq!(v1, BISMILLAH);
    }

    #[test]
    fn bismillah_translation_available_in_non_arabic_languages() {
        ensure_resources();
        let langs = ["en", "fr", "es", "tr"];
        for lang in langs {
            let quran = get_quran(lang);
            let verse_one = quran
                .iter()
                .find(|surah| surah.id == 1)
                .and_then(|surah| surah.verses.iter().find(|verse| verse.id == 1));
            assert!(verse_one.is_some(), "Bismillah verse missing in {}", lang);
            let verse = verse_one.unwrap();
            assert!(
                !verse.translation.is_empty(),
                "Bismillah translation empty in {}",
                lang
            );
            assert_ne!(
                verse.translation, BISMILLAH,
                "Translation is Arabic in {}",
                lang
            );
        }
    }

    #[test]
    fn marker_indices_load() {
        ensure_resources();
        assert_eq!(get_juz_index().len(), 30);
        assert_eq!(get_hizb_quarter_index().len(), 240);
    }

    #[test]
    fn navigation_lands_on_correct_start_page_for_all_surahs() {
        ensure_resources();
        for surah_num in 1..=114 {
            let Some(page) = get_surah_start_page(surah_num) else {
                panic!("missing start page for surah {}", surah_num);
            };
            let verses = get_page_verses(page).expect("missing page verses");
            assert!(
                verses
                    .iter()
                    .any(|page_verse| page_verse.surah_num == surah_num),
                "surah {} not found on reported start page {}",
                surah_num,
                page
            );
        }
    }

    #[test]
    fn shared_page_surah_starts_have_header_and_bismillah() {
        let shared = shared_page_surah_starts();
        assert!(!shared.is_empty());
        for (page, _prev_surah_num, new_surah_num) in shared {
            let markers = page_markers(page);

            let verse_idx = markers
                .iter()
                .position(|marker| marker.2 == new_surah_num && marker.3 == 1)
                .expect("missing new surah verse 1");

            let header_present = markers[..verse_idx]
                .iter()
                .rev()
                .any(|marker| marker.0 == Some(new_surah_num));
            assert!(
                header_present,
                "missing header for surah {} on page {}",
                new_surah_num, page
            );

            if new_surah_num != 9 && new_surah_num != 1 {
                let bismillah_present = markers[..verse_idx]
                    .iter()
                    .rev()
                    .any(|marker| marker.1 == Some(new_surah_num));
                assert!(
                    bismillah_present,
                    "missing bismillah for surah {} on page {}",
                    new_surah_num, page
                );
            }
        }
    }

    #[test]
    fn in_page_header_when_surah_starts_at_top_of_page() {
        ensure_resources();
        for surah_num in 1..=114 {
            let Some(page) = get_surah_start_page(surah_num) else {
                continue;
            };
            let Some(verses) = get_page_verses(page) else {
                continue;
            };
            let Some(first) = verses.first() else {
                continue;
            };
            if first.surah_num == surah_num && first.verse == 1 {
                let markers = page_markers(page);
                let verse_idx = markers
                    .iter()
                    .position(|marker| marker.2 == surah_num && marker.3 == 1)
                    .expect("missing verse 1");
                let header_present = markers[..verse_idx]
                    .iter()
                    .any(|marker| marker.0 == Some(surah_num));
                assert!(
                    header_present,
                    "missing in-page header for surah {} on its own start page {}",
                    surah_num, page
                );
            }
        }
    }

    #[test]
    fn page_indicator_is_global_only() {
        let text = page_label_text(106, 604);
        assert!(!text.contains("•"));
    }

    #[test]
    fn shared_page_531_verse_integrity() {
        ensure_resources();
        let verses = get_page_verses(531).expect("page 531 should exist");
        assert!(
            verses
                .iter()
                .any(|verse| verse.surah_num == 55 && verse.verse == 1),
            "page 531 should contain 55:1"
        );
        assert!(is_verse_on_page(55, 1, 531), "55:1 should be on page 531");
        assert_eq!(
            get_verse_page(55, 1),
            Some(531),
            "get_verse_page(55,1) should return 531"
        );

        let bounds = compute_verse_boundaries(531);
        let offset_55v1 = bounds
            .iter()
            .find(|boundary| boundary.surah_num == 55 && boundary.verse == 1)
            .map(|boundary| boundary.byte_start)
            .expect("55:1 must have a byte_start");
        assert_eq!(find_verse_at_offset(offset_55v1, &bounds), Some((55, 1)));
    }

    #[test]
    fn all_shared_pages_have_correct_get_verse_page() {
        ensure_resources();
        for (page, _prev, new_surah_num) in shared_page_surah_starts() {
            assert_eq!(
                get_verse_page(new_surah_num, 1),
                Some(page),
                "get_verse_page({},1) should be page {}",
                new_surah_num,
                page
            );
        }
    }
}
