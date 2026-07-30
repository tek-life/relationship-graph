use crate::db::{get_conn, interaction, AppState};
use crate::types::{CreateEntityMentionRequest, CreateInteractionRequest, EntityMention, Interaction};
use std::time::Instant;
use tauri::State;

#[tauri::command]
pub fn create_interaction(state: State<AppState>, req: CreateInteractionRequest) -> Result<Interaction, String> {
    let started = Instant::now();
    log::info!(
        target: "interaction_cmd",
        "create_interaction_start person_id={} content_len={} topic_count={} action_count={}",
        req.person_id,
        req.content.chars().count(),
        req.topics.len(),
        req.action_items.len()
    );
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = get_conn(&guard)?;
    let result = interaction::create(conn, req).map_err(|e| e.to_string())?;
    log::info!(
        target: "interaction_cmd",
        "create_interaction_success interaction_id={} elapsed_ms={}",
        result.id,
        started.elapsed().as_millis()
    );
    Ok(result)
}

#[tauri::command]
pub fn list_interactions_by_person(state: State<AppState>, person_id: String) -> Result<Vec<Interaction>, String> {
    let started = Instant::now();
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = get_conn(&guard)?;
    let result = interaction::list_by_person(conn, &person_id).map_err(|e| e.to_string())?;
    log::info!(
        target: "interaction_cmd",
        "list_interactions_by_person_success person_id={} count={} elapsed_ms={}",
        person_id,
        result.len(),
        started.elapsed().as_millis()
    );
    Ok(result)
}

#[tauri::command]
pub fn list_recent_interactions(state: State<AppState>, limit: i64) -> Result<Vec<Interaction>, String> {
    let started = Instant::now();
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = get_conn(&guard)?;
    let result = interaction::list_recent(conn, limit).map_err(|e| e.to_string())?;
    log::info!(
        target: "interaction_cmd",
        "list_recent_interactions_success limit={} count={} elapsed_ms={}",
        limit,
        result.len(),
        started.elapsed().as_millis()
    );
    Ok(result)
}

#[tauri::command]
pub fn create_entity_mention(state: State<AppState>, req: CreateEntityMentionRequest) -> Result<EntityMention, String> {
    let started = Instant::now();
    log::info!(
        target: "interaction_cmd",
        "create_entity_mention_start interaction_id={} mention_len={} resolved={}",
        req.interaction_id,
        req.mention_text.chars().count(),
        req.resolved
    );
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = get_conn(&guard)?;
    let result = interaction::create_mention(conn, req).map_err(|e| e.to_string())?;
    log::info!(
        target: "interaction_cmd",
        "create_entity_mention_success mention_id={} elapsed_ms={}",
        result.id,
        started.elapsed().as_millis()
    );
    Ok(result)
}

#[tauri::command]
pub fn list_mentions_by_interaction(state: State<AppState>, interaction_id: String) -> Result<Vec<EntityMention>, String> {
    let started = Instant::now();
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = get_conn(&guard)?;
    let result = interaction::list_mentions_by_interaction(conn, &interaction_id).map_err(|e| e.to_string())?;
    log::info!(
        target: "interaction_cmd",
        "list_mentions_by_interaction_success interaction_id={} count={} elapsed_ms={}",
        interaction_id,
        result.len(),
        started.elapsed().as_millis()
    );
    Ok(result)
}
