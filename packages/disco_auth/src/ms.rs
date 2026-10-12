//! Microsoft hesap girişi — device code flow.
//!
//! Akış: device code → tarayıcıda onay → Xbox Live (XBL) → XSTS →
//! Minecraft services → profil. Token'lar `settings.json`'da saklanır ve
//! son kullanma tarihinden önce otomatik yenilenir.

use disco_common::{DiscoError, Result};
use disco_net::Http;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Ayarlarda değiştirilebilir OAuth client_id. Varsayılan, açık kaynak
/// launcher ekosisteminde yaygın kullanılan kamu Prism Launcher kaydıdır.
pub const DEFAULT_CLIENT_ID: &str = "c36a9fb6-4f2a-41ff-90bd-4772f7c1e6b6";
pub const SCOPE: &str = "XboxLive.signin offline_access";

const DEVICE_ENDPOINT: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode";
const TOKEN_ENDPOINT: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";
const XBL_ENDPOINT: &str = "https://user.auth.xboxlive.com/user/authenticate";
const XSTS_ENDPOINT: &str = "https://xsts.auth.xboxlive.com/xsts/authorize";
const MC_LOGIN_ENDPOINT: &str = "https://api.minecraftservices.com/authentication/login_with_xbox";
const MC_PROFILE_ENDPOINT: &str = "https://api.minecraftservices.com/minecraft/profile";

// ---------------------------------------------------------------------------
// Yanıt tipleri
// ---------------------------------------------------------------------------

#[derive(Deserialize, Clone, Debug)]
struct DeviceCodeResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    #[serde(default)]
    expires_in: u64,
    #[serde(default = "default_interval")]
    interval: u64,
}

fn default_interval() -> u64 {
    5
}

#[derive(Deserialize, Clone, Debug)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    expires_in: i64,
    #[serde(default)]
    id_token: Option<String>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct MsTokens {
    pub access_token: String,
    pub refresh_token: String,
    /// Unix epoch saniyesi.
    pub expires_at: i64,
    #[serde(default)]
    pub xuid: Option<String>,
}

#[derive(Deserialize, Clone, Debug)]
struct XblResponse {
    #[serde(rename = "Token")]
    token: String,
    #[serde(rename = "DisplayClaims")]
    claims: XblClaims,
}

#[derive(Deserialize, Clone, Debug)]
struct XblClaims {
    #[serde(rename = "xui")]
    xui: Vec<XuiClaim>,
}

#[derive(Deserialize, Clone, Debug)]
struct XuiClaim {
    #[serde(rename = "uhs", default)]
    uhs: String,
}

#[derive(Deserialize, Clone, Debug)]
struct McLoginResponse {
    access_token: String,
    #[serde(default)]
    expires_in: i64,
}

#[derive(Deserialize, Clone, Debug)]
struct McProfile {
    name: String,
    id: String,
}

// ---------------------------------------------------------------------------
// Device code akışı
// ---------------------------------------------------------------------------

/// Kullanıcının tarayıcıda onaylaması gereken giriş oturumu.
pub struct DeviceLogin {
    device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    interval: u64,
    expires_at: std::time::Instant,
    http: Http,
    client_id: String,
}

impl DeviceLogin {
    /// Yeni device code ister. Dönen kodu kullanıcıya göster.
    pub async fn start(http: &Http, client_id: &str) -> Result<Self> {
        let raw = http
            .post_form_raw(
                DEVICE_ENDPOINT,
                &[("client_id", client_id), ("scope", SCOPE)],
            )
            .await?;
        let response: DeviceCodeResponse = serde_json::from_str(&raw)
            .map_err(|e| DiscoError::Invalid(format!("device code yanıtı bozuk: {e}")))?;
        Ok(Self {
            device_code: response.device_code,
            user_code: response.user_code,
            verification_uri: response.verification_uri,
            interval: response.interval.max(1),
            expires_at: std::time::Instant::now()
                + Duration::from_secs(response.expires_in.max(60)),
            http: http.clone(),
            client_id: client_id.to_string(),
        })
    }

    /// Onay gelene dek yoklar. Oturum reddedilirse/boşa düşerse hata verir.
    pub async fn wait(&self) -> Result<MsTokens> {
        loop {
            if std::time::Instant::now() > self.expires_at {
                return Err(DiscoError::Other("giriş oturumu zaman aşımına uğradı".into()));
            }
            tokio::time::sleep(Duration::from_secs(self.interval)).await;

            let raw = self
                .http
                .post_form_raw(
                    TOKEN_ENDPOINT,
                    &[
                        (
                            "grant_type",
                            "urn:ietf:params:oauth:grant-type:device_code",
                        ),
                        ("client_id", self.client_id.as_str()),
                        ("device_code", self.device_code.as_str()),
                    ],
                )
                .await?;

            // önce başarı, sonra bilinen bekleyici/hata kodları
            if let Ok(token) = serde_json::from_str::<TokenResponse>(&raw) {
                return Ok(finish_tokens(token));
            }
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) {
                let code = value
                    .get("error")
                    .and_then(|e| e.as_str())
                    .unwrap_or("unknown");
                match code {
                    "authorization_pending" | "slow_down" => continue,
                    "expired_token" => {
                        return Err(DiscoError::Other(
                            "giriş oturumu zaman aşımına uğradı".into(),
                        ));
                    }
                    "authorization_declined" => {
                        return Err(DiscoError::Other(
                            "giriş reddedildi — tekrar deneyebilirsin".into(),
                        ));
                    }
                    other => {
                        return Err(DiscoError::Other(format!(
                            "Microsoft girişi başarısız ({other})"
                        )));
                    }
                }
            }
            return Err(DiscoError::Invalid("token yanıtı anlaşılamadı".into()));
        }
    }
}

fn finish_tokens(response: TokenResponse) -> MsTokens {
    let expires_at = chrono_now() + response.expires_in.max(60);
    MsTokens {
        access_token: response.access_token,
        refresh_token: response.refresh_token.unwrap_or_default(),
        expires_at,
        xuid: response
            .id_token
            .as_deref()
            .and_then(extract_xuid),
    }
}

fn chrono_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// id_token JWT'sinin payload'ından xuid çıkarır.
fn extract_xuid(id_token: &str) -> Option<String> {
    use base64::Engine as _;
    let payload = id_token.split('.').nth(1)?;
    let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload)
        .ok()?;
    let claims: serde_json::Value = serde_json::from_slice(&decoded).ok()?;
    claims
        .get("xuid")
        .and_then(|x| x.as_str())
        .map(str::to_string)
}

// ---------------------------------------------------------------------------
// Token yenileme
// ---------------------------------------------------------------------------

/// Süresi yaklaşan token'ları refresh_token ile yeniler.
pub async fn refresh_tokens(http: &Http, client_id: &str, refresh_token: &str) -> Result<MsTokens> {
    let raw = http
        .post_form_raw(
            TOKEN_ENDPOINT,
            &[
                ("grant_type", "refresh_token"),
                ("client_id", client_id),
                ("refresh_token", refresh_token),
                ("scope", SCOPE),
            ],
        )
        .await?;
    let mut response: TokenResponse = serde_json::from_str(&raw)
        .map_err(|e| DiscoError::Invalid(format!("token yanıtı bozuk: {e}")))?;
    if response.refresh_token.is_none() {
        // yenilemede refresh dönmeyebilir; eskisini koru
        response.refresh_token = Some(refresh_token.to_string());
    }
    Ok(finish_tokens(response))
}

// ---------------------------------------------------------------------------
// Xbox/Minecraft zinciri
// ---------------------------------------------------------------------------

/// MS token'dan gerçek Minecraft hesabına ulaşır (XBL → XSTS → MC → profil).
pub async fn login_minecraft(http: &Http, tokens: &MsTokens) -> Result<super::Account> {
    // 1) Xbox Live
    let xbl: XblResponse = http
        .post_json(
            XBL_ENDPOINT,
            serde_json::json!({
                "Properties": {
                    "AuthMethod": "RPS",
                    "SiteName": "user.auth.xboxlive.com",
                    "RpsTicket": format!("d={}", tokens.access_token),
                },
                "RelyingParty": "http://auth.xboxlive.com",
                "TokenType": "JWT",
            }),
        )
        .await?;

    let uhs = xbl
        .claims
        .xui
        .first()
        .map(|c| c.uhs.clone())
        .ok_or_else(|| DiscoError::Invalid("XBL yanıtında uhs yok".into()))?;

    // 2) XSTS
    let xsts: XblResponse = http
        .post_json(
            XSTS_ENDPOINT,
            serde_json::json!({
                "Properties": {
                    "SandboxId": "RETAIL",
                    "UserTokens": [xbl.token],
                },
                "RelyingParty": "rp://api.minecraftservices.com/",
                "TokenType": "JWT",
            }),
        )
        .await
        .map_err(|e| map_xsts_error(&e))?;

    let uhs = xsts
        .claims
        .xui
        .first()
        .map(|c| c.uhs.clone())
        .unwrap_or(uhs);

    // 3) Minecraft services
    let mc_login: McLoginResponse = http
        .post_json(
            MC_LOGIN_ENDPOINT,
            serde_json::json!({
                "identityToken": format!("XBL3.0 x={uhs};{}", xsts.token),
            }),
        )
        .await?;

    // 4) Profil (isim + UUID)
    let profile: McProfile = http
        .get_json_bearer(MC_PROFILE_ENDPOINT, &mc_login.access_token)
        .await?;

    Ok(super::Account {
        name: profile.name,
        uuid: dashed_uuid(&profile.id),
        kind: super::AccountKind::Microsoft {
            access_token: mc_login.access_token,
            refresh_token: tokens.refresh_token.clone(),
            expires_at: chrono_now() + mc_login.expires_in.max(60),
            xuid: tokens.xuid.clone(),
        },
    })
}

fn map_xsts_error(err: &DiscoError) -> DiscoError {
    let text = err.to_string();
    if text.contains("2148916233") || text.contains("The request was denied") {
        return DiscoError::Other(
            "Hesap yaşı uygun değil (çocuk hesabı XBOX onayı gerektirebilir)".into(),
        );
    }
    if text.contains("2148916238") {
        return DiscoError::Other("Bu Microsoft hesabına bağlı bir Xbox hesabı yok".into());
    }
    DiscoError::Http(text)
}

/// 32 haneli UUID'yi dashed formata çevirir.
fn dashed_uuid(raw: &str) -> String {
    let clean: String = raw.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    if clean.len() != 32 {
        return raw.to_string();
    }
    format!(
        "{}-{}-{}-{}-{}",
        &clean[0..8],
        &clean[8..12],
        &clean[12..16],
        &clean[16..20],
        &clean[20..32],
    )
}
