//! 会话 token 管理：主密码解锁成功后签发短期 token，替代桌面版的系统密钥链。
//! token 仅存内存，服务重启即失效；日志中禁止输出 token 明文。

use rand::RngCore;
use std::collections::HashMap;
use std::time::{Duration, Instant};

const TOKEN_TTL: Duration = Duration::from_secs(12 * 60 * 60);

#[derive(Default)]
pub struct TokenStore {
    tokens: HashMap<String, Instant>,
}

impl TokenStore {
    pub fn issue(&mut self) -> String {
        self.purge_expired();
        let mut bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut bytes);
        let token = hex::encode(bytes);
        self.tokens.insert(token.clone(), Instant::now());
        log::info!(target: "auth", "token_issued active_tokens={}", self.tokens.len());
        token
    }

    pub fn validate(&mut self, token: &str) -> bool {
        self.purge_expired();
        self.tokens.contains_key(token)
    }

    pub fn revoke_all(&mut self) {
        let count = self.tokens.len();
        self.tokens.clear();
        log::info!(target: "auth", "tokens_revoked count={}", count);
    }

    fn purge_expired(&mut self) {
        self.tokens
            .retain(|_, issued| issued.elapsed() < TOKEN_TTL);
    }
}
