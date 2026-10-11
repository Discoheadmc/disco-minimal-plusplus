//! Dosya tabanlı instance modeli.

use std::path::PathBuf;

use disco_common::{DiscoError, Result};
use serde::{Deserialize, Serialize};

use crate::AccentColor;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Instance {
    pub name: String,
    pub version_id: String,
    pub created_at: String,
    pub last_played: Option<String>,
    #[serde(default)]
    pub accent: AccentColor,
}

impl Instance {
    pub fn dir(&self) -> Result<PathBuf> {
        Ok(disco_common::paths::instances_dir()?.join(&self.name))
    }

    /// Oyunun `--gameDir`'i.
    pub fn game_dir(&self) -> Result<PathBuf> {
        Ok(self.dir()?.join("minecraft"))
    }

    /// LWJGL natives'in açılacağı dizin.
    pub fn natives_dir(&self) -> Result<PathBuf> {
        Ok(self.dir()?.join("natives"))
    }
}

/// Instance adı: dosya sisteminde klasör adı olduğu için katı kurallı.
pub fn valid_name(name: &str) -> Result<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(DiscoError::Invalid("instance adı boş olamaz".into()));
    }
    if trimmed.len() > 48 {
        return Err(DiscoError::Invalid("instance adı en fazla 48 karakter".into()));
    }
    let ok = trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | ' '));
    if !ok {
        return Err(DiscoError::Invalid(
            "instance adı sadece harf, rakam, `-`, `_` ve boşluk içerebilir".into(),
        ));
    }
    Ok(trimmed.to_string())
}

fn instance_json_path(name: &str) -> Result<PathBuf> {
    Ok(disco_common::paths::instances_dir()?.join(name).join("instance.json"))
}

/// Tüm instance'ları okur; bozuk olanları sessizce atlar (kilitlenme yok).
pub fn list() -> Result<Vec<Instance>> {
    let root = disco_common::paths::instances_dir()?;
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&root)? {
        let Ok(entry) = entry else { continue };
        if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let Some(name) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        let Ok(json) = instance_json_path(&name) else {
            continue;
        };
        let Ok(raw) = std::fs::read_to_string(json) else {
            continue;
        };
        match serde_json::from_str::<Instance>(&raw) {
            Ok(mut instance) => {
                // klasör adı gerçektir; JSON bozulmuşsa düzelt
                instance.name = name;
                out.push(instance);
            }
            Err(err) => {
                tracing::warn!("instance.json bozuk ({name}): {err}");
            }
        }
    }
    out.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(out)
}

/// Yeni instance oluşturur; aynı isimli varsa hata verir.
pub fn create(name: &str, version_id: &str) -> Result<Instance> {
    let name = valid_name(name)?;
    let existing = list()?;
    if existing.iter().any(|i| i.name == name) {
        return Err(DiscoError::Invalid(format!(
            "'{name}' adlı instance zaten var"
        )));
    }
    let instance = Instance {
        name: name.clone(),
        version_id: version_id.to_string(),
        created_at: chrono::Utc::now().to_rfc3339(),
        last_played: None,
        accent: AccentColor::default(),
    };
    let dir = instance.dir()?;
    std::fs::create_dir_all(&dir)?;
    std::fs::create_dir_all(instance.game_dir()?)?;
    std::fs::create_dir_all(instance.natives_dir()?)?;
    let json = instance_json_path(&name)?;
    std::fs::write(json, serde_json::to_string_pretty(&instance)?)?;
    Ok(instance)
}

/// `last_played` damgasını günceller.
pub fn touch_played(instance: &Instance) {
    let mut updated = instance.clone();
    updated.last_played = Some(chrono::Utc::now().to_rfc3339());
    let _ = save(&updated);
}

fn save(instance: &Instance) -> Result<()> {
    let json = instance_json_path(&instance.name)?;
    std::fs::write(json, serde_json::to_string_pretty(instance)?)?;
    Ok(())
}

/// Instance'ı diskten siler (çöp kutusuna değil, doğrudan).
pub fn delete(name: &str) -> Result<()> {
    let dir = disco_common::paths::instances_dir()?.join(name);
    if !dir.exists() {
        return Err(DiscoError::NotFound(format!("instance bulunamadı: {name}")));
    }
    std::fs::remove_dir_all(dir)?;
    Ok(())
}
