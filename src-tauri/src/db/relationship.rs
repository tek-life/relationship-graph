use crate::types::{CreateRelationshipRequest, Relationship};
use chrono::Utc;
use rusqlite::{params, Connection, Row};
use uuid::Uuid;

pub fn create(conn: &Connection, req: CreateRelationshipRequest) -> Result<Relationship, rusqlite::Error> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO relationships (id, from_person_id, to_person_id, relationship_type, strength, description, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![id, req.from_person_id, req.to_person_id, req.relationship_type, req.strength, req.description, now],
    )?;
    get_by_id(conn, &id)?.ok_or(rusqlite::Error::QueryReturnedNoRows)
}

pub fn get_by_id(conn: &Connection, id: &str) -> Result<Option<Relationship>, rusqlite::Error> {
    let sql = RELATIONSHIP_SELECT_SQL.to_owned() + " WHERE id = ?1";
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query(params![id])?;
    if let Some(row) = rows.next()? {
        Ok(Some(map_relationship(row)?))
    } else {
        Ok(None)
    }
}

pub fn list_all(conn: &Connection) -> Result<Vec<Relationship>, rusqlite::Error> {
    let mut stmt = conn.prepare(&(RELATIONSHIP_SELECT_SQL.to_owned() + " ORDER BY created_at DESC"))?;
    let rows = stmt.query_map([], map_relationship)?;
    rows.collect()
}

pub fn list_by_person(conn: &Connection, person_id: &str) -> Result<Vec<Relationship>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        &(RELATIONSHIP_SELECT_SQL.to_owned() + " WHERE from_person_id = ?1 OR to_person_id = ?1 ORDER BY created_at DESC")
    )?;
    let rows = stmt.query_map(params![person_id], map_relationship)?;
    rows.collect()
}

pub fn delete(conn: &Connection, id: &str) -> Result<(), rusqlite::Error> {
    conn.execute("DELETE FROM relationships WHERE id = ?1", params![id])?;
    Ok(())
}

const RELATIONSHIP_SELECT_SQL: &str = "SELECT id, from_person_id, to_person_id, relationship_type, strength, description, created_at FROM relationships";

fn map_relationship(row: &Row) -> Result<Relationship, rusqlite::Error> {
    Ok(Relationship {
        id: row.get(0)?,
        from_person_id: row.get(1)?,
        to_person_id: row.get(2)?,
        relationship_type: row.get(3)?,
        strength: row.get(4)?,
        description: row.get(5)?,
        created_at: row.get(6)?,
    })
}
