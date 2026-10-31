use std::collections::HashMap;

use futures::future::join_all;
use futures::stream::StreamExt;
use serde_json::Value;
use tauri::State;

use crate::config::Settings;
use crate::db::LpSnapshot;
use crate::models::{
    AccountOverview, BuildItem, ChampSelectView, ChampionStat, DraftInsight, LcuStatus,
    LiveGameView, LivePlayer, LobbyMember, LobbyView, LpPoint, MatchDetail, MatchPlayer,
    MatchSummary, MatchTeam, MetaReport, PersonalBuild, PlayerCard, RankView, ScoutCache,
    SessionSummary, SummonerInfo,
};
use crate::state::AppState;

/// Scout verisi 10 dakika taze kalır; derece ve son maçlar bu sürede değişmez.
const SCOUT_TTL_SECS: i64 = 600;
/// Kart başına son maç penceresi.
const SCOUT_MATCH_COUNT: u32 = 20;
/// Hesap özeti ve maç detayı aynı geçmişi paylaşır; 60 saniye taze kalır.
const HISTORY_CACHE_SECS: i64 = 60;
/// Ustalık puanları yavaş değişir; bir saat cache yeterli.
const MASTERY_CACHE_SECS: i64 = 3_600;
/// Geçmiş sorgusu her zaman aynı genişlikte yapılır; farklı komutlar paylaşır.
const HISTORY_COUNT: u32 = 50;
const GAME_DETAIL_CACHE_SECS: i64 = 7 * 86_400;

const POSITION_ORDER: [&str; 5] = ["TOP", "JUNGLE", "MIDDLE", "BOTTOM", "UTILITY"];
const BOOTS: [i64; 12] = [
    3006, 3009, 3010, 3020, 3047, 3111, 3117, 3158, 3168, 3172, 3173, 3174,
];
const TRINKETS: [i64; 4] = [3340, 3350, 3363, 3364];

#[tauri::command]
pub async fn lcu_status(state: State<'_, AppState>) -> Result<LcuStatus, String> {
    Ok(state.lcu.status().await)
}

#[tauri::command]
pub async fn champ_select(state: State<'_, AppState>) -> Result<Option<ChampSelectView>, String> {
    let app = state.inner();
    let session = app
        .lcu
        .champ_select()
        .await
        .map_err(|error| error.to_string())?;

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

    let local_player = my_team
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
        });

    let my_champion_id = local_player
        .and_then(|player| player.get("championId"))
        .and_then(|value| value.as_i64());
    let my_position = local_player
        .and_then(|player| player.get("assignedPosition"))
        .and_then(|value| value.as_str())
        .filter(|position| POSITION_ORDER.contains(position))
        .map(String::from);

    let (allies, enemies) = tokio::join!(
        build_team(app, &session, "myTeam", true),
        build_team(app, &session, "theirTeam", false),
    );

    Ok(Some(ChampSelectView {
        phase,
        allies,
        enemies,
        my_champion_id,
        my_position,
        timer,
    }))
}

#[tauri::command]
pub async fn draft_insights(state: State<'_, AppState>) -> Result<Vec<DraftInsight>, String> {
    let app = state.inner();
    let puuid = current_puuid(app).await?;
    let history = cached_history(app, &puuid)
        .await
        .ok_or_else(|| "Maç geçmişi alınamadı. LoL istemcisi açık olmalı.".to_string())?;

    let games: Vec<Value> = history_games(&history)
        .iter()
        .take(HISTORY_COUNT as usize)
        .filter(|game| {
            game.get("queueId")
                .and_then(Value::as_i64)
                .is_some_and(is_summoners_rift)
        })
        .cloned()
        .collect();

    // Özet geçmişinde rol bulunmayabilir. Tam maç detaylarını cache'leyerek
    // rol istatistiklerini sonraki taslaklarda ek LCU yükü oluşturmadan kullan.
    let detailed_games: Vec<Value> = futures::stream::iter(games)
        .map(|summary| {
            let app = app;
            let puuid = puuid.clone();
            async move {
                let has_position = lcu_participant(&summary, &puuid)
                    .and_then(|participant| role_from_timeline(participant.get("timeline")))
                    .is_some();
                if has_position {
                    return summary;
                }

                let game_id = summary.get("gameId").and_then(Value::as_i64);
                match game_id {
                    Some(game_id) => cached_game_detail(app, game_id).await.unwrap_or(summary),
                    None => summary,
                }
            }
        })
        .buffered(6)
        .collect()
        .await;

    Ok(draft_insights_from_games(&detailed_games, &puuid))
}

async fn build_team(
    app: &AppState,
    session: &Value,
    team_key: &str,
    is_ally: bool,
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

    // Kart zenginleştirmesini paralel yürüt; sıra korunur.
    join_all(cards.into_iter().map(|mut card| async move {
        if !card.hidden {
            enrich_card(app, &mut card).await;
        }
        card
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
            } else if let Some(name) = summoner.get("displayName").and_then(|value| value.as_str())
            {
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

async fn enrich_card(app: &AppState, card: &mut PlayerCard) {
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

    if let Ok(stats) = app.lcu.ranked_stats(&puuid).await {
        if let Some(entry) = pick_solo_entry(&stats) {
            scout.tier = entry
                .get("tier")
                .and_then(|value| value.as_str())
                .map(String::from);
            scout.division = entry
                .get("division")
                .and_then(|value| value.as_str())
                .filter(|value| !value.is_empty() && *value != "NA")
                .map(String::from);
            scout.lp = entry.get("leaguePoints").and_then(|value| value.as_i64());
            scout.wins = entry.get("wins").and_then(|value| value.as_i64());
            scout.losses = entry.get("losses").and_then(|value| value.as_i64());
        }
    }

    if let Ok(history) = app.lcu.match_history(&puuid, 0, SCOUT_MATCH_COUNT).await {
        let mut wins = 0i64;
        let mut games = 0i64;

        for game in history_games(&history) {
            let queue_id = game
                .get("queueId")
                .and_then(|value| value.as_i64())
                .unwrap_or(0);
            if !is_summoners_rift(queue_id) {
                continue;
            }

            if let Some(participant) = lcu_participant(game, &puuid) {
                games += 1;
                if participant
                    .pointer("/stats/win")
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

fn history_games(history: &Value) -> &[Value] {
    history
        .pointer("/games/games")
        .and_then(|value| value.as_array())
        .map(|value| value.as_slice())
        .unwrap_or(&[])
}

fn is_summoners_rift(queue_id: i64) -> bool {
    matches!(queue_id, 400 | 420 | 430 | 440 | 480 | 490 | 700)
}

async fn current_puuid(app: &AppState) -> Result<String, String> {
    let summoner = app
        .lcu
        .current_summoner()
        .await
        .map_err(|error| error.to_string())?;

    summoner
        .get("puuid")
        .and_then(|value| value.as_str())
        .map(String::from)
        .ok_or_else(|| "PUUID bulunamadı. LoL istemcisi açık mı?".to_string())
}

/// Maç geçmişini kısa süreliğine cache'ler. Hesap özeti 20 saniyede bir
/// yenilenirken 50 maçlık isteğin her seferinde tekrarlanmasını engeller.
async fn cached_history(app: &AppState, puuid: &str) -> Option<Value> {
    let key = format!("history:v2:{}", puuid);

    if let Some(cached) = app.db.kv_get(&key, HISTORY_CACHE_SECS) {
        if let Ok(value) = serde_json::from_str::<Value>(&cached) {
            return Some(value);
        }
    }

    let value = app.lcu.match_history(puuid, 0, HISTORY_COUNT).await.ok()?;
    if let Ok(serialized) = serde_json::to_string(&value) {
        let _ = app.db.kv_set(&key, &serialized);
    }

    Some(value)
}

async fn cached_game_detail(app: &AppState, game_id: i64) -> Option<Value> {
    let key = format!("match-detail:v1:{game_id}");
    if let Some(cached) = app.db.kv_get(&key, GAME_DETAIL_CACHE_SECS) {
        if let Ok(value) = serde_json::from_str::<Value>(&cached) {
            return Some(value);
        }
    }

    let value = app.lcu.game_detail(game_id).await.ok()?;
    if let Ok(serialized) = serde_json::to_string(&value) {
        let _ = app.db.kv_set(&key, &serialized);
    }
    Some(value)
}

async fn cached_mastery(app: &AppState, puuid: &str) -> HashMap<i64, i64> {
    let key = format!("mastery:v2:{}", puuid);

    if let Some(cached) = app.db.kv_get(&key, MASTERY_CACHE_SECS) {
        if let Ok(map) = serde_json::from_str::<HashMap<i64, i64>>(&cached) {
            return map;
        }
    }

    let map = mastery_points(app, puuid).await;
    if let Ok(serialized) = serde_json::to_string(&map) {
        let _ = app.db.kv_set(&key, &serialized);
    }

    map
}

fn start_of_day() -> i64 {
    use chrono::{Local, TimeZone};

    let now = Local::now();
    now.date_naive()
        .and_hms_opt(0, 0, 0)
        .and_then(|naive| Local.from_local_datetime(&naive).single())
        .map(|value| value.timestamp())
        .unwrap_or_else(|| now.timestamp() - 86_400)
}

fn pick_solo_entry(stats: &Value) -> Option<&Value> {
    queue_entry(
        stats,
        &["RANKED_SOLO_5x5", "JADE_RANKED_SOLO_5x5", "RANKED_FLEX_SR"],
    )
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

fn lcu_participant<'a>(game: &'a Value, puuid: &str) -> Option<&'a Value> {
    let participant_id = game
        .get("participantIdentities")?
        .as_array()?
        .iter()
        .find(|identity| {
            identity
                .pointer("/player/puuid")
                .and_then(|value| value.as_str())
                == Some(puuid)
        })?
        .get("participantId")?
        .as_i64()?;

    game.get("participants")?
        .as_array()?
        .iter()
        .find(|participant| {
            participant
                .get("participantId")
                .and_then(|value| value.as_i64())
                == Some(participant_id)
        })
}

#[tauri::command]
pub async fn recent_matches(
    state: State<'_, AppState>,
    count: Option<u32>,
) -> Result<Vec<MatchSummary>, String> {
    let app = state.inner();
    let summoner = app
        .lcu
        .current_summoner()
        .await
        .map_err(|error| error.to_string())?;

    let puuid = summoner
        .get("puuid")
        .and_then(|value| value.as_str())
        .ok_or_else(|| "PUUID bulunamadı. LoL istemcisi açık mı?".to_string())?
        .to_string();

    let limit = count.unwrap_or(20).clamp(1, 50);
    let history = cached_history(app, &puuid)
        .await
        .ok_or_else(|| "Maç geçmişi alınamadı. LoL istemcisi açık olmalı.".to_string())?;

    // Özet listede rol bilgisi yok; oyun detayından tamamla (en fazla 6 eşzamanlı).
    let mut summaries: Vec<MatchSummary> = history_games(&history)
        .iter()
        .take(limit as usize)
        .filter_map(|game| parse_match_summary(game, &puuid))
        .collect();

    let game_ids: Vec<i64> = summaries
        .iter()
        .filter(|summary| is_summoners_rift(summary.queue_id))
        .filter_map(|summary| summary.match_id.parse::<i64>().ok())
        .collect();

    let roles: Vec<(i64, Option<String>)> = futures::stream::iter(game_ids)
        .map(|game_id| {
            let app = app;
            let puuid = puuid.clone();

            async move {
                let role = cached_game_detail(app, game_id).await.and_then(|detail| {
                    lcu_participant(&detail, &puuid)
                        .and_then(|participant| role_from_timeline(participant.get("timeline")))
                });

                (game_id, role)
            }
        })
        .buffered(6)
        .collect()
        .await;

    let roles: HashMap<i64, Option<String>> = roles.into_iter().collect();

    for summary in &mut summaries {
        if let Ok(game_id) = summary.match_id.parse::<i64>() {
            if let Some(role) = roles.get(&game_id) {
                summary.role = role.clone();
            }
        }
    }

    let names = queue_names(app).await;
    for summary in &mut summaries {
        summary.queue_name = names.get(&summary.queue_id).cloned();
    }

    Ok(summaries)
}

/// Kuyruk adları istemciden gelir ve dile göre değişir; bir gün cache'lenir.
async fn queue_names(app: &AppState) -> HashMap<i64, String> {
    const QUEUE_NAMES_TTL_SECS: i64 = 86_400;
    const CACHE_KEY: &str = "queues:v1";

    if let Some(cached) = app.db.kv_get(CACHE_KEY, QUEUE_NAMES_TTL_SECS) {
        if let Ok(map) = serde_json::from_str::<HashMap<i64, String>>(&cached) {
            return map;
        }
    }

    let Ok(list) = app.lcu.queues().await else {
        return HashMap::new();
    };

    let mut map = HashMap::new();
    for queue in list.as_array().into_iter().flatten() {
        let id = queue.get("id").and_then(|value| value.as_i64());
        let name = queue.get("name").and_then(|value| value.as_str());

        if let (Some(id), Some(name)) = (id, name) {
            map.insert(id, name.to_string());
        }
    }

    if let Ok(serialized) = serde_json::to_string(&map) {
        let _ = app.db.kv_set(CACHE_KEY, &serialized);
    }

    map
}

/// LP geçmişinde gösterilen kayıt sayısı.
const LP_HISTORY_LIMIT: u32 = 12;

#[tauri::command]
pub async fn account_overview(state: State<'_, AppState>) -> Result<AccountOverview, String> {
    let app = state.inner();
    let summoner = app
        .lcu
        .current_summoner()
        .await
        .map_err(|error| error.to_string())?;

    let puuid = summoner
        .get("puuid")
        .and_then(|value| value.as_str())
        .ok_or_else(|| "PUUID bulunamadı. LoL istemcisi açık mı?".to_string())?
        .to_string();

    let game_name = summoner
        .get("gameName")
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .to_string();
    let tag_line = summoner
        .get("tagLine")
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .to_string();

    let info = SummonerInfo {
        name: if tag_line.is_empty() {
            game_name
        } else {
            format!("{}#{}", game_name, tag_line)
        },
        level: summoner
            .get("summonerLevel")
            .and_then(|value| value.as_i64()),
        icon_id: summoner
            .get("profileIconId")
            .and_then(|value| value.as_i64()),
    };

    let stats = app.lcu.ranked_stats(&puuid).await.ok();

    let solo_entry = stats
        .as_ref()
        .and_then(|stats| queue_entry(stats, &["RANKED_SOLO_5x5", "JADE_RANKED_SOLO_5x5"]));
    let flex_entry = stats
        .as_ref()
        .and_then(|stats| queue_entry(stats, &["RANKED_FLEX_SR"]));

    if let Some(entry) = solo_entry {
        record_lp(app, &puuid, "solo", entry);
    }
    if let Some(entry) = flex_entry {
        record_lp(app, &puuid, "flex", entry);
    }

    let ranks = vec![
        rank_view("solo", "Dereceli Tek/Çift", solo_entry),
        rank_view("flex", "Dereceli Esnek", flex_entry),
    ];

    // LP geçmişi: solo varsa solo, yoksa flex.
    let lp_queue = if solo_entry.is_some() { "solo" } else { "flex" };
    let lp_history = lp_points(app, &puuid, lp_queue);

    let mut champions = Vec::new();
    let mut recent_games = 0;
    let mut recent_wins = 0;
    let mut recent_form = Vec::new();
    let since = start_of_day();
    let mut session = SessionSummary {
        games: 0,
        wins: 0,
        losses: 0,
        winrate: None,
        avg_kda: None,
        lp_delta: None,
        streak: 0,
        since: Some(since),
    };

    if let Some(history) = cached_history(app, &puuid).await {
        let mastery = cached_mastery(app, &puuid).await;
        champions = champion_stats(&history, &puuid, &mastery);
        session = session_from_history(&history, &puuid, since);

        for game in history_games(&history) {
            let queue_id = game
                .get("queueId")
                .and_then(|value| value.as_i64())
                .unwrap_or(0);
            if !is_summoners_rift(queue_id) {
                continue;
            }

            let Some(participant) = lcu_participant(game, &puuid) else {
                continue;
            };

            let win = participant
                .pointer("/stats/win")
                .and_then(|value| value.as_bool())
                .unwrap_or(false);

            recent_games += 1;
            if win {
                recent_wins += 1;
            }
            if recent_form.len() < 20 {
                recent_form.push(win);
            }
        }
    }

    session.lp_delta = session_lp_delta(app, &puuid, lp_queue, since);
    let streak = streak_of(&recent_form);

    Ok(AccountOverview {
        summoner: info,
        ranks,
        lp_history,
        champions,
        recent_games,
        recent_wins,
        recent_winrate: rate(recent_wins, recent_games),
        recent_form,
        streak,
        session,
    })
}

#[tauri::command]
pub async fn lobby_view(state: State<'_, AppState>) -> Result<Option<LobbyView>, String> {
    let app = state.inner();

    let lobby = match app.lcu.lobby().await {
        Ok(Some(lobby)) => lobby,
        Ok(None) => return Ok(None),
        Err(error) => return Err(error.to_string()),
    };

    let queue_id = lobby
        .pointer("/gameConfig/queueId")
        .and_then(|value| value.as_i64())
        .unwrap_or(0);
    let queue_name = queue_names(app).await.get(&queue_id).cloned();

    let summoner = app.lcu.current_summoner().await.ok();
    let self_puuid = summoner
        .as_ref()
        .and_then(|value| value.get("puuid"))
        .and_then(|value| value.as_str())
        .map(String::from);
    let self_name = summoner
        .as_ref()
        .and_then(|value| value.get("gameName"))
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .to_string();
    let self_tag = summoner
        .as_ref()
        .and_then(|value| value.get("tagLine"))
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .to_string();

    let members_json: Vec<Value> = lobby
        .get("members")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();

    let members = join_all(members_json.into_iter().map(|member| {
        let self_puuid = self_puuid.clone();
        let self_name = self_name.clone();
        let self_tag = self_tag.clone();

        async move {
            let puuid = member
                .get("puuid")
                .and_then(|value| value.as_str())
                .map(String::from);
            let summoner_id = member.get("summonerId").and_then(|value| value.as_i64());
            let is_self = puuid.is_some() && puuid == self_puuid;

            let mut name = String::new();
            if is_self {
                name = if self_tag.is_empty() {
                    self_name.clone()
                } else {
                    format!("{}#{}", self_name, self_tag)
                };
            } else if let Some(value) = member.get("summonerName").and_then(|value| value.as_str())
            {
                name = value.to_string();
            }

            let mut level = member
                .get("summonerLevel")
                .and_then(|value| value.as_i64())
                .filter(|value| *value > 0);
            let mut icon_id = member
                .get("summonerIconId")
                .and_then(|value| value.as_i64())
                .filter(|value| *value > 0);

            if name.is_empty() {
                if let Some(summoner_id) = summoner_id {
                    if let Ok(info) = app.lcu.summoner_by_id(summoner_id).await {
                        let game_name = info
                            .get("gameName")
                            .and_then(|value| value.as_str())
                            .unwrap_or("");
                        let tag = info
                            .get("tagLine")
                            .and_then(|value| value.as_str())
                            .unwrap_or("");

                        if !game_name.is_empty() {
                            name = format!("{}#{}", game_name, tag);
                        }
                        level = level.or_else(|| {
                            info.get("summonerLevel")
                                .and_then(|value| value.as_i64())
                                .filter(|value| *value > 0)
                        });
                        icon_id = icon_id.or_else(|| {
                            info.get("profileIconId")
                                .and_then(|value| value.as_i64())
                                .filter(|value| *value > 0)
                        });
                    }
                }
            }

            if name.is_empty() {
                name = "Gizli oyuncu".to_string();
            }

            let mut tier = None;
            let mut division = None;
            let mut lp = None;
            let mut wins = None;
            let mut losses = None;
            let mut winrate = None;

            if let Some(puuid) = puuid.as_deref() {
                if let Ok(stats) = app.lcu.ranked_stats(puuid).await {
                    if let Some(entry) = pick_solo_entry(&stats) {
                        let tier_raw = entry
                            .get("tier")
                            .and_then(|value| value.as_str())
                            .unwrap_or("");
                        tier = Some(tier_name(tier_raw).to_string());
                        division = entry
                            .get("division")
                            .and_then(|value| value.as_str())
                            .filter(|value| !value.is_empty() && *value != "NA")
                            .map(title_case);
                        lp = entry.get("leaguePoints").and_then(|value| value.as_i64());
                        wins = entry.get("wins").and_then(|value| value.as_i64());
                        losses = entry.get("losses").and_then(|value| value.as_i64());
                        winrate = match (wins, losses) {
                            (Some(wins), Some(losses)) if wins + losses > 0 => {
                                Some(wins as f64 / (wins + losses) as f64 * 100.0)
                            }
                            _ => None,
                        };
                    }
                }
            }

            let position = member
                .get("firstPositionPreference")
                .and_then(|value| value.as_str())
                .filter(|value| !value.is_empty() && *value != "UNSELECTED" && *value != "FILL")
                .map(String::from);

            LobbyMember {
                name,
                level,
                icon_id,
                tier,
                division,
                lp,
                wins,
                losses,
                winrate,
                position,
                is_self,
            }
        }
    }))
    .await;

    Ok(Some(LobbyView {
        queue_id,
        queue_name,
        members,
    }))
}

fn queue_entry<'a>(stats: &'a Value, keys: &[&str]) -> Option<&'a Value> {
    let queue_map = stats.get("queueMap")?;

    keys.iter()
        .filter_map(|key| queue_map.get(key))
        .find(|entry| {
            entry
                .get("tier")
                .and_then(|value| value.as_str())
                .is_some_and(|tier| !tier.is_empty())
        })
}

fn rank_view(queue: &str, label: &str, entry: Option<&Value>) -> RankView {
    let Some(entry) = entry else {
        return RankView {
            queue: queue.to_string(),
            label: label.to_string(),
            tier: None,
            division: None,
            lp: None,
            wins: 0,
            losses: 0,
            winrate: None,
            provisional: false,
        };
    };

    let tier_raw = entry
        .get("tier")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    let wins = entry
        .get("wins")
        .and_then(|value| value.as_i64())
        .unwrap_or(0);
    let losses = entry
        .get("losses")
        .and_then(|value| value.as_i64())
        .unwrap_or(0);

    RankView {
        queue: queue.to_string(),
        label: label.to_string(),
        tier: Some(tier_name(tier_raw).to_string()),
        division: entry
            .get("division")
            .and_then(|value| value.as_str())
            .filter(|value| !value.is_empty() && *value != "NA")
            .map(title_case),
        lp: entry.get("leaguePoints").and_then(|value| value.as_i64()),
        wins,
        losses,
        winrate: rate(wins, wins + losses),
        provisional: entry
            .get("isProvisional")
            .and_then(|value| value.as_bool())
            .unwrap_or(false),
    }
}

/// Derece değişimini yerelde kaydeder. LCU geçmiş vermediği için LP geçmişi
/// Tlas'ın kendi gözlemlerinden oluşur.
fn record_lp(app: &AppState, puuid: &str, queue: &str, entry: &Value) {
    let Some(tier) = entry
        .get("tier")
        .and_then(|value| value.as_str())
        .filter(|tier| !tier.is_empty())
    else {
        return;
    };

    let division = entry
        .get("division")
        .and_then(|value| value.as_str())
        .filter(|value| !value.is_empty() && *value != "NA")
        .map(String::from);
    let lp = entry
        .get("leaguePoints")
        .and_then(|value| value.as_i64())
        .unwrap_or(0);
    let wins = entry
        .get("wins")
        .and_then(|value| value.as_i64())
        .unwrap_or(0);
    let losses = entry
        .get("losses")
        .and_then(|value| value.as_i64())
        .unwrap_or(0);

    if let Some(previous) = app.db.lp_last(puuid, queue) {
        let unchanged = previous.tier == tier
            && previous.division == division
            && previous.lp == lp
            && previous.wins == wins
            && previous.losses == losses;

        if unchanged {
            return;
        }
    }

    let snapshot = LpSnapshot {
        ts: chrono::Utc::now().timestamp(),
        tier: tier.to_string(),
        division,
        lp,
        wins,
        losses,
    };

    let _ = app.db.lp_push(puuid, queue, &snapshot);
}

fn lp_points(app: &AppState, puuid: &str, queue: &str) -> Vec<LpPoint> {
    let snapshots = app.db.lp_recent(puuid, queue, LP_HISTORY_LIMIT);

    let mut points: Vec<LpPoint> = snapshots
        .iter()
        .map(|snapshot| LpPoint {
            ts: snapshot.ts,
            tier: tier_name(&snapshot.tier).to_string(),
            division: snapshot.division.clone(),
            lp: snapshot.lp,
            delta: None,
        })
        .collect();

    // En yeniden eskiye sıralı: bir önceki kayıtla farkı hesapla.
    for index in 0..points.len() {
        let Some(next) = snapshots.get(index + 1) else {
            break;
        };

        if snapshots[index].tier == next.tier && snapshots[index].division == next.division {
            points[index].delta = Some(points[index].lp - points[index + 1].lp);
        }
    }

    points
}

/// Bugünkü maçları özetler. Geçmiş en yeniden eskiye sıralı gelir.
fn session_from_history(history: &Value, puuid: &str, since: i64) -> SessionSummary {
    let mut games = 0i64;
    let mut wins = 0i64;
    let mut kda_total = 0.0f64;
    let mut form = Vec::new();

    for game in history_games(history) {
        let queue_id = game
            .get("queueId")
            .and_then(|value| value.as_i64())
            .unwrap_or(0);
        if !is_summoners_rift(queue_id) {
            continue;
        }

        let created = game
            .get("gameCreation")
            .and_then(|value| value.as_i64())
            .unwrap_or(0);
        let created_secs = if created > 100_000_000_000 {
            created / 1_000
        } else {
            created
        };
        if created_secs < since {
            continue;
        }

        let Some(participant) = lcu_participant(game, puuid) else {
            continue;
        };
        let Some(stats) = participant.get("stats") else {
            continue;
        };

        let win = stats
            .get("win")
            .and_then(|value| value.as_bool())
            .unwrap_or(false);
        let kills = stats
            .get("kills")
            .and_then(|value| value.as_i64())
            .unwrap_or(0);
        let deaths = stats
            .get("deaths")
            .and_then(|value| value.as_i64())
            .unwrap_or(0);
        let assists = stats
            .get("assists")
            .and_then(|value| value.as_i64())
            .unwrap_or(0);

        games += 1;
        if win {
            wins += 1;
        }
        kda_total += (kills + assists) as f64 / deaths.max(1) as f64;
        form.push(win);
    }

    SessionSummary {
        games,
        wins,
        losses: games - wins,
        winrate: rate(wins, games),
        avg_kda: (games > 0).then(|| round2(kda_total / games as f64)),
        lp_delta: None,
        streak: streak_of(&form),
        since: Some(since),
    }
}

/// Bugünün LP değişimi. Terfi/düşüş olduysa fark gösterilmez.
fn session_lp_delta(app: &AppState, puuid: &str, queue: &str, since: i64) -> Option<i64> {
    let snapshots: Vec<LpSnapshot> = app
        .db
        .lp_recent(puuid, queue, 200)
        .into_iter()
        .filter(|snapshot| snapshot.ts >= since)
        .collect();

    let latest = snapshots.first()?;
    let earliest = snapshots.last()?;

    (snapshots.len() >= 2 && latest.tier == earliest.tier && latest.division == earliest.division)
        .then(|| latest.lp - earliest.lp)
}

async fn mastery_points(app: &AppState, puuid: &str) -> HashMap<i64, i64> {
    let mut map = HashMap::new();

    if let Ok(list) = app.lcu.champion_mastery(puuid).await {
        for entry in list.as_array().into_iter().flatten() {
            let champion_id = entry
                .get("championId")
                .and_then(|value| value.as_i64())
                .unwrap_or(0);
            let points = entry
                .get("championPoints")
                .and_then(|value| value.as_i64())
                .unwrap_or(0);

            if champion_id > 0 {
                map.insert(champion_id, points);
            }
        }
    }

    map
}

fn champion_stats(history: &Value, puuid: &str, mastery: &HashMap<i64, i64>) -> Vec<ChampionStat> {
    #[derive(Default)]
    struct Acc {
        games: i64,
        wins: i64,
        kills: i64,
        deaths: i64,
        assists: i64,
    }

    let mut acc: HashMap<i64, Acc> = HashMap::new();

    for game in history_games(history) {
        let queue_id = game
            .get("queueId")
            .and_then(|value| value.as_i64())
            .unwrap_or(0);
        if !is_summoners_rift(queue_id) {
            continue;
        }

        let Some(participant) = lcu_participant(game, puuid) else {
            continue;
        };

        let champion_id = participant
            .get("championId")
            .and_then(|value| value.as_i64())
            .unwrap_or(0);
        if champion_id == 0 {
            continue;
        }

        let stats = participant.get("stats");
        let entry = acc.entry(champion_id).or_default();

        entry.games += 1;
        if stats
            .and_then(|value| value.get("win"))
            .and_then(|value| value.as_bool())
            .unwrap_or(false)
        {
            entry.wins += 1;
        }
        entry.kills += stats
            .and_then(|value| value.get("kills"))
            .and_then(|value| value.as_i64())
            .unwrap_or(0);
        entry.deaths += stats
            .and_then(|value| value.get("deaths"))
            .and_then(|value| value.as_i64())
            .unwrap_or(0);
        entry.assists += stats
            .and_then(|value| value.get("assists"))
            .and_then(|value| value.as_i64())
            .unwrap_or(0);
    }

    let mut champions: Vec<ChampionStat> = acc
        .into_iter()
        .map(|(champion_id, entry)| ChampionStat {
            champion_id,
            games: entry.games,
            wins: entry.wins,
            winrate: entry.wins as f64 / entry.games.max(1) as f64 * 100.0,
            kda: round2((entry.kills + entry.assists) as f64 / entry.deaths.max(1) as f64),
            points: mastery.get(&champion_id).copied(),
        })
        .collect();

    // Sıralama deterministik olmalı: eşit maç sayısında ustalık puanı, sonra
    // şampiyon kimliği kullanılır. HashMap sırası doğrudan kullanılırsa liste
    // her yenilemede farklı görünür.
    champions.sort_by(|left, right| {
        right
            .games
            .cmp(&left.games)
            .then_with(|| right.winrate.total_cmp(&left.winrate))
            .then_with(|| right.points.unwrap_or(0).cmp(&left.points.unwrap_or(0)))
            .then_with(|| left.champion_id.cmp(&right.champion_id))
    });
    champions.truncate(6);

    champions
}

fn draft_insights_from_games(games: &[Value], puuid: &str) -> Vec<DraftInsight> {
    #[derive(Default)]
    struct Acc {
        games: i64,
        wins: i64,
        kills: i64,
        deaths: i64,
        assists: i64,
    }

    let mut acc: HashMap<(i64, String), Acc> = HashMap::new();
    for game in games {
        let queue_id = game.get("queueId").and_then(Value::as_i64).unwrap_or(0);
        if !is_summoners_rift(queue_id) {
            continue;
        }

        let Some(participant) = lcu_participant(game, puuid) else {
            continue;
        };
        let Some(position) = role_from_timeline(participant.get("timeline")) else {
            continue;
        };
        let champion_id = participant
            .get("championId")
            .and_then(Value::as_i64)
            .unwrap_or(0);
        let Some(stats) = participant.get("stats") else {
            continue;
        };
        if champion_id <= 0 {
            continue;
        }

        let entry = acc.entry((champion_id, position)).or_default();
        entry.games += 1;
        if stats.get("win").and_then(Value::as_bool).unwrap_or(false) {
            entry.wins += 1;
        }
        entry.kills += stats_i64(Some(stats), "kills");
        entry.deaths += stats_i64(Some(stats), "deaths");
        entry.assists += stats_i64(Some(stats), "assists");
    }

    let mut insights: Vec<DraftInsight> = acc
        .into_iter()
        .map(|((champion_id, position), entry)| DraftInsight {
            champion_id,
            position: Some(position),
            games: entry.games,
            wins: entry.wins,
            winrate: round2(entry.wins as f64 / entry.games.max(1) as f64 * 100.0),
            kda: round2((entry.kills + entry.assists) as f64 / entry.deaths.max(1) as f64),
        })
        .collect();

    insights.sort_by(|left, right| {
        right
            .games
            .cmp(&left.games)
            .then_with(|| right.winrate.total_cmp(&left.winrate))
            .then_with(|| left.champion_id.cmp(&right.champion_id))
            .then_with(|| {
                position_rank(left.position.as_deref())
                    .cmp(&position_rank(right.position.as_deref()))
            })
    });
    insights
}

fn streak_of(form: &[bool]) -> i64 {
    let Some(first) = form.first() else {
        return 0;
    };

    let mut count = 0i64;
    for win in form {
        if win != first {
            break;
        }
        count += 1;
    }

    if *first { count } else { -count }
}

fn rate(part: i64, total: i64) -> Option<f64> {
    (total > 0).then(|| round2(part as f64 / total as f64 * 100.0))
}

fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

fn parse_match_summary(game: &Value, puuid: &str) -> Option<MatchSummary> {
    let participant = lcu_participant(game, puuid)?;
    let stats = participant.get("stats")?;

    let raw_duration = game
        .get("gameDuration")
        .and_then(|value| value.as_i64())
        .unwrap_or(0);
    let duration_secs = if raw_duration > 100_000 {
        raw_duration / 1_000
    } else {
        raw_duration
    };

    let cs = stats
        .get("totalMinionsKilled")
        .and_then(|value| value.as_i64())
        .unwrap_or(0)
        + stats
            .get("neutralMinionsKilled")
            .and_then(|value| value.as_i64())
            .unwrap_or(0);

    Some(MatchSummary {
        match_id: game
            .get("gameId")
            .and_then(|value| value.as_i64())
            .unwrap_or(0)
            .to_string(),
        champion_name: String::new(),
        champion_id: participant
            .get("championId")
            .and_then(|value| value.as_i64())
            .unwrap_or(0),
        win: stats
            .get("win")
            .and_then(|value| value.as_bool())
            .unwrap_or(false),
        kills: stats
            .get("kills")
            .and_then(|value| value.as_i64())
            .unwrap_or(0),
        deaths: stats
            .get("deaths")
            .and_then(|value| value.as_i64())
            .unwrap_or(0),
        assists: stats
            .get("assists")
            .and_then(|value| value.as_i64())
            .unwrap_or(0),
        cs,
        duration_secs,
        queue_id: game
            .get("queueId")
            .and_then(|value| value.as_i64())
            .unwrap_or(0),
        queue_name: None,
        game_creation: game
            .get("gameCreation")
            .and_then(|value| value.as_i64())
            .unwrap_or(0),
        role: role_from_timeline(participant.get("timeline")),
    })
}

fn role_from_timeline(timeline: Option<&Value>) -> Option<String> {
    let timeline = timeline?;
    let lane = timeline.get("lane").and_then(|value| value.as_str())?;
    let role = timeline
        .get("role")
        .and_then(|value| value.as_str())
        .unwrap_or("");

    let position = match (lane, role) {
        ("JUNGLE", _) => "JUNGLE",
        ("TOP", _) => "TOP",
        ("MIDDLE", _) => "MIDDLE",
        ("BOTTOM", "DUO_SUPPORT") | ("BOTTOM", "SUPPORT") => "UTILITY",
        ("BOTTOM", _) => "BOTTOM",
        _ => return None,
    };

    Some(position.to_string())
}

#[tauri::command]
pub async fn match_detail(state: State<'_, AppState>, game_id: i64) -> Result<MatchDetail, String> {
    let app = state.inner();
    let puuid = current_puuid(app).await?;

    let game = app
        .lcu
        .game_detail(game_id)
        .await
        .map_err(|error| error.to_string())?;

    let mut detail = parse_match_detail(&game, &puuid);
    detail.queue_name = queue_names(app).await.get(&detail.queue_id).cloned();

    Ok(detail)
}

#[tauri::command]
pub async fn personal_build(
    state: State<'_, AppState>,
    champion_id: i64,
) -> Result<PersonalBuild, String> {
    let app = state.inner();
    let puuid = current_puuid(app).await?;

    let history = cached_history(app, &puuid)
        .await
        .ok_or_else(|| "Maç geçmişi alınamadı. LoL istemcisi açık olmalı.".to_string())?;

    Ok(build_from_history(champion_id, &history, &puuid))
}

#[tauri::command]
pub async fn live_game(state: State<'_, AppState>) -> Result<Option<LiveGameView>, String> {
    let app = state.inner();

    let Some(session) = app
        .lcu
        .gameflow_session()
        .await
        .map_err(|error| error.to_string())?
    else {
        return Ok(None);
    };

    let phase = session
        .get("phase")
        .and_then(|value| value.as_str())
        .unwrap_or("");

    if !matches!(phase, "InProgress" | "GameStart" | "Reconnect") {
        return Ok(None);
    }

    let Some(data) = session.get("gameData") else {
        return Ok(None);
    };

    let team_one = data
        .get("teamOne")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();
    let team_two = data
        .get("teamTwo")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();

    if team_one.is_empty() && team_two.is_empty() {
        return Ok(None);
    }

    let current = app.lcu.current_summoner().await.ok();
    let self_id = current
        .as_ref()
        .and_then(|value| value.get("summonerId"))
        .and_then(|value| value.as_i64());
    let self_name = current.as_ref().map(summoner_display_name);

    let allies = join_all(team_one.iter().map(|player| {
        let app = app;
        let self_id = self_id;
        let self_name = self_name.clone();

        async move {
            let summoner_id = player
                .get("summonerId")
                .and_then(|value| value.as_i64())
                .filter(|value| *value > 0);

            let display_name = if summoner_id.is_some() && summoner_id == self_id {
                self_name.unwrap_or_else(|| "Sen".to_string())
            } else if let Some(summoner_id) = summoner_id {
                match app.lcu.summoner_by_id(summoner_id).await {
                    Ok(info) => summoner_display_name(&info),
                    Err(_) => "Takım arkadaşı".to_string(),
                }
            } else {
                "Takım arkadaşı".to_string()
            };

            LivePlayer {
                display_name,
                champion_id: player
                    .get("championId")
                    .and_then(|value| value.as_i64())
                    .unwrap_or(0),
                position: live_position(player),
                is_ally: true,
                is_me: summoner_id.is_some() && summoner_id == self_id,
            }
        }
    }))
    .await;

    let enemies = team_two
        .iter()
        .enumerate()
        .map(|(index, player)| LivePlayer {
            // Rakip kimliği hiçbir zaman çözülmez; yalnızca şampiyon gösterilir.
            display_name: format!("Rakip {}", index + 1),
            champion_id: player
                .get("championId")
                .and_then(|value| value.as_i64())
                .unwrap_or(0),
            position: live_position(player),
            is_ally: false,
            is_me: false,
        })
        .collect();

    let queue_id = data
        .pointer("/queue/id")
        .and_then(|value| value.as_i64())
        .or_else(|| data.get("queueId").and_then(|value| value.as_i64()))
        .unwrap_or(0);
    let queue_name = queue_names(app).await.get(&queue_id).cloned();

    Ok(Some(LiveGameView {
        game_id: data
            .get("gameId")
            .and_then(|value| value.as_i64())
            .unwrap_or(0),
        queue_id,
        queue_name,
        allies,
        enemies,
    }))
}

#[tauri::command]
pub async fn opgg_meta(
    state: State<'_, AppState>,
    champion: String,
    position: String,
) -> Result<MetaReport, String> {
    let app = state.inner();

    if !app.config.get().experimental_meta {
        return Err("Deneysel meta kapalı. Ayarlardan açabilirsin.".to_string());
    }

    crate::opgg::fetch(&app.db, &champion, &position).await
}

fn parse_match_detail(game: &Value, puuid: &str) -> MatchDetail {
    let mut identities: HashMap<i64, &Value> = HashMap::new();

    for identity in game
        .get("participantIdentities")
        .and_then(|value| value.as_array())
        .into_iter()
        .flatten()
    {
        if let (Some(id), Some(player)) = (
            identity
                .get("participantId")
                .and_then(|value| value.as_i64()),
            identity.get("player"),
        ) {
            identities.insert(id, player);
        }
    }

    let mut teams: HashMap<i64, MatchTeam> = HashMap::new();

    for participant in game
        .get("participants")
        .and_then(|value| value.as_array())
        .into_iter()
        .flatten()
    {
        let team_id = participant
            .get("teamId")
            .and_then(|value| value.as_i64())
            .unwrap_or(0);
        let stats = participant.get("stats");

        let kills = stats_i64(stats, "kills");
        let deaths = stats_i64(stats, "deaths");
        let assists = stats_i64(stats, "assists");
        let win = stats
            .and_then(|value| value.get("win"))
            .and_then(|value| value.as_bool())
            .unwrap_or(false);

        let items = (0..=6)
            .filter_map(|slot| {
                let item = stats?
                    .get(format!("item{slot}"))
                    .and_then(|value| value.as_i64())?;
                (item > 0).then_some(item)
            })
            .collect();

        let identity = participant
            .get("participantId")
            .and_then(|value| value.as_i64())
            .and_then(|id| identities.get(&id))
            .copied();
        let player_puuid = identity
            .and_then(|value| value.get("puuid"))
            .and_then(|value| value.as_str())
            .map(String::from);
        let is_me = player_puuid.as_deref() == Some(puuid);

        let entry = teams.entry(team_id).or_insert(MatchTeam {
            team_id,
            win: false,
            kills: 0,
            players: Vec::new(),
        });

        entry.win |= win;
        entry.kills += kills;
        entry.players.push(MatchPlayer {
            puuid: player_puuid,
            display_name: player_name(identity),
            champion_id: participant
                .get("championId")
                .and_then(|value| value.as_i64())
                .unwrap_or(0),
            team_id,
            position: role_from_timeline(participant.get("timeline")),
            kills,
            deaths,
            assists,
            cs: stats_i64(stats, "totalMinionsKilled") + stats_i64(stats, "neutralMinionsKilled"),
            gold: stats_i64(stats, "goldEarned"),
            damage: stats_i64(stats, "totalDamageDealtToChampions"),
            vision: stats_i64(stats, "visionScore"),
            items,
            is_me,
        });
    }

    let mut teams: Vec<MatchTeam> = teams.into_values().collect();
    teams.sort_by_key(|team| team.team_id);

    for team in &mut teams {
        team.players.sort_by(|left, right| {
            position_rank(left.position.as_deref())
                .cmp(&position_rank(right.position.as_deref()))
                .then_with(|| left.display_name.cmp(&right.display_name))
        });
    }

    MatchDetail {
        match_id: game
            .get("gameId")
            .and_then(|value| value.as_i64())
            .unwrap_or(0)
            .to_string(),
        queue_id: game
            .get("queueId")
            .and_then(|value| value.as_i64())
            .unwrap_or(0),
        queue_name: None,
        duration_secs: normalize_duration(game),
        game_creation: game
            .get("gameCreation")
            .and_then(|value| value.as_i64())
            .unwrap_or(0),
        teams,
    }
}

fn build_from_history(champion_id: i64, history: &Value, puuid: &str) -> PersonalBuild {
    #[derive(Default)]
    struct ItemAcc {
        count: i64,
        wins: i64,
    }

    let mut items: HashMap<i64, ItemAcc> = HashMap::new();
    let mut positions: HashMap<String, i64> = HashMap::new();
    let mut games = 0i64;
    let mut wins = 0i64;
    let mut kills_total = 0i64;
    let mut deaths_total = 0i64;
    let mut assists_total = 0i64;
    let mut cs_total = 0i64;

    for game in history_games(history) {
        let queue_id = game
            .get("queueId")
            .and_then(|value| value.as_i64())
            .unwrap_or(0);
        if !is_summoners_rift(queue_id) {
            continue;
        }

        let Some(participant) = lcu_participant(game, puuid) else {
            continue;
        };
        if participant
            .get("championId")
            .and_then(|value| value.as_i64())
            != Some(champion_id)
        {
            continue;
        }

        let Some(stats) = participant.get("stats") else {
            continue;
        };

        let win = stats
            .get("win")
            .and_then(|value| value.as_bool())
            .unwrap_or(false);

        games += 1;
        if win {
            wins += 1;
        }
        kills_total += stats_i64(Some(stats), "kills");
        deaths_total += stats_i64(Some(stats), "deaths");
        assists_total += stats_i64(Some(stats), "assists");
        cs_total += stats_i64(Some(stats), "totalMinionsKilled")
            + stats_i64(Some(stats), "neutralMinionsKilled");

        for slot in 0..=5 {
            let item = stats
                .get(format!("item{slot}"))
                .and_then(|value| value.as_i64())
                .unwrap_or(0);
            if item <= 0 {
                continue;
            }

            let entry = items.entry(item).or_default();
            entry.count += 1;
            if win {
                entry.wins += 1;
            }
        }

        if let Some(position) = role_from_timeline(participant.get("timeline")) {
            *positions.entry(position).or_default() += 1;
        }
    }

    let to_build_item = |(item_id, acc): (&i64, &ItemAcc)| BuildItem {
        item_id: *item_id,
        count: acc.count,
        winrate: round2(acc.wins as f64 / acc.count.max(1) as f64 * 100.0),
    };

    let mut core_items: Vec<BuildItem> = items
        .iter()
        .filter(|(item_id, _)| is_completed_item(**item_id))
        .map(to_build_item)
        .collect();
    core_items.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.item_id.cmp(&right.item_id))
    });
    core_items.truncate(5);

    let mut boots: Vec<BuildItem> = items
        .iter()
        .filter(|(item_id, _)| BOOTS.contains(*item_id))
        .map(to_build_item)
        .collect();
    boots.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.item_id.cmp(&right.item_id))
    });
    boots.truncate(2);

    PersonalBuild {
        champion_id,
        games,
        wins,
        winrate: rate(wins, games).unwrap_or(0.0),
        kda: round2((kills_total + assists_total) as f64 / deaths_total.max(1) as f64),
        avg_cs: if games > 0 {
            round2(cs_total as f64 / games as f64)
        } else {
            0.0
        },
        position: positions
            .into_iter()
            .max_by_key(|(_, count)| *count)
            .map(|(position, _)| position),
        core_items,
        boots,
    }
}

fn stats_i64(stats: Option<&Value>, key: &str) -> i64 {
    stats
        .and_then(|value| value.get(key))
        .and_then(|value| value.as_i64())
        .unwrap_or(0)
}

fn normalize_duration(game: &Value) -> i64 {
    let raw = game
        .get("gameDuration")
        .and_then(|value| value.as_i64())
        .unwrap_or(0);

    if raw > 100_000 { raw / 1_000 } else { raw }
}

fn player_name(player: Option<&Value>) -> String {
    let Some(player) = player else {
        return "Gizli oyuncu".to_string();
    };

    if let Some(name) = player
        .get("summonerName")
        .and_then(|value| value.as_str())
        .filter(|value| !value.is_empty())
    {
        return name.to_string();
    }

    let game_name = player
        .get("gameName")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    let tag_line = player
        .get("tagLine")
        .and_then(|value| value.as_str())
        .unwrap_or("");

    if game_name.is_empty() {
        "Gizli oyuncu".to_string()
    } else if tag_line.is_empty() {
        game_name.to_string()
    } else {
        format!("{}#{}", game_name, tag_line)
    }
}

fn summoner_display_name(summoner: &Value) -> String {
    player_name(Some(summoner))
}

fn live_position(player: &Value) -> Option<String> {
    let value = player
        .get("selectedPosition")
        .or_else(|| player.get("assignedPosition"))
        .and_then(|value| value.as_str())?;

    POSITION_ORDER.contains(&value).then(|| value.to_string())
}

fn position_rank(position: Option<&str>) -> usize {
    position
        .and_then(|value| POSITION_ORDER.iter().position(|item| *item == value))
        .unwrap_or(POSITION_ORDER.len())
}

fn is_completed_item(item_id: i64) -> bool {
    item_id >= 3000 && !BOOTS.contains(&item_id) && !TRINKETS.contains(&item_id)
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Settings {
    state.config.get()
}

#[tauri::command]
pub fn save_settings(state: State<'_, AppState>, settings: Settings) -> Result<(), String> {
    state
        .config
        .set(settings)
        .map_err(|error| error.to_string())
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
        let stats = json!({
            "queueMap": {
                "RANKED_FLEX_SR": { "tier": "GOLD", "division": "I" },
                "RANKED_SOLO_5x5": { "tier": "PLATINUM", "division": "II" },
                "RANKED_TFT": { "tier": "" }
            }
        });

        assert_eq!(
            pick_solo_entry(&stats)
                .and_then(|entry| entry.get("tier"))
                .and_then(|value| value.as_str()),
            Some("PLATINUM")
        );
    }

    #[test]
    fn falls_back_to_flex_queue() {
        let stats = json!({
            "queueMap": {
                "RANKED_SOLO_5x5": { "tier": "" },
                "RANKED_FLEX_SR": { "tier": "DIAMOND", "division": "IV" }
            }
        });

        assert_eq!(
            pick_solo_entry(&stats)
                .and_then(|entry| entry.get("tier"))
                .and_then(|value| value.as_str()),
            Some("DIAMOND")
        );
    }

    #[test]
    fn parses_match_summary_for_player() {
        let game = json!({
            "gameId": 1754219635_i64,
            "gameDuration": 1_800,
            "queueId": 420,
            "gameCreation": 1_700_000_000_000_i64,
            "participantIdentities": [{
                "participantId": 1,
                "player": { "puuid": "p1" }
            }],
            "participants": [{
                "participantId": 1,
                "championId": 103,
                "timeline": { "lane": "MIDDLE", "role": "SOLO" },
                "stats": {
                    "win": true,
                    "kills": 7,
                    "deaths": 2,
                    "assists": 9,
                    "totalMinionsKilled": 180,
                    "neutralMinionsKilled": 12
                }
            }]
        });

        let summary = parse_match_summary(&game, "p1").expect("summary");

        assert_eq!(summary.match_id, "1754219635");
        assert_eq!(summary.cs, 192);
        assert!(summary.win);
        assert_eq!(summary.role.as_deref(), Some("MIDDLE"));
    }

    #[test]
    fn maps_duo_support_to_utility() {
        let timeline = json!({ "lane": "BOTTOM", "role": "DUO_SUPPORT" });
        assert_eq!(
            role_from_timeline(Some(&timeline)).as_deref(),
            Some("UTILITY")
        );

        let unknown = json!({ "lane": "NONE", "role": "SOLO" });
        assert_eq!(role_from_timeline(Some(&unknown)), None);
    }

    #[test]
    fn counts_only_rift_queues() {
        assert!(is_summoners_rift(420));
        assert!(is_summoners_rift(440));
        assert!(!is_summoners_rift(450));
        assert!(!is_summoners_rift(1700));
    }

    #[test]
    fn aggregates_draft_insights_by_champion_and_role() {
        let jungle_win = history_game(1, 141, true, &[], 1_700_000_000_000);
        let jungle_loss = history_game(2, 141, false, &[], 1_700_000_001_000);
        let mut middle = history_game(3, 141, true, &[], 1_700_000_002_000);
        middle["participants"][0]["timeline"]["lane"] = json!("MIDDLE");
        let mut unknown_role = history_game(4, 141, true, &[], 1_700_000_003_000);
        unknown_role["participants"][0]["timeline"]["lane"] = json!("NONE");
        let mut aram = history_game(5, 141, true, &[], 1_700_000_004_000);
        aram["queueId"] = json!(450);

        let insights =
            draft_insights_from_games(&[jungle_win, jungle_loss, middle, unknown_role, aram], "p1");

        assert_eq!(insights.len(), 2);
        assert_eq!(insights[0].position.as_deref(), Some("JUNGLE"));
        assert_eq!(insights[0].games, 2);
        assert_eq!(insights[0].wins, 1);
        assert_eq!(insights[0].winrate, 50.0);
        assert_eq!(insights[0].kda, 6.0);
        assert_eq!(insights[1].position.as_deref(), Some("MIDDLE"));
        assert_eq!(insights[1].games, 1);
    }

    #[test]
    fn formats_rank_titles() {
        assert_eq!(title_case("GRANDMASTER"), "Grandmaster");
        assert_eq!(title_case(""), "");
        assert_eq!(tier_name("PLATINUM"), "Platin");
        assert_eq!(tier_name("EMERALD"), "Zümrüt");
        assert_eq!(tier_name("UNKNOWN"), "UNKNOWN");
    }

    fn history_game(
        game_id: i64,
        champion_id: i64,
        win: bool,
        items: &[i64],
        creation: i64,
    ) -> Value {
        json!({
            "gameId": game_id,
            "gameDuration": 1_800,
            "queueId": 420,
            "gameCreation": creation,
            "participantIdentities": [{
                "participantId": 1,
                "player": { "puuid": "p1", "summonerName": "Ben" }
            }],
            "participants": [{
                "participantId": 1,
                "championId": champion_id,
                "teamId": 100,
                "timeline": { "lane": "JUNGLE", "role": "NONE" },
                "stats": {
                    "win": win,
                    "kills": 5,
                    "deaths": 2,
                    "assists": 7,
                    "totalMinionsKilled": 100,
                    "neutralMinionsKilled": 50,
                    "item0": items.first().copied().unwrap_or(0),
                    "item1": items.get(1).copied().unwrap_or(0),
                    "item2": items.get(2).copied().unwrap_or(0),
                    "item3": items.get(3).copied().unwrap_or(0),
                    "item4": items.get(4).copied().unwrap_or(0),
                    "item5": items.get(5).copied().unwrap_or(0),
                    "item6": items.get(6).copied().unwrap_or(0)
                }
            }]
        })
    }

    #[test]
    fn parses_full_match_detail() {
        let mut participants = Vec::new();
        let mut identities = Vec::new();

        for index in 0..10 {
            let id = index + 1;
            let ally = index < 5;

            participants.push(json!({
                "participantId": id,
                "teamId": if ally { 100 } else { 200 },
                "championId": 141 + index,
                "timeline": {
                    "lane": if index == 0 { "JUNGLE" } else { "MIDDLE" },
                    "role": "NONE"
                },
                "stats": {
                    "win": ally,
                    "kills": 10 - index,
                    "deaths": index,
                    "assists": index,
                    "totalMinionsKilled": 100,
                    "neutralMinionsKilled": 20,
                    "goldEarned": 12_000,
                    "totalDamageDealtToChampions": 20_000,
                    "visionScore": 20,
                    "item0": 3153,
                    "item1": 3006,
                    "item2": 0,
                    "item3": 0,
                    "item4": 0,
                    "item5": 0,
                    "item6": 3340
                }
            }));

            identities.push(json!({
                "participantId": id,
                "player": {
                    "puuid": if id == 1 { "p1".to_string() } else { format!("p{id}") },
                    "summonerName": if id == 1 { "Ben".to_string() } else { format!("Oyuncu {id}") }
                }
            }));
        }

        let game = json!({
            "gameId": 42,
            "gameDuration": 1_800,
            "queueId": 420,
            "gameCreation": 1_700_000_000_000_i64,
            "participantIdentities": identities,
            "participants": participants
        });

        let detail = parse_match_detail(&game, "p1");

        assert_eq!(detail.match_id, "42");
        assert_eq!(detail.duration_secs, 1_800);
        assert_eq!(detail.teams.len(), 2);
        assert_eq!(detail.teams[0].players.len(), 5);
        assert!(detail.teams[0].win);
        assert!(!detail.teams[1].win);

        let me = detail.teams[0]
            .players
            .iter()
            .find(|player| player.is_me)
            .expect("me");

        assert_eq!(me.position.as_deref(), Some("JUNGLE"));
        assert_eq!(me.cs, 120);
        assert_eq!(me.items, vec![3153, 3006, 3340]);
        assert_eq!(me.display_name, "Ben");
    }

    #[test]
    fn builds_personal_item_summary() {
        let now = chrono::Utc::now().timestamp_millis();
        let history = json!({
            "games": {
                "games": [
                    history_game(1, 141, true, &[3153, 3006, 3031], now),
                    history_game(2, 141, true, &[3153, 3031, 3006], now - 3_600_000),
                    history_game(3, 141, false, &[6672, 3006, 0], now - 7_200_000),
                    history_game(4, 103, true, &[6655, 3020, 0], now - 10_800_000)
                ]
            }
        });

        let build = build_from_history(141, &history, "p1");

        assert_eq!(build.games, 3);
        assert_eq!(build.wins, 2);
        assert_eq!(build.position.as_deref(), Some("JUNGLE"));
        assert!(
            build
                .core_items
                .iter()
                .any(|item| item.item_id == 3153 && item.count == 2)
        );
        assert_eq!(build.boots[0].item_id, 3006);
        assert!(
            build
                .core_items
                .iter()
                .all(|item| !BOOTS.contains(&item.item_id))
        );
    }

    #[test]
    fn session_only_counts_today() {
        let now = chrono::Utc::now().timestamp_millis();
        let history = json!({
            "games": {
                "games": [
                    history_game(1, 141, true, &[3153], now),
                    history_game(2, 141, false, &[3153], now - 2 * 86_400_000)
                ]
            }
        });

        let session = session_from_history(&history, "p1", now / 1_000 - 3_600);

        assert_eq!(session.games, 1);
        assert_eq!(session.wins, 1);
        assert_eq!(session.losses, 0);
        assert_eq!(session.streak, 1);
    }

    #[test]
    fn classifies_items_for_builds() {
        assert!(is_completed_item(3153));
        assert!(!is_completed_item(3006));
        assert!(!is_completed_item(3340));
        assert!(!is_completed_item(1055));
    }
}
