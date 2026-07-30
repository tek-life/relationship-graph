pub mod crypto;
pub mod interaction;
pub mod person;
pub mod relationship;
pub mod schema;

#[cfg(test)]
mod tests;

use rusqlite::Connection;
use std::sync::Mutex;

pub struct AppState {
    pub db: Mutex<Option<Connection>>,
}

pub fn get_conn<'a>(guard: &'a Option<Connection>) -> Result<&'a Connection, String> {
    guard
        .as_ref()
        .ok_or_else(|| "数据库尚未初始化或解锁".to_string())
}
