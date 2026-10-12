//! Mojang java-runtime otomatik kurulumu.
//!
//! Sistemde uygun Java yoksa, Mojang'ın resmi java-runtime arşivinden
//! gereken sürüm (8/17/21) indirilip `data/java/<sürüm>` altına açılır.

use std::collections::HashMap;
use std::path::PathBuf;

use disco_common::{paths, DiscoError, Result};
use disco_net::{DownloadItem, Http, ProgressTx};
use serde::Deserialize;

const JAVA_RUNTIME_URL: &str = "https://api.mojang.com/products/java_runtime";

#[derive(Deserialize, Clone, Debug)]
struct RuntimeResponse(HashMap<String, Vec<RuntimeEntry>>);

#[derive(Deserialize, Clone, Debug)]
struct RuntimeEntry {
    manifest: RuntimeManifestRef,
    version: RuntimeVersion,
}

#[derive(Deserialize, Clone, Debug)]
struct RuntimeManifestRef {
    url: String,
}

#[derive(Deserialize, Clone, Debug)]
struct RuntimeVersion {
    name: String,
}

#[derive(Deserialize, Clone, Debug)]
struct RuntimeManifest {
    #[serde(default)]
    files: HashMap<String, RuntimeFile>,
}

#[derive(Deserialize, Clone, Debug)]
struct RuntimeFile {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    downloads: HashMap<String, RuntimeDownload>,
}

#[derive(Deserialize, Clone, Debug)]
struct RuntimeDownload {
    url: String,
    #[serde(default)]
    sha1: Option<String>,
}

fn os_param() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    }
}

fn arch_param() -> &'static str {
    if cfg!(target_arch = "aarch64") {
        "arm64"
    } else if cfg!(target_arch = "x86") {
        "x86"
    } else {
        "x64"
    }
}

fn java_binary_name() -> &'static str {
    if cfg!(windows) { "java.exe" } else { "java" }
}

/// Sürüm adından ana sürüm çıkarır ("21.0.3" → 21, "1.8.0_51" → 8).
fn parse_major(version_name: &str) -> Option<u32> {
    let first = version_name
        .split(|c: char| !c.is_ascii_digit() && c != '.')
        .find(|part| !part.is_empty())?;
    let mut parts = first.split('.');
    let major: u32 = parts.next()?.parse().ok()?;
    if major == 1 {
        // "1.8.0_51" → Java 8
        let minor: u32 = parts.next()?.parse().ok()?;
        return Some(minor);
    }
    Some(major)
}

/// Sistemdeki Java'yı bulur; sürüm uyuşmuyorsa veya yoksa None döner.
pub async fn find_suitable(required_major: Option<u32>) -> Option<PathBuf> {
    tokio::task::spawn_blocking(move || find_suitable_blocking(required_major))
        .await
        .ok()
        .flatten()
}

fn find_suitable_blocking(required_major: Option<u32>) -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();

    if let Ok(home) = std::env::var("JAVA_HOME") {
        candidates.push(std::path::Path::new(&home).join("bin").join(java_binary_name()));
    }
    if let Ok(path) = std::env::var("PATH") {
        candidates.extend(
            std::env::split_paths(&path).map(|dir| dir.join(java_binary_name())),
        );
    }

    for candidate in candidates {
        if !candidate.exists() {
            continue;
        }
        match required_major {
            None => return Some(candidate),
            Some(required) => match java_major_of(&candidate) {
                Some(Some(found)) if found != required => continue,
                _ => return Some(candidate), // uyan ya da doğrulanamayan
            },
        }
    }
    None
}

/// `java -version` çıktısından ana sürümü ayrıştırır.
fn java_major_of(java: &std::path::Path) -> Option<Option<u32>> {
    let output = std::process::Command::new(java)
        .arg("-version")
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stderr);
    let version = text.split('"').nth(1)?.to_string();
    Some(parse_major(&version))
}

/// Gerekli ana sürüm için Mojang runtime'ı indirir; Java yolunu döner.
pub async fn ensure_installed(
    http: &Http,
    required_major: Option<u32>,
    progress: Option<ProgressTx>,
) -> Result<PathBuf> {
    let url = format!("{JAVA_RUNTIME_URL}?os={}&arch={}", os_param(), arch_param());
    let response: RuntimeResponse = http
        .get_json(&url)
        .await
        .map_err(|e| DiscoError::Http(format!("java runtime listesi alınamadı: {e}")))?;

    let entries = response
        .0
        .values()
        .flatten()
        .collect::<Vec<_>>();
    if entries.is_empty() {
        return Err(DiscoError::NotFound(
            "bu platform için Mojang java runtime'ı yok".into(),
        ));
    }

    // istenen ana sürüme uyan ilk giriş; yoksa en yenisi (sondaki)
    let entry = required_major
        .and_then(|major| {
            entries
                .iter()
                .copied()
                .find(|e| parse_major(&e.version.name) == Some(major))
        })
        .or_else(|| entries.last().copied())
        .ok_or_else(|| DiscoError::NotFound("java runtime listesi boş".into()))?;

    let java_dir = paths::data_dir()?.join("java").join(&entry.version.name);
    let executable = java_dir.join("bin").join(java_binary_name());
    if executable.exists() {
        return Ok(executable); // kurulu
    }

    // manifest'i çek/önbellekle
    let manifest_path = java_dir.with_extension("manifest.json");
    let raw = match tokio::fs::read_to_string(&manifest_path).await {
        Ok(raw) => raw,
        Err(_) => {
            let raw = http.get_text(&entry.manifest.url).await?;
            if let Some(parent) = manifest_path.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            tokio::fs::write(&manifest_path, &raw).await?;
            raw
        }
    };
    let manifest: RuntimeManifest = serde_json::from_str(&raw)
        .map_err(|e| DiscoError::Invalid(format!("java manifesti bozuk: {e}")))?;

    // tüm dosya girdilerini indir; klasörler otomatik oluşur
    let items: Vec<DownloadItem> = manifest
        .files
        .iter()
        .filter(|(_, file)| file.kind == "file")
        .filter_map(|(path, file)| {
            let download = file.downloads.get("mojang").or_else(|| file.downloads.values().next())?;
            Some(DownloadItem {
                url: download.url.clone(),
                dest: java_dir.join(path),
                sha1: download.sha1.clone(),
            })
        })
        .collect();

    tracing::info!(
        "java runtime indiriliyor: {} sürümü, {} dosya",
        entry.version.name,
        items.len()
    );
    http.download_batch(items, progress).await?;

    if !executable.exists() {
        return Err(DiscoError::NotFound(format!(
            "java kuruldu ama {} bulunamadı",
            executable.display()
        )));
    }
    Ok(executable)
}
