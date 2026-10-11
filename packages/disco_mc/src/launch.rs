//! Disco Minimal++ — oyunu başlatan komutu üretir.
//!
//! Modern (`arguments`) ve eski (`minecraftArguments`) sürüm biçimlerinin
//! ikisini de destekler; `${...}` yer tutucuları bağlamdan doldurulur.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::ResolvedVersion;

/// Oyuncu kimliği (hesap modelinden düzleştirilmiş).
#[derive(Clone, Debug)]
pub struct LaunchIdentity {
    pub name: String,
    pub uuid: String,
    pub access_token: String,
    pub user_type: &'static str,
    pub xuid: Option<String>,
}

/// Java çalıştırıcısı ayarları.
#[derive(Clone, Debug)]
pub struct LaunchJava {
    pub path: PathBuf,
    pub memory_mb: u32,
}

/// Yol bağlamı.
#[derive(Clone, Debug)]
pub struct LaunchDirs {
    pub game_dir: PathBuf,
    pub natives_dir: PathBuf,
    pub assets_dir: PathBuf,
}

struct Placeholders {
    natives_dir: String,
    classpath: String,
    identity: LaunchIdentity,
    version_id: String,
    game_dir: String,
    assets_dir: String,
    assets_index: String,
}

impl Placeholders {
    fn apply(&self, text: &str) -> String {
        text.replace("${natives_directory}", &self.natives_dir)
            .replace("${launcher_name}", "Disco Minimal++")
            .replace("${launcher_version}", disco_common::constants::VERSION)
            .replace("${classpath}", &self.classpath)
            .replace("${auth_player_name}", &self.identity.name)
            .replace("${auth_uuid}", &self.identity.uuid)
            .replace("${auth_access_token}", &self.identity.access_token)
            .replace("${auth_xuid}", self.identity.xuid.as_deref().unwrap_or("0"))
            .replace("${auth_session}", &self.identity.access_token)
            .replace("${clientid}", "disco-minimal-plusplus")
            .replace("${user_type}", self.identity.user_type)
            .replace("${version_name}", &self.version_id)
            .replace("${game_directory}", &self.game_dir)
            .replace("${assets_root}", &self.assets_dir)
            .replace("${assets_index_name}", &self.assets_index)
            .replace("${version_type}", "release")
            .replace("${user_properties}", "{}")
            .replace("${resolution_width}", "854")
            .replace("${resolution_height}", "480")
    }
}

/// Java komutunu üretir; çalıştırma çağıraya bırakılır.
pub fn build_command(
    resolved: &ResolvedVersion,
    files: &crate::prepare::PreparedFiles,
    java: &LaunchJava,
    dirs: &LaunchDirs,
    identity: &LaunchIdentity,
) -> Result<Command, String> {
    let mut classpath: Vec<String> = files
        .classpath
        .iter()
        .chain(std::iter::once(&files.client_jar))
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    classpath.dedup();

    let classpath_sep = if cfg!(windows) { ";" } else { ":" };
    let placeholders = Placeholders {
        natives_dir: dirs.natives_dir.to_string_lossy().into_owned(),
        classpath: classpath.join(classpath_sep),
        identity: identity.clone(),
        version_id: resolved.id.clone(),
        game_dir: dirs.game_dir.to_string_lossy().into_owned(),
        assets_dir: dirs.assets_dir.to_string_lossy().into_owned(),
        assets_index: resolved
            .asset_index
            .as_ref()
            .map(|i| i.id.clone())
            .or_else(|| resolved.assets.clone())
            .unwrap_or_else(|| "legacy".into()),
    };

    if resolved.main_class.is_empty() {
        return Err("sürüm JSON'unda mainClass eksik".into());
    }

    let mut args: Vec<String> = Vec::new();
    args.push(format!("-Xmx{}m", java.memory_mb));

    // JVM argümanları
    let mut saw_classpath = false;
    for value in resolved.jvm_args.iter().flat_map(|a| a.resolve()) {
        if value.contains("${classpath}") {
            saw_classpath = true;
        }
        args.push(placeholders.apply(&value));
    }
    if !saw_classpath {
        // eski/özel sürümler: sınıf yolunu kendimiz kuralım
        args.push("-Djava.library.path=${natives_directory}".replace(
            "${natives_directory}",
            &placeholders.natives_dir,
        ));
        args.push("-cp".into());
        args.push(placeholders.classpath.clone());
    }

    args.push(resolved.main_class.clone());

    // oyun argümanları
    let game_args: Vec<String> = if resolved.game_args.is_empty() {
        resolved
            .legacy_arguments
            .as_deref()
            .map(|legacy| legacy.split_whitespace().map(str::to_string).collect())
            .unwrap_or_default()
    } else {
        resolved.game_args.iter().flat_map(|a| a.resolve()).collect()
    };
    for value in game_args {
        args.push(placeholders.apply(&value));
    }

    let mut command = Command::new(&java.path);
    command.args(args).current_dir(&dirs.game_dir);
    Ok(command)
}

/// Windows'ta `java.exe` bulunamadıysa `PATH` içinde arar.
pub fn java_file_name() -> &'static str {
    if cfg!(windows) { "java.exe" } else { "java" }
}

/// Basit Java keşfi: `JAVA_HOME` sonra `PATH`.
pub fn find_java() -> Option<PathBuf> {
    if let Ok(home) = std::env::var("JAVA_HOME") {
        let candidate = Path::new(&home).join("bin").join(java_file_name());
        if candidate.exists() {
            return Some(candidate);
        }
    }
    if let Ok(path) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path) {
            let candidate = dir.join(java_file_name());
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }
    None
}
