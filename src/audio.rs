use rodio::{Decoder, OutputStreamBuilder, Sink};
use std::collections::{HashMap, HashSet};
use std::io::Cursor;
use std::path::Path;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::thread;
use std::time::Duration;

use adw::prelude::*;
use gtk4::glib;
use libadwaita as adw;

use crate::config::AppConfig;

static AUDIO_SENDER: OnceLock<Sender<AudioCommand>> = OnceLock::new();
static IS_PLAYING: AtomicBool = AtomicBool::new(false);
static BUILTIN_AUDIO: OnceLock<HashMap<String, Vec<u8>>> = OnceLock::new();

pub(crate) static RECITATION_ACTIVE: AtomicBool = AtomicBool::new(false);
pub(crate) static LAST_FINISHED_SURAH: AtomicU32 = AtomicU32::new(0);
pub(crate) static LAST_FINISHED_VERSE: AtomicU32 = AtomicU32::new(0);
pub(crate) static VERSE_FINISHED_PENDING: AtomicBool = AtomicBool::new(false);

enum AudioCommand {
    Play(String, f32),
    PlayVerse(String, u32, u32),
    PlaySurah(String, u32, u32, u32),
    Stop,
}

struct DlRequest {
    reciter: String,
    surah: u32,
    verse: u32,
}

struct DlResult {
    surah: u32,
    verse: u32,
    ok: bool,
}

fn spawn_downloader(res_tx: Sender<DlResult>) -> Sender<DlRequest> {
    let (req_tx, req_rx) = channel::<DlRequest>();
    thread::spawn(move || {
        while let Ok(req) = req_rx.recv() {
            let ok = download_verse(&req.reciter, req.surah, req.verse);
            let _ = res_tx.send(DlResult {
                surah: req.surah,
                verse: req.verse,
                ok,
            });
        }
    });
    req_tx
}

fn ensure_audio_thread() -> &'static Sender<AudioCommand> {
    AUDIO_SENDER.get_or_init(|| {
        let (tx, rx) = channel();
        thread::spawn(move || {
            run_audio_loop(rx);
        });
        tx
    })
}

pub fn preload_builtin_audio() {
    BUILTIN_AUDIO.get_or_init(|| {
        let mut map = HashMap::new();
        let presets = [
            ("Madinah.mp3", "audio/Madinah.mp3"),
            ("Makkah.mp3", "audio/Makkah.mp3"),
        ];
        for (key, resource_path) in presets {
            if let Ok(bytes) = gtk4::gio::resources_lookup_data(
                &format!("/io/github/sniper1720/khushu/{resource_path}"),
                gtk4::gio::ResourceLookupFlags::NONE,
            ) {
                map.insert(key.to_string(), bytes.to_vec());
            } else {
                log::error!("Failed to preload builtin audio: {resource_path}");
            }
        }
        map
    });
}

pub fn play_adhan(path_str: &str, volume: f32) {
    let _ = ensure_audio_thread().send(AudioCommand::Play(path_str.to_string(), volume));
}

pub fn stop() {
    let _ = ensure_audio_thread().send(AudioCommand::Stop);
}

pub fn is_playing() -> bool {
    IS_PLAYING.load(Ordering::Acquire)
}

pub fn play_verse(reciter: &str, surah: u32, verse: u32) {
    let _ = ensure_audio_thread().send(AudioCommand::PlayVerse(
        reciter.to_string(),
        surah,
        verse,
    ));
}

pub fn play_surah(reciter: &str, surah: u32, start_verse: u32, end_verse: u32) {
    let _ = ensure_audio_thread().send(AudioCommand::PlaySurah(
        reciter.to_string(),
        surah,
        start_verse,
        end_verse,
    ));
}

pub fn poll_verse_finished() -> Option<(u32, u32)> {
    if VERSE_FINISHED_PENDING.swap(false, Ordering::Acquire) {
        let surah = LAST_FINISHED_SURAH.load(Ordering::Relaxed);
        let verse = LAST_FINISHED_VERSE.load(Ordering::Relaxed);
        Some((surah, verse))
    } else {
        None
    }
}

fn validate_audio_file(path: &str) -> bool {
    std::fs::File::open(path)
        .ok()
        .and_then(|file| Decoder::new(std::io::BufReader::new(file)).ok())
        .is_some()
}

pub fn validate_audio_async(path: String, combo: adw::ComboRow, parent: adw::ApplicationWindow) {
    if validate_audio_file(&path) {
        let c = AppConfig::load();
        c.set_adhan_sound_path(Some(path.clone()));
        c.save();
        gtk4::glib::spawn_future_local(async move {
            combo.set_subtitle(&path);
        });
    } else if let Some(overlay) = find_toast_overlay(&parent) {
        gtk4::glib::spawn_future_local(async move {
            overlay.add_toast(adw::Toast::new(&crate::i18n::tr(
                "File not usable or unsupported format",
            )));
        });
    }
}

fn find_toast_overlay(window: &adw::ApplicationWindow) -> Option<adw::ToastOverlay> {
    let mut child = window.first_child();
    while let Some(w) = child {
        if let Some(o) = w.downcast_ref::<adw::ToastOverlay>() {
            return Some(o.clone());
        }
        child = w.next_sibling();
    }
    None
}

fn get_builtin_bytes(path_str: &str) -> Option<&'static [u8]> {
    let file_name = path_str
        .trim_start_matches("assets/audio/")
        .trim_start_matches("assets/");
    BUILTIN_AUDIO
        .get()
        .and_then(|map| map.get(file_name))
        .map(|v| v.as_slice())
}

const FALLBACK_KEY: &str = "Madinah.mp3";

fn try_play_custom(path_str: &str, sink: &Sink) -> bool {
    if let Ok(file) = std::fs::File::open(path_str) {
        if let Ok(decoder) = Decoder::new(std::io::BufReader::new(file)) {
            sink.append(decoder);
            return true;
        } else {
            log::error!("Failed to decode audio file: {}", path_str);
        }
    } else {
        log::error!("Failed to open audio file: {}", path_str);
    }
    false
}

fn try_play_builtin(path_str: &str, sink: &Sink) -> bool {
    if let Some(bytes) = get_builtin_bytes(path_str)
        && let Ok(decoder) = Decoder::new(Cursor::new(bytes))
    {
        sink.append(decoder);
        return true;
    }
    log::error!("Builtin audio not available: {}", path_str);
    false
}

pub(crate) fn cache_path(reciter: &str, surah: u32, verse: u32) -> String {
    format!(
        "{}/khushu/recitations/{}/{:03}{:03}.mp3",
        glib::user_cache_dir().to_string_lossy(),
        reciter,
        surah,
        verse
    )
}

pub(crate) fn download_verse(reciter: &str, surah: u32, verse: u32) -> bool {
    let path = cache_path(reciter, surah, verse);
    if Path::new(&path).exists() {
        return true;
    }
    let url = format!(
        "https://everyayah.com/data/{}/{:03}{:03}.mp3",
        reciter, surah, verse
    );
    log::info!("Downloading: {}", url);
    match reqwest::blocking::get(&url) {
        Ok(resp) => match resp.bytes() {
            Ok(bytes) => {
                if let Some(parent) = Path::new(&path).parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                std::fs::write(&path, &bytes).ok();
                true
            }
            Err(e) => {
                log::error!("Failed to read response bytes for {}: {}", url, e);
                false
            }
        },
        Err(e) => {
            log::error!("Failed to download {}: {}", url, e);
            false
        }
    }
}

fn load_verse_to_sink(reciter: &str, surah: u32, verse: u32, sink: &Sink) -> bool {
    let path = cache_path(reciter, surah, verse);
    if let Ok(file) = std::fs::File::open(&path) {
        if let Ok(decoder) = Decoder::new(std::io::BufReader::new(file)) {
            sink.append(decoder);
            return true;
        } else {
            log::error!("Failed to decode cached verse: {}", path);
        }
    }
    false
}

fn run_audio_loop(rx: Receiver<AudioCommand>) {
    let stream = match OutputStreamBuilder::open_default_stream() {
        Ok(s) => s,
        Err(e) => {
            log::error!("Failed to open default audio output stream: {}", e);
            return;
        }
    };

    let (result_tx, result_rx) = channel::<DlResult>();
    let dl_tx = spawn_downloader(result_tx);

    let mut current_sink: Option<Sink> = None;
    let mut is_reciting = false;
    let mut _current_reciter = String::new();
    let mut current_verse_surah: u32 = 0;
    let mut current_verse_num: u32 = 0;
    let mut end_verse: u32 = 0;
    let mut verse_queue: Vec<u32> = vec![];
    let mut pending_first_verse = false;
    let mut last_preloaded: u32 = 0;
    let mut prev_sink_len: usize = 0;
    let mut downloaded: HashSet<(u32, u32)> = HashSet::new();

    loop {
        while let Ok(result) = result_rx.try_recv() {
            if result.ok {
                downloaded.insert((result.surah, result.verse));
            }
        }

        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(command) => match command {
                AudioCommand::Play(path_str, volume) => {
                    current_sink = None;
                    verse_queue.clear();
                    is_reciting = false;
                    pending_first_verse = false;
                    RECITATION_ACTIVE.store(false, Ordering::Relaxed);
                    IS_PLAYING.store(true, Ordering::Release);
                    crate::settings_ui::on_audio_state_changed(true);

                    let sink = Sink::connect_new(stream.mixer());
                    sink.set_volume(volume.clamp(0.0, 1.0));

                    let is_asset = path_str.starts_with("assets/");
                    let played = if is_asset {
                        try_play_builtin(&path_str, &sink)
                    } else {
                        try_play_custom(&path_str, &sink)
                    };

                    if !played {
                        log::warn!(
                            "Audio playback failed for '{}', falling back to builtin",
                            path_str
                        );
                        if !try_play_builtin(FALLBACK_KEY, &sink) {
                            log::error!("No fallback audio available");
                            IS_PLAYING.store(false, Ordering::Release);
                            crate::settings_ui::on_audio_state_changed(false);
                            continue;
                        }
                    }
                    current_sink = Some(sink);
                }
                AudioCommand::PlayVerse(reciter, surah, verse) => {
                    log::info!(
                        "PlayVerse: reciter={}, surah={}, verse={}",
                        reciter,
                        surah,
                        verse,
                    );
                    verse_queue.clear();
                    is_reciting = true;
                    _current_reciter = reciter.clone();
                    current_verse_surah = surah;
                    current_verse_num = verse;
                    downloaded.clear();
                    pending_first_verse = true;
                    RECITATION_ACTIVE.store(true, Ordering::Relaxed);
                    IS_PLAYING.store(true, Ordering::Release);

                    let sink = Sink::connect_new(stream.mixer());

                    let _ = dl_tx.send(DlRequest {
                        reciter,
                        surah,
                        verse,
                    });

                    current_sink = Some(sink);
                }
                AudioCommand::PlaySurah(reciter, surah, start_verse, end_verse_val) => {
                    log::info!(
                        "PlaySurah: reciter={}, surah={}, start_verse={}, end_verse={}",
                        reciter,
                        surah,
                        start_verse,
                        end_verse_val,
                    );
                    verse_queue.clear();
                    downloaded.clear();
                    is_reciting = true;
                    _current_reciter = reciter.clone();
                    current_verse_surah = surah;
                    current_verse_num = start_verse;
                    end_verse = end_verse_val;
                    pending_first_verse = true;
                    RECITATION_ACTIVE.store(true, Ordering::Relaxed);
                    IS_PLAYING.store(true, Ordering::Release);

                    let sink = Sink::connect_new(stream.mixer());

                    let _ = dl_tx.send(DlRequest {
                        reciter: reciter.clone(),
                        surah,
                        verse: start_verse,
                    });

                    if start_verse < end_verse_val {
                        let preload_end = (start_verse + 10).min(end_verse_val);
                        let preload_reciter = reciter.clone();
                        for v in (start_verse + 1)..=preload_end {
                            let _ = dl_tx.send(DlRequest {
                                reciter: preload_reciter.clone(),
                                surah,
                                verse: v,
                            });
                        }
                    }

                    verse_queue = if start_verse < end_verse_val {
                        (start_verse + 1..=end_verse_val).collect()
                    } else {
                        vec![]
                    };
                    current_sink = Some(sink);
                }
                AudioCommand::Stop => {
                    if let Some(sink) = current_sink.take() {
                        sink.stop();
                    }
                    verse_queue.clear();
                    downloaded.clear();
                    is_reciting = false;
                    pending_first_verse = false;
                    RECITATION_ACTIVE.store(false, Ordering::Relaxed);
                    IS_PLAYING.store(false, Ordering::Release);
                    crate::settings_ui::on_audio_state_changed(false);
                }
            },
            Err(RecvTimeoutError::Timeout) => {
                if is_reciting && let Some(sink) = current_sink.as_ref() {
                    let current_len = sink.len();

                    if !pending_first_verse && prev_sink_len >= 2 && current_len < prev_sink_len {
                        let finished = last_preloaded - 1;
                        if finished >= 1 {
                            LAST_FINISHED_SURAH.store(current_verse_surah, Ordering::Relaxed);
                            LAST_FINISHED_VERSE.store(finished, Ordering::Relaxed);
                            VERSE_FINISHED_PENDING.store(true, Ordering::Release);
                        }
                    }

                    if pending_first_verse {
                        if downloaded.remove(&(current_verse_surah, current_verse_num))
                            && load_verse_to_sink(
                                &_current_reciter,
                                current_verse_surah,
                                current_verse_num,
                                sink,
                            )
                        {
                            pending_first_verse = false;
                        }
                    } else if current_len <= 1 && !verse_queue.is_empty() {
                        let next = verse_queue[0];
                        if downloaded.remove(&(current_verse_surah, next)) {
                            verse_queue.remove(0);
                            if load_verse_to_sink(
                                &_current_reciter,
                                current_verse_surah,
                                next,
                                sink,
                            ) {
                                current_verse_num = next;
                                last_preloaded = next;

                                let next_preload = next + 10;
                                if next_preload <= end_verse
                                    && !downloaded.contains(&(current_verse_surah, next_preload))
                                {
                                    let _ = dl_tx.send(DlRequest {
                                        reciter: _current_reciter.clone(),
                                        surah: current_verse_surah,
                                        verse: next_preload,
                                    });
                                }
                            }
                        }
                    }
                    prev_sink_len = current_len;
                    if sink.empty() && !pending_first_verse && verse_queue.is_empty() {
                        LAST_FINISHED_SURAH.store(current_verse_surah, Ordering::Relaxed);
                        LAST_FINISHED_VERSE.store(current_verse_num, Ordering::Relaxed);
                        VERSE_FINISHED_PENDING.store(true, Ordering::Release);
                        is_reciting = false;
                        RECITATION_ACTIVE.store(false, Ordering::Relaxed);
                        IS_PLAYING.store(false, Ordering::Release);
                        crate::settings_ui::on_audio_state_changed(false);
                        current_sink = None;
                    }
                }
            }
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
}
