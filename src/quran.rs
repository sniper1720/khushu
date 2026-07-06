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
use std::time::Duration;

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
pub struct TranslationChapter {
    pub id: u32,
    pub name: String,
    pub transliteration: String,
    pub translation: String,
    #[serde(rename = "type")]
    pub chapter_type: String,
    #[serde(rename = "total_verses")]
    pub total_verses: u32,
    pub verses: Vec<Verse>,
}

#[derive(Clone, Debug, Deserialize)]
struct ArabicVerse {
    #[allow(dead_code)]
    chapter: u32,
    verse: u32,
    text: String,
}

#[derive(Clone, Debug, Deserialize)]
struct ArabicData(HashMap<String, Vec<ArabicVerse>>);

#[derive(Clone, Debug, Deserialize)]
struct ChapterInfo {
    id: u32,
    name: String,
    transliteration: String,
    #[allow(dead_code)]
    english: String,
    #[serde(rename = "type")]
    chapter_type: String,
    #[allow(dead_code)]
    order: u32,
    #[serde(rename = "total_verses")]
    total_verses: u32,
    #[allow(dead_code)]
    rukus: u32,
    #[allow(dead_code)]
    start_verse: u32,
}

#[derive(Clone, Debug, Deserialize)]
struct ChaptersWrapper {
    data: Vec<ChapterInfo>,
}

#[derive(Clone, Debug, Deserialize)]
struct DataWrapper<T> {
    data: Vec<T>,
}

#[derive(Clone, Debug, Deserialize)]
struct MarkerPos {
    id: u32,
    surah: u32,
    verse: u32,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PageVerse {
    verse: u32,
    surah: u32,
    content: String,
}

#[derive(Clone, Debug, Deserialize)]
struct PageStart {
    surah: u32,
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

fn get_chapter_info() -> Option<Vec<ChapterInfo>> {
    if let Ok(bytes) = gtk::gio::resources_lookup_data(
        "/io/github/sniper1720/khushu/quran/chapters.json",
        gtk::gio::ResourceLookupFlags::NONE,
    ) && let Ok(content) = std::str::from_utf8(&bytes)
    {
        if let Ok(wrapper) = serde_json::from_str::<ChaptersWrapper>(content) {
            return Some(wrapper.data);
        }
        if let Ok(info) = serde_json::from_str::<Vec<ChapterInfo>>(content) {
            return Some(info);
        }
    }
    None
}

fn parse_arabic_data(json: &str) -> Vec<TranslationChapter> {
    if let Ok(data) = serde_json::from_str::<ArabicData>(json) {
        let chapter_info = get_chapter_info();
        let mut chapters: Vec<TranslationChapter> = Vec::new();
        for (key, verses) in data.0 {
            if let Ok(chapter_num) = key.parse::<u32>() {
                let info = chapter_info
                    .as_ref()
                    .and_then(|ci| ci.iter().find(|c| c.id == chapter_num).cloned());
                let chapter_verses: Vec<Verse> = verses
                    .into_iter()
                    .map(|v| Verse {
                        id: v.verse,
                        text: v.text,
                        translation: String::new(),
                    })
                    .collect();
                let chapter_name = info.as_ref().map(|i| i.name.clone()).unwrap_or_default();
                let chapter_translit = info
                    .as_ref()
                    .map(|i| i.transliteration.clone())
                    .unwrap_or_default();
                let chapter_type = info
                    .as_ref()
                    .map(|i| i.chapter_type.clone())
                    .unwrap_or_else(|| String::from("meccan"));
                let chapter_total = info
                    .as_ref()
                    .map(|i| i.total_verses)
                    .unwrap_or(chapter_verses.len() as u32);
                chapters.push(TranslationChapter {
                    id: chapter_num,
                    name: chapter_name,
                    transliteration: chapter_translit,
                    translation: String::new(),
                    chapter_type,
                    total_verses: chapter_total,
                    verses: chapter_verses,
                });
            }
        }
        chapters.sort_by_key(|c| c.id);
        chapters
    } else {
        Vec::new()
    }
}

type NormalizedIndex = HashMap<(u32, u32), String>;
type MarkerIndexU32 = HashMap<(u32, u32), u32>;

thread_local! {
    static QURAN_CACHE: std::cell::RefCell<Option<HashMap<String, Vec<TranslationChapter>>>> = const { std::cell::RefCell::new(None) };
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
        for chapter in &arabic_quran {
            for verse in &chapter.verses {
                index.insert((chapter.id, verse.id), normalize_arabic(&verse.text));
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
        let mut index = HashMap::new();
        for m in wrapper.data {
            index.insert((m.surah, m.verse), m.id);
        }
        return Some(index);
    }
    None
}

fn load_marker_positions(resource_path: &str) -> Option<Vec<MarkerPos>> {
    if let Ok(bytes) =
        gtk::gio::resources_lookup_data(resource_path, gtk::gio::ResourceLookupFlags::NONE)
        && let Ok(content) = std::str::from_utf8(&bytes)
        && let Ok(wrapper) = serde_json::from_str::<DataWrapper<MarkerPos>>(content)
    {
        let mut data = wrapper.data;
        data.sort_by_key(|m| (m.surah, m.verse));
        return Some(data);
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

pub fn get_surah_start_page(surah: u32) -> Option<u32> {
    get_page_index()
        .and_then(|idx| idx.surah_start_pages.get(&surah).copied())
        .or_else(|| get_verse_page(surah, 1))
}

pub fn get_surah_page_count(surah: u32) -> Option<u32> {
    get_page_index().and_then(|idx| idx.surah_page_count.get(&surah).copied())
}

pub fn get_total_pages() -> u32 {
    get_page_index().map(|idx| idx.total_pages).unwrap_or(604)
}

pub fn get_verse_page(surah: u32, verse: u32) -> Option<u32> {
    let page_index = get_page_index()?;
    let page_starts = &page_index.page_starts;
    let target = (surah, verse);
    let total_pages = page_index.total_pages;

    for page_id in 1..=total_pages {
        let Some(start) = page_starts.get(&page_id) else {
            continue;
        };
        let start_pos = (start.surah, start.verse);

        let end_pos = if let Some(next_start) = page_starts.get(&(page_id + 1)) {
            (next_start.surah, next_start.verse)
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
    let mut current_surah = page_start.surah;
    let mut current_verse = page_start.verse;

    let end_surah;
    let end_verse;
    if let Some(next_start) = page_index.page_starts.get(&next_page) {
        end_surah = next_start.surah;
        end_verse = next_start.verse;
    } else {
        end_surah = 115;
        end_verse = 0;
    }

    loop {
        if current_surah == end_surah && current_verse >= end_verse {
            break;
        }
        if current_surah > end_surah {
            break;
        }

        if let Some(chapter) = arabic.iter().find(|c| c.id == current_surah)
            && let Some(verse) = chapter.verses.iter().find(|v| v.id == current_verse)
        {
            verses.push(PageVerse {
                verse: current_verse,
                surah: current_surah,
                content: verse.text.clone(),
            });
        }

        current_verse += 1;
        if current_verse
            > page_index
                .surah_verse_counts
                .get(&current_surah)
                .copied()
                .unwrap_or(0)
        {
            current_surah += 1;
            current_verse = 1;
            if current_surah > 114 {
                break;
            }
        }
    }

    Some(verses)
}

fn load_quran(lang: &str) -> Vec<TranslationChapter> {
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
                if let Ok(quran) = serde_json::from_str::<Vec<TranslationChapter>>(content) {
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

fn get_quran(lang: &str) -> Vec<TranslationChapter> {
    QURAN_CACHE.with(|cache| {
        let mut cache_ref = cache.borrow_mut();
        if cache_ref.is_none() {
            *cache_ref = Some(HashMap::new());
        }
        if let Some(ref mut map) = cache_ref.as_mut() {
            if let Some(data) = map.get(lang) {
                return data.clone();
            }
            let data = load_quran(lang);
            map.insert(lang.to_string(), data.clone());
            data
        } else {
            load_quran(lang)
        }
    })
}

#[allow(dead_code)]
pub fn get_chapter(chapter_num: u32, lang: &str) -> Option<TranslationChapter> {
    let quran = get_quran(lang);
    quran.iter().find(|c| c.id == chapter_num).cloned()
}

#[allow(dead_code)]
pub fn get_verse(chapter: u32, verse: u32, lang: &str) -> Option<Verse> {
    let quran = get_quran(lang);
    quran
        .iter()
        .find(|c| c.id == chapter)
        .and_then(|c| c.verses.iter().find(|v| v.id == verse).cloned())
}

pub fn get_arabic_text(chapter: u32, verse: u32) -> Option<String> {
    let quran = get_quran("ar");
    quran.iter().find(|c| c.id == chapter).and_then(|c| {
        c.verses
            .iter()
            .find(|v| v.id == verse)
            .map(|v| v.text.clone())
    })
}

#[allow(dead_code)]
pub fn get_translation(chapter: u32, verse: u32, lang: &str) -> Option<String> {
    if lang == "ar" {
        get_arabic_text(chapter, verse)
    } else {
        get_verse(chapter, verse, lang).map(|v| v.translation)
    }
}

#[allow(dead_code)]
pub fn get_total_chapters() -> u32 {
    114
}

#[allow(dead_code)]
pub fn get_chapter_verse_count(chapter: u32, lang: &str) -> Option<u32> {
    get_chapter(chapter, lang).map(|c| c.total_verses)
}

#[allow(dead_code)]
pub fn get_chapter_name(chapter: u32, lang: &str) -> Option<String> {
    get_chapter(chapter, lang).map(|c| c.translation.clone())
}

#[allow(dead_code)]
pub fn get_chapter_transliteration(chapter: u32, lang: &str) -> Option<String> {
    get_chapter(chapter, lang).map(|c| c.transliteration.clone())
}

#[allow(dead_code)]
pub fn is_meccan(chapter: u32, lang: &str) -> Option<bool> {
    get_chapter(chapter, lang).map(|c| c.chapter_type == "meccan")
}

#[derive(Clone, Debug)]
pub struct SurahListItem {
    pub id: u32,
    pub name: String,
    pub transliteration: String,
    pub translation: String,
    pub chapter_type: String,
    pub total_verses: u32,
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct VerseMatch {
    pub surah_id: u32,
    pub verse_id: u32,
    pub arabic_text: String,
    pub translation_text: String,
    pub surah_name: String,
    pub surah_translation: String,
}

pub fn get_surah_list(lang: &str) -> Vec<SurahListItem> {
    get_quran(lang)
        .iter()
        .map(|c| SurahListItem {
            id: c.id,
            name: c.name.clone(),
            transliteration: c.transliteration.clone(),
            translation: c.translation.clone(),
            chapter_type: c.chapter_type.clone(),
            total_verses: c.total_verses,
        })
        .collect()
}

fn is_arabic_ignorable(c: char) -> bool {
    let code = c as u32;
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

/// Normalize an Arabic letter to its canonical base form for search.
fn normalize_arabic_char(c: char) -> char {
    match c {
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
        _ => c,
    }
}

// Strips all diacritics, tatweel, and Quranic annotations, then maps letter variants to their canonical forms.
fn normalize_arabic(text: &str) -> String {
    text.chars()
        .filter(|c| {
            let code = *c as u32;
            if code < 0x0600 {
                return true;
            }
            if is_arabic_ignorable(*c) {
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
    let arabic_quran = get_quran("ar");
    let mut matches = Vec::new();

    let is_arabic_query = query.chars().all(|c| {
        let code = c as u32;
        c.is_whitespace()
            || is_arabic_ignorable(c)
            || (0x0600..=0x06FF).contains(&code)
            || (0x0750..=0x077F).contains(&code)
            || (0x08A0..=0x08FF).contains(&code)
            || (0xFB50..=0xFDFF).contains(&code)
            || (0xFE70..=0xFEFF).contains(&code)
    });

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

    for chapter in quran.iter() {
        for verse in chapter.verses.iter() {
            let matches_arabic = is_arabic_query
                && normalized_index
                    .as_ref()
                    .and_then(|idx| idx.get(&(chapter.id, verse.id)))
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
                let arabic_text = arabic_quran
                    .iter()
                    .find(|c| c.id == chapter.id)
                    .and_then(|ac| ac.verses.iter().find(|v| v.id == verse.id))
                    .map(|v| v.text.clone())
                    .unwrap_or_default();

                matches.push(VerseMatch {
                    surah_id: chapter.id,
                    verse_id: verse.id,
                    arabic_text,
                    translation_text: if lang == "ar" {
                        verse.text.clone()
                    } else {
                        translation_text
                    },
                    surah_name: chapter.name.clone(),
                    surah_translation: chapter.translation.clone(),
                });
            }
        }
    }

    matches.sort_by(|a, b| {
        let a_key = format!("{:03}{:04}", a.surah_id, a.verse_id);
        let b_key = format!("{:03}{:04}", b.surah_id, b.verse_id);
        a_key.cmp(&b_key)
    });

    matches
}

pub fn create_quran_page(
    current_lang: &str,
    view_stack: &adw::ViewStack,
    config: AppConfig,
) -> gtk::Widget {
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

    let supported_langs = ["en", "ar", "fr", "es", "tr", "id"];
    let quran_lang = if supported_langs.contains(&current_lang) {
        current_lang
    } else {
        "en"
    };
    let surah_list = get_surah_list(quran_lang);

    let list_box_rc: Rc<RefCell<ListBox>> = Rc::new(RefCell::new(list_box));

    let bookmarks_row = adw::ExpanderRow::new();
    bookmarks_row.set_widget_name("bookmarks_expander");
    bookmarks_row.set_title(&tr("Bookmarks"));
    bookmarks_row.set_expanded(false);
    let mut bookmarks = config.quran_bookmarks();
    if bookmarks.is_empty()
        && let (Some(surah), Some(page)) =
            (config.quran_bookmark_surah(), config.quran_bookmark_page())
    {
        bookmarks.push(QuranBookmark {
            page,
            surah,
            verse: 1,
        });
    }
    bookmarks.sort_by_key(|b| b.page);
    bookmarks.dedup_by_key(|b| b.page);
    let total = get_total_pages();
    for b in &bookmarks {
        let row = build_bookmark_row(b, quran_lang, total, view_stack, config.clone(), None);
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
        let list_box = search_list_box.borrow_mut();
        while let Some(child) = list_box.first_child() {
            list_box.remove(&child);
        }

        if query.is_empty() {
            for surah in search_surah_list.borrow().iter() {
                let row = build_surah_row_for_list(
                    surah,
                    &quran_lang_for_search,
                    &view_stack_for_search,
                    config_for_search.clone(),
                );
                list_box.append(&row);
            }
        } else {
            let quran_lang =
                if ["en", "ar", "fr", "es", "tr", "id"].contains(&quran_lang_for_search.as_str()) {
                    quran_lang_for_search.as_str()
                } else {
                    "en"
                };
            let verse_matches = search_quran(&query, quran_lang);

            let is_arabic_query = query.chars().all(|c| {
                let code = c as u32;
                c.is_whitespace()
                    || is_arabic_ignorable(c)
                    || (0x0600..=0x06FF).contains(&code)
                    || (0x0750..=0x077F).contains(&code)
                    || (0x08A0..=0x08FF).contains(&code)
                    || (0xFB50..=0xFDFF).contains(&code)
                    || (0xFE70..=0xFEFF).contains(&code)
            });

            let query_lower = if is_arabic_query {
                normalize_arabic(&query)
            } else {
                query.to_lowercase()
            };

            for verse_match in verse_matches.iter().take(50) {
                let row = build_verse_match_row(
                    verse_match,
                    quran_lang,
                    &view_stack_for_search,
                    config_for_search.clone(),
                );
                list_box.append(&row);
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
                    list_box.prepend(&row);
                }
            }
        }
    });

    container.upcast()
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
    if bookmarks.is_empty()
        && let (Some(surah), Some(page)) =
            (config.quran_bookmark_surah(), config.quran_bookmark_page())
    {
        bookmarks.push(QuranBookmark {
            page,
            surah,
            verse: 1,
        });
    }
    bookmarks.sort_by_key(|b| b.page);
    bookmarks.dedup_by_key(|b| b.page);
    let total = get_total_pages();
    for b in &bookmarks {
        let row = build_bookmark_row(b, quran_lang, total, view_stack, config.clone(), None);
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
    b: &QuranBookmark,
    lang: &str,
    total_pages: u32,
    view_stack: &adw::ViewStack,
    config: AppConfig,
    popover: Option<&gtk::Popover>,
) -> adw::ActionRow {
    let meta = surah_meta(b.surah, lang);
    let name = if lang == "ar" || meta.translated.trim().is_empty() {
        meta.arabic
    } else {
        meta.translated
    };
    let row = adw::ActionRow::new();
    row.set_title(&name);
    row.set_subtitle(&page_label_text(b.page, total_pages));
    row.set_activatable(true);
    row.set_selectable(false);
    let view_stack_row = view_stack.clone();
    let lang_row = lang.to_string();
    let surah_row = b.surah;
    let verse_row = b.verse;
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
        if let Some(ref p) = popover_opt {
            p.popdown();
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

    let subtitle = if surah.chapter_type == "meccan" {
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
    let visible = view_stack.visible_child_name().map(|s| s.to_string());
    let was_quran_related = visible
        .as_deref()
        .is_some_and(|n| n == "quran" || n.starts_with("surah_"));

    let quran_lang = if ["en", "ar", "fr", "es", "tr", "id"].contains(&lang) {
        lang
    } else {
        "en"
    };

    if let Some(quran_child) = view_stack.child_by_name("quran") {
        let list_box = find_widget_by_name(&quran_child, "surah_list_box");
        let search_entry = find_widget_by_name(&quran_child, "quran_search");

        if let Some(w) = search_entry
            && let Some(entry) = w.downcast_ref::<gtk::SearchEntry>()
        {
            entry.set_placeholder_text(Some(&tr("Search surahs")));
        }

        if let Some(w) = list_box
            && let Some(list) = w.downcast_ref::<gtk::ListBox>()
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
        for i in 0..pages.n_items() {
            if let Some(page_obj) = pages.item(i)
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
        && let Ok(surah) = rest.parse::<u32>()
    {
        let page = SURAH_READING_POSITIONS.with(|pos| pos.borrow().get(&surah).copied());
        let verse = page
            .and_then(get_page_verses)
            .and_then(|vs| {
                vs.into_iter()
                    .find(|pv| pv.surah == surah)
                    .map(|pv| pv.verse)
            })
            .unwrap_or(1);
        let surah_view = create_surah_view(
            surah,
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

pub fn open_last_read_or_list(view_stack: &adw::ViewStack, lang: &str, config: AppConfig) {
    if let (Some(surah), Some(page)) = (config.quran_last_surah(), config.quran_last_page()) {
        SURAH_READING_POSITIONS.with(|pos| pos.borrow_mut().insert(surah, page));
        let page_name = format!("surah_{}", surah);
        if let Some(old) = view_stack.child_by_name(&page_name) {
            view_stack.remove(&old);
        }
        let surah_view =
            create_surah_view(surah, lang, view_stack, None, None, None, config.clone());
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
        let s = start.max(0) as usize;
        let e = end.max(0) as usize;
        if s == e {
            return text;
        }
        let (a, b) = if s < e { (s, e) } else { (e, s) };
        let mut out = String::new();
        for (idx, ch) in text.chars().enumerate() {
            if idx >= a && idx < b {
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
            .and_then(|(surah, verse)| get_arabic_text(surah, verse))
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
    gesture.connect_pressed(move |g, _, x, y| {
        let parent = label_for_click
            .root()
            .and_then(|r| r.downcast::<gtk::Window>().ok())
            .map(|w| w.upcast::<gtk::Widget>())
            .unwrap_or_else(|| label_for_click.clone().upcast());

        if popover_for_click.parent().is_none() {
            popover_for_click.set_parent(&parent);
        }

        let (px, py) = label_for_click
            .translate_coordinates(&parent, x, y)
            .unwrap_or((x, y));

        popover_for_click
            .set_pointing_to(Some(&gtk::gdk::Rectangle::new(px as i32, py as i32, 1, 1)));
        popover_for_click.popup();
        g.set_state(gtk::EventSequenceState::Claimed);
    });
    label.add_controller(gesture);
}

fn find_widget_by_name(root: &gtk::Widget, name: &str) -> Option<gtk::Widget> {
    if root.widget_name() == name {
        return Some(root.clone());
    }
    let mut child = root.first_child();
    while let Some(w) = child {
        if let Some(found) = find_widget_by_name(&w, name) {
            return Some(found);
        }
        child = w.next_sibling();
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
            .map(|(surah, verse)| {
                let arabic = get_arabic_text(surah, verse).unwrap_or_default();
                let translation = get_verse(surah, verse, &lang_c)
                    .map(|v| v.translation)
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
    gesture.connect_pressed(move |g, _, x, y| {
        let parent = widget_for_click
            .root()
            .and_then(|r| r.downcast::<gtk::Window>().ok())
            .map(|w| w.upcast::<gtk::Widget>())
            .unwrap_or_else(|| widget_for_click.clone());

        if popover_for_click.parent().is_none() {
            popover_for_click.set_parent(&parent);
        }

        let (px, py) = widget_for_click
            .translate_coordinates(&parent, x, y)
            .unwrap_or((x, y));

        popover_for_click
            .set_pointing_to(Some(&gtk::gdk::Rectangle::new(px as i32, py as i32, 1, 1)));
        popover_for_click.popup();
        g.set_state(gtk::EventSequenceState::Claimed);
    });
    verse_box.add_controller(gesture);
}

fn to_arabic_indic(num: u32) -> String {
    num.to_string()
        .chars()
        .map(|c| match c {
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
            _ => c,
        })
        .collect()
}

const BISMILLAH: &str = "بِسْمِ ٱللَّهِ ٱلرَّحْمَـٰنِ ٱلرَّحِيمِ";

#[derive(Clone, Debug)]
struct SurahMeta {
    arabic: String,
    transliteration: String,
    translated: String,
    chapter_type: String,
}

fn surah_meta(surah: u32, lang: &str) -> SurahMeta {
    let mut meta = SurahMeta {
        arabic: String::new(),
        transliteration: String::new(),
        translated: String::new(),
        chapter_type: String::new(),
    };

    if let Some(info) = get_chapter_info()
        && let Some(c) = info.iter().find(|c| c.id == surah)
    {
        meta.arabic = c.name.clone();
        meta.transliteration = c.transliteration.clone();
        meta.chapter_type = c.chapter_type.clone();
    }

    if lang != "ar"
        && let Some(ch) = get_chapter(surah, lang)
    {
        meta.translated = ch.translation.clone();
        if meta.arabic.is_empty() {
            meta.arabic = ch.name.clone();
        }
    }

    meta
}

fn page_label_text(global_page: u32, total_pages: u32) -> String {
    format!("{} {} / {}", tr("page"), global_page, total_pages)
}

pub(crate) fn surah_total_verses(surah: u32) -> Option<u32> {
    get_chapter_info().and_then(|info| info.iter().find(|c| c.id == surah).map(|c| c.total_verses))
}

fn marker_id_for_page(
    page: u32,
    marker_index: &MarkerIndexU32,
    marker_list: &[MarkerPos],
) -> Option<u32> {
    let page_index = get_page_index()?;
    let verses = get_page_verses(page)?;

    let mut best_in_page: Option<u32> = None;
    for pv in &verses {
        if let Some(id) = marker_index.get(&(pv.surah, pv.verse)) {
            best_in_page = Some(best_in_page.map(|b| b.max(*id)).unwrap_or(*id));
        }
    }
    if best_in_page.is_some() {
        return best_in_page;
    }

    let start = page_index.page_starts.get(&page)?;
    let pos = (start.surah, start.verse);
    let mut best: Option<u32> = None;
    for m in marker_list {
        if (m.surah, m.verse) <= pos {
            best = Some(m.id);
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
    if let Some(j) = markers.juz {
        let n = if lang == "ar" {
            to_arabic_indic(j)
        } else {
            j.to_string()
        };
        parts.push(format!("{} {}", tr("Juz"), n));
    }
    if let Some(h) = markers.hizb {
        let n = if lang == "ar" {
            to_arabic_indic(h)
        } else {
            h.to_string()
        };
        parts.push(format!("{} {}", tr("Hizb"), n));
    }
    if let Some(q) = markers.quarter {
        let n = if lang == "ar" {
            to_arabic_indic(q)
        } else {
            q.to_string()
        };
        parts.push(format!("{} {}", tr("Quarter"), n));
    }

    for (idx, text) in parts.iter().enumerate() {
        if idx > 0 {
            let sep = gtk::Label::new(Some("•"));
            sep.add_css_class("dim-label");
            frame.append(&sep);
        }
        let l = gtk::Label::new(Some(text));
        l.set_wrap(true);
        l.set_xalign(0.5);
        l.add_css_class("dim-label");
        if lang == "ar" {
            l.add_css_class("arabic-text");
        }
        frame.append(&l);
    }
    frame.set_visible(true);
}

#[derive(Clone)]
struct RecitationState {
    selected_verse: Cell<Option<(u32, u32)>>,
    playing: Cell<bool>,
    stop_boundary: Cell<Option<(u32, u32)>>,
    current_playing_surah: Cell<u32>,
}

#[derive(Clone, Debug)]
struct VerseBoundary {
    surah: u32,
    verse: u32,
    byte_start: usize,
    byte_end: usize,
}

fn get_verse_display_len(content: &str, verse: u32) -> usize {
    content.len() + 7 + (to_arabic_indic(verse).chars().count() * 2)
}

fn compute_verse_boundaries(page: u32, _chapter: u32) -> Vec<VerseBoundary> {
    let mut bounds = Vec::new();
    let mut offset: usize = 0;
    if let Some(verses_data) = get_page_verses(page) {
        for pv in verses_data.iter() {
            let len = get_verse_display_len(&pv.content, pv.verse);
            bounds.push(VerseBoundary {
                surah: pv.surah,
                verse: pv.verse,
                byte_start: offset,
                byte_end: offset + len,
            });
            offset += len + 1;
        }
    }
    bounds
}

fn find_verse_at_offset(offset: usize, boundaries: &[VerseBoundary]) -> Option<(u32, u32)> {
    for b in boundaries {
        if offset >= b.byte_start && offset < b.byte_end {
            return Some((b.surah, b.verse));
        }
    }
    None
}

fn attach_arabic_verse_clicks(
    content: &gtk::Box,
    page: u32,
    chapter: u32,
    rec_state: Rc<RefCell<RecitationState>>,
    boundaries_cache: Rc<RefCell<HashMap<u32, Vec<VerseBoundary>>>>,
    rebuild_fn: RebuildFn,
    label_ranges: Rc<RefCell<Vec<(usize, usize)>>>,
) {
    if !boundaries_cache.borrow().contains_key(&page) {
        let b = compute_verse_boundaries(page, chapter);
        boundaries_cache.borrow_mut().insert(page, b);
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
) {
    let mut child = content.first_child();
    while let Some(widget) = child {
        if let Some(box_widget) = widget.downcast_ref::<gtk::Box>()
            && box_widget.has_css_class("quran-verse-box")
        {
            // SAFETY: qdata was set by us earlier — guaranteed valid u32 values.
            let surah = unsafe {
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
            click.connect_pressed(move |_, _, _, _| {
                rec_state_c
                    .borrow()
                    .selected_verse
                    .set(Some((surah, verse)));
                if let Some(ref rebuild) = *rebuild_fn_c.borrow() {
                    rebuild();
                }
            });
            widget.add_controller(click);
        }
        child = widget.next_sibling();
    }
}

fn compute_stop_boundary(surah: u32, verse: u32, stop: StopCondition) -> Option<(u32, u32)> {
    match stop {
        StopCondition::None => None,
        StopCondition::Ayah => Some((surah, verse)),
        StopCondition::Page => {
            let page = get_verse_page(surah, verse)?;
            let verses = get_page_verses(page)?;
            verses.last().map(|v| (v.surah, v.verse))
        }
        StopCondition::Juz => {
            let juz_index = get_juz_index();
            let current_juz = juz_index.get(&(surah, verse)).copied()?;
            let mut last = (surah, verse);
            for s in surah..=114 {
                let total = surah_total_verses(s).unwrap_or(u32::MAX);
                let start = if s == surah { verse + 1 } else { 1 };
                for v in start..=total {
                    match juz_index.get(&(s, v)) {
                        Some(&j) if j == current_juz => last = (s, v),
                        _ => return Some(last),
                    }
                }
            }
            Some(last)
        }
        StopCondition::Surah => Some((surah, surah_total_verses(surah)?)),
    }
}

fn next_verse_on_page_or_next(surah: u32, verse: u32) -> Option<(u32, u32)> {
    let page = get_verse_page(surah, verse)?;
    let verses = get_page_verses(page)?;
    let pos = verses.iter().position(|v| v.surah == surah && v.verse == verse)?;
    if let Some(next) = verses.get(pos + 1) {
        return Some((next.surah, next.verse));
    }
    let next_page = page + 1;
    let next_verses = get_page_verses(next_page)?;
    next_verses.first().map(|v| (v.surah, v.verse))
}

fn build_recitation_toolbar(
    rec_state: Rc<RefCell<RecitationState>>,
    config: &AppConfig,
    play_fn: PlayFn,
    chapter: u32,
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
        let rec_state = rec_state.clone();
        let prev_btn = prev_btn.clone();
        let next_btn = next_btn.clone();
        move || {
            let total = surah_total_verses(chapter).unwrap_or(u32::MAX);
            let prev_enabled = rec_state
                .borrow()
                .selected_verse
                .get()
                .is_some_and(|(_, v)| v > 1);
            let next_enabled = rec_state
                .borrow()
                .selected_verse
                .get()
                .is_some_and(|(_, v)| v < total);
            prev_btn.set_sensitive(prev_enabled);
            next_btn.set_sensitive(next_enabled);
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
        .position(|r| r.slug == current_slug);
    let reciter_box = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    let reciter_label = gtk::Label::new(Some(
        current_pos
            .and_then(|p| crate::reciter_ui::RECITERS.get(p))
            .map(|r| tr(r.display))
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
    let stop_refs: Vec<&str> = stop_items.iter().map(|s| s.as_str()).collect();
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
    stop_dropdown.connect_selected_notify(move |dd| {
        let cond = match dd.selected() {
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
    let play_chapter = chapter;
    play_btn.connect_clicked(move |btn| {
        let was_playing = play_rec_state.borrow().playing.get();
        let (surah, verse) = play_rec_state
            .borrow()
            .selected_verse
            .get()
            .unwrap_or((play_chapter, 1));
        if was_playing {
            play_rec_state.borrow().playing.set(false);
            crate::audio::stop();
            btn.set_icon_name("media-playback-start-symbolic");
        } else {
            if let Some(ref play) = *play_fn_c.borrow() {
                play(surah, verse);
            }
            btn.set_icon_name("media-playback-pause-symbolic");
        }
    });

    let prev_sync = sync_nav.clone();
    let prev_rec_state = rec_state.clone();
    let play_fn_c2 = play_fn.clone();
    let prev_play_btn = play_btn.clone();
    prev_btn.connect_clicked(move |_| {
        let v = prev_rec_state
            .borrow()
            .selected_verse
            .get()
            .filter(|&(_, v)| v > 1);
        if let Some((s, verse)) = v {
            prev_rec_state
                .borrow()
                .selected_verse
                .set(Some((s, verse - 1)));
            if let Some(ref play) = *play_fn_c2.borrow() {
                play(s, verse - 1);
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
        let v = next_rec_state.borrow().selected_verse.get();
        if let Some((s, verse)) = v {
            next_rec_state
                .borrow()
                .selected_verse
                .set(Some((s, verse + 1)));
            if let Some(ref play) = *play_fn_c3.borrow() {
                play(s, verse + 1);
            }
            next_play_btn.set_icon_name("media-playback-pause-symbolic");
        }
        next_sync();
    });

    (toolbar, play_btn_return, prev_btn, next_btn)
}

fn is_verse_on_page(surah: u32, verse: u32, page: u32) -> bool {
    if let Some(verses) = get_page_verses(page) {
        verses
            .iter()
            .any(|pv| pv.surah == surah && pv.verse == verse)
    } else {
        false
    }
}

fn get_page_for_verse(surah: u32, verse: u32) -> Option<u32> {
    (1..=get_total_pages()).find(|&p| is_verse_on_page(surah, verse, p))
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

#[allow(dead_code)]
pub fn create_surah_view(
    chapter: u32,
    lang: &str,
    view_stack: &adw::ViewStack,
    target_page: Option<u32>,
    scroll_to_verse: Option<u32>,
    highlight_verse: Option<u32>,
    config: AppConfig,
) -> gtk::Widget {
    let container = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let toast_overlay = adw::ToastOverlay::new();

    let supported_langs = ["en", "ar", "fr", "es", "tr", "id"];
    let quran_lang = if supported_langs.contains(&lang) {
        lang
    } else {
        "en"
    };

    let meta = surah_meta(chapter, quran_lang);
    let surah_arabic_name = if meta.arabic.is_empty() {
        format!("Surah {}", chapter)
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
    header_start.append(&back_btn);
    header_start.append(&start_btn);
    header_center.set_start_widget(Some(&header_start));

    let title_box = gtk::Box::new(gtk::Orientation::Vertical, 2);
    title_box.set_halign(gtk::Align::Center);
    if quran_lang == "ar" {
        let surah_title = gtk::Label::new(Some(&surah_arabic_name));
        surah_title.add_css_class("title-2");
        surah_title.add_css_class("quran-arabic");

        title_box.append(&surah_title);

        if !meta.chapter_type.trim().is_empty() {
            let typ = if meta.chapter_type.trim().eq_ignore_ascii_case("meccan") {
                tr("Meccan")
            } else {
                tr("Medinan")
            };
            let type_label = gtk::Label::new(Some(&typ));
            type_label.add_css_class("caption");
            type_label.add_css_class("quran-translation");
            type_label.set_margin_bottom(6);

            title_box.append(&type_label);
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

        title_box.append(&surah_title);

        let mut sub_parts = Vec::new();
        if !meta.transliteration.trim().is_empty() {
            sub_parts.push(meta.transliteration.trim().to_string());
        }
        if !meta.chapter_type.trim().is_empty() {
            let typ = if meta.chapter_type.trim().eq_ignore_ascii_case("meccan") {
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

    let quran_lang_arabic = quran_lang.to_string();
    let config_for_arabic = config.clone();
    arabic_adj.connect_value_changed(move |adj| {
        config_for_arabic.set_quran_arabic_font_px(adj.value());
        config_for_arabic.save();
        crate::apply_font_css(&quran_lang_arabic, &config_for_arabic);
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

    let quran_lang_trans = quran_lang.to_string();
    let config_for_trans = config.clone();
    trans_adj.connect_value_changed(move |adj| {
        config_for_trans.set_quran_translation_font_px(adj.value());
        config_for_trans.save();
        crate::apply_font_css(&quran_lang_trans, &config_for_trans);
    });

    let lh_adj = gtk::Adjustment::new(cfg_typo.quran_line_height(), 1.0, 3.0, 0.1, 0.0, 0.0);
    let lh_spin = adw::SpinRow::builder()
        .title(tr("Line Spacing"))
        .subtitle(tr("Multiplier (1.0–3.0)"))
        .adjustment(&lh_adj)
        .digits(1)
        .build();
    typo_group.add(&lh_spin);

    let quran_lang_lh = quran_lang.to_string();
    let config_for_lh = config.clone();
    lh_adj.connect_value_changed(move |adj| {
        config_for_lh.set_quran_line_height(adj.value());
        config_for_lh.save();
        crate::apply_font_css(&quran_lang_lh, &config_for_lh);
    });

    let reset_btn = gtk::Button::with_label(&tr("Reset to Default"));
    reset_btn.set_margin_top(8);
    reset_btn.set_margin_start(4);
    reset_btn.set_margin_end(4);
    let quran_lang_reset = quran_lang.to_string();
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
        AppConfig::save_shared(&config_for_reset);
        crate::apply_font_css(&quran_lang_reset, &config_for_reset);
    });
    typo_outer.append(&reset_btn);

    typo_popover.set_child(Some(&typo_outer));
    typography_btn.set_popover(Some(&typo_popover));

    let header_actions = gtk::Box::new(gtk::Orientation::Horizontal, 0);
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

    let surah_chapter = get_chapter(chapter, quran_lang);
    let nominal_start = get_surah_start_page(chapter).unwrap_or(1);
    let page_count = get_surah_page_count(chapter).unwrap_or(1);
    let total_pages = get_total_pages();
    let end_page = nominal_start.saturating_add(page_count).saturating_sub(1);
    let start_page = get_verse_page(chapter, 1).unwrap_or(nominal_start);

    let initial_page = if let Some(page) = target_page {
        page
    } else if let Some(verse) = scroll_to_verse {
        get_verse_page(chapter, verse).unwrap_or(start_page)
    } else {
        SURAH_READING_POSITIONS.with(|positions| {
            positions
                .borrow()
                .get(&chapter)
                .copied()
                .filter(|&p| p >= start_page && p <= end_page)
                .unwrap_or(start_page)
        })
    };
    SURAH_READING_POSITIONS.with(|positions| {
        positions.borrow_mut().insert(chapter, initial_page);
    });
    config.set_quran_last_surah(Some(chapter));
    config.set_quran_last_page(Some(initial_page));
    config.save();
    update_marker_frame(&marker_frame, initial_page, quran_lang);

    let current_page = Rc::new(RefCell::new(initial_page));
    let surah_chapter_rc = Rc::new(surah_chapter);
    let quran_lang_rc = Rc::new(quran_lang.to_string());
    let config_rc = config.clone();

    let rec_state = Rc::new(RefCell::new(RecitationState {
        selected_verse: Cell::new(None),
        playing: Cell::new(false),
        stop_boundary: Cell::new(None),
        current_playing_surah: Cell::new(0),
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
        chapter: u32,
        quran_lang: &str,
        surah_chapter: Option<TranslationChapter>,
        highlight_verse: Option<(u32, u32)>,
        rec_state: Rc<RefCell<RecitationState>>,
    ) -> (gtk::Box, Vec<(usize, usize)>) {
        let box_content = gtk::Box::new(gtk::Orientation::Vertical, 8);
        box_content.set_margin_top(12);
        box_content.set_margin_bottom(12);
        box_content.set_margin_start(12);
        box_content.set_margin_end(12);

        let mut label_ranges: Vec<(usize, usize)> = Vec::new();

        if let Some(verses_data) = get_page_verses(page) {
            let mut header_cache: HashMap<u32, SurahMeta> = HashMap::new();

            if quran_lang == "ar" {
                let mut last_surah: Option<u32> = None;
                let mut chunk_surah: Option<u32> = None;
                let mut chunk_text = String::new();
                let mut chunk_start = 0;

                for (i, pv) in verses_data.iter().enumerate() {
                    let surah_changed = last_surah.is_some() && last_surah != Some(pv.surah);

                    if surah_changed {
                        if !chunk_text.is_empty() {
                            label_ranges.push((chunk_start, i));
                            let mushaf_label = gtk::Label::new(None);
                            mushaf_label.set_markup(&chunk_text);
                            mushaf_label.set_wrap(true);
                            mushaf_label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
                            mushaf_label.set_selectable(true);
                            attach_arabic_context_menu(&mushaf_label, rec_state.clone());
                            if chapter == 1 && chunk_surah == Some(1) {
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
                        chunk_surah = None;
                    }

                    let is_surah_start = pv.verse == 1 && last_surah != Some(pv.surah);
                    if is_surah_start {
                        let meta = header_cache
                            .entry(pv.surah)
                            .or_insert_with(|| surah_meta(pv.surah, quran_lang))
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

                        if !meta.chapter_type.trim().is_empty() {
                            let typ = if meta.chapter_type.trim().eq_ignore_ascii_case("meccan") {
                                tr("Meccan")
                            } else {
                                tr("Medinan")
                            };
                            let typ_label = gtk::Label::new(Some(&typ));
                            typ_label.set_wrap(true);
                            typ_label.set_xalign(0.5);
                            typ_label.add_css_class("quran-translation");
                            typ_label.set_margin_bottom(6);
                            header_box.append(&typ_label);
                        }

                        box_content.append(&header_box);

                        if pv.surah != 1 && pv.surah != 9 {
                            let b = gtk::Label::new(Some(BISMILLAH));
                            b.set_wrap(true);
                            b.set_xalign(0.5);
                            b.set_justify(gtk::Justification::Center);
                            b.add_css_class("quran-arabic");
                            b.set_selectable(true);
                            attach_arabic_context_menu(&b, rec_state.clone());
                            b.set_margin_bottom(6);
                            box_content.append(&b);
                        }
                    }

                    if pv.surah == 1 && pv.verse == 1 {
                        label_ranges.push((i, i + 1));
                        let b = gtk::Label::new(None);
                        let content = if highlight_verse == Some((1, 1)) {
                            format!("<span underline='single' underline_color='#3584e4'>{}</span>", BISMILLAH)
                        } else {
                            BISMILLAH.to_string()
                        };
                        b.set_markup(&format!(
                            "{} <span size='small' color='gray'>﴿{}﴾</span>",
                            content,
                            to_arabic_indic(1)
                        ));
                        b.set_wrap(true);
                        b.set_xalign(0.5);
                        b.set_justify(gtk::Justification::Center);
                        b.add_css_class("quran-arabic");
                        b.add_css_class("quran-verse-block");
                        b.set_selectable(true);
                        attach_arabic_context_menu(&b, rec_state.clone());
                        b.set_margin_bottom(6);
                        box_content.append(&b);
                        last_surah = Some(pv.surah);
                        continue;
                    }

                    if chunk_surah.is_none() {
                        chunk_surah = Some(pv.surah);
                        chunk_start = i;
                    }

                    if !chunk_text.is_empty() {
                        chunk_text.push(' ');
                    }
                    let escaped = gtk::glib::markup_escape_text(&pv.content);
                    if highlight_verse == Some((pv.surah, pv.verse)) {
                        chunk_text.push_str(&format!(
                            "<span underline='single' underline_color='#3584e4'>{}</span>",
                            escaped
                        ));
                    } else {
                        chunk_text.push_str(&escaped);
                    }
                    let v = to_arabic_indic(pv.verse);
                    chunk_text.push_str(&format!(" ﴿{}﴾", v));

                    last_surah = Some(pv.surah);
                }

                if !chunk_text.is_empty() {
                    label_ranges.push((chunk_start, verses_data.len()));
                    let mushaf_label = gtk::Label::new(None);
                    mushaf_label.set_markup(&chunk_text);
                    mushaf_label.set_wrap(true);
                    mushaf_label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
                    mushaf_label.set_selectable(true);
                    attach_arabic_context_menu(&mushaf_label, rec_state.clone());
                    if chapter == 1 && chunk_surah == Some(1) {
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
                let mut last_surah: Option<u32> = None;
                let mut translation_cache: HashMap<u32, Option<TranslationChapter>> =
                    HashMap::new();

                for pv in verses_data.iter() {
                    let is_surah_start = pv.verse == 1 && last_surah != Some(pv.surah);
                    if is_surah_start {
                        let meta = header_cache
                            .entry(pv.surah)
                            .or_insert_with(|| surah_meta(pv.surah, quran_lang))
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

                        if !meta.chapter_type.trim().is_empty() {
                            let typ = if meta.chapter_type.trim().eq_ignore_ascii_case("meccan") {
                                tr("Meccan")
                            } else {
                                tr("Medinan")
                            };
                            let typ_label = gtk::Label::new(Some(&typ));
                            typ_label.set_wrap(true);
                            typ_label.set_xalign(0.5);
                            typ_label.add_css_class("quran-translation");
                            typ_label.set_margin_bottom(6);
                            header_box.append(&typ_label);
                        }

                        box_content.append(&header_box);

                        if pv.surah != 1 && pv.surah != 9 {
                            let bismillah_box = gtk::Box::new(gtk::Orientation::Vertical, 4);
                            bismillah_box.set_margin_bottom(6);

                            let arabic_label = gtk::Label::new(Some(BISMILLAH));
                            arabic_label.set_wrap(true);
                            arabic_label.set_xalign(0.5);
                            arabic_label.set_justify(gtk::Justification::Center);
                            arabic_label.add_css_class("quran-arabic");
                            arabic_label.set_selectable(true);
                            attach_arabic_context_menu(&arabic_label, rec_state.clone());
                            bismillah_box.append(&arabic_label);

                            if quran_lang != "ar" {
                                let bismillah_chapter = translation_cache
                                    .entry(1)
                                    .or_insert_with(|| get_chapter(1, quran_lang));
                                if let Some(ch) = bismillah_chapter.as_ref()
                                    && let Some(verse_data) = ch.verses.iter().find(|v| v.id == 1)
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
                    verse_box.set_widget_name(&format!("verse_{}_{}", pv.surah, pv.verse));
                    unsafe {
                        verse_box.set_qdata(glib::Quark::from_str("khushu-verse-surah"), pv.surah);
                        verse_box.set_qdata(glib::Quark::from_str("khushu-verse-verse"), pv.verse);
                    }

                    let arabic_label = gtk::Label::new(None);
                    let escaped = gtk::glib::markup_escape_text(&pv.content);
                    arabic_label.set_markup(&format!(
                        "{} <span size='small' color='gray'>﴿{}﴾</span>",
                        escaped, pv.verse
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

                    if highlight_verse == Some((pv.surah, pv.verse)) {
                        verse_box.add_css_class("quran-highlight");
                    }

                    let ch_entry = translation_cache
                        .entry(pv.surah)
                        .or_insert_with(|| get_chapter(pv.surah, quran_lang));
                    if let Some(ch) = ch_entry.as_ref() {
                        if let Some(verse_data) = ch.verses.iter().find(|v| v.id == pv.verse)
                            && !verse_data.translation.is_empty()
                        {
                            let translation_label = gtk::Label::new(None);
                            let t_escaped = gtk::glib::markup_escape_text(&verse_data.translation);
                            translation_label.set_markup(&format!(
                                "<span size='small' color='gray'>{}:{}</span>  {}",
                                pv.surah, pv.verse, t_escaped
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
                    } else if let Some(ch) = surah_chapter.as_ref()
                        && pv.surah == chapter
                        && let Some(verse_data) = ch.verses.iter().find(|v| v.id == pv.verse)
                        && !verse_data.translation.is_empty()
                    {
                        let translation_label = gtk::Label::new(None);
                        let t_escaped = gtk::glib::markup_escape_text(&verse_data.translation);
                        translation_label.set_markup(&format!(
                            "<span size='small' color='gray'>{}:{}</span>  {}",
                            pv.surah, pv.verse, t_escaped
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
                    last_surah = Some(pv.surah);
                }
            }
        }

        (box_content, label_ranges)
    }

    let content_stack = gtk::Box::new(gtk::Orientation::Vertical, 0);
    content_stack.set_vexpand(true);

    let scrolled = gtk::ScrolledWindow::builder().vexpand(true).build();
    let (initial_content, initial_label_ranges) = build_page_content(
        initial_page,
        chapter,
        quran_lang,
        (*surah_chapter_rc).clone(),
        highlight_verse.map(|v| (chapter, v)),
        rec_state.clone(),
    );
    *page_label_ranges.borrow_mut() = initial_label_ranges;
    *page_content_box.borrow_mut() = Some(initial_content.clone());
    scrolled.set_child(Some(&initial_content));
    content_stack.append(&scrolled);

    fn scroll_to_translation_verse(
        content: &gtk::Box,
        scrolled: &gtk::ScrolledWindow,
        surah: u32,
        verse: u32,
    ) {
        let target_name = format!("verse_{}_{}", surah, verse);
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
                    let t = target.clone();
                    let c = content_for_scroll.clone();
                    let s = tick_scrolled.clone();
                    glib::idle_add_local(move || {
                        if let Some((_, y)) = t.translate_coordinates(&c, 0.0, 0.0) {
                            let adj = s.vadjustment();
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
        chapter: u32,
        verse: u32,
        boundaries: &HashMap<u32, Vec<VerseBoundary>>,
        label_ranges: &[(usize, usize)],
        page: u32,
    ) {
        if let Some(boundaries_data) = boundaries.get(&page)
            && let Some(target_idx) = boundaries_data
                .iter()
                .position(|b| b.surah == chapter && b.verse == verse)
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
                        let l = label.clone();
                        let s = tick_scrolled.clone();
                        let c = content_widget.clone();
                        glib::idle_add_local(move || {
                            let layout = l.layout();
                            let rect = layout.index_to_pos(offset as i32);
                            if let Some((_, label_y)) = l.translate_coordinates(&c, 0.0, 0.0) {
                                let y_pixels =
                                    label_y + rect.y() as f64 / f64::from(gtk::pango::SCALE);
                                let adj = s.vadjustment();
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
            let b = compute_verse_boundaries(initial_page, chapter);
            verse_boundaries.borrow_mut().insert(initial_page, b);
        }
        if quran_lang != "ar" {
            scroll_to_translation_verse(&initial_content, &scrolled, chapter, verse);
        } else {
            scroll_to_arabic_verse(
                &initial_content,
                &scrolled,
                chapter,
                verse,
                &verse_boundaries.borrow(),
                &page_label_ranges.borrow(),
                initial_page,
            );
        }
    }

    if highlight_verse.is_some() {
        let rebuild_fn_for_hl = rebuild_fn.clone();
        gtk::glib::timeout_add_local(std::time::Duration::from_millis(5000), move || {
            if let Some(ref rebuild) = *rebuild_fn_for_hl.borrow() {
                rebuild();
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
    prev_btn.set_sensitive(initial_page > 1 || chapter > 1);

    let next_btn = gtk::Button::new();
    next_btn.set_icon_name("go-next-symbolic");
    next_btn.set_sensitive(initial_page < total_pages || chapter < 114);

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

    let rebuild_scrolled = scrolled.clone();
    let rebuild_chapter = chapter;
    let rebuild_lang = quran_lang_rc.clone();
    let rebuild_surah = surah_chapter_rc.clone();
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
    *rebuild_fn.borrow_mut() = Some(Box::new(move || {
        let page = *rebuild_current_page.borrow();
        let verse = rebuild_rec_state.borrow().selected_verse.get();
        let highlight = verse.filter(|(s, v)| is_verse_on_page(*s, *v, page));
        let (new_content, new_label_ranges) = build_page_content(
            page,
            rebuild_chapter,
            &rebuild_lang,
            (*rebuild_surah).clone(),
            highlight,
            rebuild_rec_state.clone(),
        );
        *rebuild_label_ranges.borrow_mut() = new_label_ranges;
        *page_content_box.borrow_mut() = Some(new_content.clone());
        rebuild_scrolled.set_child(Some(&new_content));
        if *rebuild_lang == "ar" {
            attach_arabic_verse_clicks(
                &new_content,
                page,
                rebuild_chapter,
                rebuild_rec_state.clone(),
                rebuild_bounds.clone(),
                rebuild_follow_fn_c.clone(),
                rebuild_label_ranges.clone(),
            );
        } else {
            attach_translation_verse_clicks(
                &new_content,
                rebuild_rec_state.clone(),
                rebuild_follow_fn_c.clone(),
            );
        }
        update_marker_frame(&rebuild_marker, page, &rebuild_lang);
        gtk::prelude::EditableExt::set_text(&rebuild_entry, &page.to_string());
        rebuild_prev_btn.set_sensitive(page > 1 || rebuild_chapter > 1);
        rebuild_next_btn.set_sensitive(page < rebuild_total_pages || rebuild_chapter < 114);
        SURAH_READING_POSITIONS.with(|pos| pos.borrow_mut().insert(rebuild_chapter, page));
        rebuild_config.set_quran_last_surah(Some(rebuild_chapter));
        rebuild_config.set_quran_last_page(Some(page));
        rebuild_config.save();
    }));

    *rebuild_follow_fn.borrow_mut() = Some(Box::new(move || {
        let page = *rebuild_current_page_for_follow.borrow();
        let verse = rebuild_rec_state_for_follow.borrow().selected_verse.get();
        if let Some((s, v)) = verse
            && !is_verse_on_page(s, v, page)
            && let Some(fp) = get_page_for_verse(s, v)
        {
            *rebuild_current_page_for_follow.borrow_mut() = fp;
        }
        if let Some(ref rebuild) = *rebuild_fn_for_follow.borrow() {
            rebuild();
        }
        if let Some((s, v)) = verse {
            let new_page = *rebuild_current_page_for_follow.borrow();
            if let Some(content) = page_content_box_for_follow.borrow().clone() {
                let cb_scrolled = rebuild_scrolled_for_follow.clone();
                if *rebuild_lang_for_follow != "ar" {
                    scroll_to_translation_verse(&content, &cb_scrolled, s, v);
                } else {
                    let cb_bounds = rebuild_bounds_for_follow.clone();
                    let cb_label_ranges = page_label_ranges_for_follow.clone();
                    scroll_to_arabic_verse(
                        &content,
                        &cb_scrolled,
                        s,
                        v,
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
    *play_fn.borrow_mut() = Some(Box::new(move |surah, verse| {
        log::info!("Play fn called: surah={}, verse={}", surah, verse);
        play_rec_state.borrow().playing.set(true);
        play_rec_state
            .borrow()
            .selected_verse
            .set(Some((surah, verse)));
        play_rec_state.borrow().current_playing_surah.set(surah);
        if let Some(ref rebuild) = *play_rebuild.borrow() {
            rebuild();
        }
        let slug = play_config.reciter_slug();
        log::info!(
            "Playing verse: slug={}, surah={}, verse={}",
            slug,
            surah,
            verse,
        );
        let stop = play_config.stop_condition();
        let boundary = compute_stop_boundary(surah, verse, stop);
        play_rec_state.borrow().stop_boundary.set(boundary);
        match stop {
            StopCondition::Ayah => {
                crate::audio::play_verse(&slug, surah, verse);
            }
            StopCondition::None => {
                let total = surah_total_verses(surah).unwrap_or(u32::MAX);
                crate::audio::play_surah(&slug, surah, verse, total);
            }
            _ => {
                if let Some((end_surah, end_verse)) = boundary {
                    let end = if end_surah == surah {
                        end_verse
                    } else {
                        surah_total_verses(surah).unwrap_or(u32::MAX)
                    };
                    crate::audio::play_surah(&slug, surah, verse, end);
                } else {
                    let total = surah_total_verses(surah).unwrap_or(u32::MAX);
                    crate::audio::play_surah(&slug, surah, verse, total);
                }
            }
        }
    }));

    let (toolbar, rec_play_btn, rec_prev_btn, rec_next_btn) =
        build_recitation_toolbar(rec_state.clone(), &config_rc, play_fn.clone(), chapter);
    content_stack.insert_child_after(&toolbar, Some(&scrolled));

    if quran_lang == "ar" {
        attach_arabic_verse_clicks(
            &initial_content,
            initial_page,
            chapter,
            rec_state.clone(),
            verse_boundaries.clone(),
            rebuild_fn.clone(),
            page_label_ranges.clone(),
        );
    } else {
        attach_translation_verse_clicks(&initial_content, rec_state.clone(), rebuild_fn.clone());
    }

    let poll_rec_state = rec_state.clone();
    let poll_rebuild = rebuild_follow_fn.clone();
    let poll_play_btn = rec_play_btn.clone();
    let poll_prev_btn = rec_prev_btn.clone();
    let poll_next_btn = rec_next_btn.clone();
    let poll_play_fn = play_fn.clone();
    glib::timeout_add_local(Duration::from_millis(200), move || {
        poll_play_btn.set_icon_name(if crate::audio::is_playing() {
            "media-playback-pause-symbolic"
        } else {
            "media-playback-start-symbolic"
        });

        {
            let state = poll_rec_state.borrow();
            let total = state
                .selected_verse
                .get()
                .map(|(s, _)| surah_total_verses(s).unwrap_or(u32::MAX))
                .unwrap_or(u32::MAX);
            poll_prev_btn.set_sensitive(
                state
                    .selected_verse
                    .get()
                    .is_some_and(|(_, v)| v > 1),
            );
            poll_next_btn.set_sensitive(
                state
                    .selected_verse
                    .get()
                    .is_some_and(|(_, v)| v < total),
            );
        }

        if let Some((surah, verse)) = crate::audio::poll_verse_finished() {
            let playing_surah = poll_rec_state.borrow().current_playing_surah.get();
            if surah == playing_surah {
                let currently_playing = poll_rec_state.borrow().playing.get();
                if currently_playing {
                    if crate::audio::is_playing() {
                        let next_verse = verse + 1;
                        poll_rec_state
                            .borrow()
                            .selected_verse
                            .set(Some((surah, next_verse)));
                        if let Some(ref rebuild) = *poll_rebuild.borrow() {
                            rebuild();
                        }
                    } else {
                        let current = (surah, verse);
                        let boundary = poll_rec_state.borrow().stop_boundary.get();
                        if boundary.is_some_and(|b| current >= b) {
                            poll_rec_state.borrow().playing.set(false);
                        } else if let Some((ns, nv)) =
                            next_verse_on_page_or_next(surah, verse)
                        {
                            if let Some(ref play_fn) = *poll_play_fn.borrow() {
                                play_fn(ns, nv);
                            }
                        } else {
                            poll_rec_state.borrow().playing.set(false);
                        }
                    }
                }
            }
        } else {
            let currently_playing = poll_rec_state.borrow().playing.get();
            if !crate::audio::is_playing() && currently_playing {
                let selected = poll_rec_state.borrow().selected_verse.get();
                if let Some((surah, verse)) = selected {
                    let boundary = poll_rec_state.borrow().stop_boundary.get();
                    if boundary.is_none_or(|b| (surah, verse) < b)
                        && let Some((ns, nv)) =
                            next_verse_on_page_or_next(surah, verse)
                        {
                            if let Some(ref play_fn) = *poll_play_fn.borrow() {
                                play_fn(ns, nv);
                            }
                            return glib::ControlFlow::Continue;
                        }
                }
                poll_rec_state.borrow().playing.set(false);
            }
        }
        glib::ControlFlow::Continue
    });

    let scrolled_for_prev = scrolled.clone();
    let _page_entry_for_prev = page_entry.clone();
    let current_page_for_prev = current_page.clone();
    let lang_for_prev = quran_lang_rc.clone();
    let view_stack_for_prev = view_stack.clone();
    let config_for_prev = config_rc.clone();
    let rebuild_fn_for_prev = rebuild_fn.clone();

    fn navigate_to_surah(
        target_chapter: u32,
        target_page: Option<u32>,
        scroll_verse: Option<u32>,
        vs: &adw::ViewStack,
        cfg: AppConfig,
        lang: &str,
    ) {
        let page_name = format!("surah_{}", target_chapter);
        if let Some(existing) = vs.child_by_name(&page_name) {
            if target_page.is_some() {
                vs.remove(&existing);
                let surah_view = create_surah_view(
                    target_chapter,
                    lang,
                    vs,
                    target_page,
                    scroll_verse,
                    None,
                    cfg,
                );
                surah_view.set_vexpand(true);
                vs.add_named(&surah_view, Some(&page_name));
                vs.set_visible_child_name(&page_name);
            } else {
                vs.set_visible_child(&existing);
            }
        } else {
            let surah_view = create_surah_view(
                target_chapter,
                lang,
                vs,
                target_page,
                scroll_verse,
                None,
                cfg,
            );
            surah_view.set_vexpand(true);
            vs.add_named(&surah_view, Some(&page_name));
            vs.set_visible_child_name(&page_name);
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
            } else if chapter > 1 {
                let target_surah = get_page_index()
                    .and_then(|idx| idx.page_starts.get(&new_page).map(|s| s.surah))
                    .unwrap_or(chapter - 1);
                navigate_to_surah(
                    target_surah,
                    Some(new_page),
                    None,
                    &view_stack_for_prev,
                    config_for_prev.clone(),
                    &lang_for_prev,
                );
            }
        } else if chapter > 1 {
            navigate_to_surah(
                chapter - 1,
                None,
                None,
                &view_stack_for_prev,
                config_for_prev.clone(),
                &lang_for_prev,
            );
        }
    });

    let scrolled_for_next = scrolled.clone();
    let _page_entry_for_next = page_entry.clone();
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
            } else if chapter < 114 {
                let target_surah = get_page_index()
                    .and_then(|idx| idx.page_starts.get(&new_page).map(|s| s.surah))
                    .unwrap_or(chapter + 1);
                navigate_to_surah(
                    target_surah,
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
            for i in 0..pages.n_items() {
                if let Some(page_obj) = pages.item(i)
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
        config.quran_bookmarks().iter().any(|b| b.page == page)
            || config.quran_bookmark_page() == Some(page)
    }

    fn bookmark_for_page(page: u32) -> QuranBookmark {
        if let Some(idx) = get_page_index()
            && let Some(start) = idx.page_starts.get(&page)
        {
            return QuranBookmark {
                page,
                surah: start.surah,
                verse: start.verse,
            };
        }
        QuranBookmark {
            page,
            surah: 1,
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
        let p = *current_page_for_bm.borrow();
        let is_bookmarked = is_page_bookmarked(&config_for_toggle, p);
        if is_bookmarked {
            let mut bookmarks = config_for_toggle.quran_bookmarks();
            bookmarks.retain(|b| b.page != p);
            config_for_toggle.set_quran_bookmarks(bookmarks);
            if config_for_toggle.quran_bookmark_page() == Some(p) {
                config_for_toggle.set_quran_bookmark_page(None);
                config_for_toggle.set_quran_bookmark_surah(None);
            }
        } else {
            let mut bookmarks = config_for_toggle.quran_bookmarks();
            bookmarks.push(bookmark_for_page(p));
            bookmarks.sort_by_key(|b| b.page);
            bookmarks.dedup_by_key(|b| b.page);
            config_for_toggle.set_quran_bookmarks(bookmarks);
        }
        AppConfig::save_shared(&config_for_toggle);
        set_bookmark_state(btn, p, &config_for_toggle);
        set_bookmark_state(&bookmark_btn_for_toggle_in_toggle, p, &config_for_toggle);
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
        if bookmarks.is_empty()
            && let (Some(surah), Some(page)) = (
                config_for_popover.quran_bookmark_surah(),
                config_for_popover.quran_bookmark_page(),
            )
        {
            bookmarks.push(QuranBookmark {
                page,
                surah,
                verse: 1,
            });
        }
        bookmarks.sort_by_key(|b| b.page);
        bookmarks.dedup_by_key(|b| b.page);

        for b in &bookmarks {
            let row = build_bookmark_row(
                b,
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

        let p = *current_page_for_popover.borrow();
        set_bookmark_state(&bookmark_btn_for_toggle_in_popover, p, &config_for_popover);
        bookmarks_popover.popup();
    });

    let current_page_for_input = current_page.clone();
    let lang_for_input = quran_lang_rc.clone();
    let toast_overlay_for_input = toast_overlay.clone();
    let view_stack_for_input = view_stack.clone();
    let config_for_input = config_rc.clone();
    gtk::prelude::EntryExt::connect_activate(&page_entry, move |e| {
        let text = gtk::prelude::EditableExt::text(e).trim().to_string();
        let Ok(page) = text.parse::<u32>() else {
            gtk::prelude::EditableExt::set_text(e, &current_page_for_input.borrow().to_string());
            toast_overlay_for_input.add_toast(adw::Toast::new(&tr("Invalid page number")));
            return;
        };
        if page < 1 || page > total_pages {
            gtk::prelude::EditableExt::set_text(e, &current_page_for_input.borrow().to_string());
            toast_overlay_for_input.add_toast(adw::Toast::new(&tr("Invalid page number")));
            return;
        }

        let Some(idx) = get_page_index() else {
            return;
        };
        let Some(start) = idx.page_starts.get(&page) else {
            return;
        };
        let target_surah = start.surah;
        let target_verse = start.verse;

        let page_name = format!("surah_{}", target_surah);
        if let Some(old) = view_stack_for_input.child_by_name(&page_name) {
            view_stack_for_input.remove(&old);
        }
        let surah_view = create_surah_view(
            target_surah,
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
            let mut last_surah: Option<u32> = None;
            for pv in verses {
                if let Some(prev) = last_surah
                    && prev != pv.surah
                    && pv.verse == 1
                {
                    out.push((page, prev, pv.surah));
                }
                last_surah = Some(pv.surah);
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
        let mut last_surah: Option<u32> = None;

        for pv in verses.iter() {
            let is_surah_start = pv.verse == 1 && last_surah != Some(pv.surah);
            if is_surah_start {
                markers.push((Some(pv.surah), None, 0, 0));
                if pv.surah != 1 && pv.surah != 9 {
                    markers.push((None, Some(pv.surah), 0, 0));
                }
            }
            markers.push((None, None, pv.surah, pv.verse));
            last_surah = Some(pv.surah);
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
            .find(|c| c.id == 1)
            .and_then(|c| c.verses.iter().find(|v| v.id == 1))
            .map(|v| v.text.as_str())
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
                .find(|c| c.id == 1)
                .and_then(|c| c.verses.iter().find(|v| v.id == 1));
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
        for surah in 1..=114 {
            let Some(page) = get_surah_start_page(surah) else {
                panic!("missing start page for surah {}", surah);
            };
            let verses = get_page_verses(page).expect("missing page verses");
            assert!(
                verses.iter().any(|pv| pv.surah == surah),
                "surah {} not found on reported start page {}",
                surah,
                page
            );
        }
    }

    #[test]
    fn shared_page_surah_starts_have_header_and_bismillah() {
        let shared = shared_page_surah_starts();
        assert!(!shared.is_empty());
        for (page, _prev_surah, new_surah) in shared {
            let markers = page_markers(page);

            let verse_idx = markers
                .iter()
                .position(|m| m.2 == new_surah && m.3 == 1)
                .expect("missing new surah verse 1");

            let header_present = markers[..verse_idx]
                .iter()
                .rev()
                .any(|m| m.0 == Some(new_surah));
            assert!(
                header_present,
                "missing header for surah {} on page {}",
                new_surah, page
            );

            if new_surah != 9 && new_surah != 1 {
                let bismillah_present = markers[..verse_idx]
                    .iter()
                    .rev()
                    .any(|m| m.1 == Some(new_surah));
                assert!(
                    bismillah_present,
                    "missing bismillah for surah {} on page {}",
                    new_surah, page
                );
            }
        }
    }

    #[test]
    fn in_page_header_when_surah_starts_at_top_of_page() {
        ensure_resources();
        for surah in 1..=114 {
            let Some(page) = get_surah_start_page(surah) else {
                continue;
            };
            let Some(verses) = get_page_verses(page) else {
                continue;
            };
            let Some(first) = verses.first() else {
                continue;
            };
            if first.surah == surah && first.verse == 1 {
                let markers = page_markers(page);
                let verse_idx = markers
                    .iter()
                    .position(|m| m.2 == surah && m.3 == 1)
                    .expect("missing verse 1");
                let header_present = markers[..verse_idx].iter().any(|m| m.0 == Some(surah));
                assert!(
                    header_present,
                    "missing in-page header for surah {} on its own start page {}",
                    surah, page
                );
            }
        }
    }

    #[test]
    fn page_indicator_is_global_only() {
        let s = page_label_text(106, 604);
        assert!(!s.contains("•"));
    }

    #[test]
    fn shared_page_531_verse_integrity() {
        ensure_resources();
        let verses = get_page_verses(531).expect("page 531 should exist");
        assert!(verses.iter().any(|v| v.surah == 55 && v.verse == 1),
            "page 531 should contain 55:1");
        assert!(is_verse_on_page(55, 1, 531),
            "55:1 should be on page 531");
        assert_eq!(get_page_for_verse(55, 1), Some(531),
            "get_page_for_verse(55,1) should return 531");

        let bounds = compute_verse_boundaries(531, 54);
        let offset_55v1 = bounds.iter()
            .find(|b| b.surah == 55 && b.verse == 1)
            .map(|b| b.byte_start)
            .expect("55:1 must have a byte_start");
        assert_eq!(find_verse_at_offset(offset_55v1, &bounds), Some((55, 1)));
    }

    #[test]
    fn all_shared_pages_have_correct_get_page_for_verse() {
        ensure_resources();
        for (page, _prev, new_surah) in shared_page_surah_starts() {
            assert_eq!(get_page_for_verse(new_surah, 1), Some(page),
                "get_page_for_verse({},1) should be page {}", new_surah, page);
        }
    }
}
