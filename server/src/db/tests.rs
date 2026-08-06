#[cfg(test)]
mod tests {
    use crate::db::{person, schema};
    use crate::types::CreatePersonRequest;
    use rusqlite::Connection;

    fn in_memory_db() -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory db");
        schema::migrate(&conn).expect("schema migration");
        conn
    }

    #[test]
    fn creates_and_lists_person() {
        let conn = in_memory_db();
        let created = person::create(
            &conn,
            CreatePersonRequest {
                name: "张三".to_string(),
                aliases: vec!["老张".to_string()],
                avatar: None,
                phone: Some("13800000000".to_string()),
                email: None,
                company: Some("某公司".to_string()),
                title: Some("工程师".to_string()),
                location: Some("上海".to_string()),
                background: Some("通过朋友介绍认识".to_string()),
                relationship_strength: Some("strong".to_string()),
                resource_tags: vec!["地产".to_string(), "融资".to_string()],
                sensitivity_level: "low".to_string(),
                status: Some("active".to_string()),
                next_step: Some("约饭聊项目".to_string()),
                notes: None,
                school: Some("某大学".to_string()),
                projects: vec!["某项目".to_string()],
            },
        )
        .expect("create person");

        let persons = person::list_all(&conn).expect("list persons");
        assert_eq!(persons.len(), 1);
        assert_eq!(persons[0].id, created.id);
        assert_eq!(persons[0].aliases, vec!["老张"]);
        assert_eq!(persons[0].resource_tags, vec!["地产", "融资"]);
        assert_eq!(persons[0].school.as_deref(), Some("某大学"));
        assert_eq!(persons[0].projects, vec!["某项目"]);
    }

    /// 老库升级：早期版本的 users 表没有 role/profile_doc 等列，
    /// migrate 必须先补列再建索引，否则 idx_users_role 创建失败（解锁报 no such column: role）
    #[test]
    fn migrates_legacy_users_table_without_role_column() {
        let conn = Connection::open_in_memory().expect("in-memory db");
        conn.execute_batch(
            "CREATE TABLE users (
                id TEXT PRIMARY KEY,
                username TEXT UNIQUE NOT NULL,
                password_hash TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            INSERT INTO users (id, username, password_hash, created_at, updated_at)
            VALUES ('u1', 'admin', 'hash', '2026-01-01', '2026-01-01');",
        )
        .expect("legacy users table");

        schema::migrate(&conn).expect("schema migration on legacy db");

        let role: String = conn
            .query_row("SELECT role FROM users WHERE id = 'u1'", [], |row| row.get(0))
            .expect("role column exists with default");
        assert_eq!(role, "user");

        let index_exists: bool = conn
            .query_row(
                "SELECT EXISTS (SELECT 1 FROM sqlite_master WHERE type = 'index' AND name = 'idx_users_role')",
                [],
                |row| row.get(0),
            )
            .expect("index check");
        assert!(index_exists);
    }

    /// 数据完整性修复：早期占位 user_id（'default'）创建的孤儿会话，
    /// 再次 migrate 时应归属到首个真实用户，而不是报错或丢数据
    #[test]
    fn repairs_orphan_sessions_with_placeholder_user_id() {
        let conn = in_memory_db();
        conn.execute(
            "INSERT INTO users (id, username, password_hash, display_name, role, profile_doc, profile_completed, created_at, updated_at)
             VALUES ('u-admin', 'admin', 'hash', '管理员', 'admin', NULL, 0, '2026-01-01', '2026-01-01')",
            [],
        )
        .expect("insert admin user");
        // 临时关闭外键约束以模拟历史脏数据（旧版本连接未强制外键）
        conn.execute_batch("PRAGMA foreign_keys = OFF;")
            .expect("disable fk for test");
        conn.execute(
            "INSERT INTO sessions (id, user_id, title, created_at, updated_at)
             VALUES ('s1', 'default', '旧会话', '2026-01-02', '2026-01-02')",
            [],
        )
        .expect("insert orphan session");
        conn.execute_batch("PRAGMA foreign_keys = ON;")
            .expect("re-enable fk for test");

        schema::migrate(&conn).expect("schema migration repairs orphans");

        let owner: String = conn
            .query_row("SELECT user_id FROM sessions WHERE id = 's1'", [], |row| row.get(0))
            .expect("session still exists");
        assert_eq!(owner, "u-admin");
    }
}
