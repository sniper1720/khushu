project('khushu', 'rust',
  version: '1.2.0',
  meson_version: '>= 1.2.0',
)

i18n = import('i18n')
gnome = import('gnome')

prefix = get_option('prefix')
bindir = join_paths(prefix, get_option('bindir'))
datadir = join_paths(prefix, get_option('datadir'))
localedir = join_paths(prefix, get_option('localedir'))

khushu_langs = {
  'ar': 'ar_SA',
  'es': 'es_ES',
  'fr': 'fr_FR',
  'id': 'id_ID',
  'tr': 'tr_TR',
}

summary({
    'prefix': prefix,
    'bindir': bindir,
    'datadir': datadir,
    'localedir': localedir,
})

subdir('data')
subdir('po')
subdir('src')

generate_locales = get_option('generate_locales')
locales_to_generate = []
if generate_locales == 'auto'
  foreach lang, locale : khushu_langs
    locales_to_generate += locale + '.UTF-8'
  endforeach
elif generate_locales != ''
  locales_to_generate = generate_locales.split(',')
endif

if locales_to_generate.length() > 0
  locales_str = ','.join(locales_to_generate)
  meson.add_install_script(find_program('python3'), '-c', '''
import os, subprocess, sys
locales = sys.argv[1].split(",")
destdir = os.environ.get("DESTDIR", "")
prefix = os.environ["MESON_INSTALL_PREFIX"]
locpath = destdir + prefix + "/usr/lib/locale"
os.makedirs(locpath, exist_ok=True)
for loc in locales:
    base = loc.split(".")[0]
    subprocess.check_call([
        "localedef", "-f", "UTF-8", "-i", base, loc,
        "--prefix", destdir + prefix, "--no-archive",
    ])
''', locales_str)
endif
