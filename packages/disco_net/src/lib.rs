//! Disco Minimal++ — ağ katmanı.
//!
//! Tek sorumluluk: HTTP istekleri ve SH A1 doğrulamalı dosya indirmeleri.
//! Tüm launcher trafiği buradan geçer; böylece UA, timeout ve proxy politikası
//! tek yerden yönetilir.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use disco_common::{constants, DiscoError, Result};
use futures_util::StreamExt;
use sha1::{Digest, Sha1};
use tokio::io::AsyncWriteExt;

/// Tek dosyanın indirme iş tanımı.
#[derive(Clone, Debug)]
pub struct DownloadItem {
    pub url: String,
    pub dest: PathBuf,
    /// Beklenen SHA1 hash; dosya zaten yerindeyse ve hash uyuyorsa indirilmez.
    pub sha1: Option<String>,
}

/// Paralel indirme grubunun ilerlemesi.
#[derive(Clone, Copy, Debug)]
pub struct BatchProgress {
    pub done: u64,
    pub total: u64,
}

/// Arayüze ilerleme bildirmek için basit kanal tipi.
pub type ProgressTx = tokio::sync::mpsc::UnboundedSender<BatchProgress>;

/// Aynı anda açılan bağlantı sayısı. Mojang CDN'i naziktir; 24 makul.
const CONCURRENCY: usize = 24;

pub struct Http {
    client: reqwest::Client,
}

impl Http {
    pub fn new() -> Result<Self> {
        let client = reqwest::Client::builder()
            .user_agent(constants::USER_AGENT)
            .connect_timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| DiscoError::Http(e.to_string()))?;
        Ok(Self { client })
    }

    /// URL'i çekip serde tipine çevirir.
    pub async fn get_json<T: serde::de::DeserializeOwned>(&self, url: &str) -> Result<T> {
        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| DiscoError::Http(e.to_string()))?;
        let status = response.status();
        if !status.is_success() {
            return Err(DiscoError::Http(format!("HTTP {status}: {url}")));
        }
        response
            .json::<T>()
            .await
            .map_err(|e| DiscoError::Http(e.to_string()))
    }

    /// Ham metin çeker (asset index gibi küçük JSON'lar için).
    pub async fn get_text(&self, url: &str) -> Result<String> {
        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| DiscoError::Http(e.to_string()))?;
        let status = response.status();
        if !status.is_success() {
            return Err(DiscoError::Http(format!("HTTP {status}: {url}")));
        }
        response
            .text()
            .await
            .map_err(|e| DiscoError::Http(e.to_string()))
    }

    /// Tek dosyayı SHA1 doğrulamayla indirir. Hedef zaten geçerliyse atlar.
    pub async fn download_file(&self, item: &DownloadItem) -> Result<()> {
        if let Some(expected) = &item.sha1
            && matches!(file_sha1(&item.dest).await, Ok(existing) if existing == expected.to_lowercase())
        {
            return Ok(()); // zaten yerinde ve doğru
        }

        if let Some(parent) = item.dest.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let response = self
            .client
            .get(&item.url)
            .send()
            .await
            .map_err(|e| DiscoError::Http(e.to_string()))?;
        let status = response.status();
        if !status.is_success() {
            return Err(DiscoError::Http(format!("HTTP {status}: {}", item.url)));
        }

        // Önce .part dosyasına yaz, sonra atomik adlandır: yarım dosya asla hedefe gitmez.
        let tmp: PathBuf = item.dest.with_extension("part");
        let mut file = tokio::fs::File::create(&tmp).await?;
        let mut hasher = Sha1::new();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| DiscoError::Http(e.to_string()))?;
            hasher.update(&chunk);
            file.write_all(&chunk).await?;
        }
        file.flush().await?;
        drop(file);

        let actual = to_hex(&hasher.finalize());
        if let Some(expected) = &item.sha1 {
            let expected = expected.to_lowercase();
            if actual != expected {
                let _ = tokio::fs::remove_file(&tmp).await;
                return Err(DiscoError::Invalid(format!(
                    "SHA1 uyuşmazlığı (beklenen {expected}, gelen {actual}): {}",
                    item.url
                )));
            }
        }

        tokio::fs::rename(&tmp, &item.dest).await?;
        Ok(())
    }

    /// Dosya grubunu paralel indirir; her tamamlanan dosya için ilerleme gönderir.
    pub async fn download_batch(
        &self,
        items: Vec<DownloadItem>,
        progress: Option<ProgressTx>,
    ) -> Result<()> {
        let total = items.len() as u64;
        if total == 0 {
            return Ok(());
        }
        let done = Arc::new(AtomicU64::new(0));
        let failures: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        let stream = futures_util::stream::iter(items).map(|item| {
            let done = done.clone();
            let failures = failures.clone();
            let progress = progress.clone();
            async move {
                if let Err(err) = self.download_file(&item).await {
                    tracing::warn!("indirme hatası {}: {err}", item.url);
                    failures.lock().unwrap().push(format!("{}: {err}", item.url));
                }
                let finished = done.fetch_add(1, Ordering::Relaxed) + 1;
                if let Some(tx) = &progress && finished % 25 == 0 {
                    let _ = tx.send(BatchProgress { done: finished, total });
                }
            }
        });
        let mut buffered = stream.buffer_unordered(CONCURRENCY);
        while let Some(_) = buffered.next().await {}

        // son ilerlemeyi zorla gönder
        if let Some(tx) = &progress {
            let _ = tx.send(BatchProgress { done: total, total });
        }

        let failures = failures.lock().unwrap();
        if failures.is_empty() {
            Ok(())
        } else {
            let first = &failures[0];
            Err(DiscoError::Http(format!(
                "{}/{} dosya başarısız, ilk hata: {first}",
                failures.len(),
                total
            )))
        }
    }
}

/// Dosyanın SHA1 hash'ini hesaplar (dosya yoksa hata).
pub async fn file_sha1(path: &Path) -> Result<String> {
    let bytes = tokio::fs::read(path).await?;
    let hash = Sha1::digest(&bytes);
    Ok(to_hex(&hash))
}

fn to_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}
