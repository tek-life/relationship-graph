use crate::db::agent_config;
use rusqlite::Connection;
use std::time::Instant;

pub fn migrate(conn: &Connection) -> Result<(), rusqlite::Error> {
    let started = Instant::now();
    log::info!(target: "db", "schema_migrate_start");
    // 老库升级必须在 CREATE INDEX 之前完成：
    // 老 users 表缺少 role 列会导致 idx_users_role 创建失败，整个迁移中止
    ensure_user_columns(conn)?;
    let result = conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS persons (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            aliases TEXT NOT NULL DEFAULT '[]',
            avatar TEXT,
            phone TEXT,
            email TEXT,
            company TEXT,
            title TEXT,
            location TEXT,
            background TEXT,
            relationship_strength TEXT,
            resource_tags TEXT NOT NULL DEFAULT '[]',
            sensitivity_level TEXT NOT NULL DEFAULT 'low',
            status TEXT NOT NULL DEFAULT 'active',
            next_step TEXT,
            notes TEXT,
            school TEXT,
            projects TEXT NOT NULL DEFAULT '[]',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS relationships (
            id TEXT PRIMARY KEY,
            from_person_id TEXT NOT NULL,
            to_person_id TEXT NOT NULL,
            relationship_type TEXT NOT NULL,
            strength TEXT,
            description TEXT,
            created_at TEXT NOT NULL,
            source TEXT NOT NULL DEFAULT 'manual',
            confidence REAL,
            confirmation_status TEXT NOT NULL DEFAULT 'confirmed',
            inference_reason TEXT,
            FOREIGN KEY (from_person_id) REFERENCES persons(id) ON DELETE CASCADE,
            FOREIGN KEY (to_person_id) REFERENCES persons(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS interactions (
            id TEXT PRIMARY KEY,
            person_id TEXT NOT NULL,
            timestamp TEXT NOT NULL,
            content TEXT NOT NULL,
            summary TEXT,
            topics TEXT NOT NULL DEFAULT '[]',
            action_items TEXT NOT NULL DEFAULT '[]',
            created_at TEXT NOT NULL,
            FOREIGN KEY (person_id) REFERENCES persons(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS entity_mentions (
            id TEXT PRIMARY KEY,
            interaction_id TEXT NOT NULL,
            person_id TEXT,
            mention_text TEXT NOT NULL,
            confidence REAL NOT NULL,
            resolved INTEGER NOT NULL DEFAULT 0,
            FOREIGN KEY (interaction_id) REFERENCES interactions(id) ON DELETE CASCADE,
            FOREIGN KEY (person_id) REFERENCES persons(id) ON DELETE SET NULL
        );

        CREATE TABLE IF NOT EXISTS settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_persons_name ON persons(name);
        CREATE INDEX IF NOT EXISTS idx_persons_location ON persons(location);
        CREATE INDEX IF NOT EXISTS idx_persons_status ON persons(status);
        CREATE INDEX IF NOT EXISTS idx_persons_sensitivity ON persons(sensitivity_level);
        CREATE INDEX IF NOT EXISTS idx_relationships_from ON relationships(from_person_id);
        CREATE INDEX IF NOT EXISTS idx_relationships_to ON relationships(to_person_id);
        CREATE INDEX IF NOT EXISTS idx_interactions_person ON interactions(person_id);
        CREATE INDEX IF NOT EXISTS idx_interactions_timestamp ON interactions(timestamp);

        -- 用户与邀请
        CREATE TABLE IF NOT EXISTS users (
            id TEXT PRIMARY KEY,
            username TEXT UNIQUE NOT NULL,
            password_hash TEXT NOT NULL,
            display_name TEXT,
            role TEXT NOT NULL DEFAULT 'user',
            profile_doc TEXT,
            profile_completed INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS invite_tokens (
            id TEXT PRIMARY KEY,
            token TEXT UNIQUE NOT NULL,
            created_by TEXT NOT NULL,
            used_by TEXT,
            expires_at TEXT NOT NULL,
            created_at TEXT NOT NULL,
            FOREIGN KEY (created_by) REFERENCES users(id)
        );

        -- 会话与消息
        CREATE TABLE IF NOT EXISTS sessions (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            title TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
        );
        CREATE TABLE IF NOT EXISTS chat_messages (
            id TEXT PRIMARY KEY,
            session_id TEXT NOT NULL,
            role TEXT NOT NULL,
            content TEXT NOT NULL,
            metadata_json TEXT,
            created_at TEXT NOT NULL,
            FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
        );

        -- 数字人配置
        CREATE TABLE IF NOT EXISTS digital_agents (
            id TEXT PRIMARY KEY,
            display_name TEXT NOT NULL,
            mention TEXT UNIQUE NOT NULL,
            aliases TEXT NOT NULL DEFAULT '[]',
            route_mode TEXT NOT NULL DEFAULT 'chat',
            avatar_url TEXT,
            description TEXT,
            skill_description TEXT,
            is_active INTEGER NOT NULL DEFAULT 1,
            sort_order INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS agent_skills (
            id TEXT PRIMARY KEY,
            agent_id TEXT NOT NULL,
            skill_name TEXT NOT NULL,
            skill_config_json TEXT NOT NULL,
            trigger_scenario TEXT,
            is_active INTEGER NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            FOREIGN KEY (agent_id) REFERENCES digital_agents(id) ON DELETE CASCADE
        );

        -- Profile QA 指令配置
        CREATE TABLE IF NOT EXISTS qa_instruction_modules (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            description TEXT,
            system_prompt TEXT NOT NULL,
            guidance_text TEXT,
            sort_order INTEGER NOT NULL DEFAULT 0,
            trigger_scenario TEXT NOT NULL DEFAULT 'new_user',
            is_active INTEGER NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_users_username ON users(username);
        CREATE INDEX IF NOT EXISTS idx_users_role ON users(role);
        CREATE INDEX IF NOT EXISTS idx_invite_tokens_token ON invite_tokens(token);
        CREATE INDEX IF NOT EXISTS idx_sessions_user ON sessions(user_id);
        CREATE INDEX IF NOT EXISTS idx_sessions_updated ON sessions(updated_at);
        CREATE INDEX IF NOT EXISTS idx_chat_messages_session ON chat_messages(session_id);
        CREATE INDEX IF NOT EXISTS idx_chat_messages_created ON chat_messages(created_at);
        CREATE INDEX IF NOT EXISTS idx_digital_agents_active ON digital_agents(is_active);
        CREATE INDEX IF NOT EXISTS idx_agent_skills_agent ON agent_skills(agent_id);
        CREATE INDEX IF NOT EXISTS idx_qa_modules_active ON qa_instruction_modules(is_active);
        "
    );
    match &result {
        Ok(_) => log::info!(
            target: "db",
            "schema_migrate_success elapsed_ms={}",
            started.elapsed().as_millis()
        ),
        Err(error) => log::warn!(target: "db", "schema_migrate_failed error={}", error),
    }
    result?;
    ensure_relationship_columns(conn)?;
    ensure_person_columns(conn)?;
    repair_orphan_sessions(conn)?;
    agent_config::seed_defaults(conn)
}

/// 数据完整性修复：早期版本曾用不存在的占位 user_id（如 'default'）创建会话，
/// 这些孤儿会话违反 sessions.user_id → users.id 外键，且无法被任何用户查到。
/// 将其归属到首个用户（通常是 admin），不删除任何数据。
fn repair_orphan_sessions(conn: &Connection) -> Result<(), rusqlite::Error> {
    let repaired = conn.execute(
        "UPDATE sessions SET user_id = (
             SELECT id FROM users ORDER BY created_at ASC LIMIT 1
         )
         WHERE user_id NOT IN (SELECT id FROM users)
           AND EXISTS (SELECT 1 FROM users)",
        [],
    )?;
    if repaired > 0 {
        log::info!(target: "db", "repair_orphan_sessions count={}", repaired);
    }
    Ok(())
}

/// 老库升级：为 users 表补充角色/画像列（v1.3 多用户与画像功能）
/// 老库中 users 表已存在但缺少这些列，CREATE TABLE IF NOT EXISTS 不会补齐，
/// 必须先 ALTER 再建索引，否则 idx_users_role 创建失败导致解锁报错。
fn ensure_user_columns(conn: &Connection) -> Result<(), rusqlite::Error> {
    // 全新库尚无 users 表，由后续 CREATE TABLE 建出完整结构，跳过
    let table_exists: bool = conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'users')",
        [],
        |row| row.get(0),
    )?;
    if !table_exists {
        return Ok(());
    }

    let mut stmt = conn.prepare("PRAGMA table_info(users)")?;
    let existing: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<Result<_, _>>()?;

    let columns = [
        ("display_name", "TEXT"),
        ("role", "TEXT NOT NULL DEFAULT 'user'"),
        ("profile_doc", "TEXT"),
        ("profile_completed", "INTEGER NOT NULL DEFAULT 0"),
    ];
    for (name, ddl) in columns {
        if !existing.iter().any(|col| col == name) {
            conn.execute(&format!("ALTER TABLE users ADD COLUMN {} {}", name, ddl), [])?;
            log::info!(target: "db", "schema_migrate_add_column table=users column={}", name);
        }
    }
    Ok(())
}

/// 老库升级：为 persons 表补充学校/项目列（v1.4 推断规则扩展）
fn ensure_person_columns(conn: &Connection) -> Result<(), rusqlite::Error> {
    let mut stmt = conn.prepare("PRAGMA table_info(persons)")?;
    let existing: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<Result<_, _>>()?;

    let columns = [
        ("school", "TEXT"),
        ("projects", "TEXT NOT NULL DEFAULT '[]'"),
    ];
    for (name, ddl) in columns {
        if !existing.iter().any(|col| col == name) {
            conn.execute(&format!("ALTER TABLE persons ADD COLUMN {} {}", name, ddl), [])?;
            log::info!(target: "db", "schema_migrate_add_column table=persons column={}", name);
        }
    }
    Ok(())
}

/// 老库升级：为 relationships 表补充推断相关列（v1.3 关系推断设计）
fn ensure_relationship_columns(conn: &Connection) -> Result<(), rusqlite::Error> {
    let mut stmt = conn.prepare("PRAGMA table_info(relationships)")?;
    let existing: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<Result<_, _>>()?;

    let columns = [
        ("source", "TEXT NOT NULL DEFAULT 'manual'"),
        ("confidence", "REAL"),
        ("confirmation_status", "TEXT NOT NULL DEFAULT 'confirmed'"),
        ("inference_reason", "TEXT"),
    ];
    for (name, ddl) in columns {
        if !existing.iter().any(|col| col == name) {
            conn.execute(&format!("ALTER TABLE relationships ADD COLUMN {} {}", name, ddl), [])?;
            log::info!(target: "db", "schema_migrate_add_column table=relationships column={}", name);
        }
    }
    Ok(())
}
