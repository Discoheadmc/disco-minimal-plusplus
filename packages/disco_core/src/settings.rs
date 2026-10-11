//! Launcher ayarları — tek `settings.json`.

use disco_common::{paths, Result};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct LauncherSettings {
    /// Varsayılan oyuncu adı (çevrimdışı hesap).
    pub player_name: String,
    /// Java yolu; None ise JAVA_HOME/PATH'ten bulunur.
    pub java_path: Option<String>,
    /// Oyuna verilecek bellek (MB).
    pub memory_mb: u32,
}

impl Default for LauncherSettings {
    fn default() -> Self {
        Self {
            player_name: "Player".into(),
            java_path: None,
            memory_mb: 2048,
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
