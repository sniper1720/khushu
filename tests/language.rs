//! Language integration tests for Khushu

use std::fs;
use std::path::Path;

fn assert_po_file_contains_keys(po_path: &Path, keys: &[&str]) {
    assert!(po_path.exists(), "PO file does not exist: {:?}", po_path);
    let content = fs::read_to_string(po_path)
        .unwrap_or_else(|_| panic!("Failed to read PO file: {:?}", po_path));
    for key in keys {
        assert!(
            content.contains(&format!("msgid \"{}\"", key)),
            "PO file {:?} does not contain translation for '{}'\nFile content:\n{}",
            po_path,
            key,
            content
        );
    }
}

fn assert_file_contains_patterns(file_path: &Path, patterns: &[&str]) {
    assert!(file_path.exists(), "File does not exist: {:?}", file_path);
    let content = fs::read_to_string(file_path)
        .unwrap_or_else(|_| panic!("Failed to read file: {:?}", file_path));
    for pattern in patterns {
        assert!(
            content.contains(pattern),
            "File {:?} does not contain pattern: {}\nFile content:\n{}",
            file_path,
            pattern,
            content
        );
    }
}

#[test]
fn test_arabic_translations_exist_in_po_file() {
    let arabic_po = Path::new("po/ar.po");
    let required_keys = ["Open Khushu", "Quit"];
    assert_po_file_contains_keys(arabic_po, &required_keys);
}

#[test]
fn test_default_arabic_font_includes_amiri() {
    let config_file = Path::new("src/config.rs");
    let patterns = [
        "fn default_global_arabic_font_family()",
        "\"Amiri, Noto Sans Arabic\"",
        "global_arabic_font_family: default_global_arabic_font_family()",
    ];
    assert_file_contains_patterns(config_file, &patterns);
}

#[test]
fn test_css_generation_for_arabic_language() {
    let main_file = Path::new("src/main.rs");
    let patterns = [
        "generate_font_css",
        "lang == \"ar\"",
        "arabic_font",
        "window, popover.background",
        "popover.background list row label",
        ".arabic-text",
        ".quran-arabic",
    ];
    let content = fs::read_to_string(main_file).expect("Failed to read main.rs");
    for pattern in patterns {
        assert!(
            content.contains(pattern),
            "main.rs should contain pattern: {}",
            pattern
        );
    }
}

#[test]
fn test_arabic_locale_special_handling() {
    let i18n_file = Path::new("src/i18n.rs");
    let patterns = [
        "fn locale_candidates",
        "fn update_locale_internal",
        "fn detect_system_locale",
    ];
    assert_file_contains_patterns(i18n_file, &patterns);
}

#[test]
fn test_rtl_direction_pattern_in_codebase() {
    let rtl_patterns = ["set_default_direction", "TextDirection::Rtl", "== \"ar\""];
    let files = ["src/main.rs", "src/pages.rs", "src/welcome.rs"];
    for file in files {
        let path = Path::new(file);
        let content =
            fs::read_to_string(path).unwrap_or_else(|_| panic!("Failed to read {:?}", path));
        let mut found = false;
        for pattern in &rtl_patterns {
            if content.contains(pattern) {
                found = true;
                break;
            }
        }
        assert!(
            found,
            "File {:?} does not contain any RTL direction patterns for Arabic",
            path
        );
    }
}

#[test]
fn test_tray_label_update_functionality() {
    let background_file = Path::new("src/background.rs");
    let patterns = [
        "pub fn update_tray_labels",
        "tr(\"Open Khushu\"",
        "tr(\"Quit\"",
    ];
    assert_file_contains_patterns(background_file, &patterns);
    let i18n_file = Path::new("src/i18n.rs");
    let i18n_content = fs::read_to_string(i18n_file).expect("Failed to read i18n.rs");
    assert!(
        i18n_content.contains("update_tray_labels"),
        "i18n should call update_tray_labels"
    );
}

#[test]
fn test_arabic_locale_handling_with_language_env() {
    let i18n_file = Path::new("src/i18n.rs");
    let content = fs::read_to_string(i18n_file).expect("Failed to read i18n.rs");
    assert!(
        content.contains("LANGUAGE"),
        "i18n should use LANGUAGE env var for locale switching"
    );
    assert!(
        content.contains("fn locale_candidates"),
        "i18n should have locale_candidates for domain discovery"
    );
}

#[test]
fn test_comprehensive_arabic_support() {
    let translation_files = ["po/ar.po", "po/es.po", "po/fr.po", "po/tr.po"];
    for file in &translation_files {
        let path = Path::new(file);
        assert!(path.exists(), "Translation file does not exist: {:?}", path);
    }

    let arabic_po = Path::new("po/ar.po");
    let required_keys = ["Open Khushu", "Quit"];
    assert_po_file_contains_keys(arabic_po, &required_keys);

    let source_files = [
        "src/config.rs",
        "src/main.rs",
        "src/i18n.rs",
        "src/background.rs",
        "src/pages.rs",
        "src/welcome.rs",
    ];
    for file in &source_files {
        let path = Path::new(file);
        assert!(path.exists(), "Source file does not exist: {:?}", path);
    }

    let config_content =
        fs::read_to_string(Path::new("src/config.rs")).expect("Failed to read config.rs");
    assert!(
        config_content.contains("\"Amiri, Noto Sans Arabic\""),
        "Default Arabic font family should be 'Amiri, Noto Sans Arabic'"
    );

    let rtl_files = ["src/main.rs", "src/pages.rs", "src/welcome.rs"];
    let rtl_patterns = ["set_default_direction", "TextDirection::Rtl", "== \"ar\""];
    let mut found_rtl = false;
    for file in &rtl_files {
        let content = fs::read_to_string(Path::new(file))
            .unwrap_or_else(|_| panic!("Failed to read {}", file));
        if rtl_patterns.iter().any(|p| content.contains(p)) {
            found_rtl = true;
            break;
        }
    }
    assert!(
        found_rtl,
        "No source files contain RTL direction patterns for Arabic"
    );

    println!("Arabic support configuration checks passed.");
    println!("Note: Actual font rendering and RTL layout require GTK runtime.");
    println!("Run application with LANGUAGE=ar to verify complete functionality.");
}

#[test]
fn test_css_generation_includes_arabic_font_families() {
    let main_file = Path::new("src/main.rs");
    let content = fs::read_to_string(main_file).expect("Failed to read main.rs");

    assert!(
        content.contains("font-family:"),
        "CSS generation should include font-family rules"
    );

    assert!(
        content.contains("'Amiri Quran'"),
        "CSS should include Amiri Quran font for Quran text"
    );

    assert!(
        content.contains(".combo list row label"),
        "CSS should apply Arabic font to combo box items"
    );
}

#[test]
fn test_i18n_tray_integration() {
    let i18n_file = Path::new("src/i18n.rs");
    let content = fs::read_to_string(i18n_file).expect("Failed to read i18n.rs");

    assert!(
        content.contains("update_tray_labels()"),
        "i18n should call update_tray_labels when language changes"
    );

    assert!(
        content.contains("fn detect_system_locale"),
        "i18n should have detect_system_locale function"
    );

    assert!(
        content.contains("set_locale_env"),
        "i18n should set LANGUAGE env var for locale switching"
    );
}
