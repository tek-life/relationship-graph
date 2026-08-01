use crate::db::{get_conn, person, AppState};
use crate::types::{CreatePersonRequest, Person};
use std::time::Instant;
use tauri::State;

#[tauri::command]
pub fn create_person(state: State<AppState>, req: CreatePersonRequest) -> Result<Person, String> {
    let started = Instant::now();
    log::info!(
        target: "person_cmd",
        "create_person_start alias_count={} tag_count={} sensitivity={} status={:?}",
        req.aliases.len(),
        req.resource_tags.len(),
        req.sensitivity_level,
        req.status
    );
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = get_conn(&guard)?;
    let result = person::create(conn, req).map_err(|e| e.to_string())?;
    log::info!(
        target: "person_cmd",
        "create_person_success person_id={} elapsed_ms={}",
        result.id,
        started.elapsed().as_millis()
    );
    Ok(result)
}

#[tauri::command]
pub fn update_person(state: State<AppState>, id: String, req: CreatePersonRequest) -> Result<Person, String> {
    let started = Instant::now();
    log::info!(target: "person_cmd", "update_person_start person_id={}", id);
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = get_conn(&guard)?;
    let result = person::update(conn, &id, req).map_err(|e| e.to_string())?;
    log::info!(
        target: "person_cmd",
        "update_person_success person_id={} elapsed_ms={}",
        result.id,
        started.elapsed().as_millis()
    );
    Ok(result)
}

#[tauri::command]
pub fn get_person(state: State<AppState>, id: String) -> Result<Option<Person>, String> {
    let started = Instant::now();
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = get_conn(&guard)?;
    let result = person::get_by_id(conn, &id).map_err(|e| e.to_string())?;
    log::info!(
        target: "person_cmd",
        "get_person_success person_id={} found={} elapsed_ms={}",
        id,
        result.is_some(),
        started.elapsed().as_millis()
    );
    Ok(result)
}

#[tauri::command]
pub fn list_persons(state: State<AppState>) -> Result<Vec<Person>, String> {
    let started = Instant::now();
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = get_conn(&guard)?;
    let result = person::list_all(conn).map_err(|e| e.to_string())?;
    log::info!(
        target: "person_cmd",
        "list_persons_success count={} elapsed_ms={}",
        result.len(),
        started.elapsed().as_millis()
    );
    Ok(result)
}

#[tauri::command]
pub fn delete_person(state: State<AppState>, id: String) -> Result<(), String> {
    let started = Instant::now();
    log::info!(target: "person_cmd", "delete_person_start person_id={}", id);
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = get_conn(&guard)?;
    person::delete(conn, &id).map_err(|e| e.to_string())?;
    log::info!(
        target: "person_cmd",
        "delete_person_success person_id={} elapsed_ms={}",
        id,
        started.elapsed().as_millis()
    );
    Ok(())
}

#[tauri::command]
pub fn search_person_candidates(state: State<AppState>, mention: String) -> Result<Vec<Person>, String> {
    let started = Instant::now();
    let mention_len = mention.chars().count();
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = get_conn(&guard)?;
    let result = person::search_by_mention(conn, &mention).map_err(|e| e.to_string())?;
    log::info!(
        target: "person_cmd",
        "search_person_candidates_success mention_len={} count={} elapsed_ms={}",
        mention_len,
        result.len(),
        started.elapsed().as_millis()
    );
    Ok(result)
}
