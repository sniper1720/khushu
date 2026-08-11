#!/usr/bin/env bash
set -euo pipefail

# update-pot.sh — Extract new strings, add to pot in the correct section,
# remove stale entries, refresh #: refs from source, then msgmerge all .po files.

cd "$(dirname "$0")/.."

POT="po/khushu.pot"
AUTO="po/auto.pot"
AUTO_RS="po/auto-rs.pot"
AUTO_META="po/auto-meta.pot"

# Step 1a: Extract tr() calls from .rs sources plus .desktop fields (via POTFILES.in)
xgettext \
  --from-code=UTF-8 \
  --package-name=khushu \
  --add-location=file \
  --msgid-bugs-address=https://github.com/sniper1720/khushu/issues \
  --keyword=tr:1 \
  --add-comments \
  --output="$AUTO_RS" \
  --files-from=po/POTFILES.in

# Step 1b: Extract translatable strings from AppStream metainfo XML via ITS
ITS_DIR=$(dirname "$(find /usr/share -name 'metainfo.its' -path '*/its/*' 2>/dev/null | head -1)" 2>/dev/null || echo "")
if [ -z "$ITS_DIR" ]; then
  echo "Warning: metainfo.its not found; skipping AppStream extraction" >&2
  echo "" > "$AUTO_META"
else
  xgettext \
    --from-code=UTF-8 \
    --package-name=khushu \
    --add-location=file \
    --msgid-bugs-address=https://github.com/sniper1720/khushu/issues \
    --its="$ITS_DIR/metainfo.its" \
    --output="$AUTO_META" \
    data/appdata/io.github.sniper1720.khushu.metainfo.xml.in || {
      echo "Warning: ITS extraction failed; skipping AppStream strings" >&2
      echo "" > "$AUTO_META"
    }
fi

# Step 1c: Combine both into one auto.pot
msgcat --use-first --output="$AUTO" "$AUTO_RS" "$AUTO_META" 2>/dev/null
rm -f "$AUTO_RS" "$AUTO_META"

# Step 2: Merge auto.pot into the curated pot (placement-aware, with #: refresh)
python3 << 'PYEOF' || exit 1
import re, os, sys
from pathlib import Path

POT = Path("po/khushu.pot")
AUTO = Path("po/auto.pot")

auto_text = AUTO.read_text(encoding="utf-8")
pot_text = POT.read_text(encoding="utf-8")

# ── Configuration ────────────────────────────────────────────────────
# Ordered list of (path_pattern, section_number).  First match wins.
# When adding a new .rs file, add its mapping here so new strings land
# in the right section automatically.
FILE_SECTION_MAP = [
    ("data/io.github.sniper1720.khushu.desktop.in", 1),
    ("data/appdata/",                                1),
    ("src/calendar.rs",                               2),
    ("src/time.rs",                                   2),
    ("src/timer_controller.rs",                       3),
    ("src/qibla_ui.rs",                               4),
    ("src/quran.rs",                                  5),
    ("src/reciter_ui.rs",                             5),
    ("src/adkar.rs",                                  6),
    ("src/settings_ui.rs",                            7),
    ("src/audio.rs",                                  7),
    ("src/location.rs",                               7),
    ("src/tz_dialog.rs",                              7),
    ("src/main.rs",                                   8),
    ("src/nav_ui.rs",                                 8),
    ("src/pages.rs",                                  8),
    ("src/background.rs",                             8),
    ("src/welcome.rs",                                9),
    ("src/home_ui.rs",                                9),
    ("src/i18n.rs",                                   9),
    ("src/mawaqit.rs",                                9),
]

SECTION_RE = re.compile(
    r'^# ={68,}\n# SECTION (\d+): (.+)\n# ={68,}$', re.MULTILINE
)


# ── Helpers ──────────────────────────────────────────────────────────

def parse_blocks(text):
    """Split text by \n\n, returning (block, is_section_header, section_num, section_name)."""
    parts = re.split(r'\n\n(?=#|msgid)', text.strip())
    result = []
    cur_sec = 0
    for p in parts:
        p = p.strip()
        if not p:
            continue
        m = SECTION_RE.match(p)
        if m:
            cur_sec = int(m.group(1))
            result.append((p, True, cur_sec, m.group(2)))
        else:
            result.append((p, False, cur_sec, None))
    return result


def extract_msgid(block):
    lines = block.split('\n')
    parts = []
    in_msgid = False
    for line in lines:
        if line.startswith('msgid '):
            in_msgid = True
            m = re.match(r'^msgid "(.*)"$', line)
            parts.append(m.group(1) if m else '')
        elif in_msgid:
            if line.startswith('"'):
                m = re.match(r'^"(.*)"$', line)
                if m:
                    parts.append(m.group(1))
            else:
                break
    result = ''.join(parts)
    return result if result else None


def extract_refs(block):
    return re.findall(r'#:\s+(\S+)', block)


def replace_refs(block, new_refs):
    """Replace all #: lines in *block* with *new_refs* (formatted nicely)."""
    lines = block.split('\n')
    out = []
    in_refs = False
    for line in lines:
        if line.startswith('#:'):
            if not in_refs:
                # write new refs
                cur = []
                for r in new_refs:
                    cand = ' '.join(cur + [r])
                    if len(cand) > 70 and cur:
                        out.append('#: ' + ' '.join(cur))
                        cur = [r]
                    else:
                        cur.append(r)
                if cur:
                    out.append('#: ' + ' '.join(cur))
                in_refs = True
        else:
            if in_refs:
                in_refs = False
            out.append(line)
    return '\n'.join(out)


# ── Build auto lookup by msgid ──────────────────────────────────────

auto_blocks = parse_blocks(auto_text)
auto_by_msgid = {}
for block, is_sec, sec, name in auto_blocks:
    if not is_sec:
        mid = extract_msgid(block)
        if mid:
            auto_by_msgid[mid] = block

auto_msgids = set(auto_by_msgid.keys())

# ── Parse pot ───────────────────────────────────────────────────────
pot_blocks = parse_blocks(pot_text)

header_blocks = []
section_data = {}  # sec_num -> (header_block, name, [entry_blocks])

for block, is_sec, sec, name in pot_blocks:
    if sec == 0 and not is_sec:
        header_blocks.append(block)
    elif is_sec:
        section_data[sec] = [block, name, []]
    else:
        if sec in section_data:
            section_data[sec][2].append(block)

# ── Process each section: remove stale, refresh #: refs ────────────
removed = 0
existing_msgids = set()

for sec in sorted(section_data.keys()):
    hdr, name, entries = section_data[sec]
    kept = []
    for blk in entries:
        mid = extract_msgid(blk)
        refs = extract_refs(blk)

        if mid is None or mid == "":
            kept.append(blk)
            if mid:
                existing_msgids.add(mid)
            continue

        existing_msgids.add(mid)

        if not refs:
            # Manual entry (no #: refs) — keep unconditionally
            kept.append(blk)
            continue

        rs_refs = [r for r in refs if r.split(":")[0].endswith(".rs")]
        has_rs = bool(rs_refs)

        if mid not in auto_msgids:
            if has_rs:
                # Stale .rs entry — msgid no longer extracted by xgettext.
                # Convert to manual entry (remove #: refs) instead of deleting,
                # because the string may still be in the app via arrays/structs
                # that xgettext cannot extract.
                blk = replace_refs(blk, [])
                kept.append(blk)
            else:
                # Stale entry from .xml.in / other sources — no longer exists
                # in the source. Delete it entirely.
                pass  # skip appending
            removed += 1
            continue

        # Refresh #: refs from auto.pot when available
        if mid in auto_msgids:
            auto_blk = auto_by_msgid[mid]
            auto_refs = extract_refs(auto_blk)
            if set(auto_refs) != set(refs):
                blk = replace_refs(blk, auto_refs)

        kept.append(blk)

    section_data[sec] = (hdr, name, kept)


# ── Add new entries into the correct section ────────────────────────

def find_target_section(block):
    refs = extract_refs(block)
    for ref in refs:
        fp = ref.split(":")[0]
        for pattern, sec_num in FILE_SECTION_MAP:
            if pattern in fp:
                return sec_num
    return None


max_sec = max(section_data.keys()) if section_data else 0
added = 0

for mid, auto_blk in auto_by_msgid.items():
    if mid not in existing_msgids:
        added += 1
        target = find_target_section(auto_blk)
        if target is None or target not in section_data:
            target = max_sec
        hdr, name, entries = section_data[target]
        entries.append(auto_blk)
        existing_msgids.add(mid)


# ── Reassemble pot ──────────────────────────────────────────────────
# Pin a canonical header. The POT is a template: POT-Creation-Date keeps
# the same placeholder pattern as PO-Revision-Date, because the "creation
# date" that matters lives in each PO (its first git commit date), not in
# a template regenerated on every run. Project-Id-Version tracks Cargo.toml.
def canonical_header():
    version = re.search(
        r'^version = "([^"]+)"', Path("Cargo.toml").read_text(encoding="utf-8"), re.M
    )
    version = version.group(1) if version else "unknown"
    return "\n".join(
        [
            "# SOME DESCRIPTIVE TITLE.",
            "# Copyright (C) YEAR THE PACKAGE'S COPYRIGHT HOLDER",
            "# This file is distributed under the same license as the khushu package.",
            "#",
            'msgid ""',
            'msgstr ""',
            f'"Project-Id-Version: Khushu {version}\\n"',
            '"Report-Msgid-Bugs-To: https://github.com/sniper1720/khushu/issues\\n"',
            '"POT-Creation-Date: YEAR-MO-DA HO:MI+ZONE\\n"',
            '"PO-Revision-Date: YEAR-MO-DA HO:MI+ZONE\\n"',
            '"Last-Translator: FULL NAME <EMAIL@ADDRESS>\\n"',
            '"Language-Team: LANGUAGE <LL@li.org>\\n"',
            '"MIME-Version: 1.0\\n"',
            '"Content-Type: text/plain; charset=UTF-8\\n"',
            '"Content-Transfer-Encoding: 8bit\\n"',
        ]
    )

output_parts = [canonical_header()]

for sec in sorted(section_data.keys()):
    hdr, name, entries = section_data[sec]
    output_parts.append(hdr)
    if entries:
        output_parts.append("\n\n".join(entries))

result = "\n\n".join(output_parts) + "\n"
POT.write_text(result, encoding="utf-8")

print(f"Added {added} new entries; {removed} stale entries dropped or demoted to manual")
PYEOF

# Step 3: Validate pot
msgfmt -c -o /dev/null "$POT" || { echo "ERROR: $POT is invalid"; exit 1; }

# Step 4: Merge into all .po files (propagates #: refs from pot → .po).
#
# Header policy:
#   - POT-Creation-Date: frozen per PO to that file's first git commit
#     date. msgmerge overwrites it from the POT on any change, so it is
#     restored after merging (fallback: the value already in the PO).
#   - Project-Id-Version: synced to the Cargo.toml version on mismatch.
#   - PO-Revision-Date: bumped whenever the file actually changed
#     (msgmerge diff, frozen-date restore, or version sync).
CARGO_VERSION=$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -1)
for po in po/*.po; do
  [ -f "$po" ] || continue
  case "$po" in
    *gtk40*|*libadwaita*) continue ;;
  esac

  # Frozen creation date for this PO: first git commit, else keep current.
  frozen=$(git log --diff-filter=A --format='%ad' --date=format:'%Y-%m-%d %H:%M%z' -- "$po" 2>/dev/null | tail -1)
  if [ -z "$frozen" ]; then
    frozen=$(sed -n 's/^"POT-Creation-Date: \([^"]*\)\\n"$/\1/p' "$po" | head -1)
  fi

  before=$(sha256sum "$po" | cut -d' ' -f1)
  msgmerge --no-fuzzy-matching --backup=off --add-location=file --update --quiet "$po" "$POT"
  after=$(sha256sum "$po" | cut -d' ' -f1)

  # Restore the PO's own POT-Creation-Date (msgmerge copies the POT's).
  if [ -n "$frozen" ]; then
    sed -i "s|^\"POT-Creation-Date: [^\"]*\\\\n\"$|\"POT-Creation-Date: $frozen\\\\n\"|" "$po"
  fi

  # Sync Project-Id-Version to the current package version.
  if [ -n "$CARGO_VERSION" ] && ! grep -Fq "Project-Id-Version: Khushu $CARGO_VERSION\\n" "$po"; then
    sed -i "s|^\"Project-Id-Version: Khushu [^\"]*\\\\n\"$|\"Project-Id-Version: Khushu $CARGO_VERSION\\\\n\"|" "$po"
  fi

  final=$(sha256sum "$po" | cut -d' ' -f1)
  if [ "$before" != "$after" ] || [ "$after" != "$final" ]; then
    REV_LINE=$(date -u '+%Y-%m-%d %H:%M+0000')
    sed -i "s|\"PO-Revision-Date: [^\"]*\\\\n\"|\"PO-Revision-Date: $REV_LINE\\\\n\"|" "$po"
    echo "  Updated $po (PO-Revision-Date bumped)"
  else
    echo "  $po unchanged"
  fi
done

# Step 5: Guard against header corruption — a PO whose header lost its
# Language field (e.g. it was recreated from the POT template) would ship
# a broken catalog silently. Fail loudly instead.
for po in po/*.po; do
  [ -f "$po" ] || continue
  case "$po" in
    *gtk40*|*libadwaita*) continue ;;
  esac
  lang=$(basename "$po" .po)
  if ! grep -Fq "Language: $lang\\n" "$po"; then
    echo "ERROR: $po header is missing \"Language: $lang\" — refusing to continue" >&2
    exit 1
  fi
done

# Step 6: Cleanup
rm -f "$AUTO"

echo "Done! $(grep -c '^msgid "' "$POT") msgids in $POT"
