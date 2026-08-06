use crate::types::{CreateChatMessageRequest, CreateSessionRequest, ChatMessage, Session};
use chrono::Utc;
use rusqlite::{params, Connection, Row};
use uuid::Uuid;

// === sessions ===

pub fn create_session(conn: &Connection, req: CreateSessionRequest) -> Result<Session, rusqlite::Error> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO sessions (id, user_id, title, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, req.user_id, req.title, now, now],
    )?;
    log::info!(target: "db", "create_session id={} user_id={}", id, req.user_id);
    get_session(conn, &id)?.ok_or(rusqlite::Error::QueryReturnedNoRows)
}

pub fn list_sessions_by_user(conn: &Connection, user_id: &str) -> Result<Vec<Session>, rusqlite::Error> {
    let sql = SESSION_SELECT.to_owned() + " WHERE user_id = ?1 ORDER BY updated_at DESC";
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![user_id], map_session)?;
    rows.collect()
}

pub fn get_session(conn: &Connection, id: &str) -> Result<Option<Session>, rusqlite::Error> {
    let sql = SESSION_SELECT.to_owned() + " WHERE id = ?1";
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query(params![id])?;
    if let Some(row) = rows.next()? {
        Ok(Some(map_session(row)?))
    } else {
        Ok(None)
    }
}

pub fn update_session_title(conn: &Connection, id: &str, title: &str) -> Result<(), rusqlite::Error> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE sessions SET title = ?1, updated_at = ?2 WHERE id = ?3",
        params![title, now, id],
    )?;
    Ok(())
}

pub fn update_session_timestamp(conn: &Connection, id: &str) -> Result<(), rusqlite::Error> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE sessions SET updated_at = ?1 WHERE id = ?2",
        params![now, id],
    )?;
    Ok(())
}

pub fn delete_session(conn: &Connection, id: &str) -> Result<(), rusqlite::Error> {
    conn.execute("DELETE FROM sessions WHERE id = ?1", params![id])?;
    log::info!(target: "db", "delete_session id={}", id);
    Ok(())
}

// === chat_messages ===

pub fn create_message(conn: &Connection, req: CreateChatMessageRequest) -> Result<ChatMessage, rusqlite::Error> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO chat_messages (id, session_id, role, content, metadata_json, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![id, req.session_id, req.role, req.content, req.metadata_json, now],
    )?;
    // 同步更新 session 的 updated_at
    update_session_timestamp(conn, &req.session_id)?;
    get_message(conn, &id)?.ok_or(rusqlite::Error::QueryReturnedNoRows)
}

pub fn list_messages_by_session(
    conn: &Connection,
    session_id: &str,
    limit: i64,
    offset: i64,
) -> Result<Vec<ChatMessage>, rusqlite::Error> {
    let sql = MSG_SELECT.to_owned() + " WHERE session_id = ?1 ORDER BY created_at ASC LIMIT ?2 OFFSET ?3";
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![session_id, limit, offset], map_message)?;
    rows.collect()
}

pub fn count_messages_by_session(conn: &Connection, session_id: &str) -> Result<usize, rusqlite::Error> {
    let mut stmt = conn.prepare("SELECT COUNT(*) FROM chat_messages WHERE session_id = ?1")?;
    let count: usize = stmt.query_row(params![session_id], |row| row.get(0))?;
    Ok(count)
}

pub fn delete_old_messages(conn: &Connection, session_id: &str, keep_count: i64) -> Result<(), rusqlite::Error> {
    // 保留最新的 keep_count 条，删除更早的
    conn.execute(
        "DELETE FROM chat_messages WHERE session_id = ?1 AND id NOT IN
         (SELECT id FROM chat_messages WHERE session_id = ?1 ORDER BY created_at DESC LIMIT ?2)",
        params![session_id, keep_count],
    )?;
    log::info!(target: "db", "delete_old_messages session_id={} keep={}", session_id, keep_count);
    Ok(())
}

// === helpers ===

fn get_message(conn: &Connection, id: &str) -> Result<Option<ChatMessage>, rusqlite::Error> {
    let sql = MSG_SELECT.to_owned() + " WHERE id = ?1";
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query(params![id])?;
    if let Some(row) = rows.next()? {
        Ok(Some(map_message(row)?))
    } else {
        Ok(None)
    }
}

const SESSION_SELECT: &str = "SELECT id, user_id, title, created_at, updated_at FROM sessions";

fn map_session(row: &Row) -> Result<Session, rusqlite::Error> {
    Ok(Session {
        id: row.get(0)?,
        user_id: row.get(1)?,
        title: row.get(2)?,
        created_at: row.get(3)?,
        updated_at: row.get(4)?,
    })
}

const MSG_SELECT: &str = "SELECT id, session_id, role, content, metadata_json, created_at FROM chat_messages";

fn map_message(row: &Row) -> Result<ChatMessage, rusqlite::Error> {
    Ok(ChatMessage {
        id: row.get(0)?,
        session_id: row.get(1)?,
        role: row.get(2)?,
        content: row.get(3)?,
        metadata_json: row.get(4)?,
        created_at: row.get(5)?,
    })
}
