mod commands;
mod config;
mod db;
mod lcu;
mod models;
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
        .setup(|app| {
            let state = AppState::new(app.handle())?;
            app.manage(state);
            tray::setup(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                // Kapatma isteğini tepsiye küçültmeye çevir; çıkış tepsi menüsünden yapılır.
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::lcu_status,
            commands::champ_select,
            commands::recent_matches,
            commands::account_overview,
            commands::lobby_view,
            commands::get_settings,
            commands::save_settings,
            commands::clear_cache,
        ])
        .run(tauri::generate_context!())
        .expect("Tlas başlatılamadı");
}
