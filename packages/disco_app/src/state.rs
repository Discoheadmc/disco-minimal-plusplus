//! UI durum modeli — tek `AppState`, kanal bazlı abonelik.

use freya::radio::RadioChannel;

use disco_core::{Instance, LauncherSettings};

/// Yazma kanalları: her kanal sadece kendi abonelerini uyandırır.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AppChannel {
    Instances,
    Manifest,
    Launch,
    Settings,
    Account,
}

impl RadioChannel<AppState> for AppChannel {}

pub struct AppState {
    pub instances: Vec<Instance>,
    pub manifest: ManifestState,
    pub launch: LaunchState,
    pub settings: LauncherSettings,
    /// Microsoft giriş akışının UI tarafı.
    pub login: LoginUiState,
    /// İptal edilen eski oturumların sonucu geçersiz kalsın diye nesil sayacı.
    pub login_gen: u64,
}

/// Sürüm manifesti (UI için sadeleştirilmiş).
#[derive(Clone, PartialEq)]
pub enum ManifestState {
    Loading,
    Ready { latest: String, releases: Vec<String> },
    Failed(String),
}

/// Başlatma durum makinesi.
#[derive(Clone, PartialEq)]
pub enum LaunchState {
    Idle,
    Working { stage: String, done: Option<u64>, total: Option<u64> },
    Running { pid: Option<u32> },
    Failed(String),
}

/// Microsoft device-code girişinin ekrandaki hali.
#[derive(Clone, PartialEq)]
pub enum LoginUiState {
    None,
    Waiting { user_code: String, verification_uri: String },
    Failed(String),
}

impl AppState {
    pub fn initial() -> Self {
        Self {
            instances: Vec::new(),
            manifest: ManifestState::Loading,
            launch: LaunchState::Idle,
            settings: disco_core::settings::load(),
            login: LoginUiState::None,
            login_gen: 0,
        }
    }
}
