Name:           khushu
Version:        1.2.0
Release:        1%{?dist}
Summary:        An all-in-one Muslim app for Linux

License:        GPLv3+
URL:            https://github.com/sniper1720/khushu
Source0:        %{url}/archive/refs/tags/v%{version}.tar.gz

%define debug_package %{nil}

BuildRequires:  meson
BuildRequires:  ninja-build
BuildRequires:  gcc
BuildRequires:  cargo
BuildRequires:  rust
BuildRequires:  gtk4-devel
BuildRequires:  libadwaita-devel
BuildRequires:  pkgconf-pkg-config
BuildRequires:  alsa-lib-devel
BuildRequires:  openssl-devel
BuildRequires:  gettext

Requires:       hicolor-icon-theme

%description
Khushu is a modern GNOME application that provides prayer times,
Qibla direction, Adkar, and mosque search. It features a clean,
adaptive UI built with GTK4 and Libadwaita.

%prep
%autosetup

%build
%meson
%meson_build

%install
%meson_install

%check
%meson_test

%files
%license LICENSE
%doc README.md
%{_bindir}/%{name}
%{_datadir}/applications/io.github.sniper1720.khushu.desktop
%{_datadir}/dbus-1/services/io.github.sniper1720.khushu.service
%{_datadir}/icons/hicolor/scalable/apps/io.github.sniper1720.khushu.svg
%{_datadir}/icons/hicolor/symbolic/apps/io.github.sniper1720.khushu-symbolic.svg
%{_datadir}/metainfo/io.github.sniper1720.khushu.metainfo.xml
%{_datadir}/%{name}/
%{_datadir}/locale/*/LC_MESSAGES/khushu.mo
%{_datadir}/fonts/truetype/%{name}/

%changelog
* Thu Jun 25 2026 Djalel Oukid <sniper1720@linuxtechmore.com> - 1.2.0-1
- Added Noble Quran Recitation with ayah-by-ayah audio playback and reciter selection
- Rewrote locale switching for consistent i18n
- Fixed KDE dark panel tray icon visibility
- Fixed Arabic mushaf search scroll navigation
- Fixed pagination boundary alignment
* Thu May 28 2026 Djalel Oukid <sniper1720@linuxtechmore.com> - 1.1.4-1
- Added Indonesian (id) translation
- Fixed custom audio save crash on non-main thread
- Fixed language not persisting after app restart
* Tue May 26 2026 Djalel Oukid <sniper1720@linuxtechmore.com> - 1.1.3-1
- Fixed config data loss on exit with synchronous atomic write
- Fixed autostart portal command double-wrapping
- Fixed notification toggles reverting on restart (sync_ui init override)
- Fixed audio preset reverting on restart (GResource path validated as file)
* Wed May 13 2026 Djalel Oukid <sniper1720@linuxtechmore.com> - 1.1.2-1
- Improved audio playback engine with streaming decode
- Improved Qibla compass performance with Pango/cardinal/bearing caching
- Improved timer controller with DailyState caching
- Improved config persistence with singleton save channel
- Fixed RefCell re-entrancy panics with freeze_notify guards
- Fixed config TOCTOU race by removing disk re-read
- Fixed thread-unsafe locale setup, FFI bindtextdomain, and GIO thread safety
- Fixed code quality with expect() replacing unwrap() calls

* Sat Apr 25 2026 Djalel Oukid <sniper1720@linuxtechmore.com> - 1.1.1-1
- Added Adhan Only Mode toggle to keep only Adhan alerts
- Added Iqamah Alert toggle to separately control Iqamah notifications
- Added new prayer calculation methods for France (UOIF), Algeria (Ministry of Religious Affairs and Wakfs), and KEMENAG (Indonesia)
- Fixed Flatpak tray icon visibility with XDG portal icon path support
- Improved tray activation: clicking "Open Khushu" from system tray now directly shows the app window

* Fri Apr 10 2026 Djalel Oukid <sniper1720@linuxtechmore.com> - 1.1.0-1
- Added a fully offline Noble Quran module featuring Uthmanic text, translations, mushaf-style navigation, diacritic-aware search, saved reading positions, and adjustable typography.
- Added Mawaqit API integration as an alternative prayer times source.
- Added ICU-backed localization for location and timezone labels.
- Added validated custom IANA timezone editing with cleaner inline feedback.
- Reorganized Settings into clearer General, Prayer Setup, and Alerts sections.
- Fixed Snap tray icon visibility via an upstream patch to ksni (resolves AppArmor D-Bus blocking).

* Mon Mar 30 2026 Djalel Oukid <sniper1720@linuxtechmore.com> - 1.0.3-1
- Fixed Snap autostart and notification icon rendering.
- Improved locale detection and fallback logic.

* Tue Mar 24 2026 Djalel Oukid <sniper1720@linuxtechmore.com> - 1.0.2-1
- Fixed Flatpak autostart failure and missing --background flag.
- Fixed over-translation of proper nouns (e.g., Muslim).
- Added new localized AppStream screenshots.

* Wed Mar 18 2026 Djalel Oukid <sniper1720@linuxtechmore.com> - 1.0.1-1
- Fixed tray icon translation extraction (Quit and Open Khushu).
- Fixed AppStream metadata translation (descriptions, keywords).
- Improved tray behavior to update labels immediately on language change.

* Fri Mar 06 2026 Djalel Oukid <sniper1720@linuxtechmore.com> - 1.0.0-1
- Initial release.
