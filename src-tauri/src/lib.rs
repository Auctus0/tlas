mod commands;
mod config;
mod db;
mod lcu;
mod models;
mod opgg;
mod overlay;
mod state;
mod tray;

use tauri::Manager;

use state::AppState;

fn focus_main(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Tek örnek: ikinci kez açılırsa mevcut pencereyi öne getir.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            focus_main(app);
        }))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let state = AppState::new(app.handle())?;
            app.manage(state);
            tray::setup(app)?;

            // Şampiyon seçimi kaplaması: ayrı, çerçevesiz, odak çalmayan pencere.
            // Oyun içine injection yapılmaz.
            tauri::WebviewWindowBuilder::new(
                app,
                "overlay",
                tauri::WebviewUrl::App("overlay".into()),
            )
            .title("Tlas kaplama")
            .inner_size(360.0, 400.0)
            .resizable(false)
            .decorations(false)
            .shadow(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .focusable(false)
            .visible(false)
            .build()?;

            overlay::spawn_phase_watch(app.handle().clone());
            spawn_lcu_automation(app.handle().clone());

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                // Kapatma isteğini tepsiye küçültmeye çevir; çıkış tepsi menüsünden yapılır.
                api.prevent_close();
                let _ = window.hide();
                // Kaplama bilinçli olarak gizlenmez: faz izleyicisi onu şampiyon
                // seçimi/aktif maç boyunca yönetmeye devam eder.
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::lcu_status,
            commands::champ_select,
            commands::draft_insights,
            commands::recent_matches,
            commands::match_detail,
            commands::personal_build,
            commands::live_game,
            commands::account_overview,
            commands::lobby_view,
            commands::opgg_meta,
            commands::get_settings,
            commands::save_settings,
            commands::clear_cache,
            overlay::set_overlay_visible,
            overlay::place_overlay,
        ])
        .run(tauri::generate_context!())
        .expect("Tlas başlatılamadı");
}

fn jitter_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| (d.subsec_nanos() % 1001) as u64)
        .unwrap_or(500)
}

/// Hata sonrası bekleme: 2, 4, 8, 16, 30 saniye.
fn backoff_delay(failures: u32) -> std::time::Duration {
    let secs = 1u64 << failures.clamp(1, 4);
    std::time::Duration::from_secs(secs.min(30))
}

/// Kendi sırası gelmiş (isInProgress), tamamlanmamış pick/ban aksiyonunu seçer.
fn next_auto_action(
    session: &serde_json::Value,
    cell_id: i64,
    settings: &crate::config::Settings,
) -> Option<(i64, i64)> {
    let actions = session.get("actions")?.as_array()?;

    for group in actions {
        let Some(items) = group.as_array() else {
            continue;
        };

        for action in items {
            if action.get("actorCellId").and_then(|value| value.as_i64()) != Some(cell_id) {
                continue;
            }
            if action.get("completed").and_then(|value| value.as_bool()) != Some(false) {
                continue;
            }
            if action.get("isInProgress").and_then(|value| value.as_bool()) != Some(true) {
                continue;
            }

            let champion_id = match action.get("type").and_then(|value| value.as_str()) {
                Some("pick") => settings.auto_pick_champion_id,
                Some("ban") => settings.auto_ban_champion_id,
                _ => None,
            };
            let action_id = action.get("id").and_then(|value| value.as_i64());

            if let (Some(champion_id), Some(action_id)) = (champion_id, action_id) {
                return Some((action_id, champion_id));
            }
        }
    }

    None
}

fn spawn_lcu_automation(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut last_ready_attempt = false;
        let mut failures: u32 = 0;
        let mut retry_at: Option<std::time::Instant> = None;

        loop {
            let state = app.state::<AppState>();
            let settings = state.config.get();
            let can_act = retry_at.is_none_or(|at| std::time::Instant::now() >= at);

            if let Ok(phase) = state.lcu.phase().await {
                if settings.auto_accept && phase == "ReadyCheck" && !last_ready_attempt {
                    last_ready_attempt = true;
                    tokio::time::sleep(std::time::Duration::from_millis(2000 + jitter_millis()))
                        .await;
                    if matches!(state.lcu.phase().await.as_deref(), Ok("ReadyCheck")) {
                        let _ = state.lcu.accept_ready_check().await;
                    }
                }
                if phase != "ReadyCheck" {
                    last_ready_attempt = false;
                }

                let champ_session = if phase == "ChampSelect" && can_act {
                    state.lcu.champ_select().await.ok().flatten()
                } else {
                    None
                };

                if let Some(session) = champ_session {
                    let cell_id = session
                        .get("localPlayerCellId")
                        .and_then(|value| value.as_i64());

                    if let Some((action_id, champion_id)) =
                        cell_id.and_then(|cell_id| next_auto_action(&session, cell_id, &settings))
                    {
                        tokio::time::sleep(std::time::Duration::from_millis(
                            2000 + jitter_millis(),
                        ))
                        .await;

                        // Bekleme sırasında faz değişmiş, aksiyon kapanmış veya
                        // kullanıcı otomasyonu kapatmış olabilir; tazele ve doğrula.
                        let still_wanted = match state.lcu.champ_select().await {
                            Ok(Some(session)) => {
                                session
                                    .get("localPlayerCellId")
                                    .and_then(|value| value.as_i64())
                                    .and_then(|cell_id| {
                                        next_auto_action(&session, cell_id, &state.config.get())
                                    })
                                    == Some((action_id, champion_id))
                            }
                            _ => false,
                        };

                        if still_wanted
                            && matches!(state.lcu.phase().await.as_deref(), Ok("ChampSelect"))
                        {
                            match state
                                .lcu
                                .complete_champ_action(action_id, champion_id)
                                .await
                            {
                                Ok(()) => {
                                    failures = 0;
                                    retry_at = None;
                                }
                                Err(_) => {
                                    failures = failures.saturating_add(1);
                                    retry_at =
                                        Some(std::time::Instant::now() + backoff_delay(failures));
                                }
                            }
                        }
                    }
                }
            }

            tokio::time::sleep(std::time::Duration::from_millis(800)).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::{backoff_delay, next_auto_action};
    use crate::config::Settings;

    fn settings(pick: Option<i64>, ban: Option<i64>) -> Settings {
        Settings {
            auto_pick_champion_id: pick,
            auto_ban_champion_id: ban,
            ..Settings::default()
        }
    }

    fn session(actions: serde_json::Value) -> serde_json::Value {
        serde_json::json!({
            "localPlayerCellId": 2,
            "actions": actions,
        })
    }

    #[test]
    fn picks_only_in_progress_action() {
        let session = session(serde_json::json!([[
            {"id": 10, "actorCellId": 2, "type": "pick", "completed": false, "isInProgress": false},
            {"id": 11, "actorCellId": 2, "type": "ban", "completed": false, "isInProgress": true},
        ]]));

        assert_eq!(
            next_auto_action(&session, 2, &settings(Some(103), Some(24))),
            Some((11, 24))
        );
    }

    #[test]
    fn ignores_unstarted_and_completed_actions() {
        let unstarted = session(serde_json::json!([[
            {"id": 10, "actorCellId": 2, "type": "pick", "completed": false, "isInProgress": false},
        ]]));
        let completed = session(serde_json::json!([[
            {"id": 11, "actorCellId": 2, "type": "pick", "completed": true, "isInProgress": true},
        ]]));

        assert_eq!(
            next_auto_action(&unstarted, 2, &settings(Some(103), None)),
            None
        );
        assert_eq!(
            next_auto_action(&completed, 2, &settings(Some(103), None)),
            None
        );
    }

    #[test]
    fn respects_other_player_and_missing_setting() {
        let other = session(serde_json::json!([[
            {"id": 10, "actorCellId": 4, "type": "pick", "completed": false, "isInProgress": true},
        ]]));
        let mine = session(serde_json::json!([[
            {"id": 11, "actorCellId": 2, "type": "pick", "completed": false, "isInProgress": true},
        ]]));

        assert_eq!(
            next_auto_action(&other, 2, &settings(Some(103), None)),
            None
        );
        assert_eq!(next_auto_action(&mine, 2, &settings(None, None)), None);
    }

    #[test]
    fn backoff_grows_and_caps() {
        assert_eq!(backoff_delay(1).as_secs(), 2);
        assert_eq!(backoff_delay(2).as_secs(), 4);
        assert_eq!(backoff_delay(3).as_secs(), 8);
        assert_eq!(backoff_delay(4).as_secs(), 16);
        assert_eq!(backoff_delay(5).as_secs(), 16);
        assert_eq!(backoff_delay(99).as_secs(), 16);
    }
}
