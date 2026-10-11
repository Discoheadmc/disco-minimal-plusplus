//! Başlatma akışı: sürümü çöz → dosyaları indir → komutu kur → çalıştır.
//!
//! Tüm ilerleme `LaunchEvent` olarak arayüze akıtılır; burada UI'a dair
//! hiçbir bilgi tutulmaz.

use disco_auth::Account;
use disco_common::{DiscoError, Result};
use disco_mc::launch::{
    build_command, find_java, LaunchDirs, LaunchIdentity, LaunchJava,
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

    send(
        &events,
        LaunchEvent::Stage("Dosyalar hazırlanıyor (ilk seferde indirme uzun sürebilir)…".into()),
    );
    let (progress_tx, mut progress_rx) =
        tokio::sync::mpsc::unbounded_channel::<disco_net::BatchProgress>();
    let events_for_progress = events.clone();
    let progress_task = tokio::spawn(async move {
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

    let files = prepare_files(&http, &resolved, Some(progress_tx)).await?;
    let _ = progress_task.abort();

    send(&events, LaunchEvent::Stage("Natives çıkarılıyor…".into()));
    extract_natives(&files.native_jars, &instance.natives_dir()?).await?;

    send(&events, LaunchEvent::Stage("Java bulunuyor…".into()));
    let java_path = settings
        .java_path
        .as_ref()
        .map(std::path::PathBuf::from)
        .or_else(find_java)
        .ok_or_else(|| {
            DiscoError::NotFound(
                "Java bulunamadı. JAVA_HOME tanımlayın veya Ayarlar'dan yol girin.".into(),
            )
        })?;

    let account = Account::offline(&settings.player_name);
    let identity = LaunchIdentity {
        // modern MC düz UUID bekliyor
        uuid: account.uuid.replace('-', ""),
        name: account.name.clone(),
        access_token: "0".into(),
        user_type: account.user_type(),
        xuid: None,
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
