//! Disco Minimal++ — Minecraft sürüm metaverisi.
//!
//! Mojang piston-meta servisinden manifest ve sürüm JSON'larını çözer;
//! `inheritsFrom` zincirlerini düzleştirir ve kural (rules) değerlendirmesini
//! tek yerden yönetir.

pub mod launch;
pub mod prepare;

use std::collections::HashMap;

use disco_common::paths;
use disco_common::{DiscoError, Result};
use disco_net::Http;
use serde::{Deserialize, Serialize};

pub const MANIFEST_URL: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
pub const ASSETS_RESOURCE_URL: &str = "https://resources.download.minecraft.net";

// ---------------------------------------------------------------------------
// Manifest
// ---------------------------------------------------------------------------

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct VersionManifest {
    pub latest: LatestVersions,
    pub versions: Vec<ManifestEntry>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct LatestVersions {
    pub release: String,
    pub snapshot: String,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct ManifestEntry {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub url: String,
    #[serde(default)]
    pub sha1: Option<String>,
    #[serde(default)]
    pub release_time: Option<String>,
}

impl VersionManifest {
    /// type == "release" olan sürümler, yeniden eskiye.
    pub fn releases(&self) -> impl Iterator<Item = &ManifestEntry> {
        self.versions
            .iter()
            .filter(|v| v.kind == "release")
    }

    pub fn find(&self, id: &str) -> Option<&ManifestEntry> {
        self.versions.iter().find(|v| v.id == id)
    }
}

/// Manifesti çeker; ağ yoksa diskteki son kopyaya düşer (çevrimdışı açılış).
pub async fn fetch_manifest(http: &Http) -> Result<VersionManifest> {
    let cache = paths::versions_dir()?.join("manifest.json");
    match http.get_json::<VersionManifest>(MANIFEST_URL).await {
        Ok(manifest) => {
            if let Ok(json) = serde_json::to_string(&manifest) {
                if let Some(parent) = cache.parent() {
                    let _ = tokio::fs::create_dir_all(parent).await;
                }
                let _ = tokio::fs::write(&cache, json).await;
            }
            Ok(manifest)
        }
        Err(err) => {
            let raw = std::fs::read_to_string(&cache)
                .map_err(|_| DiscoError::Http(format!("manifest alınamadı: {err}")))?;
            serde_json::from_str(&raw).map_err(|e| DiscoError::Http(e.to_string()))
        }
    }
}

// ---------------------------------------------------------------------------
// Sürüm JSON tipleri
// ---------------------------------------------------------------------------

#[derive(Deserialize, Clone, Debug)]
pub struct VersionDetails {
    pub id: String,
    #[serde(default)]
    pub inherits_from: Option<String>,
    #[serde(default)]
    pub asset_index: Option<AssetIndex>,
    #[serde(default)]
    pub assets: Option<String>,
    #[serde(default)]
    pub downloads: Option<VersionDownloads>,
    #[serde(default)]
    pub libraries: Vec<Library>,
    #[serde(default)]
    pub main_class: Option<String>,
    #[serde(default)]
    pub arguments: Option<Arguments>,
    #[serde(default)]
    pub minecraft_arguments: Option<String>,
    #[serde(default)]
    pub java_version: Option<JavaVersion>,
    #[serde(rename = "type", default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub release_time: Option<String>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct AssetIndex {
    pub id: String,
    pub url: String,
    #[serde(default)]
    pub sha1: Option<String>,
    #[serde(default)]
    pub total_size: Option<u64>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct VersionDownloads {
    #[serde(default)]
    pub client: Option<Artifact>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Artifact {
    #[serde(default)]
    pub path: Option<String>,
    pub url: String,
    #[serde(default)]
    pub sha1: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct JavaVersion {
    #[serde(default)]
    pub component: Option<String>,
    #[serde(default)]
    pub major_version: Option<u32>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Library {
    pub name: String,
    #[serde(default)]
    pub downloads: Option<LibraryDownloads>,
    #[serde(default)]
    pub rules: Option<Vec<Rule>>,
    /// Eski format: `{"natives": {"windows": "natives-windows"}}`
    #[serde(default)]
    pub natives: Option<HashMap<String, String>>,
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct LibraryDownloads {
    #[serde(default)]
    pub artifact: Option<Artifact>,
    #[serde(default)]
    pub classifiers: Option<HashMap<String, Artifact>>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Arguments {
    #[serde(default)]
    pub game: Vec<ArgValue>,
    #[serde(default)]
    pub jvm: Vec<ArgValue>,
}

/// Argüman: ya düz metin ya da koşullu (`rules` + değer).
#[derive(Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum ArgValue {
    Plain(String),
    Conditional {
        rules: Vec<Rule>,
        value: ArgValueList,
    },
}

#[derive(Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum ArgValueList {
    One(String),
    Many(Vec<String>),
}

// ---------------------------------------------------------------------------
// Kurallar
// ---------------------------------------------------------------------------

#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RuleAction {
    Allow,
    Deny,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Rule {
    pub action: RuleAction,
    #[serde(default)]
    pub os: Option<OsRule>,
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct OsRule {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub arch: Option<String>,
}

/// `os.name` karşılığı — Mojang'ın JSON'undaki isimlerle uyumlu.
pub fn current_os_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "osx"
    } else {
        "linux"
    }
}

fn os_rule_matches(os: &OsRule) -> bool {
    if let Some(name) = &os.name {
        if name != current_os_name() {
            return false;
        }
    }
    if let Some(arch) = &os.arch {
        let is_32 = cfg!(target_pointer_width = "32");
        match arch.as_str() {
            "x86" => {
                if !is_32 {
                    return false;
                }
            }
            other => {
                // bilinmeyen arch etiketleri eşleşmez
                let _ = other;
                return false;
            }
        }
    }
    true
}

/// Kural listesini değerlendir: boşsa her zaman doğru; doluysa eşleşen son
/// kural belirler, hiçbiri eşleşmezse yanlış (standart davranış).
pub fn rules_allow(rules: Option<&[Rule]>) -> bool {
    let Some(rules) = rules else {
        return true;
    };
    if rules.is_empty() {
        return true;
    }
    let mut allowed = false;
    for rule in rules {
        let matches = rule.os.as_ref().is_none_or(os_rule_matches);
        if matches {
            allowed = rule.action == RuleAction::Allow;
        }
    }
    allowed
}

impl Library {
    pub fn is_allowed(&self) -> bool {
        rules_allow(self.rules.as_deref())
    }
}

impl ArgValue {
    /// Kural geçerliyse argüman listesine açılır, değilse boş.
    pub fn resolve(&self) -> Vec<String> {
        match self {
            Self::Plain(text) => vec![text.clone()],
            Self::Conditional { rules, value } => {
                if rules_allow(Some(rules)) {
                    match value {
                        ArgValueList::One(one) => vec![one.clone()],
                        ArgValueList::Many(many) => many.clone(),
                    }
                } else {
                    Vec::new()
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Sürüm çözümleme (inheritance düzleştirme)
// ---------------------------------------------------------------------------

/// Zincir düzleştirilmiş, başlatmaya hazır sürüm.
#[derive(Clone, Debug, Default)]
pub struct ResolvedVersion {
    pub id: String,
    pub main_class: String,
    pub assets: Option<String>,
    pub asset_index: Option<AssetIndex>,
    pub client: Option<Artifact>,
    pub libraries: Vec<Library>,
    pub jvm_args: Vec<ArgValue>,
    pub game_args: Vec<ArgValue>,
    pub legacy_arguments: Option<String>,
    pub java_major: Option<u32>,
}

impl ResolvedVersion {
    fn merged_with(self, child: VersionDetails) -> Self {
        let VersionDetails {
            asset_index,
            assets,
            downloads,
            libraries,
            main_class,
            arguments,
            minecraft_arguments,
            java_version,
            ..
        } = child;

        let mut jvm_args = self.jvm_args;
        let mut game_args = self.game_args;
        let mut libraries_all = self.libraries;

        if let Some(arguments) = &arguments {
            jvm_args.extend(arguments.jvm.iter().cloned());
            game_args.extend(arguments.game.iter().cloned());
        }
        libraries_all.extend(libraries);

        Self {
            id: if child.id.is_empty() { self.id } else { child.id },
            main_class: main_class.unwrap_or(self.main_class),
            assets: assets.or(self.assets),
            asset_index: asset_index.or(self.asset_index),
            client: downloads
                .and_then(|d| d.client)
                .or(self.client),
            libraries: libraries_all,
            jvm_args,
            game_args,
            legacy_arguments: minecraft_arguments.or(self.legacy_arguments),
            java_major: java_version
                .and_then(|j| j.major_version)
                .or(self.java_major),
        }
    }
}

/// Sürümü kimliğiyle çözer: manifestte bulur, JSON'u (önbellek öncelikli)
/// indirir ve `inheritsFrom` zincirini düzleştirir.
pub async fn resolve_version(http: &Http, version_id: &str) -> Result<ResolvedVersion> {
    let manifest = fetch_manifest(http).await?;
    resolve_with_manifest(http, &manifest, version_id).await
}

pub async fn resolve_with_manifest(
    http: &Http,
    manifest: &VersionManifest,
    version_id: &str,
) -> Result<ResolvedVersion> {
    // Zinciri kökten isteğe doğru topla (özyineleme yerine döngü:
    // async fn'de özyineleme box gerektirir).
    let mut chain: Vec<VersionDetails> = Vec::new();
    let mut current_id = version_id.to_string();
    loop {
        let details = match manifest.find(&current_id) {
            Some(entry) => fetch_version_details(http, entry).await?,
            // manifest'te olmayan sürüm: custom sürüm (ör. fabric profili) diskte
            None => load_local_version_details(&current_id).await?,
        };
        let next = details.inherits_from.clone();
        chain.push(details);
        match next {
            Some(parent) => current_id = parent,
            None => break,
        }
    }

    // En uzak atadan başlayarak çocuklara doğru birleştir
    let mut resolved = ResolvedVersion::default();
    for details in chain.into_iter().rev() {
        resolved = resolved.merged_with(details);
    }
    Ok(resolved)
}

/// Custom sürüm JSON'unu diskten okur (versions/<id>/<id>.json).
async fn load_local_version_details(version_id: &str) -> Result<VersionDetails> {
    let path = paths::versions_dir()?
        .join(version_id)
        .join(format!("{version_id}.json"));
    let raw = tokio::fs::read_to_string(&path)
        .await
        .map_err(|_| DiscoError::NotFound(format!("sürüm bulunamadı: {version_id}")))?;
    serde_json::from_str(&raw).map_err(|e| {
        DiscoError::Invalid(format!("custom sürüm JSON'u bozuk ({version_id}): {e}"))
    })
}

/// Sürüm JSON'unu çeker; diskte geçerli kopya varsa onu kullanır.
async fn fetch_version_details(
    http: &Http,
    entry: &ManifestEntry,
) -> Result<VersionDetails> {
    let path = paths::versions_dir()?.join(&entry.id).join(format!("{}.json", entry.id));
    let raw = match tokio::fs::read_to_string(&path).await {
        Ok(raw) => raw,
        Err(_) => {
            let raw = http.get_text(&entry.url).await?;
            if let Some(parent) = path.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            tokio::fs::write(&path, &raw).await?;
            raw
        }
    };
    serde_json::from_str(&raw).map_err(|e| DiscoError::Invalid(format!(
        "sürüm JSON'u bozuk ({}): {e}",
        entry.id
    )))
}
