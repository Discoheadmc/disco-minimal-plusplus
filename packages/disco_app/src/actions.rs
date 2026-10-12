//! Arayüz eylemleri — UI ile arka plan görevleri arasındaki tek köprü.
//!
//! Her eylem ya senkron disk okuması yapar ya da `spawn_forever` ile arka plan
//! görevi açar; görev bitince sonuç `station` üzerinden ilgili kanala yazılır.

use freya::prelude::*;
use freya::radio::RadioStation;

use crate::state::{AppChannel, AppState, LaunchState, ManifestState, LoginUiState};

#[derive(Clone)]
pub struct Actions {
    station: RadioStation<AppState, AppChannel>,
}

pub fn use_provide_actions(actions: &Actions) {
    let actions = actions.clone();
    use_provide_root_context(move || actions.clone());
}

pub fn use_dispatch() -> Actions {
    consume_root_context::<Actions>()
}

impl Actions {
    pub fn new(station: RadioStation<AppState, AppChannel>) -> Self {
        Self { station }
    }

    fn write(&self, channel: AppChannel, mutate: impl FnOnce(&mut AppState)) {
        let mut guard = self.station.clone().write_channel(channel);
        mutate(&mut guard);
    }

    // ------------------------------------------------------------------
    // Instance'lar
    // ------------------------------------------------------------------

    pub fn refresh_instances(&self) {
        let instances = disco_core::instance::list().unwrap_or_default();
        self.write(AppChannel::Instances, |s| s.instances = instances);
    }

    pub fn create_instance(&self, name: String, version: String) {
        let actions = self.clone();
        spawn_forever(async move {
            match disco_core::instance::create(&name, &version) {
                Ok(_) => {
                    actions.refresh_instances();
                    actions.write(AppChannel::Launch, |s| {
                        s.launch = LaunchState::Idle;
                    });
                }
                Err(err) => actions.write(AppChannel::Launch, |s| {
                    s.launch = LaunchState::Failed(format!("Instance oluşturulamadı: {err}"));
                }),
            }
        });
    }

    pub fn delete_instance(&self, name: String) {
        let actions = self.clone();
        spawn_forever(async move {
            if let Err(err) = disco_core::instance::delete(&name) {
                actions.write(AppChannel::Launch, |s| {
                    s.launch = LaunchState::Failed(format!("Silinemedi: {err}"));
                });
            } else {
                actions.refresh_instances();
            }
        });
    }

    // ------------------------------------------------------------------
    // Sürüm manifesti
    // ------------------------------------------------------------------

    pub fn refresh_manifest(&self) {
        let actions = self.clone();
        spawn_forever(async move {
            let http = match disco_net::Http::new() {
                Ok(http) => http,
                Err(err) => {
                    actions.write(AppChannel::Manifest, |s| {
                        s.manifest = ManifestState::Failed(err.to_string());
                    });
                    return;
                }
            };
            match disco_mc::fetch_manifest(&http).await {
                Ok(manifest) => {
                    let latest = manifest.latest.release.clone();
                    let releases: Vec<String> =
                        manifest.releases().take(10).map(|e| e.id.clone()).collect();
                    actions.write(AppChannel::Manifest, |s| {
                        s.manifest = ManifestState::Ready { latest, releases };
                    });
                }
                Err(err) => actions.write(AppChannel::Manifest, |s| {
                    s.manifest = ManifestState::Failed(err.to_string());
                }),
            }
        });
    }

    // ------------------------------------------------------------------
    // Başlatma
    // ------------------------------------------------------------------

    pub fn launch_instance(&self, instance_name: String) {
        let actions = self.clone();
        actions.write(AppChannel::Launch, |s| {
            if matches!(s.launch, LaunchState::Idle | LaunchState::Failed(_)) {
                s.launch = LaunchState::Working {
                    stage: "Hazırlanıyor…".into(),
                    done: None,
                    total: None,
                };
            } else {
                // zaten bir başlatma sürüyor; yeni istek kabul edilmez
                return;
            }
        });

        spawn_forever(async move {
            let Some(instance) = disco_core::instance::list()
                .ok()
                .and_then(|list| list.into_iter().find(|i| i.name == instance_name))
            else {
                actions.write(AppChannel::Launch, |s| {
                    s.launch = LaunchState::Failed("Instance bulunamadı".into());
                });
                return;
            };
            let settings = disco_core::settings::load();

            let (events_tx, mut events_rx) = tokio::sync::mpsc::unbounded_channel();
            let pump = actions.clone();
            spawn_forever(async move {
                while let Some(event) = events_rx.recv().await {
                    pump.apply_launch_event(event);
                }
            });

            if let Err(err) = disco_core::launcher::launch(&instance, &settings, events_tx).await {
                tracing::error!("başlatma hatası: {err}");
                actions.apply_launch_event(disco_core::LaunchEvent::Failed(err.to_string()));
            }
        });
    }

    fn apply_launch_event(&self, event: disco_core::LaunchEvent) {
        use disco_core::LaunchEvent;
        self.write(AppChannel::Launch, |s| {
            s.launch = match event {
                LaunchEvent::Stage(stage) => LaunchState::Working { stage, done: None, total: None },
                LaunchEvent::Progress { done, total } => {
                    if let LaunchState::Working { stage, .. } = &s.launch {
                        LaunchState::Working {
                            stage: stage.clone(),
                            done: Some(done),
                            total: Some(total),
                        }
                    } else {
                        s.launch.clone()
                    }
                }
                LaunchEvent::Running { pid } => LaunchState::Running { pid },
                LaunchEvent::Exited => LaunchState::Idle,
                LaunchEvent::Failed(error) => LaunchState::Failed(error),
            };
        });
    }

    // ------------------------------------------------------------------
    // Ayarlar / hesap / tema
    // ------------------------------------------------------------------

    fn persist_settings(&self, mutate: impl FnOnce(&mut disco_core::LauncherSettings)) {
        let mut settings = disco_core::settings::load();
        mutate(&mut settings);
        if let Err(err) = disco_core::settings::save(&settings) {
            tracing::warn!("ayarlar kaydedilemedi: {err}");
        }
    }

    pub fn set_player_name(&self, name: String) {
        let name = disco_auth::sanitize_name(&name);
        self.write(AppChannel::Settings, |s| {
            s.settings.player_name = name.clone();
        });
        self.persist_settings(|settings| settings.player_name = name);
    }

    pub fn set_accent(&self, hex: String) {
        let for_state = hex.clone();
        self.write(AppChannel::Settings, move |s| {
            s.settings.accent = for_state;
        });
        self.persist_settings(move |settings| settings.accent = hex);
    }

    pub fn use_offline(&self) {
        self.write(AppChannel::Account, |s| {
            s.login_gen += 1;
            s.login = LoginUiState::None;
        });
        self.write(AppChannel::Settings, |s| {
            s.settings.active = None;
        });
        self.persist_settings(|settings| settings.active = None);
    }

    /// Microsoft device-code girişini başlatır: kodu göster, onayı bekle,
    /// hesabı kaydet. `login_gen` ile iptal edilen akışların sonucu atılır.
    pub fn start_ms_login(&self) {
        let actions = self.clone();
        let generation;
        {
            let mut guard = self.station.clone().write_channel(AppChannel::Account);
            guard.login_gen += 1;
            generation = guard.login_gen;
        }

        spawn_forever(async move {
            let http = match disco_net::Http::new() {
                Ok(http) => http,
                Err(err) => {
                    actions.finish_login(generation, Err(err.to_string()));
                    return;
                }
            };
            match disco_auth::ms::DeviceLogin::start(&http, disco_auth::ms::DEFAULT_CLIENT_ID)
                .await
            {
                Ok(login) => {
                    let user_code = login.user_code.clone();
                    let verification_uri = login.verification_uri.clone();
                    actions.write(AppChannel::Account, move |s| {
                        if s.login_gen != generation {
                            return;
                        }
                        s.login = LoginUiState::Waiting {
                            user_code,
                            verification_uri,
                        };
                    });
                    match login.wait().await {
                        Ok(tokens) => {
                            match disco_auth::ms::login_minecraft(&http, &tokens).await {
                                Ok(account) => {
                                    actions.finish_login(generation, Ok(account))
                                }
                                Err(err) => {
                                    actions.finish_login(generation, Err(err.to_string()))
                                }
                            }
                        }
                        Err(err) => {
                            actions.finish_login(generation, Err(err.to_string()))
                        }
                    }
                }
                Err(err) => actions.finish_login(generation, Err(err.to_string())),
            }
        });
    }

    fn finish_login(
        &self,
        generation: u64,
        result: Result<disco_auth::Account, String>,
    ) {
        if let Ok(account) = &result {
            let account = account.clone();
            self.persist_settings(move |settings| settings.active = Some(account));
        }
        self.write(AppChannel::Account, move |s| {
            if s.login_gen != generation {
                return; // iptal edildi
            }
            match result {
                Ok(account) => {
                    s.login = LoginUiState::None;
                    s.settings.active = Some(account);
                }
                Err(error) => s.login = LoginUiState::Failed(error),
            }
        });
    }

    pub fn cancel_login(&self) {
        self.write(AppChannel::Account, |s| {
            s.login_gen += 1;
            s.login = LoginUiState::None;
        });
    }
}
