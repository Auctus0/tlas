use std::path::Path;

use parking_lot::Mutex;
use rusqlite::{params, Connection};

pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    pub fn open(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA synchronous=NORMAL;
             PRAGMA temp_store=MEMORY;
             CREATE TABLE IF NOT EXISTS kv (
                 key TEXT PRIMARY KEY,
                 value TEXT NOT NULL,
                 updated_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS match_cache (
                 match_id TEXT PRIMARY KEY,
                 json TEXT NOT NULL,
                 updated_at INTEGER NOT NULL
             );",
        )?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn kv_get(&self, key: &str, max_age_secs: i64) -> Option<String> {
        let conn = self.conn.lock();
        let mut stmt = conn
            .prepare("SELECT value, updated_at FROM kv WHERE key = ?1")
            .ok()?;
        let (value, updated_at): (String, i64) = stmt
            .query_row(params![key], |row| Ok((row.get(0)?, row.get(1)?)))
            .ok()?;

        let age = chrono::Utc::now().timestamp() - updated_at;
        (age <= max_age_secs).then_some(value)
    }

    pub fn kv_set(&self, key: &str, value: &str) -> anyhow::Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO kv (key, value, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
            params![key, value, chrono::Utc::now().timestamp()],
        )?;
        Ok(())
    }

    pub fn match_get(&self, match_id: &str, max_age_secs: i64) -> Option<String> {
        let conn = self.conn.lock();
        let mut stmt = conn
            .prepare("SELECT json, updated_at FROM match_cache WHERE match_id = ?1")
            .ok()?;
        let (json, updated_at): (String, i64) = stmt
            .query_row(params![match_id], |row| Ok((row.get(0)?, row.get(1)?)))
            .ok()?;

        let age = chrono::Utc::now().timestamp() - updated_at;
        (age <= max_age_secs).then_some(json)
    }

    pub fn match_set(&self, match_id: &str, json: &str) -> anyhow::Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO match_cache (match_id, json, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(match_id) DO UPDATE SET json = excluded.json, updated_at = excluded.updated_at",
            params![match_id, json, chrono::Utc::now().timestamp()],
        )?;
        Ok(())
    }

    pub fn clear(&self) -> anyhow::Result<()> {
        let conn = self.conn.lock();
        conn.execute_batch("DELETE FROM kv; DELETE FROM match_cache;")?;
        Ok(())
    }
}
