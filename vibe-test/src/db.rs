use anyhow::Result;
use rusqlite::Connection;
use std::path::Path;
use std::sync::Mutex;

pub struct Db {
    pub conn: Mutex<Connection>,
}

pub fn init(path: &Path) -> Result<Db> {
    let conn = Connection::open(path)?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS session(
            id TEXT PRIMARY KEY, spec TEXT, directory TEXT, model TEXT,
            tokens_in INTEGER DEFAULT 0, tokens_out INTEGER DEFAULT 0,
            tokens_total INTEGER DEFAULT 0, baseline_total INTEGER DEFAULT 0,
            steps INTEGER DEFAULT 0, status TEXT,
            fileset_count INTEGER DEFAULT 0, block_count INTEGER DEFAULT 0,
            purpose_drift_warns INTEGER DEFAULT 0, cross_block_warns INTEGER DEFAULT 0,
            verify_failures INTEGER DEFAULT 0, assertions_json TEXT,
            created_at TEXT, ended_at TEXT
        );
        CREATE TABLE IF NOT EXISTS message(
            id TEXT PRIMARY KEY, session_id TEXT, seq INTEGER, role TEXT,
            content_json TEXT, tokens_in INTEGER DEFAULT 0, tokens_out INTEGER DEFAULT 0
        );
        CREATE TABLE IF NOT EXISTS tool_call(
            id TEXT PRIMARY KEY, session_id TEXT, message_id TEXT, step INTEGER,
            command TEXT, exit_code INTEGER, output TEXT, truncated INTEGER DEFAULT 0,
            duration_ms INTEGER
        );
        CREATE INDEX IF NOT EXISTS idx_message_session ON message(session_id, seq);
        CREATE INDEX IF NOT EXISTS idx_tool_session ON tool_call(session_id, step);"
    )?;
    Ok(Db { conn: Mutex::new(conn) })
}

pub fn latest_session(path: &Path) -> Result<String> {
    let conn = Connection::open(path)?;
    let id: Option<String> = conn.query_row(
        "SELECT id FROM session ORDER BY created_at DESC LIMIT 1",
        [], |r| r.get(0)
    ).ok();
    id.ok_or_else(|| anyhow::anyhow!("no session rows"))
}

pub fn insert_session(db: &Db, id: &str, spec: &str, directory: &str, model: &str, created_at: &str) -> Result<()> {
    let c = db.conn.lock().unwrap();
    c.execute(
        "INSERT INTO session(id, spec, directory, model, status, created_at)
         VALUES (?1, ?2, ?3, ?4, 'running', ?5)",
        rusqlite::params![id, spec, directory, model, created_at]
    )?;
    Ok(())
}

pub fn finalize_session(
    db: &Db, id: &str,
    tokens_in: u64, tokens_out: u64, tokens_total: u64, baseline_total: u64,
    steps: u64, status: &str,
    fileset_count: u64, block_count: u64,
    purpose_drift_warns: u64, cross_block_warns: u64, verify_failures: u64,
    assertions_json: &str, ended_at: &str,
) -> Result<()> {
    let c = db.conn.lock().unwrap();
    c.execute(
        "UPDATE session SET
            tokens_in=?2, tokens_out=?3, tokens_total=?4, baseline_total=?5,
            steps=?6, status=?7, fileset_count=?8, block_count=?9,
            purpose_drift_warns=?10, cross_block_warns=?11, verify_failures=?12,
            assertions_json=?13, ended_at=?14
         WHERE id=?1",
        rusqlite::params![id, tokens_in, tokens_out, tokens_total, baseline_total,
            steps, status, fileset_count, block_count,
            purpose_drift_warns, cross_block_warns, verify_failures,
            assertions_json, ended_at]
    )?;
    Ok(())
}

pub fn insert_message(db: &Db, id: &str, session_id: &str, seq: u64, role: &str, content_json: &str, tokens_in: u64, tokens_out: u64) -> Result<()> {
    let c = db.conn.lock().unwrap();
    c.execute(
        "INSERT INTO message(id, session_id, seq, role, content_json, tokens_in, tokens_out)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![id, session_id, seq, role, content_json, tokens_in, tokens_out]
    )?;
    Ok(())
}

pub fn insert_tool_call(db: &Db, id: &str, session_id: &str, message_id: &str, step: u64,
    command: &str, exit_code: i32, output: &str, truncated: bool, duration_ms: u64) -> Result<()> {
    let c = db.conn.lock().unwrap();
    c.execute(
        "INSERT INTO tool_call(id, session_id, message_id, step, command, exit_code, output, truncated, duration_ms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        rusqlite::params![id, session_id, message_id, step, command, exit_code, output, if truncated {1} else {0}, duration_ms]
    )?;
    Ok(())
}

pub struct SessionRow {
    #[allow(dead_code)]
    pub id: String, #[allow(dead_code)] pub spec: String, pub status: String,
    pub tokens_in: u64, pub tokens_out: u64, pub tokens_total: u64, pub baseline_total: u64,
    pub steps: u64, pub fileset_count: u64, pub block_count: u64,
    pub purpose_drift_warns: u64, pub cross_block_warns: u64, pub verify_failures: u64,
    pub assertions_json: String,
    #[allow(dead_code)] pub created_at: String, #[allow(dead_code)] pub ended_at: String,
}

pub fn load_session(path: &Path, id: &str) -> Result<SessionRow> {
    let conn = Connection::open(path)?;
    conn.query_row(
        "SELECT id, spec, status, tokens_in, tokens_out, tokens_total, baseline_total,
                steps, fileset_count, block_count, purpose_drift_warns, cross_block_warns,
                verify_failures, assertions_json, created_at, ended_at
         FROM session WHERE id=?1",
        rusqlite::params![id],
        |r| Ok(SessionRow {
            id: r.get(0)?, spec: r.get(1)?, status: r.get(2)?,
            tokens_in: r.get(3)?, tokens_out: r.get(4)?, tokens_total: r.get(5)?, baseline_total: r.get(6)?,
            steps: r.get(7)?, fileset_count: r.get(8)?, block_count: r.get(9)?,
            purpose_drift_warns: r.get(10)?, cross_block_warns: r.get(11)?, verify_failures: r.get(12)?,
            assertions_json: r.get::<_, Option<String>>(13)?.unwrap_or_default(),
            created_at: r.get::<_, Option<String>>(14)?.unwrap_or_default(),
            ended_at: r.get::<_, Option<String>>(15)?.unwrap_or_default(),
        })
    ).map_err(|e| anyhow::anyhow!("load session: {e}"))
}