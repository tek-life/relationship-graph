use crate::types::{CreateInviteTokenRequest, CreateUserRequest, InviteToken, User};
use chrono::Utc;
use rusqlite::{params, Connection, Row};
use uuid::Uuid;

// === users ===

pub fn create_user(conn: &Connection, req: CreateUserRequest) -> Result<User, rusqlite::Error> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let role = req.role.unwrap_or_else(|| "user".to_string());

    conn.execute(
        "INSERT INTO users (id, username, password_hash, display_name, role, profile_doc, profile_completed, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, ?7, ?8)",
        params![id, req.username, req.password_hash, req.display_name, role, req.profile_doc, now, now],
    )?;
    log::info!(target: "db", "create_user id={} username={}", id, req.username);
    get_user_by_id(conn, &id)?.ok_or(rusqlite::Error::QueryReturnedNoRows)
}

pub fn get_user_by_id(conn: &Connection, id: &str) -> Result<Option<User>, rusqlite::Error> {
    let sql = USER_SELECT.to_owned() + " WHERE id = ?1";
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query(params![id])?;
    if let Some(row) = rows.next()? {
        Ok(Some(map_user(row)?))
    } else {
        Ok(None)
    }
}

pub fn get_user_by_username(conn: &Connection, username: &str) -> Result<Option<User>, rusqlite::Error> {
    let sql = USER_SELECT.to_owned() + " WHERE username = ?1";
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query(params![username])?;
    if let Some(row) = rows.next()? {
        Ok(Some(map_user(row)?))
    } else {
        Ok(None)
    }
}

pub fn list_users(conn: &Connection) -> Result<Vec<User>, rusqlite::Error> {
    let mut stmt = conn.prepare(&(USER_SELECT.to_owned() + " ORDER BY created_at ASC"))?;
    let rows = stmt.query_map([], map_user)?;
    rows.collect()
}

pub fn update_user_role(conn: &Connection, id: &str, role: &str) -> Result<(), rusqlite::Error> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE users SET role = ?1, updated_at = ?2 WHERE id = ?3",
        params![role, now, id],
    )?;
    log::info!(target: "db", "update_user_role id={} role={}", id, role);
    Ok(())
}

pub fn update_user_password(conn: &Connection, id: &str, password_hash: &str) -> Result<(), rusqlite::Error> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE users SET password_hash = ?1, updated_at = ?2 WHERE id = ?3",
        params![password_hash, now, id],
    )?;
    log::info!(target: "db", "update_user_password id={}", id);
    Ok(())
}

pub fn update_user_profile(conn: &Connection, id: &str, profile_doc: &str) -> Result<(), rusqlite::Error> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE users SET profile_doc = ?1, updated_at = ?2 WHERE id = ?3",
        params![profile_doc, now, id],
    )?;
    log::info!(target: "db", "update_user_profile id={}", id);
    Ok(())
}

pub fn update_user_profile_completed(conn: &Connection, id: &str) -> Result<(), rusqlite::Error> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE users SET profile_completed = 1, updated_at = ?1 WHERE id = ?2",
        params![now, id],
    )?;
    log::info!(target: "db", "update_user_profile_completed id={}", id);
    Ok(())
}

/// 读取用户画像文档（chat 链路常驻技能注入用）：仅查 profile_doc /
/// profile_completed 两列，不把 password_hash 等敏感字段带出。
/// 画像未完成（profile_completed=0）、文档为空/空白、用户不存在均返回 None。
pub fn get_profile_doc(conn: &Connection, user_id: &str) -> Result<Option<String>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT profile_doc, profile_completed FROM users WHERE id = ?1",
    )?;
    let mut rows = stmt.query(params![user_id])?;
    let Some(row) = rows.next()? else {
        return Ok(None);
    };
    let completed: i32 = row.get(1)?;
    if completed == 0 {
        return Ok(None);
    }
    let doc: Option<String> = row.get(0)?;
    Ok(doc.filter(|d| !d.trim().is_empty()))
}

// === invite_tokens ===

pub fn create_invite_token(conn: &Connection, req: CreateInviteTokenRequest) -> Result<(), rusqlite::Error> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO invite_tokens (id, token, created_by, used_by, expires_at, created_at)
         VALUES (?1, ?2, ?3, NULL, ?4, ?5)",
        params![id, req.token, req.created_by, req.expires_at, now],
    )?;
    log::info!(target: "db", "create_invite_token id={} token={}", id, req.token);
    Ok(())
}

pub fn get_invite_token(conn: &Connection, token: &str) -> Result<Option<InviteToken>, rusqlite::Error> {
    let sql = INVITE_SELECT.to_owned() + " WHERE token = ?1";
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query(params![token])?;
    if let Some(row) = rows.next()? {
        Ok(Some(map_invite(row)?))
    } else {
        Ok(None)
    }
}

pub fn mark_invite_used(conn: &Connection, token_id: &str, user_id: &str) -> Result<(), rusqlite::Error> {
    conn.execute(
        "UPDATE invite_tokens SET used_by = ?1 WHERE id = ?2",
        params![user_id, token_id],
    )?;
    log::info!(target: "db", "mark_invite_used token_id={} user_id={}", token_id, user_id);
    Ok(())
}

pub fn list_invites(conn: &Connection) -> Result<Vec<InviteToken>, rusqlite::Error> {
    let mut stmt = conn.prepare(&(INVITE_SELECT.to_owned() + " ORDER BY created_at DESC"))?;
    let rows = stmt.query_map([], map_invite)?;
    rows.collect()
}

// === helpers ===

const USER_SELECT: &str =
    "SELECT id, username, password_hash, display_name, role, profile_doc, profile_completed, created_at, updated_at FROM users";

fn map_user(row: &Row) -> Result<User, rusqlite::Error> {
    let completed_int: i32 = row.get(6)?;
    Ok(User {
        id: row.get(0)?,
        username: row.get(1)?,
        password_hash: row.get(2)?,
        display_name: row.get(3)?,
        role: row.get(4)?,
        profile_doc: row.get(5)?,
        profile_completed: completed_int != 0,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
    })
}

const INVITE_SELECT: &str =
    "SELECT id, token, created_by, used_by, expires_at, created_at FROM invite_tokens";

fn map_invite(row: &Row) -> Result<InviteToken, rusqlite::Error> {
    Ok(InviteToken {
        id: row.get(0)?,
        token: row.get(1)?,
        created_by: row.get(2)?,
        used_by: row.get(3)?,
        expires_at: row.get(4)?,
        created_at: row.get(5)?,
    })
}
