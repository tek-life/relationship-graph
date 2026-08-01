use crate::db::crypto::{derive_key, generate_salt, open_encrypted_db, validate_encrypted_db};
use crate::db::{schema, AppState};
use crate::security::keychain;
use serde::Serialize;
use std::path::PathBuf;
use std::time::Instant;
use tauri::State;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DbState {
    pub initialized: bool,
    pub has_stored_key: bool,
    pub unlocked: bool,
}

#[tauri::command]
pub fn check_db_state(state: State<AppState>) -> Result<DbState, String> {
    let started = Instant::now();
    let db_path = db_path()?;
    let initialized = db_path.exists() && salt_path()?.exists();
    let has_stored_key = keychain::get_key()?.is_some();
    let unlocked = state.db.lock().map_err(|e| e.to_string())?.is_some();

    log::info!(
        target: "security",
        "check_db_state initialized={} has_stored_key={} unlocked={} elapsed_ms={}",
        initialized,
        has_stored_key,
        unlocked,
        started.elapsed().as_millis()
    );

    Ok(DbState {
        initialized,
        has_stored_key,
        unlocked,
    })
}

#[tauri::command]
pub fn setup_database(state: State<AppState>, password: String) -> Result<(), String> {
    let started = Instant::now();
    log::info!(target: "security", "setup_database_start");

    if password.trim().len() < 8 {
        log::warn!(target: "security", "setup_database_rejected reason=short_password");
        return Err("主密码至少需要 8 个字符".to_string());
    }

    let db_path = db_path()?;
    let salt_path = salt_path()?;
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let salt = generate_salt();
    let key_hex = derive_key(&password, &salt).map_err(|e| e.to_string())?;
    std::fs::write(&salt_path, hex::encode(salt)).map_err(|e| e.to_string())?;

    let conn = open_encrypted_db(&db_path, &key_hex).map_err(|e| e.to_string())?;
    schema::migrate(&conn).map_err(|e| e.to_string())?;
    keychain::store_key(&key_hex)?;

    let mut guard = state.db.lock().map_err(|e| e.to_string())?;
    *guard = Some(conn);
    log::info!(
        target: "security",
        "setup_database_success elapsed_ms={}",
        started.elapsed().as_millis()
    );
    Ok(())
}

#[tauri::command]
pub fn unlock_database(state: State<AppState>, password: String) -> Result<(), String> {
    let started = Instant::now();
    log::info!(target: "security", "unlock_database_start source=manual_password");
    let salt = read_salt()?;
    let key_hex = derive_key(&password, &salt).map_err(|e| e.to_string())?;
    unlock_with_key(state, &key_hex)?;
    keychain::store_key(&key_hex)?;
    log::info!(
        target: "security",
        "unlock_database_success source=manual_password elapsed_ms={}",
        started.elapsed().as_millis()
    );
    Ok(())
}

#[tauri::command]
pub fn load_database_from_keychain(state: State<AppState>) -> Result<(), String> {
    let started = Instant::now();
    log::info!(target: "security", "unlock_database_start source=keychain");
    let key_hex = keychain::get_key()?.ok_or_else(|| "系统密钥链中没有数据库密钥".to_string())?;
    unlock_with_key(state, &key_hex)?;
    log::info!(
        target: "security",
        "unlock_database_success source=keychain elapsed_ms={}",
        started.elapsed().as_millis()
    );
    Ok(())
}

#[tauri::command]
pub fn forget_stored_key() -> Result<(), String> {
    log::info!(target: "security", "forget_stored_key_start");
    keychain::delete_key()?;
    log::info!(target: "security", "forget_stored_key_success");
    Ok(())
}

fn unlock_with_key(state: State<AppState>, key_hex: &str) -> Result<(), String> {
    let started = Instant::now();
    let conn = open_encrypted_db(db_path()?, key_hex).map_err(|e| e.to_string())?;
    validate_encrypted_db(&conn).map_err(|e| e.to_string())?;
    schema::migrate(&conn).map_err(|e| e.to_string())?;

    let mut guard = state.db.lock().map_err(|e| e.to_string())?;
    *guard = Some(conn);
    log::debug!(
        target: "security",
        "unlock_with_key_success elapsed_ms={}",
        started.elapsed().as_millis()
    );
    Ok(())
}

fn read_salt() -> Result<Vec<u8>, String> {
    let salt_hex = std::fs::read_to_string(salt_path()?).map_err(|e| e.to_string())?;
    let salt = hex::decode(salt_hex.trim()).map_err(|e| e.to_string())?;
    log::debug!(target: "security", "read_salt_success salt_len={}", salt.len());
    Ok(salt)
}

fn app_data_dir() -> Result<PathBuf, String> {
    let dir = dirs::data_dir()
        .ok_or_else(|| "无法定位系统数据目录".to_string())?
        .join("relationship-graph");
    Ok(dir)
}

fn db_path() -> Result<PathBuf, String> {
    Ok(app_data_dir()?.join("app.db"))
}

fn salt_path() -> Result<PathBuf, String> {
    Ok(app_data_dir()?.join("salt.hex"))
}
