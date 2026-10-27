use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LcuStatus {
    pub connected: bool,
    pub phase: Option<String>,
    pub summoner_name: Option<String>,
    pub platform: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerCard {
    pub puuid: Option<String>,
    pub summoner_id: Option<i64>,
    pub display_name: String,
    pub champion_id: i64,
    pub champion_name: Option<String>,
    pub assigned_position: Option<String>,
    pub rank: Option<String>,
    pub tier: Option<String>,
    pub division: Option<String>,
    pub lp: Option<i64>,
    pub wins: Option<i64>,
    pub losses: Option<i64>,
    pub recent_winrate: Option<f64>,
    pub recent_games: Option<i64>,
    pub is_ally: bool,
    pub hidden: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChampSelectView {
    pub phase: String,
    pub allies: Vec<PlayerCard>,
    pub enemies: Vec<PlayerCard>,
    pub my_champion_id: Option<i64>,
    pub timer: Option<i64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchSummary {
    pub match_id: String,
    pub champion_name: String,
    pub champion_id: i64,
    pub win: bool,
    pub kills: i64,
    pub deaths: i64,
    pub assists: i64,
    pub cs: i64,
    pub duration_secs: i64,
    pub queue_id: i64,
    pub game_creation: i64,
    pub role: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScoutCache {
    pub tier: Option<String>,
    pub division: Option<String>,
    pub lp: Option<i64>,
    pub wins: Option<i64>,
    pub losses: Option<i64>,
    pub recent_wins: Option<i64>,
    pub recent_games: Option<i64>,
}
