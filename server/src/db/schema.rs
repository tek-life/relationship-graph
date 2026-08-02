use rusqlite::Connection;
use std::time::Instant;

pub fn migrate(conn: &Connection) -> Result<(), rusqlite::Error> {
    let started = Instant::now();
    log::info!(target: "db", "schema_migrate_start");
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

        CREATE TABLE IF NOT EXISTS users (
            id TEXT PRIMARY KEY,
            username TEXT UNIQUE NOT NULL,
            email TEXT UNIQUE,
            phone TEXT UNIQUE,
            password_hash TEXT NOT NULL,
            display_name TEXT,
            oauth_provider TEXT,
            oauth_id TEXT,
            created_at TEXT NOT NULL,
            last_login_at TEXT
        );

        CREATE INDEX IF NOT EXISTS idx_users_email ON users(email);
        CREATE INDEX IF NOT EXISTS idx_users_phone ON users(phone);

        CREATE INDEX IF NOT EXISTS idx_persons_name ON persons(name);
        CREATE INDEX IF NOT EXISTS idx_persons_location ON persons(location);
        CREATE INDEX IF NOT EXISTS idx_persons_status ON persons(status);
        CREATE INDEX IF NOT EXISTS idx_persons_sensitivity ON persons(sensitivity_level);
        CREATE INDEX IF NOT EXISTS idx_relationships_from ON relationships(from_person_id);
        CREATE INDEX IF NOT EXISTS idx_relationships_to ON relationships(to_person_id);
        CREATE INDEX IF NOT EXISTS idx_interactions_person ON interactions(person_id);
        CREATE INDEX IF NOT EXISTS idx_interactions_timestamp ON interactions(timestamp);
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
    ensure_owner_id_columns(conn)
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

    // v2.0 商业关系类型扩展：建立方式、建立日期、关系强度评分
    ensure_relationship_business_columns(conn, &existing)?;

    Ok(())
}

/// v2.0 商业关系类型扩展：为 relationships 表补充 how_established / established_date / strength_rating
fn ensure_relationship_business_columns(conn: &Connection, existing: &[String]) -> Result<(), rusqlite::Error> {
    let columns = [
        ("how_established", "TEXT"),
        ("established_date", "TEXT"),
        ("strength_rating", "REAL DEFAULT 0.5"),
    ];
    for (name, ddl) in columns {
        if !existing.iter().any(|col| col == name) {
            conn.execute(&format!("ALTER TABLE relationships ADD COLUMN {} {}", name, ddl), [])?;
            log::info!(target: "db", "schema_migrate_add_column table=relationships column={}", name);
        }
    }
    Ok(())
}

/// 多用户升级：为主表添加 owner_id 列
pub fn ensure_owner_id_columns(conn: &Connection) -> Result<(), rusqlite::Error> {
    let tables = ["persons", "relationships", "interactions", "entity_mentions"];
    for table in tables {
        let mut stmt = conn.prepare(&format!("PRAGMA table_info({})", table))?;
        let existing: Vec<String> = stmt
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<Result<_, _>>()?;

        if !existing.iter().any(|col| col == "owner_id") {
            conn.execute(
                &format!("ALTER TABLE {} ADD COLUMN owner_id TEXT NOT NULL DEFAULT 'legacy'", table),
                [],
            )?;
            log::info!(target: "db", "schema_migrate_add_column table={} column=owner_id", table);
        }
    }

    // 索引
    conn.execute_batch("
        CREATE INDEX IF NOT EXISTS idx_persons_owner ON persons(owner_id, updated_at DESC);
        CREATE INDEX IF NOT EXISTS idx_relationships_owner ON relationships(owner_id);
        CREATE INDEX IF NOT EXISTS idx_interactions_owner ON interactions(owner_id);
    ")?;

    Ok(())
}
