use std::path::PathBuf;

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub riot_api_key: String,
    pub platform: String,
    pub auto_update: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            riot_api_key: String::new(),
            platform: "tr1".to_string(),
            auto_update: true,
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
