// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod db;
mod security;
mod types;

use db::AppState;
use std::sync::Mutex;

fn main() {
    env_logger::init();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(AppState {
            db: Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![
            commands::security::check_db_state,
            commands::security::setup_database,
            commands::security::unlock_database,
            commands::security::load_database_from_keychain,
            commands::security::forget_stored_key,
            commands::person::create_person,
            commands::person::update_person,
            commands::person::get_person,
            commands::person::list_persons,
            commands::person::delete_person,
            commands::person::search_person_candidates,
            commands::relationship::create_relationship,
            commands::relationship::list_relationships,
            commands::relationship::list_relationships_by_person,
            commands::relationship::delete_relationship,
            commands::interaction::create_interaction,
            commands::interaction::list_interactions_by_person,
            commands::interaction::list_recent_interactions,
            commands::interaction::create_entity_mention,
            commands::interaction::list_mentions_by_interaction,
            commands::graph::get_graph_data,
            commands::nlq::natural_language_query,
            commands::voice::transcribe_audio,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
