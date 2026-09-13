use rodio::{Decoder, DeviceSinkBuilder, Player};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::io::Cursor;
use std::path::Path;
use std::rc::{Rc, Weak};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::thread;
use std::time::Duration;

use adw::prelude::*;
use gtk4::glib;
use libadwaita as adw;

use crate::config::AppConfig;
use crate::i18n::tr;

static AUDIO_SENDER: OnceLock<Sender<AudioCommand>> = OnceLock::new();
static BUILTIN_AUDIO: OnceLock<HashMap<String, Vec<u8>>> = OnceLock::new();

/// What the audio worker is currently playing. Adhan and recitation are
/// mutually exclusive: starting one always stops the other.
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
enum PlaybackKind {
    Idle = 0,
    Adhan = 1,
    Recitation = 2,
}

static PLAYBACK_KIND: AtomicU8 = AtomicU8::new(PlaybackKind::Idle as u8);

fn set_kind(kind: PlaybackKind) {
    PLAYBACK_KIND.store(kind as u8, Ordering::Release);
}

pub fn is_adhan() -> bool {
    PLAYBACK_KIND.load(Ordering::Acquire) == PlaybackKind::Adhan as u8
}

pub fn is_reciting() -> bool {
    PLAYBACK_KIND.load(Ordering::Acquire) == PlaybackKind::Recitation as u8
}

type RecitationStateCallback = Weak<dyn Fn(bool)>;
type VerseFinishedCallback = Weak<dyn Fn(u32, u32)>;

thread_local! {
    static RECITATION_STATE_CALLBACKS: RefCell<Vec<RecitationStateCallback>> =
        const { RefCell::new(Vec::new()) };
    static VERSE_FINISHED_CALLBACKS: RefCell<Vec<VerseFinishedCallback>> =
        const { RefCell::new(Vec::new()) };
}

enum AudioCommand {
    Play(String, f32),
    PlayVerse(String, u32, u32),
    PlaySurah(String, u32, u32, u32),
    Stop,
}

struct DownloadRequest {
    reciter: String,
    surah_number: u32,
    verse: u32,
}

struct DownloadResult {
    surah_number: u32,
    verse: u32,
    ok: bool,
}

fn spawn_downloader(result_tx: Sender<DownloadResult>) -> Sender<DownloadRequest> {
    let (req_tx, req_rx) = channel::<DownloadRequest>();
    thread::spawn(move || {
        while let Ok(req) = req_rx.recv() {
            let ok = download_verse(&req.reciter, req.surah_number, req.verse);
            let _ = result_tx.send(DownloadResult {
                surah_number: req.surah_number,
                verse: req.verse,
                ok,
            });
        }
    });
    req_tx
}

fn ensure_audio_thread() -> &'static Sender<AudioCommand> {
    AUDIO_SENDER.get_or_init(|| {
        let (sender, receiver) = channel();
        thread::spawn(move || {
            run_audio_loop(receiver);
        });
        sender
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

pub fn play_verse(reciter: &str, surah_number: u32, verse: u32) {
    let _ = ensure_audio_thread().send(AudioCommand::PlayVerse(
        reciter.to_string(),
        surah_number,
        verse,
    ));
}

pub fn play_surah(reciter: &str, surah_number: u32, start_verse: u32, end_verse: u32) {
    let _ = ensure_audio_thread().send(AudioCommand::PlaySurah(
        reciter.to_string(),
        surah_number,
        start_verse,
        end_verse,
    ));
}

pub fn register_recitation_state_callback(callback: &Rc<dyn Fn(bool)>) {
    RECITATION_STATE_CALLBACKS.with(|registry| {
        registry.borrow_mut().push(Rc::downgrade(callback));
    });
}

pub fn register_verse_finished_callback(callback: &Rc<dyn Fn(u32, u32)>) {
    VERSE_FINISHED_CALLBACKS.with(|registry| {
        registry.borrow_mut().push(Rc::downgrade(callback));
    });
}

// Worker-thread callers; hop to the main context, fan out to live
// subscribers, and drop callbacks whose view was torn down.
fn notify_recitation_state(is_reciting: bool) {
    glib::MainContext::default().invoke(move || {
        RECITATION_STATE_CALLBACKS.with(|registry| {
            let mut callbacks = registry.borrow_mut();
            callbacks.retain(|callback| callback.upgrade().is_some());
            for callback in callbacks.iter() {
                if let Some(callback) = callback.upgrade() {
                    callback(is_reciting);
                }
            }
        });
    });
}

fn notify_verse_finished(surah_number: u32, verse: u32) {
    glib::MainContext::default().invoke(move || {
        VERSE_FINISHED_CALLBACKS.with(|registry| {
            let mut callbacks = registry.borrow_mut();
            callbacks.retain(|callback| callback.upgrade().is_some());
            for callback in callbacks.iter() {
                if let Some(callback) = callback.upgrade() {
                    callback(surah_number, verse);
                }
            }
        });
    });
}

fn validate_audio_file(path: &str) -> bool {
    std::fs::File::open(path)
        .ok()
        .and_then(|file| Decoder::new(std::io::BufReader::new(file)).ok())
        .is_some()
}

pub fn validate_audio_async(path: String, combo: adw::ComboRow, parent: adw::ApplicationWindow) {
    if validate_audio_file(&path) {
        let config = AppConfig::load();
        config.set_adhan_sound_path(Some(path.clone()));
        config.save();
        gtk4::glib::spawn_future_local(async move {
            combo.set_subtitle(&path);
        });
    } else if let Some(overlay) = crate::settings_ui::find_toast_overlay(&parent) {
        gtk4::glib::spawn_future_local(async move {
            overlay.add_toast(adw::Toast::new(&tr(
                "File not usable or unsupported format",
            )));
        });
    }
}

fn get_builtin_bytes(path_str: &str) -> Option<&'static [u8]> {
    let file_name = path_str
        .trim_start_matches("assets/audio/")
        .trim_start_matches("assets/");
    BUILTIN_AUDIO
        .get()
        .and_then(|map| map.get(file_name))
        .map(|value| value.as_slice())
}

const FALLBACK_KEY: &str = "Madinah.mp3";

fn try_play_custom(path_str: &str, player: &Player) -> bool {
    if let Ok(file) = std::fs::File::open(path_str) {
        if let Ok(decoder) = Decoder::new(std::io::BufReader::new(file)) {
            player.append(decoder);
            return true;
        } else {
            log::error!("Failed to decode audio file: {}", path_str);
        }
    } else {
        log::error!("Failed to open audio file: {}", path_str);
    }
    false
}

fn try_play_builtin(path_str: &str, player: &Player) -> bool {
    if let Some(bytes) = get_builtin_bytes(path_str)
        && let Ok(decoder) = Decoder::new(Cursor::new(bytes))
    {
        player.append(decoder);
        return true;
    }
    log::error!("Builtin audio not available: {}", path_str);
    false
}

pub(crate) fn cache_path(reciter: &str, surah_number: u32, verse: u32) -> String {
    format!(
        "{}/khushu/recitations/{}/{:03}{:03}.mp3",
        glib::user_cache_dir().to_string_lossy(),
        reciter,
        surah_number,
        verse
    )
}

pub(crate) fn download_verse(reciter: &str, surah_number: u32, verse: u32) -> bool {
    let path = cache_path(reciter, surah_number, verse);
    if Path::new(&path).exists() {
        return true;
    }
    let url = format!(
        "https://everyayah.com/data/{}/{:03}{:03}.mp3",
        reciter, surah_number, verse
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
            Err(err) => {
                log::error!("Failed to read response bytes for {}: {}", url, err);
                false
            }
        },
        Err(err) => {
            log::error!("Failed to download {}: {}", url, err);
            false
        }
    }
}

fn load_verse_to_player(reciter: &str, surah_number: u32, verse: u32, player: &Player) -> bool {
    let path = cache_path(reciter, surah_number, verse);
    if let Ok(file) = std::fs::File::open(&path) {
        if let Ok(decoder) = Decoder::new(std::io::BufReader::new(file)) {
            player.append(decoder);
            return true;
        } else {
            log::error!("Failed to decode cached verse: {}", path);
        }
    }
    false
}

fn run_audio_loop(receiver: Receiver<AudioCommand>) {
    let device_sink = match DeviceSinkBuilder::open_default_sink() {
        Ok(sink) => sink,
        Err(err) => {
            log::error!("Failed to open default audio output device: {}", err);
            return;
        }
    };

    let (result_tx, result_rx) = channel::<DownloadResult>();
    let dl_tx = spawn_downloader(result_tx);

    let mut current_player: Option<Player> = None;
    let mut is_reciting = false;
    let mut current_reciter = String::new();
    let mut current_verse_surah_number: u32 = 0;
    let mut current_verse_num: u32 = 0;
    let mut end_verse: u32 = 0;
    let mut verse_queue: Vec<u32> = vec![];
    let mut pending_first_verse = false;
    let mut last_preloaded: u32 = 0;
    let mut prev_player_len: usize = 0;
    let mut downloaded: HashSet<(u32, u32)> = HashSet::new();

    loop {
        while let Ok(result) = result_rx.try_recv() {
            if result.ok {
                downloaded.insert((result.surah_number, result.verse));
            }
        }

        match receiver.recv_timeout(Duration::from_millis(200)) {
            Ok(command) => match command {
                AudioCommand::Play(path_str, volume) => {
                    current_player = None;
                    verse_queue.clear();
                    is_reciting = false;
                    pending_first_verse = false;
                    set_kind(PlaybackKind::Adhan);
                    crate::settings_ui::on_audio_state_changed(true);
                    notify_recitation_state(false);

                    let player = Player::connect_new(device_sink.mixer());
                    player.set_volume(volume.clamp(0.0, 1.0));

                    let is_asset = path_str.starts_with("assets/");
                    let played = if is_asset {
                        try_play_builtin(&path_str, &player)
                    } else {
                        try_play_custom(&path_str, &player)
                    };

                    if !played {
                        log::warn!(
                            "Audio playback failed for '{}', falling back to builtin",
                            path_str
                        );
                        if !try_play_builtin(FALLBACK_KEY, &player) {
                            log::error!("No fallback audio available");
                            set_kind(PlaybackKind::Idle);
                            crate::settings_ui::on_audio_state_changed(false);
                            notify_recitation_state(false);
                            continue;
                        }
                    }
                    current_player = Some(player);
                }
                AudioCommand::PlayVerse(reciter, surah_number, verse) => {
                    log::info!(
                        "PlayVerse: reciter={}, surah_number={}, verse={}",
                        reciter,
                        surah_number,
                        verse,
                    );
                    verse_queue.clear();
                    is_reciting = true;
                    current_reciter = reciter.clone();
                    current_verse_surah_number = surah_number;
                    current_verse_num = verse;
                    downloaded.clear();
                    pending_first_verse = true;
                    set_kind(PlaybackKind::Recitation);
                    crate::settings_ui::on_audio_state_changed(false);
                    notify_recitation_state(true);

                    let player = Player::connect_new(device_sink.mixer());

                    let _ = dl_tx.send(DownloadRequest {
                        reciter,
                        surah_number,
                        verse,
                    });

                    current_player = Some(player);
                }
                AudioCommand::PlaySurah(reciter, surah_number, start_verse, end_verse_val) => {
                    log::info!(
                        "PlaySurah: reciter={}, surah_number={}, start_verse={}, end_verse={}",
                        reciter,
                        surah_number,
                        start_verse,
                        end_verse_val,
                    );
                    verse_queue.clear();
                    downloaded.clear();
                    is_reciting = true;
                    current_reciter = reciter.clone();
                    current_verse_surah_number = surah_number;
                    current_verse_num = start_verse;
                    end_verse = end_verse_val;
                    pending_first_verse = true;
                    set_kind(PlaybackKind::Recitation);
                    crate::settings_ui::on_audio_state_changed(false);
                    notify_recitation_state(true);

                    let player = Player::connect_new(device_sink.mixer());

                    let _ = dl_tx.send(DownloadRequest {
                        reciter: reciter.clone(),
                        surah_number,
                        verse: start_verse,
                    });

                    if start_verse < end_verse_val {
                        let preload_end = (start_verse + 10).min(end_verse_val);
                        let preload_reciter = reciter.clone();
                        for verse_number in (start_verse + 1)..=preload_end {
                            let _ = dl_tx.send(DownloadRequest {
                                reciter: preload_reciter.clone(),
                                surah_number,
                                verse: verse_number,
                            });
                        }
                    }

                    verse_queue = if start_verse < end_verse_val {
                        (start_verse + 1..=end_verse_val).collect()
                    } else {
                        vec![]
                    };
                    current_player = Some(player);
                }
                AudioCommand::Stop => {
                    if let Some(player) = current_player.take() {
                        player.stop();
                    }
                    verse_queue.clear();
                    downloaded.clear();
                    is_reciting = false;
                    pending_first_verse = false;
                    set_kind(PlaybackKind::Idle);
                    crate::settings_ui::on_audio_state_changed(false);
                    notify_recitation_state(false);
                }
            },
            Err(RecvTimeoutError::Timeout) => {
                if is_reciting && let Some(player) = current_player.as_ref() {
                    let current_len = player.len();

                    if !pending_first_verse && prev_player_len >= 2 && current_len < prev_player_len
                    {
                        let finished = last_preloaded - 1;
                        if finished >= 1 {
                            notify_verse_finished(current_verse_surah_number, finished);
                        }
                    }

                    if pending_first_verse {
                        if downloaded.remove(&(current_verse_surah_number, current_verse_num))
                            && load_verse_to_player(
                                &current_reciter,
                                current_verse_surah_number,
                                current_verse_num,
                                player,
                            )
                        {
                            pending_first_verse = false;
                        }
                    } else if current_len <= 1 && !verse_queue.is_empty() {
                        let next = verse_queue[0];
                        if downloaded.remove(&(current_verse_surah_number, next)) {
                            verse_queue.remove(0);
                            if load_verse_to_player(
                                &current_reciter,
                                current_verse_surah_number,
                                next,
                                player,
                            ) {
                                current_verse_num = next;
                                last_preloaded = next;

                                let next_preload = next + 10;
                                if next_preload <= end_verse
                                    && !downloaded
                                        .contains(&(current_verse_surah_number, next_preload))
                                {
                                    let _ = dl_tx.send(DownloadRequest {
                                        reciter: current_reciter.clone(),
                                        surah_number: current_verse_surah_number,
                                        verse: next_preload,
                                    });
                                }
                            }
                        }
                    }
                    prev_player_len = current_len;
                    if player.empty() && !pending_first_verse && verse_queue.is_empty() {
                        notify_verse_finished(current_verse_surah_number, current_verse_num);
                        is_reciting = false;
                        set_kind(PlaybackKind::Idle);
                        crate::settings_ui::on_audio_state_changed(false);
                        notify_recitation_state(false);
                        current_player = None;
                    }
                } else if let Some(player) = current_player.as_ref()
                    && player.empty()
                {
                    set_kind(PlaybackKind::Idle);
                    crate::settings_ui::on_audio_state_changed(false);
                    notify_recitation_state(false);
                    current_player = None;
                }
            }
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
}
