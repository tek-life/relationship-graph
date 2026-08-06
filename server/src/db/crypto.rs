use argon2::{Algorithm, Argon2, Params, Version};
use rand::RngCore;
use rusqlite::Connection;
use std::path::Path;
use std::time::Instant;

#[derive(Debug, thiserror::Error)]
pub enum CryptoError {
    #[error("argon2 key derivation failed")]
    Argon2Error,
    #[error("database error: {0}")]
    DbError(#[from] rusqlite::Error),
}

pub type Result<T> = std::result::Result<T, CryptoError>;

pub fn derive_key(password: &str, salt: &[u8]) -> Result<String> {
    let started = Instant::now();
    log::debug!(target: "crypto", "derive_key_start salt_len={}", salt.len());
    let params = Params::new(64 * 1024, 3, 4, Some(32)).map_err(|_| CryptoError::Argon2Error)?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut output = [0u8; 32];
    argon2
        .hash_password_into(password.as_bytes(), salt, &mut output)
        .map_err(|_| CryptoError::Argon2Error)?;
    log::debug!(
        target: "crypto",
        "derive_key_success elapsed_ms={}",
        started.elapsed().as_millis()
    );
    Ok(hex::encode(output))
}

pub fn generate_salt() -> [u8; 16] {
    let mut salt = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut salt);
    log::debug!(target: "crypto", "generate_salt_success salt_len={}", salt.len());
    salt
}

/// 生成随机数据库密钥（32 字节 hex），存密钥文件用。
/// 不再由主密码派生：服务端启动时读密钥文件自动解锁。
pub fn generate_db_key() -> String {
    let mut key = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut key);
    log::info!(target: "crypto", "generate_db_key_success");
    hex::encode(key)
}

/// 用新密钥重新加密已打开的数据库（SQLCipher PRAGMA rekey）。
/// 调用前 conn 必须已通过旧密钥验证可读写。
pub fn rekey_db(conn: &Connection, new_key_hex: &str) -> Result<()> {
    let started = Instant::now();
    conn.pragma_update(None, "rekey", new_key_hex)?;
    log::info!(
        target: "crypto",
        "rekey_db_success elapsed_ms={}",
        started.elapsed().as_millis()
    );
    Ok(())
}

pub fn open_encrypted_db<P: AsRef<Path>>(path: P, key_hex: &str) -> Result<Connection> {
    let started = Instant::now();
    let path_ref = path.as_ref();
    log::info!(
        target: "db",
        "open_encrypted_db_start path_ext={:?}",
        path_ref.extension().and_then(|ext| ext.to_str())
    );
    let conn = Connection::open(path_ref)?;
    conn.pragma_update(None, "key", key_hex)?;
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    log::info!(
        target: "db",
        "open_encrypted_db_success elapsed_ms={}",
        started.elapsed().as_millis()
    );
    Ok(conn)
}

pub fn validate_encrypted_db(conn: &Connection) -> Result<()> {
    let started = Instant::now();
    conn.query_row("SELECT count(*) FROM sqlite_master", [], |_row| Ok(()))?;
    log::debug!(
        target: "db",
        "validate_encrypted_db_success elapsed_ms={}",
        started.elapsed().as_millis()
    );
    Ok(())
}
