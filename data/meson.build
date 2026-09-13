desktop_file = i18n.merge_file(
  input: 'io.github.sniper1720.khushu.desktop.in',
  output: 'io.github.sniper1720.khushu.desktop',
  type: 'desktop',
  po_dir: '../po',
  install: true,
  install_dir: join_paths(datadir, 'applications')
)

appdata_file = i18n.merge_file(
  input: 'appdata/io.github.sniper1720.khushu.metainfo.xml.in',
  output: 'io.github.sniper1720.khushu.metainfo.xml',
  po_dir: '../po',
  install: true,
  install_dir: join_paths(datadir, 'metainfo')
)

configure_file(
  input: 'io.github.sniper1720.khushu.service.in',
  output: 'io.github.sniper1720.khushu.service',
  configuration: {'bindir': bindir},
  install: true,
  install_dir: join_paths(datadir, 'dbus-1', 'services')
)

install_man('khushu.1')

subdir('fonts')
subdir('icons')
