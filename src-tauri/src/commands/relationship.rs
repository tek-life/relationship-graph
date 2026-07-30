use crate::db::{get_conn, relationship, AppState};
use crate::types::{CreateRelationshipRequest, Relationship};
use std::time::Instant;
use tauri::State;

#[tauri::command]
pub fn create_relationship(state: State<AppState>, req: CreateRelationshipRequest) -> Result<Relationship, String> {
    let started = Instant::now();
    log::info!(
        target: "relationship_cmd",
        "create_relationship_start from_person_id={} to_person_id={} type={} strength={:?}",
        req.from_person_id,
        req.to_person_id,
        req.relationship_type,
        req.strength
    );
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = get_conn(&guard)?;
    let result = relationship::create(conn, req).map_err(|e| e.to_string())?;
    log::info!(
        target: "relationship_cmd",
        "create_relationship_success relationship_id={} elapsed_ms={}",
        result.id,
        started.elapsed().as_millis()
    );
    Ok(result)
}

#[tauri::command]
pub fn list_relationships(state: State<AppState>) -> Result<Vec<Relationship>, String> {
    let started = Instant::now();
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = get_conn(&guard)?;
    let result = relationship::list_all(conn).map_err(|e| e.to_string())?;
    log::info!(
        target: "relationship_cmd",
        "list_relationships_success count={} elapsed_ms={}",
        result.len(),
        started.elapsed().as_millis()
    );
    Ok(result)
}

#[tauri::command]
pub fn list_relationships_by_person(state: State<AppState>, person_id: String) -> Result<Vec<Relationship>, String> {
    let started = Instant::now();
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = get_conn(&guard)?;
    let result = relationship::list_by_person(conn, &person_id).map_err(|e| e.to_string())?;
    log::info!(
        target: "relationship_cmd",
        "list_relationships_by_person_success person_id={} count={} elapsed_ms={}",
        person_id,
        result.len(),
        started.elapsed().as_millis()
    );
    Ok(result)
}

#[tauri::command]
pub fn delete_relationship(state: State<AppState>, id: String) -> Result<(), String> {
    let started = Instant::now();
    log::info!(target: "relationship_cmd", "delete_relationship_start relationship_id={}", id);
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = get_conn(&guard)?;
    relationship::delete(conn, &id).map_err(|e| e.to_string())?;
    log::info!(
        target: "relationship_cmd",
        "delete_relationship_success relationship_id={} elapsed_ms={}",
        id,
        started.elapsed().as_millis()
    );
    Ok(())
}
