//! Disco Minimal++ — ortak temel: sabitler, veri dizinleri, hata tipleri.

pub mod constants {
    pub const APP_NAME: &str = "Disco Minimal++";
    pub const APP_ID: &str = "disco-minimal-plusplus";
    pub const ORGANIZATION: &str = "Discoheadmc";
    pub const VERSION: &str = env!("CARGO_PKG_VERSION");
    pub const WINDOW_TITLE: &str = "Disco Minimal++";
    pub const USER_AGENT: &str = concat!("DiscoMinimalPlusPlus/", env!("CARGO_PKG_VERSION"));
}

pub mod paths {
    use std::path::PathBuf;
    use std::sync::OnceLock;

    use crate::constants;
    use crate::{DiscoError, Result};

    /// Kullanıcı istediği takdirde veri dizinini başka yola yönlendirebilir.
    static DATA_DIR: OnceLock<PathBuf> = OnceLock::new();

    pub fn set_data_dir(dir: PathBuf) {
        let _ = DATA_DIR.set(dir);
    }

    /// Launcher'ın tüm verisini sakladığı kök dizin.
    pub fn data_dir() -> Result<PathBuf> {
        if let Some(dir) = DATA_DIR.get() {
            return Ok(dir.clone());
        }
        let dirs = directories::ProjectDirs::from("", constants::ORGANIZATION, constants::APP_ID)
            .ok_or(DiscoError::NoDataDir)?;
        Ok(dirs.data_local_dir().to_path_buf())
    }

    pub fn instances_dir() -> Result<PathBuf> {
        Ok(data_dir()?.join("instances"))
    }

    pub fn versions_dir() -> Result<PathBuf> {
        Ok(data_dir()?.join("versions"))
    }

    pub fn libraries_dir() -> Result<PathBuf> {
        Ok(data_dir()?.join("libraries"))
    }

    pub fn assets_dir() -> Result<PathBuf> {
        Ok(data_dir()?.join("assets"))
    }

    pub fn settings_file() -> Result<PathBuf> {
        Ok(data_dir()?.join("settings.json"))
    }

    /// Tüm kök dizinleri yoksa oluşturur.
    pub fn ensure_layout() -> Result<()> {
        for dir in [
            data_dir(),
            instances_dir(),
            versions_dir(),
            libraries_dir(),
            assets_dir(),
        ] {
            std::fs::create_dir_all(dir?)?;
        }
        Ok(())
    }
}

#[derive(thiserror::Error, Debug)]
pub enum DiscoError {
    #[error("io hatası: {0}")]
    Io(#[from] std::io::Error),

    #[error("json hatası: {0}")]
    Json(#[from] serde_json::Error),

    #[error("http hatası: {0}")]
    Http(String),

    #[error("veri dizini bulunamadı")]
    NoDataDir,

    #[error("bulunamadı: {0}")]
    NotFound(String),

    #[error("geçersiz: {0}")]
    Invalid(String),

    #[error("{0}")]
    Other(String),
}

pub type Result<T, E = DiscoError> = std::result::Result<T, E>;
