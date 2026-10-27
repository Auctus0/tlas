use std::collections::HashMap;

use futures::future::join_all;
use futures::stream::{self, StreamExt};
use serde_json::Value;
use tauri::State;

use crate::config::Settings;
use crate::models::{ChampSelectView, LcuStatus, MatchSummary, PlayerCard, ScoutCache};
use crate::riot::Riot;
use crate::state::AppState;

/// Scout verisi 5 dakika taze kalır; champ select sırasında gereksiz Riot çağrısı yapılmaz.
const SCOUT_TTL_SECS: i64 = 300;
/// Maç detayları değişmez; uzun süre cache'lenir.
const MATCH_TTL_SECS: i64 = 30 * 24 * 3_600;
/// Maç ID listesi kısa süre cache'lenir; yeni maçlar için yeterli.
const MATCH_IDS_TTL_SECS: i64 = 90;
const SCOUT_MATCH_COUNT: u32 = 5;
/// Riot API'ye aynı anda gönderilen maç isteği sayısı.
const MATCH_CONCURRENCY: usize = 6;

#[tauri::command]
pub async fn lcu_status(state: State<'_, AppState>) -> Result<LcuStatus, String> {
    Ok(state.lcu.status().await)
}

#[tauri::command]
pub async fn champ_select(state: State<'_, AppState>) -> Result<Option<ChampSelectView>, String> {
    let app = state.inner();
    let session = app.lcu.champ_select().await.map_err(|error| error.to_string())?;

    let Some(session) = session else {
        return Ok(None);
    };

    let phase = app
        .lcu
        .phase()
        .await
        .unwrap_or_else(|_| "ChampSelect".to_string());

    let timer = session
        .pointer("/timer/adjustedTimeLeftInPhase")
        .and_then(|value| value.as_i64());

    let my_team = session
        .get("myTeam")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();

    let local_cell_id = session
        .get("localPlayerCellId")
        .and_then(|value| value.as_i64());

    let my_champion_id = my_team
        .iter()
        .find(|player| {
            player
                .get("isActingPlayer")
                .and_then(|value| value.as_bool())
                .unwrap_or(false)
        })
        .or_else(|| {
            local_cell_id.and_then(|cell_id| {
                my_team.iter().find(|player| {
                    player.get("cellId").and_then(|value| value.as_i64()) == Some(cell_id)
                })
            })
        })
        .and_then(|player| player.get("championId"))
        .and_then(|value| value.as_i64());

    let settings = app.config.get();
    let (allies, enemies) = tokio::join!(
        build_team(app, &session, "myTeam", true, &settings),
        build_team(app, &session, "theirTeam", false, &settings),
    );

    Ok(Some(ChampSelectView {
        phase,
        allies,
        enemies,
        my_champion_id,
        timer,
    }))
}

async fn build_team(
    app: &AppState,
    session: &Value,
    team_key: &str,
    is_ally: bool,
    settings: &Settings,
) -> Vec<PlayerCard> {
    let Some(team) = session.get(team_key).and_then(|value| value.as_array()) else {
        return Vec::new();
    };

    // Takım arkadaşlarının görünen adlarını tek seferde paralel çöz.
    // Rakip isimleri LCU'da gizliyse asla çözülmez.
    let resolved_names: Vec<Option<Value>> = if is_ally {
        join_all(team.iter().map(|player| {
            let summoner_id = player
                .get("summonerId")
                .and_then(|value| value.as_i64())
                .filter(|value| *value > 0);

            async move {
                match summoner_id {
                    Some(summoner_id) => app.lcu.summoner_by_id(summoner_id).await.ok(),
                    None => None,
                }
            }
        }))
        .await
    } else {
        vec![None; team.len()]
    };

    let cards: Vec<PlayerCard> = team
        .iter()
        .zip(resolved_names)
        .map(|(player, summoner)| base_card(player, summoner, is_ally))
        .collect();

    if settings.riot_api_key.trim().is_empty() {
        return cards;
    }

    // Riot çağrılarını kart başına paralel yürüt; sıra korunur.
    join_all(cards.into_iter().map(|mut card| {
        let settings = settings.clone();

        async move {
            if !card.hidden {
                enrich_card(app, &settings, &mut card).await;
            }
            card
        }
    }))
    .await
}

fn base_card(player: &Value, summoner: Option<Value>, is_ally: bool) -> PlayerCard {
    let summoner_id = player
        .get("summonerId")
        .and_then(|value| value.as_i64())
        .filter(|value| *value > 0);

    let mut puuid = player
        .get("puuid")
        .and_then(|value| value.as_str())
        .filter(|value| !value.is_empty())
        .map(String::from);

    let mut display_name = player
        .get("summonerName")
        .and_then(|value| value.as_str())
        .filter(|value| !value.is_empty())
        .map(String::from);

    if is_ally {
        if let Some(summoner) = summoner {
            if let Some(game_name) = summoner.get("gameName").and_then(|value| value.as_str()) {
                let tag_line = summoner
                    .get("tagLine")
                    .and_then(|value| value.as_str())
                    .unwrap_or("");
                display_name = Some(format!("{}#{}", game_name, tag_line));
            } else if let Some(name) = summoner.get("displayName").and_then(|value| value.as_str()) {
                display_name = Some(name.to_string());
            }

            if puuid.is_none() {
                puuid = summoner
                    .get("puuid")
                    .and_then(|value| value.as_str())
                    .map(String::from);
            }
        }
    }

    let hidden = display_name.is_none();

    PlayerCard {
        puuid,
        summoner_id,
        display_name: display_name.unwrap_or_else(|| "Gizli".to_string()),
        champion_id: player
            .get("championId")
            .and_then(|value| value.as_i64())
            .unwrap_or(0),
        champion_name: None,
        assigned_position: player
            .get("assignedPosition")
            .and_then(|value| value.as_str())
            .filter(|value| !value.is_empty())
            .map(String::from),
        rank: None,
        tier: None,
        division: None,
        lp: None,
        wins: None,
        losses: None,
        recent_winrate: None,
        recent_games: None,
        is_ally,
        hidden,
    }
}

async fn enrich_card(app: &AppState, settings: &Settings, card: &mut PlayerCard) {
    let Some(puuid) = card.puuid.clone() else {
        return;
    };

    let cache_key = format!("scout:{}", puuid);
    if let Some(cached) = app.db.kv_get(&cache_key, SCOUT_TTL_SECS) {
        if let Ok(scout) = serde_json::from_str::<ScoutCache>(&cached) {
            apply_scout(card, &scout);
            return;
        }
    }

    let mut scout = ScoutCache::default();

    if let Ok(entries) = app
        .riot
        .league_by_puuid(&settings.riot_api_key, &settings.platform, &puuid)
        .await
    {
        if let Some(entry) = pick_solo_entry(&entries) {
            scout.tier = entry.get("tier").and_then(|value| value.as_str()).map(String::from);
            scout.division = entry.get("rank").and_then(|value| value.as_str()).map(String::from);
            scout.lp = entry.get("leaguePoints").and_then(|value| value.as_i64());
            scout.wins = entry.get("wins").and_then(|value| value.as_i64());
            scout.losses = entry.get("losses").and_then(|value| value.as_i64());
        }
    }

    let regional = Riot::regional(&settings.platform);

    if let Some(match_ids) = match_ids(
        app,
        &settings.riot_api_key,
        regional,
        &puuid,
        SCOUT_MATCH_COUNT,
    )
    .await
    {
        let matches = load_matches(app, &settings.riot_api_key, regional, &match_ids).await;
        let mut wins = 0i64;
        let mut games = 0i64;

        for (_, match_json) in &matches {
            if let Some(participant) = participant_for(match_json, &puuid) {
                games += 1;
                if participant
                    .get("win")
                    .and_then(|value| value.as_bool())
                    .unwrap_or(false)
                {
                    wins += 1;
                }
            }
        }

        if games > 0 {
            scout.recent_wins = Some(wins);
            scout.recent_games = Some(games);
        }
    }

    if let Ok(serialized) = serde_json::to_string(&scout) {
        let _ = app.db.kv_set(&cache_key, &serialized);
    }

    apply_scout(card, &scout);
}

async fn match_ids(
    app: &AppState,
    key: &str,
    regional: &str,
    puuid: &str,
    limit: u32,
) -> Option<Vec<String>> {
    let cache_key = format!("matchids:{}:{}:{}", regional, puuid, limit);

    if let Some(cached) = app.db.kv_get(&cache_key, MATCH_IDS_TTL_SECS) {
        if let Ok(ids) = serde_json::from_str::<Vec<String>>(&cached) {
            return Some(ids);
        }
    }

    let ids = app.riot.match_ids(key, regional, puuid, limit).await.ok()?;
    if let Ok(serialized) = serde_json::to_string(&ids) {
        let _ = app.db.kv_set(&cache_key, &serialized);
    }

    Some(ids)
}

async fn load_match(
    app: &AppState,
    key: &str,
    regional: &str,
    match_id: &str,
) -> Option<Value> {
    if let Some(cached) = app.db.match_get(match_id, MATCH_TTL_SECS) {
        if let Ok(value) = serde_json::from_str::<Value>(&cached) {
            return Some(value);
        }
    }

    let value = app.riot.match_by_id(key, regional, match_id).await.ok()?;
    if let Ok(serialized) = serde_json::to_string(&value) {
        let _ = app.db.match_set(match_id, &serialized);
    }

    Some(value)
}

/// Maçları sınırlı eşzamanlılıkla indirir, giriş sırasını korur.
async fn load_matches(
    app: &AppState,
    key: &str,
    regional: &str,
    ids: &[String],
) -> Vec<(String, Value)> {
    let fetched: Vec<(String, Value)> = stream::iter(ids.iter().cloned())
        .map(|match_id| async move {
            load_match(app, key, regional, &match_id)
                .await
                .map(|value| (match_id, value))
        })
        .buffer_unordered(MATCH_CONCURRENCY)
        .filter_map(|item| async move { item })
        .collect()
        .await;

    let mut by_id: HashMap<String, Value> = fetched.into_iter().collect();
    ids.iter()
        .filter_map(|match_id| by_id.remove(match_id).map(|value| (match_id.clone(), value)))
        .collect()
}

fn pick_solo_entry(entries: &Value) -> Option<&Value> {
    let array = entries.as_array()?;

    array
        .iter()
        .find(|entry| entry.get("queueType").and_then(|value| value.as_str()) == Some("RANKED_SOLO_5x5"))
        .or_else(|| {
            array.iter().find(|entry| {
                entry.get("queueType").and_then(|value| value.as_str()) == Some("RANKED_FLEX_SR")
            })
        })
        .or_else(|| array.first())
}

fn apply_scout(card: &mut PlayerCard, scout: &ScoutCache) {
    card.tier = scout.tier.clone();
    card.division = scout.division.clone();
    card.lp = scout.lp;
    card.wins = scout.wins;
    card.losses = scout.losses;
    card.recent_games = scout.recent_games;

    card.recent_winrate = match (scout.recent_wins, scout.recent_games) {
        (Some(wins), Some(games)) if games > 0 => Some(wins as f64 / games as f64 * 100.0),
        _ => None,
    };

    card.rank = match (&scout.tier, &scout.division) {
        (Some(tier), Some(division))
            if tier != "MASTER" && tier != "GRANDMASTER" && tier != "CHALLENGER" =>
        {
            Some(format!(
                "{} {} · {} LP",
                tier_name(tier),
                title_case(division),
                scout.lp.unwrap_or(0)
            ))
        }
        (Some(tier), _) => Some(format!(
            "{} · {} LP",
            tier_name(tier),
            scout.lp.unwrap_or(0)
        )),
        _ => Some("Derecesiz".to_string()),
    };
}

fn tier_name(tier: &str) -> &str {
    match tier {
        "IRON" => "Demir",
        "BRONZE" => "Bronz",
        "SILVER" => "Gümüş",
        "GOLD" => "Altın",
        "PLATINUM" => "Platin",
        "EMERALD" => "Zümrüt",
        "DIAMOND" => "Elmas",
        "MASTER" => "Master",
        "GRANDMASTER" => "Grandmaster",
        "CHALLENGER" => "Challenger",
        other => other,
    }
}

fn title_case(value: &str) -> String {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase(),
        None => String::new(),
    }
}

fn participant_for<'a>(match_json: &'a Value, puuid: &str) -> Option<&'a Value> {
    match_json
        .pointer("/info/participants")
        .and_then(|value| value.as_array())
        .and_then(|participants| {
            participants.iter().find(|participant| {
                participant.get("puuid").and_then(|value| value.as_str()) == Some(puuid)
            })
        })
}

#[tauri::command]
pub async fn recent_matches(
    state: State<'_, AppState>,
    count: Option<u32>,
) -> Result<Vec<MatchSummary>, String> {
    let app = state.inner();
    let settings = app.config.get();

    if settings.riot_api_key.trim().is_empty() {
        return Err(
            "Riot API anahtarı ayarlanmamış. Ayarlar sekmesinden ekleyebilirsin.".to_string(),
        );
    }

    let summoner = app
        .lcu
        .current_summoner()
        .await
        .map_err(|error| error.to_string())?;

    let puuid = summoner
        .get("puuid")
        .and_then(|value| value.as_str())
        .ok_or_else(|| "PUUID bulunamadı. League istemcisi açık mı?".to_string())?
        .to_string();

    let regional = Riot::regional(&settings.platform);
    let limit = count.unwrap_or(20).clamp(1, 50);

    let ids = match_ids(app, &settings.riot_api_key, regional, &puuid, limit)
        .await
        .ok_or_else(|| "Maç listesi alınamadı. API anahtarını kontrol et.".to_string())?;

    if ids.is_empty() {
        return Ok(Vec::new());
    }

    let matches = load_matches(app, &settings.riot_api_key, regional, &ids).await;

    Ok(matches
        .iter()
        .filter_map(|(match_id, match_json)| parse_match_summary(match_id, match_json, &puuid))
        .collect())
}

fn parse_match_summary(match_id: &str, match_json: &Value, puuid: &str) -> Option<MatchSummary> {
    let info = match_json.get("info")?;
    let participant = participant_for(match_json, puuid)?;

    let cs = participant
        .get("totalMinionsKilled")
        .and_then(|value| value.as_i64())
        .unwrap_or(0)
        + participant
            .get("neutralMinionsKilled")
            .and_then(|value| value.as_i64())
            .unwrap_or(0);

    Some(MatchSummary {
        match_id: match_id.to_string(),
        champion_name: participant
            .get("championName")
            .and_then(|value| value.as_str())
            .unwrap_or("Unknown")
            .to_string(),
        champion_id: participant
            .get("championId")
            .and_then(|value| value.as_i64())
            .unwrap_or(0),
        win: participant
            .get("win")
            .and_then(|value| value.as_bool())
            .unwrap_or(false),
        kills: participant
            .get("kills")
            .and_then(|value| value.as_i64())
            .unwrap_or(0),
        deaths: participant
            .get("deaths")
            .and_then(|value| value.as_i64())
            .unwrap_or(0),
        assists: participant
            .get("assists")
            .and_then(|value| value.as_i64())
            .unwrap_or(0),
        cs,
        duration_secs: info
            .get("gameDuration")
            .and_then(|value| value.as_i64())
            .unwrap_or(0),
        queue_id: info
            .get("queueId")
            .and_then(|value| value.as_i64())
            .unwrap_or(0),
        game_creation: info
            .get("gameCreation")
            .and_then(|value| value.as_i64())
            .unwrap_or(0),
        role: participant
            .get("teamPosition")
            .and_then(|value| value.as_str())
            .filter(|value| !value.is_empty())
            .map(String::from),
    })
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Settings {
    state.config.get()
}

#[tauri::command]
pub fn save_settings(state: State<'_, AppState>, settings: Settings) -> Result<(), String> {
    state.config.set(settings).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn clear_cache(state: State<'_, AppState>) -> Result<(), String> {
    state.db.clear().map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn prefers_solo_queue_entry() {
        let entries = json!([
            { "queueType": "RANKED_FLEX_SR", "tier": "GOLD" },
            { "queueType": "RANKED_SOLO_5x5", "tier": "PLATINUM" }
        ]);

        assert_eq!(
            pick_solo_entry(&entries)
                .and_then(|entry| entry.get("tier"))
                .and_then(|value| value.as_str()),
            Some("PLATINUM")
        );
    }

    #[test]
    fn falls_back_to_first_entry() {
        let entries = json!([{ "queueType": "RANKED_TFT", "tier": "DIAMOND" }]);

        assert_eq!(
            pick_solo_entry(&entries)
                .and_then(|entry| entry.get("tier"))
                .and_then(|value| value.as_str()),
            Some("DIAMOND")
        );
    }

    #[test]
    fn parses_match_summary_for_player() {
        let match_json = json!({
            "info": {
                "gameDuration": 1_800,
                "queueId": 420,
                "gameCreation": 1_700_000_000_000_i64,
                "participants": [{
                    "puuid": "p1",
                    "championName": "Ahri",
                    "championId": 103,
                    "win": true,
                    "kills": 7,
                    "deaths": 2,
                    "assists": 9,
                    "totalMinionsKilled": 180,
                    "neutralMinionsKilled": 12,
                    "teamPosition": "MIDDLE"
                }]
            }
        });

        let summary = parse_match_summary("TR1_1", &match_json, "p1").expect("summary");

        assert_eq!(summary.champion_name, "Ahri");
        assert_eq!(summary.cs, 192);
        assert!(summary.win);
        assert_eq!(summary.role.as_deref(), Some("MIDDLE"));
    }

    #[test]
    fn formats_rank_titles() {
        assert_eq!(title_case("GRANDMASTER"), "Grandmaster");
        assert_eq!(title_case(""), "");
        assert_eq!(tier_name("PLATINUM"), "Platin");
        assert_eq!(tier_name("EMERALD"), "Zümrüt");
        assert_eq!(tier_name("UNKNOWN"), "UNKNOWN");
    }
}
