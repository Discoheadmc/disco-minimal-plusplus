//! Disco Minimal++ — çekirdek: instance'lar, ayarlar, başlatma akışı.
//!
//! Dosya tabanlı instance modeli (XMCL/Prism geleneği): her instance kendi
//! klasöründe yaşar, tek bir `instance.json` tanımlar. Ayarlar tek
//! `settings.json` dosyasındadır. Arabirim sadece bu modül üzerinden konuşur.

pub mod instance;
pub mod java;
pub mod launcher;
pub mod settings;

pub use instance::Instance;
pub use settings::LauncherSettings;

use serde::{Deserialize, Serialize};

/// Başlatma akışının arayüze bildirdiği olaylar.
#[derive(Clone, Debug)]
pub enum LaunchEvent {
    /// İnsan tarafından okunur aşama metni.
    Stage(String),
    /// Dosya bazlı ilerleme.
    Progress { done: u64, total: u64 },
    /// Oyun süreci başladı.
    Running { pid: Option<u32> },
    /// Oyun süreci kapandı.
    Exited,
    /// Bir yer kırıldı.
    Failed(String),
}

pub type LaunchEventTx = tokio::sync::mpsc::UnboundedSender<LaunchEvent>;

/// Ayarlarda saklanan basit renk paleti (tema seçici Kilometre Taşı 2'de
/// genişleyecek; Disco Launcher'daki 8 renk geleneği buraya taşınacak).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct AccentColor {
    pub hex: String,
}

impl Default for AccentColor {
    fn default() -> Self {
        Self {
            hex: "#7c5cff".into(),
        }
    }
}
