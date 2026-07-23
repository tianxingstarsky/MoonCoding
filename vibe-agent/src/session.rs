use anyhow::Result;
use async_trait::async_trait;
use rusqlite::{Connection, OptionalExtension};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use crate::provider::Message;
pub use crate::tree::{ProjectTree, TreeNode};

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

impl Session {
    pub fn new(id: String, model: String, provider: String) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            id,
            provider,
            model,
            messages: Vec::new(),
            step: 0,
            tokens_in: 0,
            tokens_out: 0,
            project_tree: Some(ProjectTree::default()),
            tree_version: 0,
            created_at: now.clone(),
            updated_at: now,
            metadata: HashMap::new(),
        }
    }
}

/// 会话存储 trait —— 现在 SQLite, 未来 PG / JSONL 只换 impl
#[async_trait]
pub trait SessionStore: Send + Sync {
    async fn load(&self, id: &str) -> Result<Option<Session>>;
    async fn save(&self, session: &Session) -> Result<()>;
    async fn save_tree_cas(&self, session: &Session, expected_version: u64) -> Result<bool>;
    async fn list(&self) -> Result<Vec<String>>;
    async fn latest(&self) -> Result<Option<String>>;
}

// ── SQLite 实现 ──

pub struct SqliteStore {
    conn: Mutex<Connection>,
}

impl SqliteStore {
    pub fn new(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS session(
                id TEXT PRIMARY KEY, provider TEXT, model TEXT,
                messages_json TEXT, step INTEGER DEFAULT 0,
                tokens_in INTEGER DEFAULT 0, tokens_out INTEGER DEFAULT 0,
                tree_json TEXT, tree_version INTEGER DEFAULT 0,
                metadata_json TEXT, created_at TEXT, updated_at TEXT
            )",
        )?;
        migrate_session_schema(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }
}

fn migrate_session_schema(conn: &Connection) -> Result<()> {
    let required_columns = [
        ("provider", "TEXT NOT NULL DEFAULT ''"),
        ("model", "TEXT NOT NULL DEFAULT ''"),
        ("messages_json", "TEXT"),
        ("step", "INTEGER NOT NULL DEFAULT 0"),
        ("tokens_in", "INTEGER NOT NULL DEFAULT 0"),
        ("tokens_out", "INTEGER NOT NULL DEFAULT 0"),
        ("tree_json", "TEXT"),
        ("tree_version", "INTEGER NOT NULL DEFAULT 0"),
        ("metadata_json", "TEXT"),
        ("created_at", "TEXT"),
        ("updated_at", "TEXT"),
    ];
    let mut statement = conn.prepare("PRAGMA table_info(session)")?;
    let existing: std::collections::HashSet<String> = statement
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<rusqlite::Result<_>>()?;
    drop(statement);
    for (name, definition) in required_columns {
        if !existing.contains(name) {
            conn.execute(
                &format!("ALTER TABLE session ADD COLUMN {name} {definition}"),
                [],
            )?;
        }
    }
    conn.pragma_update(None, "user_version", 2)?;
    Ok(())
}

#[async_trait]
impl SessionStore for SqliteStore {
    async fn load(&self, id: &str) -> Result<Option<Session>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("session database lock poisoned"))?;
        let mut stmt = conn.prepare("SELECT id, provider, model, messages_json, step, tokens_in, tokens_out, tree_json, tree_version, metadata_json, created_at, updated_at FROM session WHERE id=?1")?;
        let mut rows = stmt.query_map(rusqlite::params![id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, i64>(6)?,
                row.get::<_, Option<String>>(7)?,
                row.get::<_, i64>(8)?,
                row.get::<_, Option<String>>(9)?,
                row.get::<_, Option<String>>(10)?,
                row.get::<_, Option<String>>(11)?,
            ))
        })?;
        if let Some(row) = rows.next() {
            let (
                id,
                provider,
                model,
                messages_json,
                step,
                tokens_in,
                tokens_out,
                tree_json,
                tree_version,
                metadata_json,
                created_at,
                updated_at,
            ) = row?;
            let messages = match messages_json.as_deref() {
                Some(json) => serde_json::from_str(json)?,
                None => Vec::new(),
            };
            let project_tree = match tree_json.as_deref() {
                Some(json) => Some(serde_json::from_str(json)?),
                None => None,
            };
            if project_tree
                .as_ref()
                .is_some_and(|tree: &ProjectTree| tree.version != tree_version as u64)
            {
                return Err(anyhow::anyhow!(
                    "session tree version mismatch: column={}, payload={}",
                    tree_version,
                    project_tree
                        .as_ref()
                        .map(|tree| tree.version)
                        .unwrap_or_default()
                ));
            }
            let metadata = match metadata_json.as_deref() {
                Some(json) => serde_json::from_str(json)?,
                None => HashMap::new(),
            };
            Ok(Some(Session {
                id,
                provider,
                model,
                messages,
                step: step as u64,
                tokens_in: tokens_in as u64,
                tokens_out: tokens_out as u64,
                project_tree,
                tree_version: tree_version as u64,
                metadata,
                created_at: created_at.unwrap_or_default(),
                updated_at: updated_at.unwrap_or_default(),
            }))
        } else {
            Ok(None)
        }
    }

    async fn save(&self, session: &Session) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("session database lock poisoned"))?;
        let messages_json = serde_json::to_string(&session.messages)?;
        let tree_json = session
            .project_tree
            .as_ref()
            .map(serde_json::to_string)
            .transpose()?;
        let metadata_json = serde_json::to_string(&session.metadata)?;
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO session(
                id, provider, model, messages_json, step, tokens_in, tokens_out,
                tree_json, tree_version, metadata_json, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11)
             ON CONFLICT(id) DO UPDATE SET
                provider=excluded.provider,
                model=excluded.model,
                messages_json=excluded.messages_json,
                step=excluded.step,
                tokens_in=excluded.tokens_in,
                tokens_out=excluded.tokens_out,
                metadata_json=excluded.metadata_json,
                updated_at=excluded.updated_at",
            rusqlite::params![
                session.id,
                session.provider,
                session.model,
                messages_json,
                session.step as i64,
                session.tokens_in as i64,
                session.tokens_out as i64,
                tree_json,
                session.tree_version as i64,
                metadata_json,
                now
            ],
        )?;
        Ok(())
    }

    async fn save_tree_cas(&self, session: &Session, expected_version: u64) -> Result<bool> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("session database lock poisoned"))?;
        let transaction = conn.transaction()?;
        let current_version: Option<i64> = transaction
            .query_row(
                "SELECT tree_version FROM session WHERE id=?1",
                rusqlite::params![session.id],
                |row| row.get(0),
            )
            .optional()?;
        if current_version
            .map(|version| version as u64)
            .unwrap_or_default()
            != expected_version
            || (current_version.is_none() && expected_version != 0)
        {
            return Ok(false);
        }

        let tree_json = session
            .project_tree
            .as_ref()
            .map(serde_json::to_string)
            .transpose()?;
        let now = chrono::Utc::now().to_rfc3339();
        if current_version.is_some() {
            let changed = transaction.execute(
                "UPDATE session
                 SET tree_json=?2, tree_version=?3, updated_at=?4
                 WHERE id=?1 AND tree_version=?5",
                rusqlite::params![
                    session.id,
                    tree_json,
                    session.tree_version as i64,
                    now,
                    expected_version as i64
                ],
            )?;
            if changed != 1 {
                return Ok(false);
            }
        } else {
            let messages_json = serde_json::to_string(&session.messages)?;
            let metadata_json = serde_json::to_string(&session.metadata)?;
            transaction.execute(
                "INSERT INTO session(
                    id, provider, model, messages_json, step, tokens_in, tokens_out,
                    tree_json, tree_version, metadata_json, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11)",
                rusqlite::params![
                    session.id,
                    session.provider,
                    session.model,
                    messages_json,
                    session.step as i64,
                    session.tokens_in as i64,
                    session.tokens_out as i64,
                    tree_json,
                    session.tree_version as i64,
                    metadata_json,
                    now
                ],
            )?;
        }
        transaction.commit()?;
        Ok(true)
    }

    async fn list(&self) -> Result<Vec<String>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("session database lock poisoned"))?;
        let mut stmt = conn.prepare("SELECT id FROM session ORDER BY updated_at DESC")?;
        let ids = stmt
            .query_map([], |row| row.get(0))?
            .collect::<rusqlite::Result<Vec<String>>>()?;
        Ok(ids)
    }

    async fn latest(&self) -> Result<Option<String>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("session database lock poisoned"))?;
        let id = conn
            .query_row(
                "SELECT id FROM session ORDER BY updated_at DESC LIMIT 1",
                [],
                |r| r.get(0),
            )
            .optional()?;
        Ok(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrates_legacy_session_table_idempotently() -> Result<()> {
        let root = std::env::temp_dir().join(format!(
            "mooncoding-session-migration-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root)?;
        let path = root.join("sessions.db");
        {
            let connection = Connection::open(&path)?;
            connection.execute_batch(
                "CREATE TABLE session(
                    id TEXT PRIMARY KEY,
                    provider TEXT NOT NULL,
                    model TEXT NOT NULL,
                    messages_json TEXT
                );",
            )?;
        }

        let store = SqliteStore::new(&path)?;
        {
            let conn = store
                .conn
                .lock()
                .map_err(|_| anyhow::anyhow!("session database lock poisoned"))?;
            migrate_session_schema(&conn)?;
        }
        let connection = store
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("session database lock poisoned"))?;
        let mut statement = connection.prepare("PRAGMA table_info(session)")?;
        let columns = statement
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<rusqlite::Result<std::collections::HashSet<_>>>()?;
        assert!(columns.contains("tree_json"));
        assert!(columns.contains("tree_version"));
        assert!(columns.contains("metadata_json"));
        drop(statement);
        drop(connection);
        drop(store);
        std::fs::remove_dir_all(root)?;
        Ok(())
    }
}
