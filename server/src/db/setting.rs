//! settings 表读写（key-value 配置存储）。
//!
//! 值统一以 JSON 文本形式存储（写入时 serde_json 序列化），
//! 读取方按需反序列化；原始文本经 get_setting 获取。
//!
//! 加密决策（API Key 等敏感配置）：数据库整体已启用 SQLCipher 加密
//!（bundled-sqlcipher，AES-256 页级加密），密钥由数据目录 db.key（0600
//! 权限）保管、服务端启动自动解锁。同一进程持有库密钥的前提下，
//! 字段级再加密无法提升实际防护强度（能读库文件的攻击者同样能读
//! db.key），故敏感配置直接以 JSON 文本存 settings，不另做字段级加密；
//! 防泄露靠两层：① SQLCipher 整库加密；② 读取接口只回掩码，绝不
//! 回传明文（见 mask_secret 与各 admin 配置端点）。

use rusqlite::Connection;

/// 云端 API Key 在 settings 表中的键名
pub const KEY_CLOUD_API_KEY: &str = "cloud_api_key";

/// 读取配置原始文本（JSON 序列化后的字符串）；键不存在返回 None
pub fn get_setting(conn: &Connection, key: &str) -> Result<Option<String>, rusqlite::Error> {
    let mut stmt = conn.prepare("SELECT value FROM settings WHERE key = ?1")?;
    let mut rows = stmt.query([key])?;
    match rows.next()? {
        Some(row) => Ok(Some(row.get::<_, String>(0)?)),
        None => Ok(None),
    }
}

/// 写入配置（值序列化为 JSON 文本），同键覆盖（upsert）
pub fn set_setting<T: serde::Serialize>(
    conn: &Connection,
    key: &str,
    value: &T,
) -> Result<(), rusqlite::Error> {
    let json = serde_json::to_string(value).map_err(|e| {
        rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            e.to_string(),
        )))
    })?;
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        rusqlite::params![key, json],
    )?;
    Ok(())
}

/// 读取配置并反序列化为 T；键不存在或 JSON 解析失败返回 None
///（解析失败视为脏数据降级，不向上传播阻断业务）
pub fn get_setting_value<T: serde::de::DeserializeOwned>(
    conn: &Connection,
    key: &str,
) -> Result<Option<T>, rusqlite::Error> {
    Ok(get_setting(conn, key)?.and_then(|raw| serde_json::from_str(&raw).ok()))
}

/// 删除配置键；键不存在时静默成功（幂等）
pub fn delete_setting(conn: &Connection, key: &str) -> Result<(), rusqlite::Error> {
    conn.execute("DELETE FROM settings WHERE key = ?1", [key])?;
    Ok(())
}

/// 列出全部配置（key, value 原始 JSON 文本）；value 可能是敏感值，
/// 调用方（admin 端点）不得直接回传前端，应经 mask_secret 处理
pub fn list_settings(conn: &Connection) -> Result<Vec<(String, String)>, rusqlite::Error> {
    let mut stmt = conn.prepare("SELECT key, value FROM settings ORDER BY key")?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    rows.collect::<Result<Vec<_>, _>>()
}

/// 敏感值掩码展示：保留末 4 位，其余以 `sk-…` 替代。
/// 长度不足 4 位时全部隐藏（避免短值被完整推断）。
/// 用于 admin 配置摘要等只读展示场景，任何接口不得回传明文。
pub fn mask_secret(value: &str) -> String {
    let chars: Vec<char> = value.trim().chars().collect();
    if chars.len() < 4 {
        return "****".to_string();
    }
    format!("sk-…{}", chars[chars.len() - 4..].iter().collect::<String>())
}
