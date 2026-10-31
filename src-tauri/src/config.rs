use std::path::PathBuf;

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub auto_update: bool,
    /// Varsayılan kapalı. Açılırsa şampiyon meta verisi OP.GG MCP'den çekilir.
    pub experimental_meta: bool,
    /// Şampiyon seçimi ve aktif maç sırasında rakip kaplamasını göster.
    pub overlay_enabled: bool,
    pub auto_accept: bool,
    pub auto_pick_champion_id: Option<i64>,
    pub auto_ban_champion_id: Option<i64>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            auto_update: true,
            experimental_meta: false,
            overlay_enabled: true,
            auto_accept: false,
            auto_pick_champion_id: None,
            auto_ban_champion_id: None,
        }
    }
}

pub struct ConfigStore {
    path: PathBuf,
    inner: Mutex<Settings>,
}

impl ConfigStore {
    pub fn new(path: PathBuf) -> anyhow::Result<Self> {
        let settings = if path.exists() {
            serde_json::from_str::<Settings>(&std::fs::read_to_string(&path)?).unwrap_or_default()
        } else {
            Settings::default()
        };

        Ok(Self {
            path,
            inner: Mutex::new(settings),
        })
    }

    pub fn get(&self) -> Settings {
        self.inner.lock().clone()
    }

    pub fn set(&self, settings: Settings) -> anyhow::Result<()> {
        let json = serde_json::to_string_pretty(&settings)?;
        std::fs::write(&self.path, json)?;
        *self.inner.lock() = settings;
        Ok(())
    }
}
