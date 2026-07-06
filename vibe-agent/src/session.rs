use anyhow::Result;
use async_trait::async_trait;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::provider::Message;

/// ProjectTree 占位 — Phase B 落地
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectTree {
    pub version: u64,
    pub nodes: Vec<TreeNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeNode {
    pub id: String,
    pub parent_id: Option<String>,
    pub title: String,
    pub kind: String,
    pub status: String,
    pub priority: u8,
    pub owner: String,
    pub human_note: Option<String>,
    pub target_files: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Session {
    pub id: String,
    pub provider: String,
    pub model: String,
    pub messages: Vec<Message>,
    pub step: u64,
    pub tokens_in: u64,
    pub tokens_out: u64,
    pub project_tree: Option<ProjectTree>,
    pub tree_version: u64,
    pub created_at: String,
    pub updated_at: String,
    pub metadata: HashMap<String, String>,
}

/// 会话存储 trait —— 现在 SQLite, 未来 PG / JSONL 只换 impl
#[async_trait]
pub trait SessionStore: Send + Sync {
    async fn load(&self, id: &str) -> Result<Option<Session>>;
    async fn save(&self, session: &Session) -> Result<()>;
    async fn list(&self) -> Result<Vec<String>>;
    async fn latest(&self) -> Result<Option<String>>;
}

// ── SQLite 实现 ──

pub struct SqliteStore {
    conn: Mutex<Connection>,
}

impl SqliteStore {
    pub fn new(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() { std::fs::create_dir_all(parent)?; }
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS session(
                id TEXT PRIMARY KEY, provider TEXT, model TEXT,
                messages_json TEXT, step INTEGER DEFAULT 0,
                tokens_in INTEGER DEFAULT 0, tokens_out INTEGER DEFAULT 0,
                tree_json TEXT, tree_version INTEGER DEFAULT 0,
                metadata_json TEXT, created_at TEXT, updated_at TEXT
            )"
        )?;
        Ok(Self { conn: Mutex::new(conn) })
    }
}

#[async_trait]
impl SessionStore for SqliteStore {
    async fn load(&self, id: &str) -> Result<Option<Session>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id, provider, model, messages_json, step, tokens_in, tokens_out, tree_json, tree_version, metadata_json, created_at, updated_at FROM session WHERE id=?1")?;
        let mut rows = stmt.query_map(rusqlite::params![id], |row| {
            Ok((
                row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?, row.get::<_, i64>(4)?,
                row.get::<_, i64>(5)?, row.get::<_, i64>(6)?,
                row.get::<_, Option<String>>(7)?,
                row.get::<_, i64>(8)?, row.get::<_, Option<String>>(9)?,
                row.get::<_, Option<String>>(10)?, row.get::<_, Option<String>>(11)?,
            ))
        })?;
        if let Some(row) = rows.next() {
            let (id, provider, model, messages_json, step, tokens_in, tokens_out, tree_json, tree_version, metadata_json, created_at, updated_at) = row?;
            let messages = messages_json.as_deref()
                .and_then(|s| serde_json::from_str(s).ok()).unwrap_or_default();
            let project_tree = tree_json.as_deref()
                .and_then(|s| serde_json::from_str(s).ok());
            let metadata = metadata_json.as_deref()
                .and_then(|s| serde_json::from_str(s).ok()).unwrap_or_default();
            Ok(Some(Session {
                id, provider, model, messages,
                step: step as u64, tokens_in: tokens_in as u64, tokens_out: tokens_out as u64,
                project_tree, tree_version: tree_version as u64,
                metadata, created_at: created_at.unwrap_or_default(),
                updated_at: updated_at.unwrap_or_default(),
            }))
        } else { Ok(None) }
    }

    async fn save(&self, session: &Session) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let messages_json = serde_json::to_string(&session.messages)?;
        let tree_json = session.project_tree.as_ref().map(|t| serde_json::to_string(t).unwrap_or_default());
        let metadata_json = serde_json::to_string(&session.metadata).unwrap_or_default();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT OR REPLACE INTO session(id, provider, model, messages_json, step, tokens_in, tokens_out, tree_json, tree_version, metadata_json, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, COALESCE((SELECT created_at FROM session WHERE id=?1), ?11), ?11)",
            rusqlite::params![
                session.id, session.provider, session.model, messages_json,
                session.step as i64, session.tokens_in as i64, session.tokens_out as i64,
                tree_json, session.tree_version as i64, metadata_json, now
            ]
        )?;
        Ok(())
    }

    async fn list(&self) -> Result<Vec<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id FROM session ORDER BY updated_at DESC")?;
        let ids: Vec<String> = stmt.query_map([], |r| r.get(0))?.filter_map(|r| r.ok()).collect();
        Ok(ids)
    }

    async fn latest(&self) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let id: Option<String> = conn.query_row(
            "SELECT id FROM session ORDER BY updated_at DESC LIMIT 1", [], |r| r.get(0),
        ).ok();
        Ok(id)
    }
}