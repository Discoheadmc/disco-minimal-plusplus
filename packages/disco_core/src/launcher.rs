//! Başlatma akışı: sürümü çöz → dosyaları indir → Java sağla → komutu kur → çalıştır.
//!
//! Tüm ilerleme `LaunchEvent` olarak arayüze akıtılır; burada UI'a dair
//! hiçbir bilgi tutulmaz.

use disco_auth::Account;
use disco_common::{DiscoError, Result};
use disco_mc::launch::{
    build_command, LaunchDirs, LaunchIdentity, LaunchJava,
};
use disco_mc::prepare::{extract_natives, prepare_files};
use disco_mc::resolve_version;
use disco_net::Http;

use crate::{instance::Instance, settings::LauncherSettings, LaunchEvent, LaunchEventTx};

/// Bir instance'ı başlatır; oyun süreci kapanana dek olay gönderir.
pub async fn launch(
    instance: &Instance,
    settings: &LauncherSettings,
    events: LaunchEventTx,
) -> Result<()> {
    send(&events, LaunchEvent::Stage("Sürüm bilgisi çözülüyor…".into()));

    let http = Http::new()?;
    let resolved = resolve_version(&http, &instance.version_id).await?;

    // ilerleme kanalını hem dosya hazırlığı hem java indirmesi paylaşır
    let (progress_tx, mut progress_rx) =
        tokio::sync::mpsc::unbounded_channel::<disco_net::BatchProgress>();
    let events_for_progress = events.clone();
    tokio::spawn(async move {
        while let Some(progress) = progress_rx.recv().await {
            send(
                &events_for_progress,
                LaunchEvent::Progress {
                    done: progress.done,
                    total: progress.total,
                },
            );
        }
    });

    send(
        &events,
        LaunchEvent::Stage("Dosyalar hazırlanıyor (ilk seferde indirme uzun sürebilir)…".into()),
    );
    let files = prepare_files(&http, &resolved, Some(progress_tx.clone())).await?;

    send(&events, LaunchEvent::Stage("Natives çıkarılıyor…".into()));
    extract_natives(&files.native_jars, &instance.natives_dir()?).await?;

    // ---- Java sağlama -------------------------------------------------------
    let required_major = resolved.java_major;
    let java_path = match &settings.java_path {
        Some(explicit) => {
            let path = std::path::PathBuf::from(explicit);
            if !path.exists() {
                return Err(DiscoError::NotFound(format!(
                    "Ayarlandaki Java yolu yok: {}",
                    path.display()
                )));
            }
            path
        }
        None => match crate::java::find_suitable(required_major).await {
            Some(system_java) => {
                tracing::info!("sistem Java'sı kullanılıyor: {}", system_java.display());
                system_java
            }
            None => {
                send(
                    &events,
                    LaunchEvent::Stage("Java bulunamadı — Mojang runtime indiriliyor…".into()),
                );
                crate::java::ensure_installed(&http, required_major, Some(progress_tx.clone()))
                    .await?
            }
        },
    };

    // ---- Hesap / token ------------------------------------------------------
    send(&events, LaunchEvent::Stage("Hesap hazırlanıyor…".into()));
    let mut account = settings
        .active
        .clone()
        .unwrap_or_else(|| Account::offline(&settings.player_name));
    disco_auth::ensure_valid(&mut account, &http).await?;

    // yenilenmiş token'ları kalıcı hale getir
    if settings.active.is_some() {
        let mut updated = settings.clone();
        updated.active = Some(account.clone());
        let _ = crate::settings::save(&updated);
    }

    let access_token = match &account.kind {
        disco_auth::AccountKind::Offline => "0".to_string(),
        disco_auth::AccountKind::Microsoft { access_token, .. } => access_token.clone(),
    };
    let xuid = match &account.kind {
        disco_auth::AccountKind::Microsoft { xuid, .. } => xuid.clone(),
        disco_auth::AccountKind::Offline => None,
    };

    let identity = LaunchIdentity {
        // modern MC düz UUID bekliyor
        uuid: account.uuid.replace('-', ""),
        name: account.name.clone(),
        access_token,
        user_type: account.user_type(),
        xuid,
    };

    let dirs = LaunchDirs {
        game_dir: instance.game_dir()?,
        natives_dir: instance.natives_dir()?,
        assets_dir: disco_common::paths::assets_dir()?,
    };
    let java = LaunchJava {
        path: java_path,
        memory_mb: settings.memory_mb.max(512),
    };

    send(&events, LaunchEvent::Stage("Oyun başlatılıyor…".into()));
    let mut command = build_command(&resolved, &files, &java, &dirs, &identity)
        .map_err(DiscoError::Other)?;

    let mut child = command
        .spawn()
        .map_err(|e| DiscoError::Other(format!("oyun başlatılamadı: {e}")))?;
    send(&events, LaunchEvent::Running {
        pid: Some(child.id()),
    });

    crate::instance::touch_played(instance);

    // Oyun açıkken yaşam döngüsünü izle: UI "çalışıyor" göstersin.
    let events_wait = events.clone();
    tokio::task::spawn_blocking(move || {
        if let Ok(status) = child.wait() {
            tracing::info!("oyun kapandı: {status}");
            send(&events_wait, LaunchEvent::Exited);
        }
    });
    Ok(())
}

fn send(events: &LaunchEventTx, event: LaunchEvent) {
    let _ = events.send(event);
}
