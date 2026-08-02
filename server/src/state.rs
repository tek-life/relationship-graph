use crate::security::auth::{JwtManager, TokenStore};
use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub struct AppState {
    pub db: Mutex<Option<Connection>>,
    pub tokens: Mutex<TokenStore>, // 保留，向后兼容 unlock
    pub jwt: JwtManager,
    pub data_dir: PathBuf,
}

pub type SharedState = Arc<AppState>;

impl AppState {
    pub fn new(data_dir: PathBuf, jwt_secret: &str) -> Self {
        Self {
            db: Mutex::new(None),
            tokens: Mutex::new(TokenStore::default()),
            jwt: JwtManager::new(jwt_secret),
            data_dir,
        }
    }

    pub fn db_path(&self) -> PathBuf {
        self.data_dir.join("app.db")
    }

    pub fn salt_path(&self) -> PathBuf {
        self.data_dir.join("salt.hex")
    }
}
