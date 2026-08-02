//! 用户数据库操作：注册、验证、查询。
//! 密码使用 argon2id 哈希，与 crypto.rs 的 derive_key 逻辑独立（后者用于 SQLCipher key derivation）。

use crate::types::User;
use argon2::{
    password_hash::{rand_core::OsRng, SaltString},
    Argon2, PasswordHash, PasswordHasher, PasswordVerifier,
};
use chrono::Utc;
use rusqlite::{params, Connection};
use uuid::Uuid;

pub fn create_user(
    conn: &Connection,
    username: &str,
    email: Option<&str>,
    phone: Option<&str>,
    password: &str,
) -> Result<User, String> {
    let id = Uuid::new_v4().to_string();
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let password_hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| format!("密码哈希失败: {}", e))?
        .to_string();
    let now = Utc::now().to_rfc3339();

    conn.execute(
        "INSERT INTO users (id, username, email, phone, password_hash, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![id, username, email, phone, password_hash, now],
    )
    .map_err(|e| {
        if e.to_string().contains("UNIQUE") {
            "用户名、邮箱或手机号已被注册".to_string()
        } else {
            format!("创建用户失败: {}", e)
        }
    })?;

    Ok(User {
        id,
        username: username.to_string(),
        email: email.map(String::from),
        phone: phone.map(String::from),
        display_name: None,
        created_at: now,
    })
}

pub fn verify_user(conn: &Connection, login: &str, password: &str) -> Result<User, String> {
    let row = conn
        .query_row(
            "SELECT id, username, email, phone, password_hash, display_name, created_at FROM users WHERE username = ?1 OR email = ?1 OR phone = ?1",
            [login],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, String>(6)?,
                ))
            },
        )
        .map_err(|_| "用户名或密码错误".to_string())?;

    let (id, username, email, phone, hash, display_name, created_at) = row;

    let parsed_hash =
        PasswordHash::new(&hash).map_err(|_| "密码验证失败".to_string())?;
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .map_err(|_| "用户名或密码错误".to_string())?;

    // 更新最后登录时间
    let now = Utc::now().to_rfc3339();
    let _ = conn.execute(
        "UPDATE users SET last_login_at = ?1 WHERE id = ?2",
        params![now, id],
    );

    Ok(User {
        id,
        username,
        email,
        phone,
        display_name,
        created_at,
    })
}

pub fn get_user_by_id(conn: &Connection, user_id: &str) -> Result<User, String> {
    conn.query_row(
        "SELECT id, username, email, phone, display_name, created_at FROM users WHERE id = ?1",
        [user_id],
        |row| {
            Ok(User {
                id: row.get(0)?,
                username: row.get(1)?,
                email: row.get(2)?,
                phone: row.get(3)?,
                display_name: row.get(4)?,
                created_at: row.get(5)?,
            })
        },
    )
    .map_err(|_| "用户不存在".to_string())
}

/// OAuth 登录：按 provider+oauth_id 查找用户，不存在则创建
pub fn find_or_create_oauth_user(
    conn: &Connection,
    provider: &str,
    oauth_id: &str,
    username: &str,
) -> Result<User, String> {
    // 先查找已有 OAuth 绑定
    let existing = conn.query_row(
        "SELECT id, username, email, phone, display_name, created_at FROM users WHERE oauth_provider = ?1 AND oauth_id = ?2",
        params![provider, oauth_id],
        |row| {
            Ok(User {
                id: row.get(0)?,
                username: row.get(1)?,
                email: row.get(2)?,
                phone: row.get(3)?,
                display_name: row.get(4)?,
                created_at: row.get(5)?,
            })
        },
    );

    match existing {
        Ok(user) => Ok(user),
        Err(_) => {
            // 创建 OAuth 用户（无密码，用随机哈希占位）
            let id = Uuid::new_v4().to_string();
            let now = Utc::now().to_rfc3339();
            let placeholder_hash = format!("oauth:{}:{}", provider, oauth_id);

            conn.execute(
                "INSERT INTO users (id, username, email, phone, password_hash, oauth_provider, oauth_id, created_at) VALUES (?1, ?2, NULL, NULL, ?3, ?4, ?5, ?6)",
                params![id, username, placeholder_hash, provider, oauth_id, now],
            )
            .map_err(|e| format!("创建 OAuth 用户失败: {}", e))?;

            Ok(User {
                id,
                username: username.to_string(),
                email: None,
                phone: None,
                display_name: None,
                created_at: now,
            })
        }
    }
}
