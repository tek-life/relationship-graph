use crate::types::{CreatePersonRequest, Person};
use chrono::Utc;
use rusqlite::{params, Connection, Row};
use uuid::Uuid;

pub fn create(conn: &Connection, req: CreatePersonRequest) -> Result<Person, rusqlite::Error> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let aliases_json = serde_json::to_string(&req.aliases).unwrap_or_else(|_| "[]".to_string());
    let tags_json = serde_json::to_string(&req.resource_tags).unwrap_or_else(|_| "[]".to_string());
    let status = req.status.unwrap_or_else(|| "active".to_string());

    conn.execute(
        "INSERT INTO persons (
            id, name, aliases, avatar, phone, email, company, title, location, background,
            relationship_strength, resource_tags, sensitivity_level, status, next_step, notes,
            created_at, updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
        params![
            id,
            req.name,
            aliases_json,
            req.avatar,
            req.phone,
            req.email,
            req.company,
            req.title,
            req.location,
            req.background,
            req.relationship_strength,
            tags_json,
            req.sensitivity_level,
            status,
            req.next_step,
            req.notes,
            now,
            now
        ],
    )?;

    get_by_id(conn, &id)?.ok_or(rusqlite::Error::QueryReturnedNoRows)
}

pub fn update(conn: &Connection, id: &str, req: CreatePersonRequest) -> Result<Person, rusqlite::Error> {
    let now = Utc::now().to_rfc3339();
    let aliases_json = serde_json::to_string(&req.aliases).unwrap_or_else(|_| "[]".to_string());
    let tags_json = serde_json::to_string(&req.resource_tags).unwrap_or_else(|_| "[]".to_string());
    let status = req.status.unwrap_or_else(|| "active".to_string());

    conn.execute(
        "UPDATE persons SET
            name = ?1,
            aliases = ?2,
            avatar = ?3,
            phone = ?4,
            email = ?5,
            company = ?6,
            title = ?7,
            location = ?8,
            background = ?9,
            relationship_strength = ?10,
            resource_tags = ?11,
            sensitivity_level = ?12,
            status = ?13,
            next_step = ?14,
            notes = ?15,
            updated_at = ?16
         WHERE id = ?17",
        params![
            req.name,
            aliases_json,
            req.avatar,
            req.phone,
            req.email,
            req.company,
            req.title,
            req.location,
            req.background,
            req.relationship_strength,
            tags_json,
            req.sensitivity_level,
            status,
            req.next_step,
            req.notes,
            now,
            id
        ],
    )?;

    get_by_id(conn, id)?.ok_or(rusqlite::Error::QueryReturnedNoRows)
}

pub fn get_by_id(conn: &Connection, id: &str) -> Result<Option<Person>, rusqlite::Error> {
    let sql = PERSON_SELECT_SQL.to_owned() + " WHERE id = ?1";
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query(params![id])?;
    if let Some(row) = rows.next()? {
        Ok(Some(map_person(row)?))
    } else {
        Ok(None)
    }
}

pub fn list_all(conn: &Connection) -> Result<Vec<Person>, rusqlite::Error> {
    let mut stmt = conn.prepare(&(PERSON_SELECT_SQL.to_owned() + " ORDER BY updated_at DESC"))?;
    let rows = stmt.query_map([], map_person)?;
    rows.collect()
}

pub fn delete(conn: &Connection, id: &str) -> Result<(), rusqlite::Error> {
    conn.execute("DELETE FROM persons WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn search_by_mention(conn: &Connection, mention: &str) -> Result<Vec<Person>, rusqlite::Error> {
    let pattern = format!("%{}%", mention);
    let mut stmt = conn.prepare(
        &(PERSON_SELECT_SQL.to_owned() + " WHERE name LIKE ?1 OR aliases LIKE ?1 ORDER BY updated_at DESC")
    )?;
    let rows = stmt.query_map(params![pattern], map_person)?;
    rows.collect()
}

const PERSON_SELECT_SQL: &str = "SELECT id, name, aliases, avatar, phone, email, company, title, location, background,
    relationship_strength, resource_tags, sensitivity_level, status, next_step, notes, created_at, updated_at FROM persons";

fn map_person(row: &Row) -> Result<Person, rusqlite::Error> {
    let aliases_json: String = row.get(2)?;
    let tags_json: String = row.get(11)?;

    Ok(Person {
        id: row.get(0)?,
        name: row.get(1)?,
        aliases: serde_json::from_str(&aliases_json).unwrap_or_default(),
        avatar: row.get(3)?,
        phone: row.get(4)?,
        email: row.get(5)?,
        company: row.get(6)?,
        title: row.get(7)?,
        location: row.get(8)?,
        background: row.get(9)?,
        relationship_strength: row.get(10)?,
        resource_tags: serde_json::from_str(&tags_json).unwrap_or_default(),
        sensitivity_level: row.get(12)?,
        status: row.get(13)?,
        next_step: row.get(14)?,
        notes: row.get(15)?,
        created_at: row.get(16)?,
        updated_at: row.get(17)?,
    })
}
