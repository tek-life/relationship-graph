//! 会话 token 管理：主密码解锁成功后签发短期 token，替代桌面版的系统密钥链。
//! token 仅存内存，服务重启即失效；日志中禁止输出 token 明文。
//!
//! 同时提供用户密码的 Argon2id 哈希与验证工具函数。
//! 注意：用户密码仅用于身份认证，与数据库加密密钥（derive_key）是分开的。

use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::{Algorithm, Argon2, Params, Version};
use rand::RngCore;
use rand::rngs::OsRng;
use std::collections::HashMap;
use std::time::{Duration, Instant};

const TOKEN_TTL: Duration = Duration::from_secs(12 * 60 * 60);

/// 单个 token 的关联信息
#[derive(Clone, Debug)]
pub struct TokenInfo {
    /// 关联的用户 ID；setup/unlock 签发的 token 为 None（向后兼容）
    pub user_id: Option<String>,
    /// 用户角色（admin / user）；setup/unlock 签发的 token 为 None
    pub role: Option<String>,
    pub issued_at: Instant,
}

#[derive(Default)]
pub struct TokenStore {
    tokens: HashMap<String, TokenInfo>,
}

impl TokenStore {
    /// 签发不关联用户的 token（setup/unlock 使用，向后兼容）
    pub fn issue(&mut self) -> String {
        self.issue_with_user(None, None)
    }

    /// 签发关联用户信息的 token（login/register 使用）
    pub fn issue_with_user(
        &mut self,
        user_id: Option<String>,
        role: Option<String>,
    ) -> String {
        self.purge_expired();
        let mut bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut bytes);
        let token = hex::encode(bytes);
        self.tokens.insert(
            token.clone(),
            TokenInfo {
                user_id,
                role,
                issued_at: Instant::now(),
            },
        );
        log::info!(target: "auth", "token_issued active_tokens={}", self.tokens.len());
        token
    }

    /// 仅判断 token 是否有效（向后兼容）
    pub fn validate(&mut self, token: &str) -> bool {
        self.purge_expired();
        self.tokens.contains_key(token)
    }

    /// 获取 token 的完整信息；不存在或已过期返回 None
    pub fn get_token_info(&mut self, token: &str) -> Option<TokenInfo> {
        self.purge_expired();
        self.tokens.get(token).cloned()
    }

    /// 更新指定用户所有 token 的角色快照。
    /// 角色变更（升/降权）后必须调用，否则已签发 token 仍携带旧角色，
    /// 导致 require_admin 按旧快照误判（新管理员 403 / 被降权者仍可操作）。
    pub fn update_user_role(&mut self, user_id: &str, role: &str) -> usize {
        self.purge_expired();
        let mut count = 0;
        for info in self.tokens.values_mut() {
            if info.user_id.as_deref() == Some(user_id) {
                info.role = Some(role.to_string());
                count += 1;
            }
        }
        log::info!(target: "auth", "token_roles_updated user_id={} role={} tokens={}", user_id, role, count);
        count
    }

    pub fn revoke_all(&mut self) {
        let count = self.tokens.len();
        self.tokens.clear();
        log::info!(target: "auth", "tokens_revoked count={}", count);
    }

    fn purge_expired(&mut self) {
        self.tokens
            .retain(|_, info| info.issued_at.elapsed() < TOKEN_TTL);
    }
}

// === 密码哈希与验证 ===

/// 使用 Argon2id 哈希用户密码，返回 PHC 格式字符串（含盐、参数）
pub fn hash_password(password: &str) -> Result<String, String> {
    // 使用 OsRng 生成密码盐（与 rand crate 兼容的 rand_core::OsRng）
    let salt = SaltString::generate(&mut OsRng);

    // 参数与 derive_key 保持一致，确保安全强度
    let params = Params::new(64 * 1024, 3, 4, Some(32))
        .map_err(|e| format!("Argon2 参数错误: {}", e))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    let hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| format!("密码哈希失败: {}", e))?;

    Ok(hash.to_string())
}

/// 验证用户密码与存储的 PHC 格式哈希是否匹配
pub fn verify_password(password: &str, hash: &str) -> bool {
    let parsed = match PasswordHash::new(hash) {
        Ok(h) => h,
        Err(_) => return false,
    };
    // 验证时使用默认参数即可，PHC 字符串中已包含哈希时的参数
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}
