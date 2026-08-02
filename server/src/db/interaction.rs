use crate::types::{CreateEntityMentionRequest, CreateInteractionRequest, EntityMention, Interaction};
use chrono::Utc;
use rusqlite::{params, Connection, Row};
use uuid::Uuid;

pub fn create(conn: &Connection, owner_id: &str, req: CreateInteractionRequest) -> Result<Interaction, rusqlite::Error> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let topics_json = serde_json::to_string(&req.topics).unwrap_or_else(|_| "[]".to_string());
    let actions_json = serde_json::to_string(&req.action_items).unwrap_or_else(|_| "[]".to_string());

    conn.execute(
        "INSERT INTO interactions (id, person_id, timestamp, content, summary, topics, action_items, created_at, owner_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![id, req.person_id, req.timestamp, req.content, req.summary, topics_json, actions_json, now, owner_id],
    )?;

    get_by_id(conn, owner_id, &id)?.ok_or(rusqlite::Error::QueryReturnedNoRows)
}

pub fn get_by_id(conn: &Connection, owner_id: &str, id: &str) -> Result<Option<Interaction>, rusqlite::Error> {
    let sql = INTERACTION_SELECT_SQL.to_owned() + " WHERE id = ?1 AND owner_id = ?2";
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query(params![id, owner_id])?;
    if let Some(row) = rows.next()? {
        Ok(Some(map_interaction(row)?))
    } else {
        Ok(None)
    }
}

pub fn list_by_person(conn: &Connection, owner_id: &str, person_id: &str) -> Result<Vec<Interaction>, rusqlite::Error> {
    let mut stmt = conn.prepare(&(INTERACTION_SELECT_SQL.to_owned() + " WHERE owner_id = ?1 AND person_id = ?2 ORDER BY timestamp DESC"))?;
    let rows = stmt.query_map(params![owner_id, person_id], map_interaction)?;
    rows.collect()
}

pub fn list_recent(conn: &Connection, owner_id: &str, limit: i64) -> Result<Vec<Interaction>, rusqlite::Error> {
    let mut stmt = conn.prepare(&(INTERACTION_SELECT_SQL.to_owned() + " WHERE owner_id = ?1 ORDER BY timestamp DESC LIMIT ?2"))?;
    let rows = stmt.query_map(params![owner_id, limit], map_interaction)?;
    rows.collect()
}

pub fn create_mention(conn: &Connection, owner_id: &str, req: CreateEntityMentionRequest) -> Result<EntityMention, rusqlite::Error> {
    let id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO entity_mentions (id, interaction_id, person_id, mention_text, confidence, resolved, owner_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![id, req.interaction_id, req.person_id, req.mention_text, req.confidence, req.resolved as i32, owner_id],
    )?;
    get_mention_by_id(conn, owner_id, &id)?.ok_or(rusqlite::Error::QueryReturnedNoRows)
}

pub fn get_mention_by_id(conn: &Connection, owner_id: &str, id: &str) -> Result<Option<EntityMention>, rusqlite::Error> {
    let sql = MENTION_SELECT_SQL.to_owned() + " WHERE id = ?1 AND owner_id = ?2";
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query(params![id, owner_id])?;
    if let Some(row) = rows.next()? {
        Ok(Some(map_mention(row)?))
    } else {
        Ok(None)
    }
}

pub fn list_mentions_by_interaction(conn: &Connection, owner_id: &str, interaction_id: &str) -> Result<Vec<EntityMention>, rusqlite::Error> {
    let mut stmt = conn.prepare(&(MENTION_SELECT_SQL.to_owned() + " WHERE owner_id = ?1 AND interaction_id = ?2"))?;
    let rows = stmt.query_map(params![owner_id, interaction_id], map_mention)?;
    rows.collect()
}

const INTERACTION_SELECT_SQL: &str = "SELECT id, person_id, timestamp, content, summary, topics, action_items, created_at FROM interactions";
const MENTION_SELECT_SQL: &str = "SELECT id, interaction_id, person_id, mention_text, confidence, resolved FROM entity_mentions";

fn map_interaction(row: &Row) -> Result<Interaction, rusqlite::Error> {
    let topics_json: String = row.get(5)?;
    let actions_json: String = row.get(6)?;
    Ok(Interaction {
        id: row.get(0)?,
        person_id: row.get(1)?,
        timestamp: row.get(2)?,
        content: row.get(3)?,
        summary: row.get(4)?,
        topics: serde_json::from_str(&topics_json).unwrap_or_default(),
        action_items: serde_json::from_str(&actions_json).unwrap_or_default(),
        created_at: row.get(7)?,
    })
}

fn map_mention(row: &Row) -> Result<EntityMention, rusqlite::Error> {
    let resolved: i32 = row.get(5)?;
    Ok(EntityMention {
        id: row.get(0)?,
        interaction_id: row.get(1)?,
        person_id: row.get(2)?,
        mention_text: row.get(3)?,
        confidence: row.get(4)?,
        resolved: resolved == 1,
    })
}
