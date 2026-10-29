use tauri::Manager;

use crate::config::ConfigStore;
use crate::db::Database;
use crate::lcu::Lcu;

pub struct AppState {
    pub lcu: Lcu,
    pub config: ConfigStore,
    pub db: Database,
}

impl AppState {
    pub fn new(app: &tauri::AppHandle) -> anyhow::Result<Self> {
        let data_dir = app.path().app_data_dir()?;
        std::fs::create_dir_all(&data_dir)?;

        Ok(Self {
            lcu: Lcu::new(),
            config: ConfigStore::new(data_dir.join("settings.json"))?,
            db: Database::open(data_dir.join("tlas.db"))?,
        })
    }
}
