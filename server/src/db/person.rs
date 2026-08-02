use crate::types::{CreatePersonRequest, Person};
use chrono::Utc;
use rusqlite::{params, Connection, Row};
use uuid::Uuid;

pub fn create(conn: &Connection, owner_id: &str, req: CreatePersonRequest) -> Result<Person, rusqlite::Error> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let aliases_json = serde_json::to_string(&req.aliases).unwrap_or_else(|_| "[]".to_string());
    let tags_json = serde_json::to_string(&req.resource_tags).unwrap_or_else(|_| "[]".to_string());
    let projects_json = serde_json::to_string(&req.projects).unwrap_or_else(|_| "[]".to_string());
    let status = req.status.unwrap_or_else(|| "active".to_string());

    conn.execute(
        "INSERT INTO persons (
            id, name, aliases, avatar, phone, email, company, title, location, background,
            relationship_strength, resource_tags, sensitivity_level, status, next_step, notes,
            school, projects, created_at, updated_at, owner_id
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21)",
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
            req.school,
            projects_json,
            now,
            now,
            owner_id
        ],
    )?;

    get_by_id(conn, owner_id, &id)?.ok_or(rusqlite::Error::QueryReturnedNoRows)
}

pub fn update(conn: &Connection, owner_id: &str, id: &str, req: CreatePersonRequest) -> Result<Person, rusqlite::Error> {
    let now = Utc::now().to_rfc3339();
    let aliases_json = serde_json::to_string(&req.aliases).unwrap_or_else(|_| "[]".to_string());
    let tags_json = serde_json::to_string(&req.resource_tags).unwrap_or_else(|_| "[]".to_string());
    let projects_json = serde_json::to_string(&req.projects).unwrap_or_else(|_| "[]".to_string());
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
            school = ?16,
            projects = ?17,
            updated_at = ?18
         WHERE id = ?19 AND owner_id = ?20",
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
            req.school,
            projects_json,
            now,
            id,
            owner_id
        ],
    )?;

    get_by_id(conn, owner_id, id)?.ok_or(rusqlite::Error::QueryReturnedNoRows)
}

pub fn get_by_id(conn: &Connection, owner_id: &str, id: &str) -> Result<Option<Person>, rusqlite::Error> {
    let sql = PERSON_SELECT_SQL.to_owned() + " WHERE id = ?1 AND owner_id = ?2";
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query(params![id, owner_id])?;
    if let Some(row) = rows.next()? {
        Ok(Some(map_person(row)?))
    } else {
        Ok(None)
    }
}

pub fn list_all(conn: &Connection, owner_id: &str) -> Result<Vec<Person>, rusqlite::Error> {
    let mut stmt = conn.prepare(&(PERSON_SELECT_SQL.to_owned() + " WHERE owner_id = ?1 ORDER BY updated_at DESC"))?;
    let rows = stmt.query_map(params![owner_id], map_person)?;
    rows.collect()
}

pub fn delete(conn: &Connection, owner_id: &str, id: &str) -> Result<(), rusqlite::Error> {
    conn.execute("DELETE FROM persons WHERE id = ?1 AND owner_id = ?2", params![id, owner_id])?;
    Ok(())
}

pub fn search_by_mention(conn: &Connection, owner_id: &str, mention: &str) -> Result<Vec<Person>, rusqlite::Error> {
    let pattern = format!("%{}%", mention);
    let mut stmt = conn.prepare(
        &(PERSON_SELECT_SQL.to_owned() + " WHERE owner_id = ?1 AND (name LIKE ?2 OR aliases LIKE ?2) ORDER BY updated_at DESC")
    )?;
    let rows = stmt.query_map(params![owner_id, pattern], map_person)?;
    rows.collect()
}

const PERSON_SELECT_SQL: &str = "SELECT id, name, aliases, avatar, phone, email, company, title, location, background,
    relationship_strength, resource_tags, sensitivity_level, status, next_step, notes, school, projects, created_at, updated_at FROM persons";

fn map_person(row: &Row) -> Result<Person, rusqlite::Error> {
    let aliases_json: String = row.get(2)?;
    let tags_json: String = row.get(11)?;
    let projects_json: Option<String> = row.get(17)?;

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
        school: row.get(16)?,
        projects: projects_json
            .as_deref()
            .map(|json| serde_json::from_str(json).unwrap_or_default())
            .unwrap_or_default(),
        created_at: row.get(18)?,
        updated_at: row.get(19)?,
    })
}
