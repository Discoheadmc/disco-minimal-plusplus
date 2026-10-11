//! Arayüz eylemleri — UI ile arka plan görevleri arasındaki tek köprü.
//!
//! Her eylem ya senkron disk okuması yapar ya da `spawn_forever` ile arka plan
//! görevi açar; görev bitince sonuç `station` üzerinden ilgili kanala yazılır.

use freya::prelude::*;
use freya::radio::RadioStation;

use crate::state::{AppChannel, AppState, LaunchState, ManifestState};

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
    // Ayarlar
    // ------------------------------------------------------------------

    pub fn set_player_name(&self, name: String) {
        let name = disco_auth::sanitize_name(&name);
        self.write(AppChannel::Settings, |s| {
            s.settings.player_name = name.clone();
        });
        let settings = disco_core::settings::load();
        let mut settings = settings;
        settings.player_name = name;
        if let Err(err) = disco_core::settings::save(&settings) {
            tracing::warn!("ayarlar kaydedilemedi: {err}");
        }
    }
}
