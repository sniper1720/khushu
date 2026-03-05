use rodio::buffer::SamplesBuffer;
use rodio::source::Source;
use rodio::{Decoder, OutputStreamBuilder, Sink};
use std::collections::HashMap;

use std::sync::OnceLock;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread;

static GLOBAL_STOP_SENDER: OnceLock<Sender<AudioCommand>> = OnceLock::new();

enum AudioCommand {
    Play(String, f32),
    Stop,
}

#[derive(Clone)]
pub struct AudioManager {
    sender: Sender<AudioCommand>,
}

impl AudioManager {
    pub fn new() -> Self {
        let (tx, rx) = channel();
        let _ = GLOBAL_STOP_SENDER.set(tx.clone());

        thread::spawn(move || {
            run_audio_loop(rx);
        });

        Self { sender: tx }
    }

    pub fn play_adhan(&self, path_str: &str, volume: f32) {
        let _ = self
            .sender
            .send(AudioCommand::Play(path_str.to_string(), volume));
    }

    pub fn stop(&self) {
        let _ = self.sender.send(AudioCommand::Stop);
    }

    pub fn stop_global() {
        if let Some(tx) = GLOBAL_STOP_SENDER.get() {
            let _ = tx.send(AudioCommand::Stop);
        }
    }
}

type CachedAudio = (Vec<f32>, u16, u32);

fn run_audio_loop(rx: Receiver<AudioCommand>) {
    let stream = match OutputStreamBuilder::open_default_stream() {
        Ok(s) => s,
        Err(e) => {
            log::error!("Failed to open default audio output stream: {}", e);
            return;
        }
    };

    let mut _current_sink: Option<Sink> = None;
    let mut cache: HashMap<String, CachedAudio> = HashMap::new();

    while let Ok(command) = rx.recv() {
        match command {
            AudioCommand::Play(path_str, volume) => {
                _current_sink = None;

                let sink = Sink::connect_new(stream.mixer());
                sink.set_volume(volume.clamp(0.0, 1.0));

                if let Some((samples, channels, rate)) = cache.get(&path_str) {
                    let source = SamplesBuffer::new(*channels, *rate, samples.clone());
                    sink.append(source);
                    _current_sink = Some(sink);
                    continue;
                }

                let resource_path = format!(
                    "/io/github/sniper1720/khushu/{}",
                    path_str.trim_start_matches("assets/")
                );
                if let Ok(bytes) = gtk4::gio::resources_lookup_data(
                    &resource_path,
                    gtk4::gio::ResourceLookupFlags::NONE,
                ) {
                    let reader = std::io::Cursor::new(bytes.to_vec());
                    if let Ok(decoder) = Decoder::new(reader) {
                        let channels = decoder.channels();
                        let rate = decoder.sample_rate();
                        let samples: Vec<f32> = decoder.collect();

                        cache.insert(path_str.clone(), (samples.clone(), channels, rate));

                        let source = SamplesBuffer::new(channels, rate, samples);
                        sink.append(source);
                        _current_sink = Some(sink);
                    } else {
                        log::error!("Failed to decode audio resource: {}", resource_path);
                    }
                } else {
                    log::error!("Audio resource not found in binary: {}", resource_path);
                }
            }
            AudioCommand::Stop => {
                _current_sink = None;
            }
        }
    }
}
