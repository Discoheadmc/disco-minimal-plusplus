//! Disco Minimal++ — Modrinth mağazası.
//!
//! Mod paketi arama, `.mrpack` kurulumu ve Fabric/Quilt loader
//! profillerinin custom sürüm olarak kaydı. Modrinth API v2 kullanır.

use std::path::{Path, PathBuf};

use disco_common::{paths, DiscoError, Result};
use disco_net::{DownloadItem, Http};
use serde::Deserialize;

const MODRINTH_API: &str = "https://api.modrinth.com/v2";
const FABRIC_META: &str = "https://meta.fabricmc.net/v2";

// ---------------------------------------------------------------------------
// Arama
// ---------------------------------------------------------------------------

#[derive(Deserialize, Clone, Debug)]
pub struct SearchResponse {
    pub hits: Vec<SearchHit>,
    #[serde(default)]
    pub total_hits: u64,
}

#[derive(Deserialize, Clone, Debug)]
pub struct SearchHit {
    pub project_id: String,
    pub slug: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub downloads: u64,
    #[serde(default)]
    pub icon_url: Option<String>,
    #[serde(default)]
    pub versions: Vec<String>,
}

/// Mod paketlerinde arar (`project_type:modpack`).
pub async fn search_modpacks(http: &Http, query: &str) -> Result<Vec<SearchHit>> {
    // boş sorgu → popüler paketler (relevance sıralaması)
    let facets = "%5B%5B%22project_type%3Amodpack%22%5D%5D"; // [["project_type:modpack"]]
    let url = format!(
        "{MODRINTH_API}/search?query={}&limit=20&index=relevance&facets={facets}",
        urlencoded(query)
    );
    let response: SearchResponse = http.get_json(&url).await?;
    Ok(response.hits)
}

fn urlencoded(text: &str) -> String {
    let mut out = String::new();
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Proje sürümleri
// ---------------------------------------------------------------------------

#[derive(Deserialize, Clone, Debug)]
pub struct ModrinthVersion {
    pub id: String,
    pub name: String,
    pub version_number: String,
    #[serde(default)]
    pub game_versions: Vec<String>,
    #[serde(default)]
    pub files: Vec<ModrinthFile>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct ModrinthFile {
    pub url: String,
    pub filename: String,
    #[serde(default)]
    pub primary: bool,
    #[serde(default)]
    pub size: u64,
}

async fn fetch_project_versions(http: &Http, project_id: &str) -> Result<Vec<ModrinthVersion>> {
    let url = format!("{MODRINTH_API}/project/{project_id}/version");
    http.get_json(&url).await
}

// ---------------------------------------------------------------------------
// .mrpack formatı
// ---------------------------------------------------------------------------

#[derive(Deserialize, Clone, Debug)]
pub struct MrpackIndex {
    #[serde(rename = "formatVersion", default)]
    pub format_version: Option<u32>,
    #[serde(default)]
    pub game: Option<String>,
    #[serde(rename = "versionId", default)]
    pub version_id: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub dependencies: MrpackDependencies,
    #[serde(default)]
    pub files: Vec<MrpackFile>,
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct MrpackDependencies {
    pub minecraft: Option<String>,
    #[serde(rename = "fabric-loader")]
    pub fabric_loader: Option<String>,
    #[serde(rename = "quilt-loader")]
    pub quilt_loader: Option<String>,
    pub forge: Option<String>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct MrpackFile {
    pub path: String,
    #[serde(default)]
    pub env: MrpackEnv,
    pub downloads: Vec<String>,
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct MrpackEnv {
    pub client: Option<String>,
}

// ---------------------------------------------------------------------------
// Kurulum
// ---------------------------------------------------------------------------

/// Bir mod paketini kurar: mrpack indirir, loader profilini kaydeder,
/// dosyaları instance oyun dizine indirir. Dönen instance adı UI listesine
/// eklenmeye hazırdır.
pub async fn install_modpack(
    http: &Http,
    project_id: &str,
    display_name: &str,
    events: disco_core::LaunchEventTx,
) -> Result<disco_core::Instance> {
    use disco_core::LaunchEvent;

    send_stage(&events, "Mod paketi bilgileri alınıyor…");
    let versions = fetch_project_versions(http, project_id).await?;
    let version = versions
        .first()
        .ok_or_else(|| DiscoError::NotFound("bu paketin indirilebilir sürümü yok".into()))?;
    let file = version
        .files
        .iter()
        .find(|f| f.primary)
        .or_else(|| version.files.first())
        .ok_or_else(|| DiscoError::NotFound("paketin mrpack dosyası yok".into()))?;

    // 1) mrpack'i önbelleğe indir
    let cache_dir = paths::data_dir()?.join("cache");
    tokio::fs::create_dir_all(&cache_dir).await?;
    let mrpack_path = cache_dir.join(&file.filename);
    http.download_file(&DownloadItem {
        url: file.url.clone(),
        dest: mrpack_path.clone(),
        sha1: None,
    })
    .await?;

    // 2) index + overrides'ı çıkar
    send_stage(&events, "Paket açılıyor…");
    let (index_raw, overrides_root) = extract_mrpack(&mrpack_path).await?;
    let index: MrpackIndex = serde_json::from_str(&index_raw)
        .map_err(|e| DiscoError::Invalid(format!("modrinth.index.json bozuk: {e}")))?;

    let game_version = index
        .dependencies
        .minecraft
        .clone()
        .ok_or_else(|| DiscoError::Invalid("paket Minecraft sürümü belirtmiyor".into()))?;

    // 3) loader profili (fabric/quilt) — custom sürüm olarak kaydet
    let version_id = if let Some(loader) = index
        .dependencies
        .fabric_loader
        .clone()
        .or_else(|| index.dependencies.quilt_loader.clone())
    {
        let loader_kind = if index.dependencies.fabric_loader.is_some() {
            "fabric"
        } else {
            "quilt"
        };
        send_stage(
            &events,
            &format!("{} loader profili hazırlanıyor…", loader_kind),
        );
        ensure_loader_profile(http, &game_version, &loader, loader_kind).await?
    } else if index.dependencies.forge.is_some() {
        return Err(DiscoError::Other(
            "Forge paketleri Kilometre Taşı 3'te gelecek — bu paket fabric/quilt değil".into(),
        ));
    } else {
        game_version.clone()
    };

    // 4) instance oluştur
    let base_name = disco_core::instance::valid_name(display_name)?;
    let name = unique_instance_name(&base_name)?;
    let instance = disco_core::instance::create(&name, &version_id)?;
    let game_dir = instance.game_dir()?;

    // 5) overrides klasörünü kopyala (varsa)
    if let Some(root) = overrides_root {
        send_stage(&events, "Paket ayarları kopyalanıyor…");
        move_tree(&root, &game_dir).await?;
        let _ = tokio::fs::remove_dir_all(&root).await;
    }

    // 6) dosyaları indir (client'ta istenmeyenleri atla)
    send_stage(&events, "Mod paketi dosyaları indiriliyor…");
    let items: Vec<DownloadItem> = index
        .files
        .iter()
        .filter(|f| f.env.client.as_deref() != Some("unsupported"))
        .filter_map(|f| {
            let url = f.downloads.first()?.clone();
            Some(DownloadItem {
                url,
                dest: game_dir.join(&f.path),
                sha1: None,
            })
        })
        .collect();

    let (progress_tx, mut progress_rx) =
        tokio::sync::mpsc::unbounded_channel::<disco_net::BatchProgress>();
    let events_clone = events.clone();
    tokio::spawn(async move {
        while let Some(progress) = progress_rx.recv().await {
            let _ = events_clone.send(LaunchEvent::Progress {
                done: progress.done,
                total: progress.total,
            });
        }
    });
    http.download_batch(items, Some(progress_tx)).await?;

    send_stage(&events, "Kurulum tamamlandı");
    Ok(instance)
}

fn send_stage(events: &disco_core::LaunchEventTx, stage: &str) {
    let _ = events.send(disco_core::LaunchEvent::Stage(stage.to_string()));
}

/// Aynı ad varsa "-2", "-3"… dener.
fn unique_instance_name(base: &str) -> Result<String> {
    let existing = disco_core::instance::list()?;
    let mut candidate = base.to_string();
    let mut counter = 2;
    while existing.iter().any(|i| i.name == candidate) {
        candidate = format!("{base}-{counter}");
        counter += 1;
    }
    Ok(candidate)
}

/// mrpack zip'inden modrinth.index.json ve overrides/ klasörünü çıkarır.
async fn extract_mrpack(mrpack: &Path) -> Result<(String, Option<PathBuf>)> {
    let mrpack = mrpack.to_path_buf();
    let staging = paths::data_dir()?.join("cache").join("mrpack-staging");
    let staging_task = staging.clone();
    let extracted = tokio::task::spawn_blocking(move || -> Result<(String, Option<PathBuf>)> {
        let file = std::fs::File::open(&mrpack)?;
        let mut archive = zip::ZipArchive::new(file)
            .map_err(|e| DiscoError::Other(format!("mrpack açılamadı: {e}")))?;

        let mut index_raw: Option<String> = None;
        let mut overrides_dir: Option<PathBuf> = None;

        for entry_index in 0..archive.len() {
            let mut entry = archive
                .by_index(entry_index)
                .map_err(|e| DiscoError::Other(format!("zip girdisi okunamadı: {e}")))?;
            if entry.is_dir() {
                continue;
            }
            let name = entry.name().to_string();
            if name == "modrinth.index.json" {
                let mut raw = String::new();
                std::io::Read::read_to_string(&mut entry, &mut raw)?;
                index_raw = Some(raw);
            } else if let Some(rest) = name.strip_prefix("overrides/") {
                if rest.is_empty() {
                    continue;
                }
                // yol güvenliği: .. içeren girdileri atla
                if rest.contains("..") {
                    continue;
                }
                let dest = staging_task.join(rest);
                if let Some(parent) = dest.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                let mut out = std::fs::File::create(&dest)?;
                std::io::copy(&mut entry, &mut out)?;
                overrides_dir = Some(staging_task.clone());
            }
        }
        let _ = &overrides_dir;
        match index_raw {
            Some(raw) => Ok((raw, overrides_dir)),
            None => Err(DiscoError::Invalid(
                "mrpack içinde modrinth.index.json yok".into(),
            )),
        }
    })
    .await
    .map_err(|e| DiscoError::Other(format!("mrpack görevi düştü: {e}")))??;
    Ok(extracted)
}

/// staging klasörünü oyun dizinine taşır.
async fn move_tree(from: &Path, to: &Path) -> Result<()> {
    let from = from.to_path_buf();
    let to = to.to_path_buf();
    tokio::task::spawn_blocking(move || {
        fn copy_dir(src: &Path, dst: &Path) -> std::io::Result<()> {
            std::fs::create_dir_all(dst)?;
            for entry in std::fs::read_dir(src)? {
                let entry = entry?;
                if entry.file_type()?.is_dir() {
                    copy_dir(&entry.path(), &dst.join(entry.file_name()))?;
                } else {
                    std::fs::copy(entry.path(), dst.join(entry.file_name()))?;
                }
            }
            Ok(())
        }
        copy_dir(&from, &to)
            .map_err(|e| DiscoError::Other(format!("overrides kopyalanamadı: {e}")))
    })
    .await
    .map_err(|e| DiscoError::Other(format!("kopyalama görevi düştü: {e}")))??;
    Ok(())
}

// ---------------------------------------------------------------------------
// Fabric/Quilt loader profili
// ---------------------------------------------------------------------------

/// Fabric/Quilt loader profilini custom sürüm olarak indirir;
/// version_id'yi döner. Zincir `inheritsFrom` üzerinden oyun sürümüne bağlanır.
pub async fn ensure_loader_profile(
    http: &Http,
    game_version: &str,
    loader_version: &str,
    kind: &str,
) -> Result<String> {
    let version_id = format!("{game_version}-{kind}-{loader_version}");
    let dir = paths::versions_dir()?.join(&version_id);
    let json_path = dir.join(format!("{version_id}.json"));

    if json_path.exists() {
        return Ok(version_id); // kurulu
    }

    let url = format!("{FABRIC_META}/versions/loader/{game_version}/{loader_version}/profile/json");
    let raw = http.get_text(&url).await?;
    let value: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|e| DiscoError::Invalid(format!("loader profili bozuk: {e}")))?;

    // id alanını kendi adımıza yeniden yaz (özyineleme ve görünürlük için)
    let mut value = value;
    if let Some(object) = value.as_object_mut() {
        object.insert("id".into(), serde_json::Value::String(version_id.clone()));
    }

    tokio::fs::create_dir_all(&dir).await?;
    tokio::fs::write(&json_path, serde_json::to_string_pretty(&value)?).await?;
    Ok(version_id)
}
