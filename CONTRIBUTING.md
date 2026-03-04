# Contributing to Khushu

Assalamu Alaikum! We are thrilled you want to contribute to Khushu. This guide will help you get started quickly, whether you're fixing a bug, adding a feature, or translating the app into a new language.

## Translation Contributions

Translations are the lifeblood of Khushu, helping Muslims around the world access accurate prayer times in their native language. Khushu uses standard GNU `gettext` (`.po` files) for localization.

### How to add or update a translation:

1. **Find your language:** Look in the `po/` directory. If your language file exists (e.g., `es.po` for Spanish), open it in a text editor or a tool like [Poedit](https://poedit.net/).
2. **Start a new language:** If your language doesn't exist, create files from the templates:
   ```bash
   # App strings (required)
   cp po/khushu.pot po/YOUR_LANG_CODE.po

   # About dialog UI strings (recommended)
   cp po/libadwaita.pot po/libadwaita.YOUR_LANG_CODE.po
   cp po/gtk40.pot po/gtk40.YOUR_LANG_CODE.po
   ```
   (Example: `cp po/khushu.pot po/de.po` for German).

### Translation Domains

Khushu uses **three translation domains**:

| Domain | Template | Purpose |
|--------|----------|---------|
| `khushu` | `khushu.pot` | All app-specific strings (prayer times, settings, pages) |
| `libadwaita` | `libadwaita.pot` | About dialog UI (Details, Credits, Legal, Report an Issue) |
| `gtk40` | `gtk40.pot` | GTK4 strings (license names, Code by, Translated by) |

The naming convention is `DOMAIN.LANG.po` for library domains (e.g., `libadwaita.fr.po`, `gtk40.ar.po`) and `LANG.po` for the main app domain (e.g., `fr.po`, `ar.po`).

3. **Translate:** Fill in the `msgstr` fields under each `msgid` in the `.po` file.
   * *Note: Do not translate variables or internal GTK placeholder strings unless you understand their context.*
4. **Test your translations locally:** The build system compiles all `.po` files automatically:
   ```bash
   cargo build
   cargo run
   ```
   Khushu will automatically load `target/locale/` during development if the directory exists.
5. **Commit:** Only commit the `.po` file! **Never commit the compiled `.mo` file.** The build pipelines (Flatpak/Snap/Arch) handle `.mo` compilation automatically.

## Code Contributions

Khushu is written purely in **Rust** and **GTK4 / libadwaita**. It is designed to be lean, fast, and native.

### Development Setup
1. **Install dependencies:** You need the Rust toolchain, GTK4 development headers, and Libadwaita.
   * Arch/Manjaro: `sudo pacman -S rust base-devel gtk4 libadwaita`
   * Fedora: `sudo dnf install rust cargo gtk4-devel libadwaita-devel`
   * Ubuntu/Debian: `sudo apt install rustc cargo libgtk-4-dev libadwaita-1-dev`
2. **Clone and Run:**
   ```bash
   git clone https://github.com/sniper1720/khushu.git
   cd khushu
   cargo run
   ```

### Coding Guidelines
* **Rust Idioms:** Run `cargo clippy` and `cargo fmt` before submitting a PR. We strive for zero Clippy warnings.
* **Keep it Native:** Avoid wrapping shell commands inside Rust. Use native crates (e.g., `reqwest` instead of `curl`, standard library instead of `date`).
* **Comments:** Write clean, human-readable comments. Explain *why* the code exists, not just *what* it is doing, unless the logic is extremely complex.

Thank you for helping make Khushu better! May Allah reward your efforts.
