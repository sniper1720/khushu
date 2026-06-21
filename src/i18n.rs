use gettextrs::{LocaleCategory, bind_textdomain_codeset, bindtextdomain, dgettext, setlocale};
use std::sync::{Mutex, OnceLock, RwLock};

static ORIGINAL_LANGUAGE: OnceLock<String> = OnceLock::new();
static CURRENT_APP_LOCALE: OnceLock<RwLock<String>> = OnceLock::new();

static LOCALE_MODIFY: Mutex<()> = Mutex::new(());

pub fn save_original_locale() {
    ORIGINAL_LANGUAGE.get_or_init(|| std::env::var("LANGUAGE").unwrap_or_default());
}

fn bind_domains(lang: &str) {
    let locale_dir = get_locale_dir();
    let pkg = option_env!("GETTEXT_PACKAGE").unwrap_or("khushu");
    let _ = bindtextdomain(pkg, &locale_dir);
    let _ = bind_textdomain_codeset(pkg, "UTF-8");
    bind_library_domains(&locale_dir, lang);
}

fn set_locale_env(lang: &str) {
    let _guard = LOCALE_MODIFY.lock().expect("LOCALE_MODIFY poisoned");

    if lang == "auto" || lang.is_empty() {
        match ORIGINAL_LANGUAGE.get() {
            Some(orig) if !orig.is_empty() => unsafe { std::env::set_var("LANGUAGE", orig) },
            _ => unsafe { std::env::remove_var("LANGUAGE") },
        }
    } else {
        unsafe { std::env::set_var("LANGUAGE", lang) };
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

fn locale_candidates(lang: &str) -> Vec<String> {
    let normalized = lang
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

fn domain_catalog_exists(dir: &str, lang: &str, domain: &str) -> bool {
    for candidate in locale_candidates(lang) {
        let mo_path = format!("{}/{}/LC_MESSAGES/{}.mo", dir, candidate, domain);
        if std::path::Path::new(&mo_path).exists() {
            return true;
        }
    }
    false
}

fn library_locale_dir_for_domain(domain: &str, lang: &str, locale_dir: &str) -> String {
    let bundled = ["gtk40", "libadwaita"];
    if bundled.contains(&domain) {
        let our_dir = std::path::Path::new(locale_dir)
            .parent()
            .map(|p| p.join("khushu/locale").to_string_lossy().to_string())
            .unwrap_or_else(|| locale_dir.to_string());
        if domain_catalog_exists(&our_dir, lang, domain) {
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
        .find(|d| domain_catalog_exists(d, lang, domain))
        .unwrap_or_else(|| locale_dir.to_string())
}

fn bind_library_domains(locale_dir: &str, lang: &str) {
    let gtk_dir = library_locale_dir_for_domain("gtk40", lang, locale_dir);
    let _ = bindtextdomain("gtk40", &gtk_dir);
    let _ = bind_textdomain_codeset("gtk40", "UTF-8");

    let adw_dir = library_locale_dir_for_domain("libadwaita", lang, locale_dir);
    let _ = bindtextdomain("libadwaita", &adw_dir);
    let _ = bind_textdomain_codeset("libadwaita", "UTF-8");
}

pub fn detect_system_locale() -> String {
    std::env::var("LC_ALL")
        .ok()
        .filter(|s| !s.is_empty() && s != "C" && s != "POSIX")
        .map(|s| s.split('.').next().unwrap_or("en").to_string())
        .or_else(|| {
            std::env::var("LANG")
                .ok()
                .filter(|s| !s.is_empty() && s != "C" && s != "POSIX")
                .map(|s| s.split('.').next().unwrap_or("en").to_string())
        })
        .unwrap_or_else(|| "en".to_string())
}

pub fn update_locale(lang: &str) {
    if lang == "auto" || lang.is_empty() {
        set_locale_env(lang);
        let system_lang = detect_system_locale();
        update_locale_internal(&system_lang);
    } else {
        set_locale_env(lang);
        update_locale_internal(lang);
    }
}

fn update_locale_internal(lang: &str) {
    CURRENT_APP_LOCALE.get_or_init(|| RwLock::new(lang.to_string()));

    bind_domains(lang);

    if let Some(lock) = CURRENT_APP_LOCALE.get()
        && let Ok(mut cur) = lock.write()
    {
        *cur = lang.to_string();
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
        let res = dgettext("khushu", key);
        if res != key && !res.is_empty() {
            return res;
        }
        return "Djalel Oukid".to_string();
    }

    let res = dgettext("khushu", key);
    if res != key && !res.is_empty() {
        return res;
    }

    let fallback = dgettext("khushu-gtk", key);
    if fallback != key && !fallback.is_empty() {
        return fallback;
    }

    let adw = dgettext("libadwaita", key);
    if adw != key && !adw.is_empty() {
        return adw;
    }

    let gtk = dgettext("gtk40", key);
    if gtk != key && !gtk.is_empty() {
        return gtk;
    }

    let gtk_legacy = dgettext("gtk", key);
    if gtk_legacy != key && !gtk_legacy.is_empty() {
        return gtk_legacy;
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
            "MO files must exist at {locale_dir} — build with `cargo build` first"
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
            "French ({fr}) and Arabic ({ar}) must differ — gettext caching bug!"
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
}
