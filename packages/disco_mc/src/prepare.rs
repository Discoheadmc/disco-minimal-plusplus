//! Disco Minimal++ — sürüm dosyalarını hazırlar.
//!
//! Client jar, kütüphaneler ve assetler tek seferde toplanır ve tek paralel
//! indirme grubuna verilir; SHA1 doğrulaması `disco_net`'te yapılır. Buradaki
//! iş, sadece *neyin* indirileceğinin listesini çıkarmaktır.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use disco_common::{DiscoError, Result};
use disco_net::{DownloadItem, Http, ProgressTx};
use serde::Deserialize;

use crate::{ResolvedVersion, current_os_name};

#[derive(Deserialize, Clone, Debug)]
pub struct AssetIndexObjects {
    #[serde(default)]
    pub objects: HashMap<String, AssetObject>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct AssetObject {
    pub hash: String,
    #[serde(default)]
    pub size: u64,
}

/// Hazırlanan dosyaların yolları — komut üretimine beslenir.
#[derive(Clone, Debug)]
pub struct PreparedFiles {
    pub client_jar: PathBuf,
    pub classpath: Vec<PathBuf>,
    pub native_jars: Vec<PathBuf>,
}

/// Bir sürümü başlatmak için gereken her şeyi indirir/doğrular.
pub async fn prepare_files(
    http: &Http,
    resolved: &ResolvedVersion,
    progress: Option<ProgressTx>,
) -> Result<PreparedFiles> {
    let versions_dir = disco_common::paths::versions_dir()?;
    let libraries_dir = disco_common::paths::libraries_dir()?;
    let assets_dir = disco_common::paths::assets_dir()?;

    let client_jar = versions_dir.join(&resolved.id).join(format!("{}.jar", resolved.id));

    let mut items: Vec<DownloadItem> = Vec::new();
    let mut classpath: Vec<PathBuf> = Vec::new();
    let mut native_jars: Vec<PathBuf> = Vec::new();

    // 1) client jar
    if let Some(client) = &resolved.client {
        items.push(DownloadItem {
            url: client.url.clone(),
            dest: client_jar.clone(),
            sha1: client.sha1.clone(),
        });
    } else {
        return Err(DiscoError::NotFound(format!(
            "sürümün client jar indirme bilgisi yok: {}",
            resolved.id
        )));
    }

    // 2) kütüphaneler (kural süzgeci + eski natives haritası)
    for library in resolved.libraries.iter().filter(|l| l.is_allowed()) {
        let downloads = match &library.downloads {
            Some(d) => d,
            None => continue, // kural bazlı (ör. eski launchwrapper) kütüphaneler
        };

        // 2a) ana artifact — her zaman classpath'tedir
        if let Some(artifact) = &downloads.artifact {
            let dest = libraries_dir.join(artifact.path.clone().unwrap_or_else(|| {
                maven_path(&library.name, None)
            }));
            classpath.push(dest.clone());
            items.push(DownloadItem {
                url: artifact.url.clone(),
                dest,
                sha1: artifact.sha1.clone(),
            });
        }

        // 2b) eski format natives classifiers
        let os_key = current_os_name();
        if let Some(classifier_key) = library
            .natives
            .as_ref()
            .and_then(|n| n.get(os_key).cloned())
        {
            if let Some(classifiers) = &downloads.classifiers
                && let Some(artifact) = classifiers.get(&classifier_key)
            {
                let dest = libraries_dir
                    .join(artifact.path.clone().unwrap_or_else(|| {
                        maven_path(&library.name, Some(&classifier_key))
                    }));
                classpath.push(dest.clone());
                native_jars.push(dest.clone());
                items.push(DownloadItem {
                    url: artifact.url.clone(),
                    dest,
                    sha1: artifact.sha1.clone(),
                });
            }
        }
    }

    // modern native jar'ları da çıkar (ör. lwjgl-*-natives-windows)
    let native_marker = format!("natives-{}", native_os_file_token());
    for path in classpath.iter() {
        let is_native = path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.contains(&native_marker));
        if is_native && !native_jars.contains(path) {
            native_jars.push(path.clone());
        }
    }

    // 3) assetler
    let asset_items = collect_asset_items(http, resolved, &assets_dir).await?;
    items.extend(asset_items);

    // 4) tek paralel grup halinde indir
    http.download_batch(items, progress).await?;

    Ok(PreparedFiles {
        client_jar,
        classpath,
        native_jars,
    })
}

/// Natives jar'larını natives dizinine açar (`.dll`/`.so`/`.dylib`).
pub async fn extract_natives(jars: &[PathBuf], natives_dir: &Path) -> Result<()> {
    tokio::fs::create_dir_all(natives_dir).await?;
    let jars = jars.to_vec();
    let natives_dir = natives_dir.to_path_buf();
    tokio::task::spawn_blocking(move || {
        for jar in &jars {
            if !jar.exists() {
                continue;
            }
            match extract_one(jar, &natives_dir) {
                Ok(count) => tracing::debug!("natives: {count} dosya çıkarıldı ({})", jar.display()),
                Err(err) => tracing::warn!("natives çıkarılamadı ({}): {err}", jar.display()),
            }
        }
        Ok::<(), DiscoError>(())
    })
    .await
    .map_err(|e| DiscoError::Other(format!("natives görevi düştü: {e}")))??;
    Ok(())
}

fn extract_one(jar: &Path, natives_dir: &Path) -> Result<usize> {
    let file = std::fs::File::open(jar)?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| DiscoError::Other(format!("zip açılamadı: {e}")))?;
    let mut extracted = 0usize;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|e| DiscoError::Other(format!("zip girdisi okunamadı: {e}")))?;
        if entry.is_dir() {
            continue;
        }
        let name = entry.name().to_string();
        if name.starts_with("META-INF") {
            continue;
        }
        let Some(file_name) = Path::new(&name).file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let is_native = ["dll", "so", "dylib"].iter().any(|ext| {
            file_name.to_ascii_lowercase().ends_with(ext)
        });
        if !is_native {
            continue;
        }
        let dest = natives_dir.join(file_name);
        let mut out = std::fs::File::create(&dest)?;
        std::io::copy(&mut entry, &mut out)?;
        extracted += 1;
    }
    Ok(extracted)
}

async fn collect_asset_items(
    http: &Http,
    resolved: &ResolvedVersion,
    assets_dir: &Path,
) -> Result<Vec<DownloadItem>> {
    let Some(asset_index) = &resolved.asset_index else {
        return Ok(Vec::new()); // çok eski sürümler; MVP'de yok sayılır
    };

    // index JSON'unu önbellekten oku, yoksa indir
    let index_path = assets_dir.join("indexes").join(format!("{}.json", asset_index.id));
    let raw = match tokio::fs::read_to_string(&index_path).await {
        Ok(raw) => raw,
        Err(_) => {
            let raw = http.get_text(&asset_index.url).await?;
            if let Some(parent) = index_path.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            tokio::fs::write(&index_path, &raw).await?;
            raw
        }
    };
    let index: AssetIndexObjects =
        serde_json::from_str(&raw).map_err(|e| DiscoError::Invalid(format!("asset index bozuk: {e}")))?;

    Ok(index
        .objects
        .into_values()
        .map(|object| {
            let prefix = &object.hash[..2.min(object.hash.len())];
            DownloadItem {
                url: format!(
                    "{}/{}/{}",
                    crate::ASSETS_RESOURCE_URL, prefix, object.hash
                ),
                dest: assets_dir.join("objects").join(prefix).join(&object.hash),
                sha1: Some(object.hash),
            }
        })
        .collect())
}

/// `group:artifact:version[:classifier]` → Maven depo yolu.
pub fn maven_path(name: &str, classifier: Option<&str>) -> String {
    let parts: Vec<&str> = name.split(':').collect();
    if parts.len() < 3 {
        return format!("{name}.jar");
    }
    let (group, artifact, version) = (parts[0], parts[1], parts[2]);
    let classifier_part = classifier
        .or_else(|| parts.get(3).copied())
        .map(|c| format!("-{c}"))
        .unwrap_or_default();
    format!(
        "{group_path}/{artifact}/{version}/{artifact}-{version}{classifier_part}.jar",
        group_path = group.replace('.', "/"),
    )
}

/// Native jar dosya adındaki OS token'ı (piston-meta isimlendirme).
fn native_os_file_token() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    }
}

#[cfg(test)]
mod tests {
    use super::maven_path;

    #[test]
    fn maven_paths() {
        assert_eq!(
            maven_path("org.lwjgl:lwjgl:3.3.3", None),
            "org/lwjgl/lwjgl/3.3.3/lwjgl-3.3.3.jar"
        );
        assert_eq!(
            maven_path("org.lwjgl:lwjgl:3.3.3:natives-windows", None),
            "org/lwjgl/lwjgl/3.3.3/lwjgl-3.3.3-natives-windows.jar"
        );
        assert_eq!(
            maven_path("tv.twitch:twitch-platform:5.16", Some("natives-windows")),
            "tv/twitch/twitch-platform/5.16/twitch-platform-5.16-natives-windows.jar"
        );
    }
}
