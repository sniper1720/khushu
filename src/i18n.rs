use gettextrs::{LocaleCategory, bind_textdomain_codeset, bindtextdomain, dgettext, setlocale};
use std::sync::{Mutex, OnceLock, RwLock};

static ORIGINAL_LANGUAGE: OnceLock<String> = OnceLock::new();
static CURRENT_APP_LOCALE: OnceLock<RwLock<String>> = OnceLock::new();

static LOCALE_MODIFY: Mutex<()> = Mutex::new(());

pub fn save_original_locale() {
    ORIGINAL_LANGUAGE.get_or_init(|| std::env::var("LANGUAGE").unwrap_or_default());
}

fn bind_domains(language: &str) {
    let locale_dir = get_locale_dir();
    let pkg = option_env!("GETTEXT_PACKAGE").unwrap_or("khushu");
    let _ = bindtextdomain(pkg, &locale_dir);
    let _ = bind_textdomain_codeset(pkg, "UTF-8");
    bind_library_domains(&locale_dir, language);
}

fn set_locale_env(language: &str) {
    let _guard = LOCALE_MODIFY.lock().expect("LOCALE_MODIFY poisoned");

    if language == "auto" || language.is_empty() {
        match ORIGINAL_LANGUAGE.get() {
            Some(orig) if !orig.is_empty() => unsafe { std::env::set_var("LANGUAGE", orig) },
            _ => unsafe { std::env::remove_var("LANGUAGE") },
        }
    } else {
        unsafe { std::env::set_var("LANGUAGE", language) };
    }

    setlocale(LocaleCategory::LcAll, "");
}

pub fn get_locale_dir() -> String {
    if let Some(dir) = option_env!("LOCALEDIR")
        && std::path::Path::new(dir).exists()
    {
        return dir.to_string();
    }

    if std::path::Path::new("/app/share/locale").exists() {
        return "/app/share/locale".to_string();
    }

    if let Ok(snap) = std::env::var("SNAP") {
        let app_locale = format!("{}/usr/share/khushu/locale", snap);
        if std::path::Path::new(&app_locale).exists() {
            return app_locale;
        }
        return format!("{}/usr/share/locale", snap);
    }

    if let Ok(canon) = std::fs::canonicalize("target/locale") {
        return canon.to_string_lossy().to_string();
    }

    if std::path::Path::new("po").exists() {
        return "po".to_string();
    }

    "./po".to_string()
}

fn current_language_hint() -> String {
    if let Some(lock) = CURRENT_APP_LOCALE.get()
        && let Ok(current) = lock.read()
        && !current.is_empty()
        && current.as_str() != "auto"
    {
        return current.clone();
    }

    detect_system_locale()
}

fn locale_candidates(language: &str) -> Vec<String> {
    let normalized = language
        .split(':')
        .next()
        .unwrap_or_default()
        .split('@')
        .next()
        .unwrap_or_default()
        .split('.')
        .next()
        .unwrap_or_default()
        .trim()
        .to_string();

    if normalized.is_empty() {
        return vec!["en".to_string()];
    }

    let mut candidates = vec![normalized.clone()];
    if normalized.contains('_') {
        candidates.push(normalized.replace('_', "-"));
    } else if normalized.contains('-') {
        candidates.push(normalized.replace('-', "_"));
    }
    if let Some(base) = normalized.split(['-', '_']).next()
        && !candidates.contains(&base.to_string())
    {
        candidates.push(base.to_string());
    }
    candidates
}

fn domain_catalog_exists(locale_dir: &str, language: &str, domain: &str) -> bool {
    for candidate in locale_candidates(language) {
        let mo_path = format!("{}/{}/LC_MESSAGES/{}.mo", locale_dir, candidate, domain);
        if std::path::Path::new(&mo_path).exists() {
            return true;
        }
    }
    false
}

fn library_locale_dir_for_domain(domain: &str, language: &str, locale_dir: &str) -> String {
    let bundled = ["gtk40", "libadwaita"];
    if bundled.contains(&domain) {
        let our_dir = std::path::Path::new(locale_dir)
            .parent()
            .map(|path| path.join("khushu/locale").to_string_lossy().to_string())
            .unwrap_or_else(|| locale_dir.to_string());
        if domain_catalog_exists(&our_dir, language, domain) {
            return our_dir;
        }
    }

    let mut candidates = vec![
        "/usr/share/locale".to_string(),
        "/usr/share/locale-langpack".to_string(),
    ];

    if let Ok(snap) = std::env::var("SNAP") {
        candidates.push(format!("{}/usr/share/locale", snap));
    }
    candidates.push(locale_dir.to_string());

    candidates
        .into_iter()
        .find(|candidate| domain_catalog_exists(candidate, language, domain))
        .unwrap_or_else(|| locale_dir.to_string())
}

fn bind_library_domains(locale_dir: &str, language: &str) {
    let gtk_dir = library_locale_dir_for_domain("gtk40", language, locale_dir);
    let _ = bindtextdomain("gtk40", &gtk_dir);
    let _ = bind_textdomain_codeset("gtk40", "UTF-8");

    let adw_dir = library_locale_dir_for_domain("libadwaita", language, locale_dir);
    let _ = bindtextdomain("libadwaita", &adw_dir);
    let _ = bind_textdomain_codeset("libadwaita", "UTF-8");
}

pub fn detect_system_locale() -> String {
    std::env::var("LC_ALL")
        .ok()
        .filter(|locale| !locale.is_empty() && locale != "C" && locale != "POSIX")
        .map(|locale| locale.split('.').next().unwrap_or("en").to_string())
        .or_else(|| {
            std::env::var("LANG")
                .ok()
                .filter(|locale| !locale.is_empty() && locale != "C" && locale != "POSIX")
                .map(|locale| locale.split('.').next().unwrap_or("en").to_string())
        })
        .unwrap_or_else(|| "en".to_string())
}

pub(crate) const SUPPORTED_LANGUAGES: [&str; 6] = ["ar", "en", "fr", "es", "tr", "id"];

/// Resolves a stored locale value to its plain language+region form: `"auto"`/empty
/// falls back to the detected system locale, and POSIX charset (`.UTF-8`), Unicode
/// modifier (`@latin`), and `":…"` path suffixes are stripped. Region and underscores
/// are preserved on purpose; ICU region-aware lookups (country/timezone names) rely on them.
pub fn resolved_locale(language: &str) -> String {
    let raw_locale = if language == "auto" || language.is_empty() {
        detect_system_locale()
    } else {
        language.to_string()
    };

    raw_locale
        .trim()
        .split(':')
        .next()
        .unwrap_or_default()
        .split('@')
        .next()
        .unwrap_or_default()
        .split('.')
        .next()
        .unwrap_or_default()
        .to_string()
}

pub fn supported_language_code(language: &str) -> String {
    let resolved = resolved_locale(language);

    // ICU4X only accepts '-', not '_', as the subtag separator.
    let normalized = resolved.replace('_', "-");

    let base = icu_locale::LanguageIdentifier::try_from_str(&normalized)
        .map(|langid| langid.language.as_str().to_string())
        .unwrap_or_else(|_| normalized.split('-').next().unwrap_or_default().to_string());

    if SUPPORTED_LANGUAGES.contains(&base.as_str()) {
        base
    } else {
        "en".to_string()
    }
}

/// Index 0 is the "System Default" row, mapped to "auto".
pub fn language_code_from_index(index: u32) -> &'static str {
    match index {
        1 => "en",
        2 => "ar",
        3 => "fr",
        4 => "es",
        5 => "tr",
        6 => "id",
        _ => "auto",
    }
}

/// Index 0 is the "System Default" row; codes start at 1.
pub fn language_index_from_code(code: &str) -> u32 {
    match code {
        "en" => 1,
        "ar" => 2,
        "fr" => 3,
        "es" => 4,
        "tr" => 5,
        "id" => 6,
        _ => 0,
    }
}

pub fn icu_locale_key(language: &str) -> String {
    let normalized = resolved_locale(language).replace('_', "-");

    normalized
        .parse::<icu_locale::Locale>()
        .map(|locale| locale.to_string())
        .or_else(|_| {
            normalized
                .split('-')
                .next()
                .unwrap_or("en")
                .parse::<icu_locale::Locale>()
                .map(|locale| locale.to_string())
        })
        .unwrap_or_else(|_| "en".to_string())
}

pub fn is_rtl(language: &str) -> bool {
    supported_language_code(language) == "ar"
}

pub fn is_arabic(language: &str) -> bool {
    supported_language_code(language) == "ar"
}

pub fn update_locale(language: &str) {
    if language == "auto" || language.is_empty() {
        set_locale_env(language);
        let system_lang = detect_system_locale();
        update_locale_internal(&system_lang);
    } else {
        set_locale_env(language);
        update_locale_internal(language);
    }
}

fn update_locale_internal(language: &str) {
    CURRENT_APP_LOCALE.get_or_init(|| RwLock::new(language.to_string()));

    bind_domains(language);

    if let Some(lock) = CURRENT_APP_LOCALE.get()
        && let Ok(mut current_locale) = lock.write()
    {
        *current_locale = language.to_string();
    }

    crate::background::update_tray_labels();
}

pub fn rebind_locale_after_adw_init() {
    let hint = current_language_hint();
    set_locale_env(&hint);
    bind_domains(&hint);
}

pub fn tr(key: &str) -> String {
    if key == "translator-credits" {
        let translated = dgettext("khushu", key);
        if translated != key && !translated.is_empty() {
            return translated;
        }
        return "Djalel Oukid".to_string();
    }

    let translated = dgettext("khushu", key);
    if translated != key && !translated.is_empty() {
        return translated;
    }

    let adw = dgettext("libadwaita", key);
    if adw != key && !adw.is_empty() {
        return adw;
    }

    let gtk = dgettext("gtk40", key);
    if gtk != key && !gtk.is_empty() {
        return gtk;
    }

    key.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_native_locale_switching() {
        let locale_dir = get_locale_dir();
        assert!(
            std::path::Path::new(&locale_dir).exists(),
            "MO files must exist at {locale_dir}: build with `cargo build` first"
        );

        let _ = bindtextdomain("khushu", &locale_dir);
        let _ = bind_textdomain_codeset("khushu", "UTF-8");

        set_locale_env("fr");
        let fr = dgettext("khushu", "Welcome to Khushu");
        assert_ne!(
            fr, "Welcome to Khushu",
            "dgettext should return French under LANGUAGE=fr, got: {fr}"
        );
        println!("LANGUAGE=fr → {fr}");

        set_locale_env("ar");
        let ar = dgettext("khushu", "Welcome to Khushu");
        assert_ne!(
            ar, "Welcome to Khushu",
            "dgettext should return Arabic under LANGUAGE=ar, got: {ar}"
        );
        println!("LANGUAGE=ar → {ar}");

        assert_ne!(
            fr, ar,
            "French ({fr}) and Arabic ({ar}) must differ: gettext caching bug!"
        );

        set_locale_env("fr");
        let fr2 = dgettext("khushu", "Welcome to Khushu");
        assert_eq!(
            fr, fr2,
            "Switching back to French should return same result: {fr} vs {fr2}"
        );
        println!("LANGUAGE=fr (again) → {fr2}");

        println!("NATIVE LOCALE SWITCHING: OK (fr ≠ ar, consistent on round-trip)");
    }

    #[test]
    fn test_supported_language_code_normalizes_locale_values() {
        assert_eq!(supported_language_code("ar_SA.UTF-8"), "ar");
        assert_eq!(supported_language_code("en_US"), "en");
        assert_eq!(supported_language_code("fr_FR@latin"), "fr");
        assert_eq!(supported_language_code("tr_TR:UTF-8"), "tr");
        assert_eq!(supported_language_code("id"), "id");
        assert_eq!(supported_language_code("es"), "es");
        assert_eq!(supported_language_code("de"), "en");
        assert_eq!(supported_language_code(""), supported_language_code("auto"));
        assert_eq!(supported_language_code("C"), "en");

        let auto = supported_language_code("auto");
        assert!(
            ["ar", "en", "fr", "es", "tr", "id"].contains(&auto.as_str()),
            "auto must resolve to a supported code, got: {auto}"
        );
    }

    #[test]
    fn test_resolved_locale_strips_suffixes_but_keeps_region() {
        assert_eq!(resolved_locale("en_US.UTF-8"), "en_US");
        assert_eq!(resolved_locale("fr_FR@latin"), "fr_FR");
        assert_eq!(resolved_locale("ar_SA"), "ar_SA");
        assert_eq!(resolved_locale("  tr_TR:UTF-8 "), "tr_TR");
        assert_eq!(resolved_locale("en"), "en");

        assert_eq!(resolved_locale("auto"), detect_system_locale());
        assert_eq!(resolved_locale(""), detect_system_locale());
    }

    #[test]
    fn test_icu_locale_key_is_region_aware() {
        assert_eq!(icu_locale_key("ar_SA"), "ar-SA");
        assert_eq!(icu_locale_key("en_US.UTF-8"), "en-US");
        assert_eq!(icu_locale_key("fr_FR@latin"), "fr-FR");
        assert_eq!(icu_locale_key("fr"), "fr");
        assert_eq!(icu_locale_key("123_invalid"), "en");
    }

    #[test]
    fn test_is_rtl_only_for_arabic() {
        assert!(is_rtl("ar"));
        assert!(is_rtl("ar_SA.UTF-8"));
        assert!(!is_rtl("en"));
        assert!(!is_rtl("en_US"));
        assert!(!is_rtl("fr"));
        assert!(!is_rtl("de"));
    }

    #[test]
    fn test_is_arabic_normalizes_before_comparing() {
        assert!(is_arabic("ar"));
        assert!(is_arabic("ar_SA.UTF-8"));
        assert!(is_arabic("ar-DZ"));
        assert!(!is_arabic("en"));
        assert!(!is_arabic("en_US"));
        assert!(!is_arabic("fr_FR.UTF-8@latin"));
        assert!(!is_arabic("tr"));
    }

    #[test]
    fn test_language_index_mapping_round_trips() {
        let codes = [
            ("en", 1),
            ("ar", 2),
            ("fr", 3),
            ("es", 4),
            ("tr", 5),
            ("id", 6),
        ];
        for (code, index) in codes {
            assert_eq!(language_index_from_code(code), index);
            assert_eq!(language_code_from_index(index), code);
        }
    }

    #[test]
    fn test_language_index_mapping_falls_back_to_default() {
        assert_eq!(language_index_from_code("auto"), 0);
        assert_eq!(language_index_from_code("xx"), 0);
        assert_eq!(language_code_from_index(0), "auto");
        assert_eq!(language_code_from_index(99), "auto");
    }
}
