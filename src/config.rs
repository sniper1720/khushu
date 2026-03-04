use crate::security;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use gtk4::glib;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum LocationMode {
    Manual,
    City,
    Auto,
}

#[derive(Serialize, Deserialize, Default, Debug, Clone, PartialEq)]
#[allow(clippy::upper_case_acronyms)]
pub enum CalculationMethod {
    #[default]
    MWL,
    ISNA,
    Egypt,
    Makkah,
    Karachi,
    Dubai,
    MoonsightingCommittee,
    Kuwait,
    Qatar,
    Singapore,
    Turkey,
}

#[derive(Serialize, Deserialize, Default, Debug, Clone, PartialEq)]
pub enum MadhabChoice {
    #[default]
    Shafi,
    Hanafi,
}

#[derive(Serialize, Deserialize, Default, Debug, Clone, PartialEq)]
pub enum ThemeMode {
    #[serde(rename = "system")]
    #[default]
    System,
    #[serde(rename = "light")]
    Light,
    #[serde(rename = "dark")]
    Dark,
}

fn default_volume() -> f32 {
    1.0
}

fn default_autostart() -> bool {
    true
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AppConfig {
    #[serde(skip)]
    pub latitude: f64,
    #[serde(skip)]
    pub longitude: f64,
    pub method: CalculationMethod,
    pub madhab: MadhabChoice,
    pub location_mode: LocationMode,
    pub city_name: Option<String>,
    pub adhan_sound_path: Option<String>,
    pub pre_prayer_notify: bool,
    pub pre_prayer_minutes: u32,
    pub hijri_offset: i64,
    #[serde(default)]
    pub favorites: Vec<String>,
    #[serde(default)]
    pub adkar_notification_enabled: bool,
    #[serde(default)]
    pub language: String,
    #[serde(default)]
    pub theme: ThemeMode,
    #[serde(default)]
    pub is_configured: bool,
    #[serde(default = "default_volume")]
    pub adhan_volume: f32,
    #[serde(default)]
    pub adhan_muted: bool,
    #[serde(default = "default_autostart")]
    pub autostart: bool,

    #[serde(default)]
    pub enc_lat: String,
    #[serde(default)]
    pub enc_lon: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            latitude: 36.75,
            longitude: 3.05,
            method: CalculationMethod::MWL,
            madhab: MadhabChoice::Shafi,
            location_mode: LocationMode::Manual,
            city_name: None,
            adhan_sound_path: None,
            pre_prayer_notify: true,
            pre_prayer_minutes: 15,
            hijri_offset: 0,
            favorites: Vec::new(),
            adkar_notification_enabled: true,
            language: "auto".to_string(),
            theme: ThemeMode::System,
            is_configured: false,
            adhan_volume: 1.0,
            adhan_muted: false,
            autostart: true,
            enc_lat: String::new(),
            enc_lon: String::new(),
        }
    }
}

impl AppConfig {
    pub fn load() -> Self {
        let path = Self::config_path();
        if path.exists()
            && let Ok(content) = fs::read_to_string(&path)
            && let Ok(mut config) = serde_json::from_str::<Self>(&content)
        {
            log::info!("Configuration loaded from {:?}", path);
            if !config.enc_lat.is_empty()
                && let Ok(dec) = security::deobfuscate(&config.enc_lat)
            {
                config.latitude = dec.parse().unwrap_or(36.75);
            }
            if !config.enc_lon.is_empty()
                && let Ok(dec) = security::deobfuscate(&config.enc_lon)
            {
                config.longitude = dec.parse().unwrap_or(3.05);
            }
            return config;
        }
        log::info!("No existing configuration found, using defaults");
        Self::default()
    }

    pub fn save(&self) {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        let mut encrypted_self = self.clone();

        if let Ok(enc) = security::obfuscate(&self.latitude.to_string()) {
            encrypted_self.enc_lat = enc;
        }

        if let Ok(enc) = security::obfuscate(&self.longitude.to_string()) {
            encrypted_self.enc_lon = enc;
        }

        if let Ok(content) = serde_json::to_string_pretty(&encrypted_self)
            && fs::write(&path, content).is_ok()
        {
            log::info!("Configuration encrypted and saved to {:?}", path);
        } else {
            log::error!("Failed to save configuration to {:?}", path);
        }
    }

    pub fn config_path() -> PathBuf {
        let mut path = glib::user_config_dir();
        path.push("khushu");
        path.push("config.json");
        path
    }
}
