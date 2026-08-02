use crate::nlq_config::NlqKeywords;
use crate::security::auth::{JwtManager, TokenStore};
use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};

pub struct AppState {
    pub db: Mutex<Option<Connection>>,
    pub tokens: Mutex<TokenStore>, // 保留，向后兼容 unlock
    pub jwt: JwtManager,
    pub data_dir: PathBuf,
    pub nlq_keywords: RwLock<Arc<NlqKeywords>>,
}

pub type SharedState = Arc<AppState>;

impl AppState {
    pub fn new(data_dir: PathBuf, jwt_secret: &str, nlq_keywords: Arc<NlqKeywords>) -> Self {
        Self {
            db: Mutex::new(None),
            tokens: Mutex::new(TokenStore::default()),
            jwt: JwtManager::new(jwt_secret),
            data_dir,
            nlq_keywords: RwLock::new(nlq_keywords),
        }
    }

    pub fn db_path(&self) -> PathBuf {
        self.data_dir.join("app.db")
    }

    pub fn salt_path(&self) -> PathBuf {
        self.data_dir.join("salt.hex")
    }

    /// 获取当前 NLQ 关键词配置的快照
    pub fn get_nlq_keywords(&self) -> Arc<NlqKeywords> {
        self.nlq_keywords.read().unwrap().clone()
    }

    /// 热加载：替换 NLQ 关键词配置
    pub fn reload_nlq_keywords(&self, keywords: NlqKeywords) {
        let mut guard = self.nlq_keywords.write().unwrap();
        *guard = Arc::new(keywords);
    }
}
