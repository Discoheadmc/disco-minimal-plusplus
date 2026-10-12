//! Launcher ayarları — tek `settings.json`.

use disco_common::{paths, Result};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct LauncherSettings {
    /// Varsayılan oyuncu adı (çevrimdışı hesap).
    pub player_name: String,
    /// Java yolu; None ise sistem Java'sı bulunur, o da yoksa Mojang
    /// java-runtime otomatik indirilir.
    pub java_path: Option<String>,
    /// Oyuna verilecek bellek (MB).
    pub memory_mb: u32,
    /// Aktif hesap; None ise çevrimdışı `player_name` kullanılır.
    #[serde(default)]
    pub active: Option<disco_auth::Account>,
    /// Tema vurgu rengi (hex). Disco Launcher'daki 8 renk geleneği.
    #[serde(default = "default_accent")]
    pub accent: String,
}

fn default_accent() -> String {
    "#7c5cff".into()
}

impl Default for LauncherSettings {
    fn default() -> Self {
        Self {
            player_name: "Player".into(),
            java_path: None,
            memory_mb: 2048,
            active: None,
            accent: default_accent(),
        }
    }
}

pub fn load() -> LauncherSettings {
    let Ok(file) = paths::settings_file() else {
        return LauncherSettings::default();
    };
    std::fs::read_to_string(file)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

pub fn save(settings: &LauncherSettings) -> Result<()> {
    let file = paths::settings_file()?;
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(file, serde_json::to_string_pretty(settings)?)?;
    Ok(())
}
