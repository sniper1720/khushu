# Changelog

## [1.3.0] — 2026-07-12

### Added
- High-latitude `LocalRelativeEstimation` rule (48.6°–66.5°) to estimate Isha/Fajr prayer times when their astronomical angles are unreachable
- `NearestLatitude` polar fallback (>66.5°) to find the closest latitude with a valid sunrise/sunset, ensuring all prayers remain calculable

### Fixed
- Blank label in KDE tray settings (Closes #16)

### Changed
- Migrated prayer time library from `salah` to `mawaqit` to support high-latitude and polar zones

## [1.2.2] — 2026-07-07

### Added
- **Surah header updates on shared-page clicks** — header title, type,
  and subtitle now reflect the clicked surah

### Fixed
- **Verse highlighting on pages with multiple surahs** — clicking a verse
  from any surah on a shared page now correctly highlights it. (Closes #15)
- **Prev/next buttons jumping 2 pages on shared surah pages** — page advance uses absolute page
  number instead of surah-relative range. (Closes #14)
- **Recitation play on shared pages** — play function now receives both
  surah and verse explicitly
- **Recitation stop conditions on shared pages** — page/juz boundaries
  correctly computed across surah boundaries
- **Poll handler race condition** — removed fallback that raced with audio
  thread, blocking verse chaining
- **Scroll-to-verse on shared surah-start pages** — now lands at the
  correct verse instead of scrolling to the wrong position
- **Verse click detection on Arabic mushaf** — now correctly detects
  which verse was clicked on pages with multiple surahs
- **Fatiha verse 1 highlight** — bismillah now underlines when selected
- **Translated surah name labels truncating on narrow windows** — labels
  now use ellipsis truncation instead of overflowing the header

### Changed
- **Quran data cached as shared reference** — now shared
  between all callers instead of being cloned
- **Verse display length allocation removed** — removes a temporary
  string allocation on every page build
- **Arabic query detection deduplicated** — extracted to helper

## [1.2.1] — 2026-06-29

### Fixed
- **Tray icon always showing dark on GNOME/XFCE in Flatpak** — replaced
  file-path workaround with standard symbolic themed icon name; so the
  symbolic icon is used correctly on all DEs (KDE, GNOME, XFCE). (Closes #13)
- **Panics across FFI/D-Bus in tray code** — replaced 3 `.expect()` lock-poison
  panics with safe `unwrap_or_else` recovery + log warning

### Changed
- **Tray icon architecture** — removed `get_flatpak_tray_icon_path()` and its
  `$XDG_RUNTIME_DIR/tray-icon/` file-copy logic; `icon_name()` now returns the
  themed name `io.github.sniper1720.khushu-symbolic`, dropping the need for the
  `--filesystem=xdg-run/tray-icon` permission

## [1.2.0] — 2026-06-25

### Added
- **Noble Quran Recitation** — ayah-by-ayah audio playback with
  real-time ayah highlighting. Audio served by VerseByVerse Quran
  with selectable reciters. (Closes #8)

### Fixed
- **Tray icon invisible on KDE dark panel** — symbolic SVGs
  refactored to KDE gold standard; icon now renders correctly on
  both light and dark panels across GNOME and KDE. (Closes #10)
- **Arabic mushaf search scroll** — search results navigate (scroll) to the correct ayah instead of landing at the top of the page
- **Pagination boundary alignment** — ajza' breaks and surah start
  offsets corrected for accurate mushaf-style navigation

### Changed
- **Locale switching rewritten** — unified i18n architecture:
  language change now propagates through a single centralized
  handler, eliminating per-page translation drift across 21 modules

## [1.1.4] — 2026-05-28

### Added
- **Indonesian (id) translation** — full Indonesian translation contributed by WiqbalRar, including UI strings, GTK4/libadwaita context menus, and Quran translation data

### Fixed
- **Custom audio save crash** — removed `ensure_validation_thread` which spawned a background worker that called `spawn_future_local` from a non-main thread, causing a panic. Config save now runs directly in `validate_audio_async` on the main thread
- **Language not persisting after restart** — added `cfg.save()` after `cfg.set_language()` so the selected language is written to disk immediately

## [1.1.3] — 2026-05-26

### Fixed
- **Config data loss on exit** — replaced daemon save-thread race with synchronous atomic write (temp file + rename). Settings changed just before closing the app are now always persisted
- **Autostart portal command** — passed `["khushu", "--background"]` instead of the full `["flatpak", "run", APP_ID, "--background"]`. The portal wraps with `--command=` internally, so the flatpak wrapper was double-wrapped and failed silently
- **Notification toggles reverting on restart** — `sync_ui()` during initialization called `set_active()` on notification toggles, overriding the saved config values. Changed to `set_sensitive()` only during init; `set_active()` enforcement now runs only when Adhan Only Mode is interactively toggled
- **Audio preset reverting on restart** — builtin presets use `"assets/audio/"` paths as GResource lookup keys, but the startup validator checked them as filesystem paths and reset them. Added `!path.starts_with("assets/")` guard

### Changed
- **Config architecture** — removed background save thread and channel. `save()` now writes synchronously. ~1KB JSON write is microseconds — the thread was unnecessary complexity that introduced a data-loss bug

## [1.1.2] — 2026-05-13

### Improved
- **Audio playback engine** — switched from full-file decode to streaming (`sink.append(decoder)`), eliminating blocking on the audio thread
- **Qibla compass performance** — hoisted Pango layout/font outside the draw loop, added CardinalData string + bearing caches, reduced redraw to 20 FPS
- **Timer controller efficiency** — `DailyState` cache, 1-second tick does only countdown math, no redundant recomputation
- **Config persistence** — singleton save channel eliminates per-save thread spawning
- **Language change handler** — extracted 200+ line inline closure into named `handle_lang_change` function
- **Config storage** — removed weak XOR obfuscation, stored as plaintext with `0o600` permissions
- **Codebase quality** — replaced 26 production `unwrap()` calls with descriptive `expect()` messages
- **Hijri date formatting** — deduplicated month names into single `pub const HIJRI_MONTH_NAMES`

### Fixed
- **RefCell re-entrancy panics** — 17 `connect_notify_local` handlers wrapped with `freeze_notify()` RAII guard, preventing recursive borrow panics in GLib property notifications
- **Config TOCTOU race** — removed `sync_quran_state_from_disk()` which re-read config from disk while a concurrent save was in progress; in-memory `AppConfig` is now the single source of truth
- **Thread-unsafe locale setup** — removed `unsafe { std::env::set_var }` calls (undefined behavior) and replaced with safe locale handling
- **Raw FFI bindtextdomain** — replaced unsafe `extern "C"` calls with safe `gettextrs` crate
- **GIO resource access on worker thread** — confined `resources_lookup_data` to main-thread startup only

## [1.1.1] — 2026-04-25

### Added
- Added **Adhan Only Mode** toggle - disables pre-prayer, Iqamah, and Adkar notifications, keeping only Adhan notifications
- Added **Iqamah Alert** toggle - separate control for Iqamah notifications
- Added **new calculation methods** for Muslim communities:
  - France (UOIF): 12°/12° angles
  - Algeria (Ministry of Religious Affairs): 18°/17° angles with +3min Maghrib adjustment
  - KEMENAG (Indonesian Ministry of Religious Affairs): 20°/18° angles

### Fixed
- Fixed Flatpak tray icon visibility by implementing XDG portal icon path support with proper filesystem permissions

### Changed
- Improved tray activation: clicking "Open Khushu" from system tray now directly shows the app window (in foreground) instead of first showing a "ready" notification

## [1.1.0] — 2026-04-22

### Added
- Added the Noble Quran module — a fully offline reader featuring all 114 surahs with Uthmanic Arabic script (Amiri Quran font), parallel translations in five languages, persistent bookmarks, reading-position memory, full-text search with Arabic diacritics normalization, mushaf-style page navigation, and user-adjustable Arabic/translation font sizes and line spacing
- Added ICU-backed localization for city, country and timezone labels
- Added validated custom IANA timezone editing with case-insensitive normalization
- Added Mawaqit API integration as an alternative prayer times source
- Added a stop Adhan button on the main page for quick access when notifications are missed
- Added Iqamah countdown timer on the prayer times page with per-prayer delay controls in Settings
- Added comprehensive test suite verifying Arabic language support including Amiri font application, RTL direction, and tray label translations

### Improved
- Reorganized Settings page into General, Prayer Setup, and Notifications & Audio sections
- Improved Qibla compass lifecycle handling to prevent excessive resource consumption
- Improved UI update logic for both manual and automatic language changes with better system locale detection

### Fixed
- Fixed Snap tray icon visibility via an [upstream patch to ksni](https://github.com/iovxw/ksni/pull/37) (resolves AppArmor D-Bus blocking)

## [1.0.3] — 2026-03-30

### Fixed
- Fixed Snap autostart and notification icon rendering
- Improved locale detection and fallback logic

## [1.0.2] — 2026-03-24

### Fixed
- Fixed critical autostart bug where GNOME Background Portal would dynamically delete autostart entries
- Fixed Flatpak autostart command to launch silently in the background instead of popping open the main UI
- Fixed translation extraction pipeline for proper nouns (e.g., Muslim, At-Tirmidhi) to prevent unintended fuzzy matching across languages

### Added
- Expanded AppStream metadata coverage with 3 additional screenshots
- Added comprehensive translations for all AppStream screenshot captions across all supported languages

## [1.0.1] — 2026-03-18

### Fixed
- Fixed tray icon translation extraction (Quit and Open Khushu)
- Fixed AppStream metadata translation (descriptions, keywords)
- Improved tray behavior to update labels immediately on language change

## [1.0.0] — 2026-03-04

### Added
- Accurate prayer times with 11 calculation methods (MWL, ISNA, Egypt, Makkah, Karachi, Dubai, Kuwait, Qatar, Singapore, Turkey, Moonsighting Committee)
- Adhan audio playback with volume control and mute toggle
- Pre-prayer notifications with configurable lead time
- Staggered Adkar notifications (morning, evening, post-prayer)
- Hijri calendar with adjustable offset
- Qibla compass direction
- Automatic location detection via GeoClue
- Manual city search
- System tray icon with quick actions
- Background mode with autostart support
- Multi-language: Arabic, English, French, Spanish, Turkish
- Dark, Light, and System theme modes
- Obfuscated coordinate storage for privacy
- Zero telemetry
