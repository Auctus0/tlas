use std::path::PathBuf;
use std::time::Duration;

use parking_lot::Mutex;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::models::LcuStatus;

#[derive(Debug, Clone)]
pub struct LcuCredentials {
    pub port: u16,
    pub password: String,
}

#[derive(Debug, thiserror::Error)]
pub enum LcuError {
    #[error("LoL istemcisi açık değil")]
    NotRunning,
    #[error("LCU isteği başarısız: {0}")]
    Http(#[from] reqwest::Error),
    #[error("LCU yanıtı okunamadı: {0}")]
    Json(#[from] serde_json::Error),
    #[error("LCU durumu: {0}")]
    Status(u16),
}

pub struct Lcu {
    client: reqwest::Client,
    creds: Mutex<Option<LcuCredentials>>,
}

impl Lcu {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .http1_only()
            .timeout(Duration::from_secs(4))
            .build()
            .expect("LCU HTTP client oluşturulamadı");

        Self {
            client,
            creds: Mutex::new(None),
        }
    }

    fn lockfile_paths() -> Vec<PathBuf> {
        let mut paths = Vec::new();

        if let Ok(path) = std::env::var("LEAGUE_LOCKFILE") {
            paths.push(PathBuf::from(path));
        }

        // Yaygın kurulumlar. Farklı bir sürücüye kurduysan LEAGUE_LOCKFILE ile override et.
        paths.push(PathBuf::from(r"C:\Riot Games\League of Legends\lockfile"));
        paths.push(PathBuf::from(
            r"C:\Program Files\Riot Games\League of Legends\lockfile",
        ));
        paths.push(PathBuf::from(
            r"C:\Program Files (x86)\Riot Games\League of Legends\lockfile",
        ));

        // Riot Client bazen farklı sürücüye kurulur; tüm sürücüleri dene.
        for letter in b'C'..=b'Z' {
            let path = format!(r"{}:\Riot Games\League of Legends\lockfile", letter as char);
            let path = PathBuf::from(path);
            if !paths.contains(&path) {
                paths.push(path);
            }
        }

        paths
    }

    fn read_credentials() -> Option<LcuCredentials> {
        for path in Self::lockfile_paths() {
            let Ok(raw) = std::fs::read_to_string(&path) else {
                continue;
            };

            // format: name:pid:port:password:protocol
            let parsed = (|| {
                let mut parts = raw.trim().split(':');
                let _name = parts.next()?;
                let _pid = parts.next()?;
                let port = parts.next()?.parse::<u16>().ok()?;
                let password = parts.next()?.to_string();
                let _protocol = parts.next()?;

                Some(LcuCredentials { port, password })
            })();

            if let Some(credentials) = parsed {
                return Some(credentials);
            }
        }

        None
    }

    fn credentials(&self) -> Result<LcuCredentials, LcuError> {
        if let Some(cached) = self.creds.lock().clone() {
            return Ok(cached);
        }

        let creds = Self::read_credentials().ok_or(LcuError::NotRunning)?;
        *self.creds.lock() = Some(creds.clone());
        Ok(creds)
    }

    fn invalidate(&self) {
        *self.creds.lock() = None;
    }

    async fn request(&self, path: &str) -> Result<Value, LcuError> {
        let mut last_error = None;

        // Lockfile yeniden başlatmalarda değişir. Bir kez kimlik bilgilerini
        // tazeleyip tekrar dene; 404 kalıcı bir cevap olduğu için tekrar denenmez.
        for attempt in 0..2 {
            let creds = self.credentials()?;
            let url = format!("https://127.0.0.1:{}{}", creds.port, path);

            let response = self
                .client
                .get(&url)
                .basic_auth("riot", Some(creds.password.as_str()))
                .send()
                .await;

            match response {
                Ok(res) => {
                    let status = res.status();
                    if status.is_success() {
                        return Ok(res.json::<Value>().await?);
                    }

                    if status == reqwest::StatusCode::NOT_FOUND {
                        return Err(LcuError::Status(404));
                    }

                    self.invalidate();
                    last_error = Some(LcuError::Status(status.as_u16()));
                }
                Err(error) => {
                    self.invalidate();
                    last_error = Some(LcuError::Http(error));
                }
            }

            if attempt == 0 {
                tokio::time::sleep(Duration::from_millis(150)).await;
            }
        }

        Err(last_error.unwrap_or(LcuError::NotRunning))
    }

    async fn write(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<Value>,
    ) -> Result<(), LcuError> {
        let creds = self.credentials()?;
        let url = format!("https://127.0.0.1:{}{}", creds.port, path);
        let mut request = self
            .client
            .request(method, &url)
            .basic_auth("riot", Some(creds.password.as_str()));
        if let Some(body) = body {
            request = request.json(&body);
        }
        let response = request.send().await?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(LcuError::Status(response.status().as_u16()))
        }
    }

    async fn post(&self, path: &str, body: Option<Value>) -> Result<(), LcuError> {
        self.write(reqwest::Method::POST, path, body).await
    }

    async fn patch(&self, path: &str, body: Option<Value>) -> Result<(), LcuError> {
        self.write(reqwest::Method::PATCH, path, body).await
    }

    pub async fn accept_ready_check(&self) -> Result<(), LcuError> {
        self.post("/lol-matchmaking/v1/ready-check/accept", None)
            .await
    }

    pub async fn complete_champ_action(
        &self,
        action_id: i64,
        champion_id: i64,
    ) -> Result<(), LcuError> {
        // LCU sözleşmesi: aksiyon güncellemesi PATCH ile yapılır.
        self.patch(
            &format!("/lol-champ-select/v1/session/actions/{}", action_id),
            Some(serde_json::json!({"championId": champion_id, "completed": true})),
        )
        .await
    }

    pub async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, LcuError> {
        let value = self.request(path).await?;
        Ok(serde_json::from_value(value)?)
    }

    pub async fn get_optional<T: DeserializeOwned>(
        &self,
        path: &str,
    ) -> Result<Option<T>, LcuError> {
        match self.get::<T>(path).await {
            Ok(value) => Ok(Some(value)),
            Err(LcuError::Status(404)) => Ok(None),
            Err(error) => Err(error),
        }
    }

    pub async fn phase(&self) -> Result<String, LcuError> {
        self.get("/lol-gameflow/v1/gameflow-phase").await
    }

    /// Aktif oyun oturumu. Şampiyon seçimi sonrası takım/şampiyon listesini taşır.
    pub async fn gameflow_session(&self) -> Result<Option<Value>, LcuError> {
        self.get_optional("/lol-gameflow/v1/session").await
    }

    pub async fn current_summoner(&self) -> Result<Value, LcuError> {
        self.get("/lol-summoner/v1/current-summoner").await
    }

    pub async fn summoner_by_id(&self, summoner_id: i64) -> Result<Value, LcuError> {
        self.get(&format!("/lol-summoner/v1/summoners/{}", summoner_id))
            .await
    }

    pub async fn champ_select(&self) -> Result<Option<Value>, LcuError> {
        self.get_optional("/lol-champ-select/v1/session").await
    }

    /// Dereceli istatistikler. Riot API anahtarı gerekmez; istemci yerel olarak yanıtlar.
    pub async fn ranked_stats(&self, puuid: &str) -> Result<Value, LcuError> {
        self.get(&format!("/lol-ranked/v1/ranked-stats/{}", puuid))
            .await
    }

    /// Son maçlar. `end` üst sınırı istemci tarafından dahil edilir.
    pub async fn match_history(
        &self,
        puuid: &str,
        begin: u32,
        end: u32,
    ) -> Result<Value, LcuError> {
        self.get(&format!(
            "/lol-match-history/v1/products/lol/{}/matches?begIndex={}&endIndex={}",
            puuid, begin, end
        ))
        .await
    }

    /// Tek maçın tam katılımcı listesi (rol/hat bilgisi burada dolu gelir).
    pub async fn game_detail(&self, game_id: i64) -> Result<Value, LcuError> {
        self.get(&format!("/lol-match-history/v1/games/{}", game_id))
            .await
    }

    /// Kuyruk listesi (yerelleştirilmiş adlar). Açılışta bir kez okunur.
    pub async fn queues(&self) -> Result<Value, LcuError> {
        self.get("/lol-game-queues/v1/queues").await
    }

    /// Aktif lobi (varsa). Lobi yoksa 404 → None.
    pub async fn lobby(&self) -> Result<Option<Value>, LcuError> {
        self.get_optional("/lol-lobby/v2/lobby").await
    }

    /// Şampiyon ustalık puanları (yerel oyuncunun tüm şampiyonları).
    pub async fn champion_mastery(&self, puuid: &str) -> Result<Value, LcuError> {
        self.get(&format!(
            "/lol-champion-mastery/v1/{}/champion-mastery",
            puuid
        ))
        .await
    }

    pub async fn status(&self) -> LcuStatus {
        match self.phase().await {
            Ok(phase) => {
                // İsim ve bölge bilgisi birbirinden bağımsızdır; seri bekleme
                // durum yenilemesini gereksiz yere iki istek süresi uzatır.
                let (summoner, region) = tokio::join!(
                    self.current_summoner(),
                    self.get::<Value>("/riotclient/region-locale"),
                );
                let summoner = summoner.ok();

                let summoner_name = summoner.as_ref().and_then(|value| {
                    let game_name = value.get("gameName").and_then(|v| v.as_str());
                    let tag_line = value.get("tagLine").and_then(|v| v.as_str()).unwrap_or("");

                    game_name
                        .map(|name| format!("{}#{}", name, tag_line))
                        .or_else(|| {
                            value
                                .get("displayName")
                                .and_then(|v| v.as_str())
                                .map(String::from)
                        })
                });

                let platform = region.ok().and_then(|value| {
                    value
                        .get("region")
                        .and_then(|v| v.as_str())
                        .map(String::from)
                });

                LcuStatus {
                    connected: true,
                    phase: Some(phase),
                    summoner_name,
                    platform,
                    error: None,
                }
            }
            Err(error) => LcuStatus {
                connected: false,
                phase: None,
                summoner_name: None,
                platform: None,
                error: Some(error.to_string()),
            },
        }
    }
}
