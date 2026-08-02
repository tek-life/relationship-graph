use crate::types::{CreateRelationshipRequest, Relationship};
use chrono::Utc;
use rusqlite::{params, Connection, Row};
use uuid::Uuid;

pub fn create(conn: &Connection, owner_id: &str, req: CreateRelationshipRequest) -> Result<Relationship, rusqlite::Error> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO relationships (id, from_person_id, to_person_id, relationship_type, strength, description, created_at, source, confidence, confirmation_status, inference_reason, owner_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'manual', NULL, 'confirmed', NULL, ?8)",
        params![id, req.from_person_id, req.to_person_id, req.relationship_type, req.strength, req.description, now, owner_id],
    )?;
    get_by_id(conn, owner_id, &id)?.ok_or(rusqlite::Error::QueryReturnedNoRows)
}

/// AI 推断关系：source=inferred、confirmation_status=pending，待用户确认
pub fn create_inferred(
    conn: &Connection,
    owner_id: &str,
    from_person_id: &str,
    to_person_id: &str,
    relationship_type: &str,
    confidence: f64,
    reason: &str,
) -> Result<Relationship, rusqlite::Error> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO relationships (id, from_person_id, to_person_id, relationship_type, strength, description, created_at, source, confidence, confirmation_status, inference_reason, owner_id)
         VALUES (?1, ?2, ?3, ?4, NULL, NULL, ?5, 'inferred', ?6, 'pending', ?7, ?8)",
        params![id, from_person_id, to_person_id, relationship_type, now, confidence, reason, owner_id],
    )?;
    get_by_id(conn, owner_id, &id)?.ok_or(rusqlite::Error::QueryReturnedNoRows)
}

pub fn get_by_id(conn: &Connection, owner_id: &str, id: &str) -> Result<Option<Relationship>, rusqlite::Error> {
    let sql = RELATIONSHIP_SELECT_SQL.to_owned() + " WHERE id = ?1 AND owner_id = ?2";
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query(params![id, owner_id])?;
    if let Some(row) = rows.next()? {
        Ok(Some(map_relationship(row)?))
    } else {
        Ok(None)
    }
}

pub fn list_all(conn: &Connection, owner_id: &str) -> Result<Vec<Relationship>, rusqlite::Error> {
    let mut stmt = conn.prepare(&(RELATIONSHIP_SELECT_SQL.to_owned() + " WHERE owner_id = ?1 ORDER BY created_at DESC"))?;
    let rows = stmt.query_map(params![owner_id], map_relationship)?;
    rows.collect()
}

pub fn list_pending(conn: &Connection, owner_id: &str) -> Result<Vec<Relationship>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        &(RELATIONSHIP_SELECT_SQL.to_owned() + " WHERE owner_id = ?1 AND confirmation_status = 'pending' ORDER BY confidence DESC, created_at DESC"),
    )?;
    let rows = stmt.query_map(params![owner_id], map_relationship)?;
    rows.collect()
}

pub fn list_by_person(conn: &Connection, owner_id: &str, person_id: &str) -> Result<Vec<Relationship>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        &(RELATIONSHIP_SELECT_SQL.to_owned() + " WHERE owner_id = ?1 AND (from_person_id = ?2 OR to_person_id = ?2) ORDER BY created_at DESC")
    )?;
    let rows = stmt.query_map(params![owner_id, person_id], map_relationship)?;
    rows.collect()
}

pub fn set_confirmation(conn: &Connection, owner_id: &str, id: &str, status: &str) -> Result<Relationship, rusqlite::Error> {
    conn.execute(
        "UPDATE relationships SET confirmation_status = ?1 WHERE id = ?2 AND owner_id = ?3",
        params![status, id, owner_id],
    )?;
    get_by_id(conn, owner_id, id)?.ok_or(rusqlite::Error::QueryReturnedNoRows)
}

/// 两人之间是否已有任意方向、任意状态的关系（含已否认，避免重复推断骚扰）
pub fn exists_between(conn: &Connection, owner_id: &str, a: &str, b: &str) -> Result<bool, rusqlite::Error> {
    let exists: i64 = conn.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM relationships
            WHERE owner_id = ?3
              AND ((from_person_id = ?1 AND to_person_id = ?2)
                OR (from_person_id = ?2 AND to_person_id = ?1))
        )",
        params![a, b, owner_id],
        |row| row.get(0),
    )?;
    Ok(exists == 1)
}

pub fn delete(conn: &Connection, owner_id: &str, id: &str) -> Result<(), rusqlite::Error> {
    conn.execute("DELETE FROM relationships WHERE id = ?1 AND owner_id = ?2", params![id, owner_id])?;
    Ok(())
}

const RELATIONSHIP_SELECT_SQL: &str = "SELECT id, from_person_id, to_person_id, relationship_type, strength, description, created_at, source, confidence, confirmation_status, inference_reason FROM relationships";

fn map_relationship(row: &Row) -> Result<Relationship, rusqlite::Error> {
    Ok(Relationship {
        id: row.get(0)?,
        from_person_id: row.get(1)?,
        to_person_id: row.get(2)?,
        relationship_type: row.get(3)?,
        strength: row.get(4)?,
        description: row.get(5)?,
        created_at: row.get(6)?,
        source: row.get(7)?,
        confidence: row.get(8)?,
        confirmation_status: row.get(9)?,
        inference_reason: row.get(10)?,
    })
}
