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
    pub queue_name: Option<String>,
    pub game_creation: i64,
    pub role: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SummonerInfo {
    pub name: String,
    pub level: Option<i64>,
    pub icon_id: Option<i64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RankView {
    pub queue: String,
    pub label: String,
    pub tier: Option<String>,
    pub division: Option<String>,
    pub lp: Option<i64>,
    pub wins: i64,
    pub losses: i64,
    pub winrate: Option<f64>,
    pub provisional: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LpPoint {
    pub ts: i64,
    pub tier: String,
    pub division: Option<String>,
    pub lp: i64,
    pub delta: Option<i64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChampionStat {
    pub champion_id: i64,
    pub games: i64,
    pub wins: i64,
    pub winrate: f64,
    pub kda: f64,
    pub points: Option<i64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSummary {
    pub games: i64,
    pub wins: i64,
    pub losses: i64,
    pub winrate: Option<f64>,
    pub avg_kda: Option<f64>,
    pub lp_delta: Option<i64>,
    pub streak: i64,
    pub since: Option<i64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountOverview {
    pub summoner: SummonerInfo,
    pub ranks: Vec<RankView>,
    pub lp_history: Vec<LpPoint>,
    pub champions: Vec<ChampionStat>,
    pub recent_games: i64,
    pub recent_wins: i64,
    pub recent_winrate: Option<f64>,
    pub recent_form: Vec<bool>,
    pub streak: i64,
    pub session: SessionSummary,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchPlayer {
    pub puuid: Option<String>,
    pub display_name: String,
    pub champion_id: i64,
    pub team_id: i64,
    pub position: Option<String>,
    pub kills: i64,
    pub deaths: i64,
    pub assists: i64,
    pub cs: i64,
    pub gold: i64,
    pub damage: i64,
    pub vision: i64,
    pub items: Vec<i64>,
    pub is_me: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchTeam {
    pub team_id: i64,
    pub win: bool,
    pub kills: i64,
    pub players: Vec<MatchPlayer>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchDetail {
    pub match_id: String,
    pub queue_id: i64,
    pub queue_name: Option<String>,
    pub duration_secs: i64,
    pub game_creation: i64,
    pub teams: Vec<MatchTeam>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildItem {
    pub item_id: i64,
    pub count: i64,
    pub winrate: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonalBuild {
    pub champion_id: i64,
    pub games: i64,
    pub wins: i64,
    pub winrate: f64,
    pub kda: f64,
    pub avg_cs: f64,
    pub position: Option<String>,
    pub core_items: Vec<BuildItem>,
    pub boots: Vec<BuildItem>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LivePlayer {
    pub display_name: String,
    pub champion_id: i64,
    pub position: Option<String>,
    pub is_ally: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveGameView {
    pub game_id: i64,
    pub queue_id: i64,
    pub queue_name: Option<String>,
    pub allies: Vec<LivePlayer>,
    pub enemies: Vec<LivePlayer>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetaReport {
    pub champion: String,
    pub position: String,
    pub tier: Option<i64>,
    pub winrate: Option<f64>,
    pub pickrate: Option<f64>,
    pub banrate: Option<f64>,
    pub core_items: Vec<String>,
    pub rune: Option<String>,
    pub cached: bool,
    pub fetched_at: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LobbyMember {
    pub name: String,
    pub level: Option<i64>,
    pub icon_id: Option<i64>,
    pub tier: Option<String>,
    pub division: Option<String>,
    pub lp: Option<i64>,
    pub wins: Option<i64>,
    pub losses: Option<i64>,
    pub winrate: Option<f64>,
    pub position: Option<String>,
    pub is_self: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LobbyView {
    pub queue_id: i64,
    pub queue_name: Option<String>,
    pub members: Vec<LobbyMember>,
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
