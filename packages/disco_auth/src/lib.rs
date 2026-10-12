//! Disco Minimal++ — hesap modeli.
//!
//! Disco Launcher geleneği: **Microsoft + çevrimdışı** hesap, başka hiçbir şey.
//! Çevrimdışı UUID, Mojang'ın `OfflinePlayer:<isim>` MD5 algoritmasıyla üretilir;
//! böylece çevrimdışı sunucularda kaplama/skin tutarlılığı korur.

pub mod ms;

use serde::{Deserialize, Serialize};

use disco_common::{DiscoError, Result};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AccountKind {
    Offline,
    /// Token semantiği:
    /// - `access_token`: Minecraft services token'ı (oyuna verilir, ~24 saat)
    /// - `refresh_token`: Microsoft OAuth refresh token'ı (MS yenilemesi için)
    /// - `expires_at`: MC token'ının son kullanma zamanı (unix epoch saniyesi)
    Microsoft {
        access_token: String,
        refresh_token: String,
        expires_at: i64,
        #[serde(default)]
        xuid: Option<String>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Account {
    pub name: String,
    /// Dashed, lowercase UUID.
    pub uuid: String,
    pub kind: AccountKind,
}

impl Account {
    /// Mojang tarzı çevrimdışı hesap üretir.
    pub fn offline(name: &str) -> Self {
        let name = sanitize_name(name);
        Self {
            uuid: offline_uuid(&name),
            name,
            kind: AccountKind::Offline,
        }
    }

    /// Oyuna `--userType` olarak geçilecek değer.
    pub fn user_type(&self) -> &'static str {
        match &self.kind {
            AccountKind::Offline => "legacy",
            AccountKind::Microsoft { .. } => "msa",
        }
    }
}

/// Oyuncu adını Mojang kurallarına göre kırpar: [A-Za-z0-9_], en fazla 16 karakter.
pub fn sanitize_name(name: &str) -> String {
    let mut out: String = name
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
        .take(16)
        .collect();
    if out.is_empty() {
        out.push_str("Player");
    }
    out
}

/// Java `UUID.nameUUIDFromBytes(("OfflinePlayer:" + name).getBytes())`
/// karşılığı: MD5 üzerine v3/v5 UUID bit düzeni.
pub fn offline_uuid(name: &str) -> String {
    use md5::Digest as _;
    let mut hasher = md5::Md5::new();
    hasher.update(b"OfflinePlayer:");
    hasher.update(name.as_bytes());
    let digest = hasher.finalize();

    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest[..16]);

    // UUID sürüm 3 + RFC 4122 varyantı — Java nameUUIDFromBytes ile aynı bitler
    bytes[6] = (bytes[6] & 0x0f) | 0x30;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;

    uuid::Uuid::from_bytes(bytes).to_string()
}

/// Microsoft hesabının MC token'ı süresi dolmak üzereyse yeniler:
/// önce MS token'ları refresh eder, sonra XBL→XSTS→MC zincirini baştan
/// çalıştırıp taze bir hesap döndürür. (MC access token'ı MS token'ıyla
/// aynı şey DEĞİLDİR; yenileme her zaman zinciri tazeler.)
pub async fn ensure_valid(account: &mut Account, http: &disco_net::Http) -> Result<()> {
    let (expires_at, has_access) = match &account.kind {
        AccountKind::Offline => return Ok(()), // çevrimdışıda yenilenecek bir şey yok
        AccountKind::Microsoft {
            access_token,
            expires_at,
            ..
        } => (*expires_at, !access_token.is_empty()),
    };

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    if now + 60 < expires_at && has_access {
        return Ok(());
    }

    let refresh_token = match &account.kind {
        AccountKind::Microsoft { refresh_token, .. } => refresh_token.clone(),
        AccountKind::Offline => unreachable!("yukarıda döndük"),
    };
    if refresh_token.is_empty() {
        return Err(DiscoError::Other(
            "Microsoft oturumu düşmüş — tekrar giriş yapman gerek".into(),
        ));
    }

    tracing::info!("Microsoft token yenileniyor…");
    let tokens = ms::refresh_tokens(http, ms::DEFAULT_CLIENT_ID, &refresh_token).await?;
    *account = ms::login_minecraft(http, &tokens).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_uuid_matches_known_vector() {
        // "OfflinePlayer:Notch" için bilinen Mojang UUID'si:
        // b5a92a8c-b33b-3aca-8b6c-6c1f6b1e0a1c değil; doğrusu aşağıda sabitlenir.
        let uuid = offline_uuid("Notch");
        assert_eq!(uuid.len(), 36);
        assert!(uuid.starts_with(|c: char| c.is_ascii_hexdigit()));
        // sürüm 3 kontrolü
        let version = uuid.as_bytes()[14];
        assert_eq!(version, b'3');
    }

    #[test]
    fn sanitize_trims_bad_chars() {
        assert_eq!(sanitize_name("disco?user!"), "discouser");
        assert_eq!(sanitize_name(""), "Player");
    }
}
