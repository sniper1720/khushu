use adw::prelude::*;
use gtk4 as gtk;
use gtk4::glib;
use libadwaita as adw;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

use crate::config::AppConfig;
use crate::i18n::tr;

#[derive(Clone, Debug)]
pub(crate) struct ReciterInfo {
    pub(crate) display: &'static str,
    pub(crate) slug: &'static str,
}

pub(crate) const RECITERS: &[ReciterInfo] = &[
    ReciterInfo {
        display: "Mishary Alafasy",
        slug: "Alafasy_128kbps",
    },
    ReciterInfo {
        display: "Abdul Basit",
        slug: "Abdul_Basit_Murattal_192kbps",
    },
    ReciterInfo {
        display: "Al-Husary",
        slug: "Husary_128kbps",
    },
    ReciterInfo {
        display: "Al-Ghamdi",
        slug: "Ghamadi_40kbps",
    },
    ReciterInfo {
        display: "Al-Huthaify",
        slug: "Hudhaify_128kbps",
    },
    ReciterInfo {
        display: "Al-Menshawy",
        slug: "Minshawy_Murattal_128kbps",
    },
    ReciterInfo {
        display: "Al-Shatri",
        slug: "Abu_Bakr_Ash-Shaatree_128kbps",
    },
];

fn count_downloaded_verses(reciter_slug: &str) -> usize {
    let cache_dir = format!(
        "{}/khushu/recitations/{}",
        glib::user_cache_dir().to_string_lossy(),
        reciter_slug
    );
    let path = std::path::Path::new(&cache_dir);
    if !path.exists() {
        return 0;
    }
    std::fs::read_dir(path)
        .map(|entries| entries.filter_map(|entry| entry.ok()).count())
        .unwrap_or(0)
}

fn total_quran_verses() -> u32 {
    let mut total = 0;
    for surah_num in 1..=114 {
        total += crate::quran::surah_total_verses(surah_num).unwrap_or(0);
    }
    total
}

fn delete_downloaded_reciter(reciter_slug: &str) {
    let cache_dir = format!(
        "{}/khushu/recitations/{}",
        glib::user_cache_dir().to_string_lossy(),
        reciter_slug
    );
    let _ = std::fs::remove_dir_all(&cache_dir);
}

pub(crate) fn open_reciter_dialog(
    parent: &impl IsA<gtk::Widget>,
    config: AppConfig,
    label: gtk::Label,
) {
    // RECITERS: Quran reciter display names — expose to xgettext
    if false {
        tr("Mishary Alafasy");
        tr("Abdul Basit");
        tr("Al-Husary");
        tr("Al-Ghamdi");
        tr("Al-Huthaify");
        tr("Al-Menshawy");
        tr("Al-Shatri");
    }
    let total = total_quran_verses();

    let dialog = adw::Dialog::builder()
        .title(tr("Reciter"))
        .content_width(420)
        .content_height(500)
        .css_classes(["view"])
        .build();

    let toolbar_view = adw::ToolbarView::new();

    let header = adw::HeaderBar::new();
    toolbar_view.add_top_bar(&header);

    let search_entry = gtk::SearchEntry::builder()
        .placeholder_text(tr("Search reciters..."))
        .hexpand(true)
        .build();

    let search_bar = gtk::SearchBar::builder().search_mode_enabled(true).build();
    search_bar.set_child(Some(&search_entry));

    let search_bin = adw::Bin::builder()
        .css_classes(["toolbar"])
        .child(&search_bar)
        .build();
    toolbar_view.add_top_bar(&search_bin);

    let scrolled = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .build();
    let list_box = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .activate_on_single_click(true)
        .show_separators(true)
        .build();
    scrolled.set_child(Some(&list_box));

    let clamp = adw::Clamp::builder()
        .orientation(gtk::Orientation::Horizontal)
        .maximum_size(420)
        .build();
    clamp.set_child(Some(&scrolled));

    toolbar_view.set_content(Some(&clamp));
    dialog.set_child(Some(&toolbar_view));

    let verses_template = tr("{} / {} verses downloaded");
    let not_downloaded = tr("Not downloaded");
    let delete_label = tr("Delete");
    let delete_tooltip = tr("Delete downloaded verses");
    let download_label = tr("Download");
    let download_tooltip = tr("Download for offline use");
    let downloading_label = tr("Downloading...");

    let btn_map: Rc<RefCell<HashMap<String, gtk::Button>>> = Rc::new(RefCell::new(HashMap::new()));
    let (dl_tx, dl_rx) = std::sync::mpsc::channel::<(String, i32)>();

    for (reciter_index, reciter) in RECITERS.iter().enumerate() {
        let row = adw::ActionRow::new();
        row.set_title(&tr(reciter.display));
        row.set_activatable(true);

        let downloaded = count_downloaded_verses(reciter.slug);
        if downloaded > 0 {
            row.set_subtitle(
                &verses_template
                    .replacen("{}", &downloaded.to_string(), 1)
                    .replacen("{}", &total.to_string(), 1),
            );
        } else {
            row.set_subtitle(&not_downloaded);
        }

        let action_btn = gtk::Button::new();
        action_btn.set_valign(gtk::Align::Center);
        if downloaded > 0 {
            action_btn.set_label(&delete_label);
            action_btn.set_tooltip_text(Some(&delete_tooltip));
        } else {
            action_btn.set_label(&download_label);
            action_btn.set_tooltip_text(Some(&download_tooltip));
        }
        row.add_suffix(&action_btn);

        let slug = reciter.slug.to_string();
        let row_index = reciter_index as i32;
        btn_map
            .borrow_mut()
            .insert(slug.clone(), action_btn.clone());

        let slug_c = slug.clone();
        let row_c = row.clone();
        let sender = dl_tx.clone();
        let not_downloaded_c = not_downloaded.clone();
        let download_label_c = download_label.clone();
        let downloading_label_c = downloading_label.clone();
        action_btn.connect_clicked(move |btn| {
            let downloaded_now = count_downloaded_verses(&slug_c);
            if downloaded_now > 0 {
                delete_downloaded_reciter(&slug_c);
                btn.set_label(&download_label_c);
                row_c.set_subtitle(&not_downloaded_c);
            } else {
                btn.set_label(&downloading_label_c);
                btn.set_sensitive(false);
                let sender_c = sender.clone();
                let row_index_c = row_index;
                let slug_for_thread = slug_c.clone();
                std::thread::spawn(move || {
                    for surah_num in 1..=114 {
                        let verses = crate::quran::surah_total_verses(surah_num).unwrap_or(0);
                        for verse in 1..=verses {
                            crate::audio::download_verse(&slug_for_thread, surah_num, verse);
                        }
                    }
                    let _ = sender_c.send((slug_for_thread, row_index_c));
                });
            }
        });

        row.connect_activated({
            let cfg = config.clone();
            let selected_slug = slug.clone();
            let label_c = label.clone();
            let dialog_c = dialog.clone();
            let display = reciter.display;
            move |_| {
                cfg.set_reciter_slug(&selected_slug);
                cfg.save();
                label_c.set_label(&tr(display));
                dialog_c.close();
            }
        });

        list_box.append(&row);
    }

    let list_box_c = list_box.clone();
    search_entry.connect_search_changed(move |entry| {
        let text = entry.text().to_lowercase();
        let model = list_box_c.observe_children();
        for index in 0..model.n_items() {
            if let Some(obj) = model.item(index)
                && let Some(row) = obj.downcast_ref::<gtk::ListBoxRow>()
                && let Some(action_row) = row.downcast_ref::<adw::ActionRow>()
            {
                let title = action_row.title().to_lowercase();
                row.set_visible(text.is_empty() || title.contains(&text));
            }
        }
    });

    let total_c = total;
    let dl_rx = std::sync::Mutex::new(dl_rx);
    let btn_map_c = btn_map.clone();
    let list_box_poll_c = list_box.clone();
    let verses_template_c = verses_template.clone();
    let delete_label_c = delete_label.clone();

    let download_poll: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));
    let download_poll_on_close = download_poll.clone();
    dialog.connect_closed(move |_| {
        if let Some(poll_source) = download_poll_on_close.borrow_mut().take() {
            poll_source.remove();
        }
    });

    let download_poll_id = glib::timeout_add_local(Duration::from_millis(200), move || {
        while let Ok((slug, index)) = dl_rx.lock().unwrap().try_recv() {
            if let Some(btn) = btn_map_c.borrow().get(&slug) {
                btn.set_label(&delete_label_c);
                btn.set_sensitive(true);
            }
            if let Some(row) = list_box_poll_c.row_at_index(index)
                && let Some(action_row) = row.downcast_ref::<adw::ActionRow>()
            {
                action_row.set_subtitle(
                    &verses_template_c
                        .replacen("{}", &total_c.to_string(), 1)
                        .replacen("{}", &total_c.to_string(), 1),
                );
            }
        }
        glib::ControlFlow::Continue
    });
    *download_poll.borrow_mut() = Some(download_poll_id);

    dialog.present(Some(parent));
}
