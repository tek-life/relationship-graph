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
    result
}
