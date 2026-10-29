use std::collections::HashMap;

use futures::future::join_all;
use futures::stream::StreamExt;
use serde_json::Value;
use tauri::State;

use crate::config::Settings;
use crate::db::LpSnapshot;
use crate::models::{
    AccountOverview, ChampionStat, ChampSelectView, LcuStatus, LobbyMember, LobbyView, LpPoint,
    MatchSummary, PlayerCard, RankView, ScoutCache, SummonerInfo,
};
use crate::state::AppState;

/// Scout verisi 10 dakika taze kalır; derece ve son maçlar bu sürede değişmez.
const SCOUT_TTL_SECS: i64 = 600;
/// Kart başına son maç penceresi.
const SCOUT_MATCH_COUNT: u32 = 20;

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

    let (allies, enemies) = tokio::join!(
        build_team(app, &session, "myTeam", true),
        build_team(app, &session, "theirTeam", false),
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
            scout.tier = entry.get("tier").and_then(|value| value.as_str()).map(String::from);
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
            let queue_id = game.get("queueId").and_then(|value| value.as_i64()).unwrap_or(0);
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
    let history = app
        .lcu
        .match_history(&puuid, 0, limit)
        .await
        .map_err(|error| error.to_string())?;

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
                let role = match app.lcu.game_detail(game_id).await {
                    Ok(detail) => lcu_participant(&detail, &puuid)
                        .and_then(|participant| role_from_timeline(participant.get("timeline"))),
                    Err(_) => None,
                };

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

/// Hesap özeti için taranan maç sayısı.
const OVERVIEW_MATCH_COUNT: u32 = 50;
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
        level: summoner.get("summonerLevel").and_then(|value| value.as_i64()),
        icon_id: summoner
            .get("profileIconId")
            .and_then(|value| value.as_i64()),
    };

    let stats = app.lcu.ranked_stats(&puuid).await.ok();

    let solo_entry = stats.as_ref().and_then(|stats| {
        queue_entry(
            stats,
            &["RANKED_SOLO_5x5", "JADE_RANKED_SOLO_5x5"],
        )
    });
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

    if let Ok(history) = app.lcu.match_history(&puuid, 0, OVERVIEW_MATCH_COUNT).await {
        let mastery = mastery_points(app, &puuid).await;
        champions = champion_stats(&history, &puuid, &mastery);

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
            } else if let Some(value) = member.get("summonerName").and_then(|value| value.as_str()) {
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
    let wins = entry.get("wins").and_then(|value| value.as_i64()).unwrap_or(0);
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
        lp: entry
            .get("leaguePoints")
            .and_then(|value| value.as_i64()),
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
    let wins = entry.get("wins").and_then(|value| value.as_i64()).unwrap_or(0);
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

fn champion_stats(
    history: &Value,
    puuid: &str,
    mastery: &HashMap<i64, i64>,
) -> Vec<ChampionStat> {
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
            kda: round2(
                (entry.kills + entry.assists) as f64 / entry.deaths.max(1) as f64,
            ),
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
            .then_with(|| {
                right
                    .points
                    .unwrap_or(0)
                    .cmp(&left.points.unwrap_or(0))
            })
            .then_with(|| left.champion_id.cmp(&right.champion_id))
    });
    champions.truncate(6);

    champions
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

    if *first {
        count
    } else {
        -count
    }
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
        assert_eq!(role_from_timeline(Some(&timeline)).as_deref(), Some("UTILITY"));

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
    fn formats_rank_titles() {
        assert_eq!(title_case("GRANDMASTER"), "Grandmaster");
        assert_eq!(title_case(""), "");
        assert_eq!(tier_name("PLATINUM"), "Platin");
        assert_eq!(tier_name("EMERALD"), "Zümrüt");
        assert_eq!(tier_name("UNKNOWN"), "UNKNOWN");
    }
}
