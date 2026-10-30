use std::path::Path;

use parking_lot::Mutex;
use rusqlite::{params, Connection};

pub struct Database {
    conn: Mutex<Connection>,
}

pub struct LpSnapshot {
    pub ts: i64,
    pub tier: String,
    pub division: Option<String>,
    pub lp: i64,
    pub wins: i64,
    pub losses: i64,
}

impl Database {
    pub fn open(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA synchronous=NORMAL;
             PRAGMA temp_store=MEMORY;",
        )?;

        let version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;

        // v1: temel tablolar. Eski kurulumlarda da güvenle çalışsın diye
        // CREATE IF NOT EXISTS kullanılır.
        if version < 1 {
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS kv (
                     key TEXT PRIMARY KEY,
                     value TEXT NOT NULL,
                     updated_at INTEGER NOT NULL
                 );
                 CREATE TABLE IF NOT EXISTS lp_history (
                     puuid TEXT NOT NULL,
                     queue TEXT NOT NULL,
                     ts INTEGER NOT NULL,
                     tier TEXT NOT NULL,
                     division TEXT,
                     lp INTEGER NOT NULL,
                     wins INTEGER NOT NULL,
                     losses INTEGER NOT NULL,
                     PRIMARY KEY (puuid, queue, ts)
                 );
                 CREATE INDEX IF NOT EXISTS idx_lp_history_lookup
                     ON lp_history (puuid, queue, ts DESC);
                 PRAGMA user_version = 1;",
            )?;
        }

        // v2: LP sorgularında kullanılan ikincil indeks.
        if version < 2 {
            conn.execute_batch(
                "CREATE INDEX IF NOT EXISTS idx_lp_history_lp
                     ON lp_history (puuid, queue, lp);
                 PRAGMA user_version = 2;",
            )?;
        }

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

    pub fn clear(&self) -> anyhow::Result<()> {
        let conn = self.conn.lock();
        conn.execute_batch("DELETE FROM kv;")?;
        Ok(())
    }

    pub fn lp_last(&self, puuid: &str, queue: &str) -> Option<LpSnapshot> {
        let conn = self.conn.lock();
        let mut stmt = conn
            .prepare(
                "SELECT ts, tier, division, lp, wins, losses FROM lp_history
                 WHERE puuid = ?1 AND queue = ?2 ORDER BY ts DESC LIMIT 1",
            )
            .ok()?;

        stmt.query_row(params![puuid, queue], |row| {
            Ok(LpSnapshot {
                ts: row.get(0)?,
                tier: row.get(1)?,
                division: row.get(2)?,
                lp: row.get(3)?,
                wins: row.get(4)?,
                losses: row.get(5)?,
            })
        })
        .ok()
    }

    pub fn lp_push(&self, puuid: &str, queue: &str, snapshot: &LpSnapshot) -> anyhow::Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT OR REPLACE INTO lp_history (puuid, queue, ts, tier, division, lp, wins, losses)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                puuid,
                queue,
                snapshot.ts,
                snapshot.tier,
                snapshot.division,
                snapshot.lp,
                snapshot.wins,
                snapshot.losses
            ],
        )?;

        // Sonsuz büyümeyi engelle: puuid/queue başına son 200 kayıt kalsın.
        conn.execute(
            "DELETE FROM lp_history
             WHERE puuid = ?1 AND queue = ?2 AND ts NOT IN (
                 SELECT ts FROM lp_history WHERE puuid = ?1 AND queue = ?2
                 ORDER BY ts DESC LIMIT 200
             )",
            params![puuid, queue],
        )?;

        Ok(())
    }

    pub fn lp_recent(&self, puuid: &str, queue: &str, limit: u32) -> Vec<LpSnapshot> {
        let conn = self.conn.lock();
        let Ok(mut stmt) = conn.prepare(
            "SELECT ts, tier, division, lp, wins, losses FROM lp_history
             WHERE puuid = ?1 AND queue = ?2 ORDER BY ts DESC LIMIT ?3",
        ) else {
            return Vec::new();
        };

        let rows = stmt.query_map(params![puuid, queue, limit], |row| {
            Ok(LpSnapshot {
                ts: row.get(0)?,
                tier: row.get(1)?,
                division: row.get(2)?,
                lp: row.get(3)?,
                wins: row.get(4)?,
                losses: row.get(5)?,
            })
        });

        rows.map(|iter| iter.filter_map(Result::ok).collect())
            .unwrap_or_default()
    }
}
