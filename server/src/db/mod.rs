pub mod agent_config;
pub mod crypto;
pub mod interaction;
pub mod model_config;
pub mod person;
pub mod relationship;
pub mod schema;
pub mod session;
pub mod setting;
pub mod skill_package;
pub mod user;

#[cfg(test)]
mod tests;

use rusqlite::Connection;

pub fn get_conn(guard: &Option<Connection>) -> Result<&Connection, String> {
    guard
        .as_ref()
        .ok_or_else(|| "数据库尚未初始化或解锁".to_string())
}
