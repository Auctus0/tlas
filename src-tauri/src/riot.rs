use std::collections::VecDeque;
use std::time::{Duration, Instant};

use serde::de::DeserializeOwned;
use serde_json::Value;
use tokio::sync::Mutex;

#[derive(Debug, thiserror::Error)]
pub enum RiotError {
    #[error("Riot API anahtarı ayarlanmamış")]
    MissingKey,
    #[error("Riot API isteği başarısız: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Riot API yanıtı okunamadı: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Riot API {0}: {1}")]
    Status(u16, String),
}

pub struct Riot {
    client: reqwest::Client,
    hits: Mutex<VecDeque<Instant>>,
}

impl Riot {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(12))
            .gzip(true)
            .build()
            .expect("Riot HTTP client oluşturulamadı");

        Self {
            client,
            hits: Mutex::new(VecDeque::with_capacity(128)),
        }
    }

    pub fn regional(platform: &str) -> &'static str {
        match platform.to_ascii_lowercase().as_str() {
            "na1" | "br1" | "la1" | "la2" => "americas",
            "kr" | "jp1" => "asia",
            "oc1" | "ph2" | "sg2" | "th2" | "tw2" | "vn2" => "sea",
            _ => "europe",
        }
    }

    /// Development/personal key için güvenli sınır: 18 istek/sn, 90 istek/2dk.
    /// Production key'e geçince bu limitleri header'lardan okuyup yükselt.
    async fn acquire(&self) {
        loop {
            let mut hits = self.hits.lock().await;
            let now = Instant::now();

            while hits
                .front()
                .is_some_and(|instant| now.duration_since(*instant) > Duration::from_secs(120))
            {
                hits.pop_front();
            }

            let last_second = hits
                .iter()
                .filter(|instant| now.duration_since(**instant) < Duration::from_secs(1))
                .count();

            if hits.len() < 90 && last_second < 18 {
                hits.push_back(now);
                return;
            }

            drop(hits);
            tokio::time::sleep(Duration::from_millis(120)).await;
        }
    }

    async fn get<T: DeserializeOwned>(&self, key: &str, url: &str) -> Result<T, RiotError> {
        if key.trim().is_empty() {
            return Err(RiotError::MissingKey);
        }

        for attempt in 0..2u8 {
            self.acquire().await;

            let response = self
                .client
                .get(url)
                .header("X-Riot-Token", key)
                .send()
                .await?;

            let status = response.status();

            if status.is_success() {
                return Ok(response.json::<T>().await?);
            }

            if status == reqwest::StatusCode::TOO_MANY_REQUESTS && attempt == 0 {
                let retry_after = response
                    .headers()
                    .get("retry-after")
                    .and_then(|value| value.to_str().ok())
                    .and_then(|value| value.parse::<u64>().ok())
                    .unwrap_or(2);

                tokio::time::sleep(Duration::from_secs(retry_after.min(30))).await;
                continue;
            }

            let body = response.text().await.unwrap_or_default();
            return Err(RiotError::Status(status.as_u16(), body));
        }

        Err(RiotError::Status(429, "rate limit".to_string()))
    }

    pub async fn league_by_puuid(
        &self,
        key: &str,
        platform: &str,
        puuid: &str,
    ) -> Result<Value, RiotError> {
        let url = format!(
            "https://{}.api.riotgames.com/lol/league/v4/entries/by-puuid/{}",
            platform, puuid
        );

        self.get(key, &url).await
    }

    pub async fn match_ids(
        &self,
        key: &str,
        regional: &str,
        puuid: &str,
        count: u32,
    ) -> Result<Vec<String>, RiotError> {
        let url = format!(
            "https://{}.api.riotgames.com/lol/match/v5/matches/by-puuid/{}/ids?start=0&count={}",
            regional, puuid, count
        );

        self.get(key, &url).await
    }

    pub async fn match_by_id(
        &self,
        key: &str,
        regional: &str,
        match_id: &str,
    ) -> Result<Value, RiotError> {
        let url = format!(
            "https://{}.api.riotgames.com/lol/match/v5/matches/{}",
            regional, match_id
        );

        self.get(key, &url).await
    }
}
