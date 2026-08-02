//! 认证系统：JWT 多用户认证 + 旧 TokenStore 兼容层。
//! JWT secret 从环境变量 RG_JWT_SECRET 读取；未设置时使用随机 secret（重启失效）。

use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

// === JWT 认证 ===

const ACCESS_TOKEN_TTL: Duration = Duration::from_secs(2 * 60 * 60); // 2h
const REFRESH_TOKEN_TTL: Duration = Duration::from_secs(30 * 24 * 60 * 60); // 30d

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,       // user_id
    pub exp: u64,
    pub iat: u64,
    pub token_type: String, // "access" or "refresh"
}

pub struct JwtManager {
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
}

impl JwtManager {
    pub fn new(secret: &str) -> Self {
        Self {
            encoding_key: EncodingKey::from_secret(secret.as_bytes()),
            decoding_key: DecodingKey::from_secret(secret.as_bytes()),
        }
    }

    pub fn issue_tokens(&self, user_id: &str) -> (String, String) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let access_claims = Claims {
            sub: user_id.to_string(),
            exp: now + ACCESS_TOKEN_TTL.as_secs(),
            iat: now,
            token_type: "access".to_string(),
        };
        let access_token = encode(&Header::default(), &access_claims, &self.encoding_key)
            .expect("JWT encode should not fail");

        let refresh_claims = Claims {
            sub: user_id.to_string(),
            exp: now + REFRESH_TOKEN_TTL.as_secs(),
            iat: now,
            token_type: "refresh".to_string(),
        };
        let refresh_token = encode(&Header::default(), &refresh_claims, &self.encoding_key)
            .expect("JWT encode should not fail");

        (access_token, refresh_token)
    }

    pub fn validate_access_token(&self, token: &str) -> Option<String> {
        let data = decode::<Claims>(token, &self.decoding_key, &Validation::default()).ok()?;
        if data.claims.token_type != "access" {
            return None;
        }
        Some(data.claims.sub)
    }

    pub fn validate_refresh_token(&self, token: &str) -> Option<String> {
        let data = decode::<Claims>(token, &self.decoding_key, &Validation::default()).ok()?;
        if data.claims.token_type != "refresh" {
            return None;
        }
        Some(data.claims.sub)
    }
}

// === 旧 TokenStore（向后兼容 unlock 端点） ===

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
