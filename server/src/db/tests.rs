#[cfg(test)]
mod tests {
    use crate::db::{agent_config, person, schema, user as user_db};
    use crate::types::{CreateAgentSkillRequest, CreateDigitalAgentRequest, CreatePersonRequest, CreateUserRequest};
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
            "owner-a",
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

        let persons = person::list_all(&conn, "owner-a").expect("list persons");
        assert_eq!(persons.len(), 1);
        assert_eq!(persons[0].id, created.id);
        assert_eq!(persons[0].aliases, vec!["老张"]);
        assert_eq!(persons[0].resource_tags, vec!["地产", "融资"]);
        assert_eq!(persons[0].school.as_deref(), Some("某大学"));
        assert_eq!(persons[0].projects, vec!["某项目"]);
    }

    /// 用户隔离：联系人及其派生数据（互动/关系）严格归属 owner，
    /// 跨用户读写一律表现为查无此数据或拒绝，不泄露存在性
    #[test]
    fn persons_are_isolated_between_users() {
        use crate::db::{interaction, relationship};
        use crate::types::{CreateInteractionRequest, CreateRelationshipRequest};

        let conn = in_memory_db();
        let req = |name: &str| CreatePersonRequest {
            name: name.to_string(),
            aliases: vec![],
            avatar: None,
            phone: None,
            email: None,
            company: None,
            title: None,
            location: None,
            background: None,
            relationship_strength: None,
            resource_tags: vec![],
            sensitivity_level: "low".to_string(),
            status: None,
            next_step: None,
            notes: None,
            school: None,
            projects: vec![],
        };

        let a = person::create(&conn, "owner-a", req("甲")).unwrap();
        let b = person::create(&conn, "owner-b", req("乙")).unwrap();

        // 列表互不可见
        assert_eq!(person::list_all(&conn, "owner-a").unwrap().len(), 1);
        assert_eq!(person::list_all(&conn, "owner-b").unwrap().len(), 1);
        assert!(person::get_by_id(&conn, "owner-b", &a.id).unwrap().is_none());
        assert!(person::get_by_id(&conn, "owner-a", &b.id).unwrap().is_none());

        // 跨用户更新/删除无效（update 回读失败报错；delete 静默无效）
        assert!(person::update(&conn, "owner-b", &a.id, req("篡改")).is_err());
        person::delete(&conn, "owner-b", &a.id).unwrap();
        assert!(person::get_by_id(&conn, "owner-a", &a.id).unwrap().is_some());

        // 跨用户建关系被拒（两端非同一 owner）
        assert!(relationship::create(
            &conn,
            "owner-a",
            CreateRelationshipRequest {
                from_person_id: a.id.clone(),
                to_person_id: b.id.clone(),
                relationship_type: "friend".to_string(),
                strength: None,
                description: None,
            },
        )
        .is_err());

        // 跨用户写互动被拒
        assert!(interaction::create(
            &conn,
            "owner-b",
            CreateInteractionRequest {
                person_id: a.id.clone(),
                timestamp: "2026-01-01T00:00:00Z".to_string(),
                content: "越权写入".to_string(),
                summary: None,
                topics: vec![],
                action_items: vec![],
            },
        )
        .is_err());

        // owner-a 自己的关系与互动正常
        let a2 = person::create(&conn, "owner-a", req("丙")).unwrap();
        relationship::create(
            &conn,
            "owner-a",
            CreateRelationshipRequest {
                from_person_id: a.id.clone(),
                to_person_id: a2.id.clone(),
                relationship_type: "friend".to_string(),
                strength: None,
                description: None,
            },
        )
        .unwrap();
        interaction::create(
            &conn,
            "owner-a",
            CreateInteractionRequest {
                person_id: a.id.clone(),
                timestamp: "2026-01-01T00:00:00Z".to_string(),
                content: "正常互动".to_string(),
                summary: None,
                topics: vec![],
                action_items: vec![],
            },
        )
        .unwrap();
        assert_eq!(relationship::list_all(&conn, "owner-a").unwrap().len(), 1);
        assert_eq!(relationship::list_all(&conn, "owner-b").unwrap().len(), 0);
        assert_eq!(interaction::list_by_person(&conn, "owner-a", &a.id).unwrap().len(), 1);
        assert_eq!(interaction::list_by_person(&conn, "owner-b", &a.id).unwrap().len(), 0);
    }

    /// 存量迁移：无 owner_id 列的老 persons 表，migrate 后补列并把
    /// 无主联系人回填到首个用户（幂等：再次 migrate 不改变归属）
    #[test]
    fn migrates_legacy_persons_table_backfills_owner() {
        let conn = Connection::open_in_memory().expect("in-memory db");
        conn.execute_batch(
            "CREATE TABLE persons (
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
            INSERT INTO persons (id, name, created_at, updated_at)
            VALUES ('p1', '存量联系人', '2026-01-01', '2026-01-01');
            CREATE TABLE users (
                id TEXT PRIMARY KEY,
                username TEXT UNIQUE NOT NULL,
                password_hash TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            INSERT INTO users (id, username, password_hash, created_at, updated_at)
            VALUES ('u1', 'admin', 'hash', '2026-01-01', '2026-01-01');",
        )
        .expect("legacy persons table");

        schema::migrate(&conn).expect("schema migration on legacy db");

        let owner: String = conn
            .query_row("SELECT owner_id FROM persons WHERE id = 'p1'", [], |row| row.get(0))
            .expect("owner_id backfilled");
        assert_eq!(owner, "u1");

        // 幂等：再次 migrate 不改变归属
        schema::migrate(&conn).expect("second migration");
        let owner: String = conn
            .query_row("SELECT owner_id FROM persons WHERE id = 'p1'", [], |row| row.get(0))
            .expect("owner_id stable");
        assert_eq!(owner, "u1");

        // 回填后归属用户可正常读到
        let persons = person::list_all(&conn, "u1").expect("list after backfill");
        assert_eq!(persons.len(), 1);
        assert!(person::list_all(&conn, "u2").unwrap().is_empty());
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

    /// 更早期的老库：users 表连 created_at/updated_at 都没有，
    /// 登录查询 SELECT ... updated_at FROM users 会报 no such column，
    /// migrate 后必须能按完整结构查询且时间戳非空
    #[test]
    fn migrates_ancient_users_table_without_timestamp_columns() {
        let conn = Connection::open_in_memory().expect("in-memory db");
        conn.execute_batch(
            "CREATE TABLE users (
                id TEXT PRIMARY KEY,
                username TEXT UNIQUE NOT NULL,
                password_hash TEXT NOT NULL
            );
            INSERT INTO users (id, username, password_hash) VALUES ('u1', 'admin', 'hash');",
        )
        .expect("ancient users table");

        schema::migrate(&conn).expect("schema migration on ancient db");

        // 与 db/user.rs 登录查询相同的列集合
        let (created_at, updated_at): (String, String) = conn
            .query_row(
                "SELECT created_at, updated_at FROM users WHERE username = 'admin'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("full users select");
        assert!(!created_at.is_empty());
        assert!(!updated_at.is_empty());
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

    // === agent_skills: skill_markdown ===

    /// 临时验证：真实库 owner_id 回填检查（手动执行，默认 #[ignore]）
    #[test]
    #[ignore]
    fn verify_real_db_owner_backfill() {
        let data_dir = std::env::var("RG_DATA_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| dirs::data_dir().unwrap().join("relationship-graph"));
        let key_hex = std::fs::read_to_string(data_dir.join("db.key")).expect("read db.key");
        let conn = crate::db::crypto::open_encrypted_db(data_dir.join("app.db"), key_hex.trim()).expect("open db");
        let total: i64 = conn.query_row("SELECT COUNT(*) FROM persons", [], |r| r.get(0)).unwrap();
        let nulls: i64 = conn.query_row("SELECT COUNT(*) FROM persons WHERE owner_id IS NULL", [], |r| r.get(0)).unwrap();
        let users: i64 = conn.query_row("SELECT COUNT(*) FROM users", [], |r| r.get(0)).unwrap();
        println!("TOTAL={} NULL_OWNER={} USERS={}", total, nulls, users);
        assert_eq!(nulls, 0, "存在未回填归属的联系人");
    }

    fn create_test_agent(conn: &Connection, mention: &str) -> String {
        let agent = agent_config::create_digital_agent(
            conn,
            CreateDigitalAgentRequest {
                display_name: "测试数字人".to_string(),
                mention: mention.to_string(),
                aliases: vec![],
                route_mode: None,
                avatar_url: None,
                description: None,
                skill_description: None,
                is_active: None,
                sort_order: None,
            },
        )
        .expect("create digital agent");
        agent.id
    }

    fn skill_request(agent_id: &str, name: &str, markdown: Option<&str>, active: bool) -> CreateAgentSkillRequest {
        CreateAgentSkillRequest {
            agent_id: agent_id.to_string(),
            skill_name: name.to_string(),
            skill_config_json: None,
            skill_markdown: markdown.map(|s| s.to_string()),
            trigger_scenario: None,
            is_active: Some(active),
        }
    }

    /// skill_config_json 请求侧为 None 时落库 "{}"，skill_markdown 往返读写一致
    #[test]
    fn skill_defaults_config_json_and_roundtrips_markdown() {
        let conn = in_memory_db();
        let agent_id = create_test_agent(&conn, "@测试一");
        let md = "---\nname: demo\ndescription: 演示技能\n---\n正文内容";

        let skill = agent_config::create_agent_skill(&conn, skill_request(&agent_id, "演示", Some(md), true))
            .expect("create skill");
        assert_eq!(skill.skill_config_json, "{}");
        assert_eq!(skill.skill_markdown.as_deref(), Some(md));

        // update 改写 markdown 并回读
        let updated_md = "---\nname: demo2\ndescription: 更新后\n---\n新正文";
        agent_config::update_agent_skill(&conn, &skill.id, skill_request(&agent_id, "演示", Some(updated_md), true))
            .expect("update skill");
        let skills = agent_config::list_agent_skills(&conn, &agent_id).expect("list skills");
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].skill_markdown.as_deref(), Some(updated_md));
        assert_eq!(skills[0].skill_config_json, "{}");
    }

    /// 老库升级：没有 skill_markdown 列的老 agent_skills 表，migrate 后应能补列并正常读写
    #[test]
    fn migrates_legacy_agent_skills_table_without_markdown_column() {
        let conn = Connection::open_in_memory().expect("in-memory db");
        conn.execute_batch(
            "CREATE TABLE digital_agents (
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
            CREATE TABLE agent_skills (
                id TEXT PRIMARY KEY,
                agent_id TEXT NOT NULL,
                skill_name TEXT NOT NULL,
                skill_config_json TEXT NOT NULL,
                trigger_scenario TEXT,
                is_active INTEGER NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            INSERT INTO digital_agents (id, display_name, mention, created_at, updated_at)
            VALUES ('a1', '老数字人', '@老数字人', '2026-01-01', '2026-01-01');
            INSERT INTO agent_skills (id, agent_id, skill_name, skill_config_json, created_at, updated_at)
            VALUES ('s1', 'a1', '老技能', '{}', '2026-01-01', '2026-01-01');",
        )
        .expect("legacy agent_skills table");

        schema::migrate(&conn).expect("schema migration on legacy db");

        let skills = agent_config::list_agent_skills(&conn, "a1").expect("list after migrate");
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].skill_markdown, None);
    }

    // === validate_skill_markdown ===

    #[test]
    fn validate_skill_markdown_accepts_valid_frontmatter() {
        let md = "---\nname: 联系人管家\ndescription: 管理联系人\n---\n# 正文\n内容";
        assert!(agent_config::validate_skill_markdown(md).is_ok());
        // BOM 头与首个键生效语义（与前端 parseFrontmatter 一致）
        let bom_md = "\u{FEFF}---\nname: a\n  name: nested\ndescription: d\n---\n正文";
        assert!(agent_config::validate_skill_markdown(bom_md).is_ok());
        // 空白内容视为未填写，直接放行
        assert!(agent_config::validate_skill_markdown("").is_ok());
        assert!(agent_config::validate_skill_markdown("   \n ").is_ok());
    }

    #[test]
    fn validate_skill_markdown_rejects_bad_frontmatter() {
        // 不以 --- 开头
        let err = agent_config::validate_skill_markdown("# 标题\n内容").unwrap_err();
        assert!(err.contains("---"));
        // 缺少闭合 ---
        let err = agent_config::validate_skill_markdown("---\nname: a\ndescription: b").unwrap_err();
        assert!(err.contains("闭合"));
        // 缺 name 键
        let err = agent_config::validate_skill_markdown("---\ndescription: b\n---\n正文").unwrap_err();
        assert!(err.contains("name"));
        // description 值为空
        let err = agent_config::validate_skill_markdown("---\nname: a\ndescription:\n---\n正文").unwrap_err();
        assert!(err.contains("description"));
    }

    // === build_skills_prompt ===

    /// 无技能返回空串；skill_markdown 为空/空白的技能跳过；停用技能不参与；
    /// 拼接前剥离每条技能的 frontmatter
    #[test]
    fn build_skills_prompt_filters_and_orders_skills() {
        let conn = in_memory_db();
        let agent_id = create_test_agent(&conn, "@测试二");

        // 无技能
        assert_eq!(agent_config::build_skills_prompt(&conn, &agent_id).unwrap(), "");

        let md1 = "---\nname: s1\ndescription: d1\n---\n技能一正文";
        let md2 = "---\nname: s2\ndescription: d2\n---\n技能二正文";
        agent_config::create_agent_skill(&conn, skill_request(&agent_id, "技能一", Some(md1), true)).unwrap();
        agent_config::create_agent_skill(&conn, skill_request(&agent_id, "无正文", None, true)).unwrap();
        agent_config::create_agent_skill(&conn, skill_request(&agent_id, "空白正文", Some("   "), true)).unwrap();
        agent_config::create_agent_skill(&conn, skill_request(&agent_id, "技能二", Some(md2), true)).unwrap();
        // 仅剩 frontmatter（剥离后正文为空）的技能同样跳过
        agent_config::create_agent_skill(&conn, skill_request(&agent_id, "纯头部", Some("---\nname: s3\ndescription: d3\n---"), true)).unwrap();
        agent_config::create_agent_skill(&conn, skill_request(&agent_id, "已停用", Some(md1), false)).unwrap();

        let prompt = agent_config::build_skills_prompt(&conn, &agent_id).unwrap();
        assert!(prompt.contains("### 技能：技能一"));
        assert!(prompt.contains("### 技能：技能二"));
        assert!(prompt.contains("技能一正文"));
        assert!(!prompt.contains("无正文"));
        assert!(!prompt.contains("空白正文"));
        assert!(!prompt.contains("纯头部"));
        assert!(!prompt.contains("已停用"));
        // frontmatter 已剥离：不包含 frontmatter 键值与分隔线
        assert!(!prompt.contains("name: s1"));
        assert!(!prompt.contains("name: s2"));
        assert!(!prompt.contains("description: d1"));
        assert!(!prompt.contains("---"));
        // 按 created_at 排序：技能一在前
        assert!(prompt.find("技能：技能一").unwrap() < prompt.find("技能：技能二").unwrap());
        // 每条以空行分隔结尾
        assert!(prompt.ends_with("\n\n"));
    }

    // === strip_skill_frontmatter ===

    /// 标准 frontmatter 剥离；无 frontmatter / 未闭合 / 空白 / BOM 等边界情形
    #[test]
    fn strip_skill_frontmatter_cases() {
        // 标准 frontmatter → 仅保留正文（含正文首行前的换行，调用方 trim）
        let md = "---\nname: s\ndescription: d\n---\n正文第一行\n正文第二行";
        assert_eq!(agent_config::strip_skill_frontmatter(md).trim(), "正文第一行\n正文第二行");
        // BOM + 首部空白同样剥离
        let bom_md = "\u{FEFF}---\nname: a\ndescription: b\n---\n正文";
        assert_eq!(agent_config::strip_skill_frontmatter(bom_md).trim(), "正文");
        // 无 frontmatter → 原文保留
        let plain = "# 标题\n内容";
        assert_eq!(agent_config::strip_skill_frontmatter(plain), plain);
        // 未闭合 → 原文保留
        let unclosed = "---\nname: a\ndescription: b\n正文";
        assert_eq!(agent_config::strip_skill_frontmatter(unclosed), unclosed);
        // 空白内容 → 原文保留
        assert_eq!(agent_config::strip_skill_frontmatter("   "), "   ");
        // 闭合后无正文 → 空串
        assert_eq!(agent_config::strip_skill_frontmatter("---\nname: a\ndescription: b\n---").trim(), "");
    }

    // === apply_skill_budget ===

    /// 预算截断：未超预算原文返回；超限时回退到最近 `### 技能：` 边界并追加说明
    #[test]
    fn apply_skill_budget_truncates_at_section_boundary() {
        // 未超预算：原文返回
        let short = "### 技能：一\n内容\n\n";
        assert_eq!(agent_config::apply_skill_budget(short, 3000), short);

        let s1 = "### 技能：一\n内容一\n\n";
        let s2 = "### 技能：二\n内容二\n\n";
        let full = format!("{}{}", s1, s2);
        let total = full.chars().count();

        // 预算恰好容纳全文：不截断
        assert_eq!(agent_config::apply_skill_budget(&full, total), full);

        // 预算能容下第一段、容不下第二段 → 截到第二段边界，仅保留第一段 + 说明行
        let budget = total - 1;
        let out = agent_config::apply_skill_budget(&full, budget);
        assert!(out.starts_with(s1));
        assert!(!out.contains("### 技能：二"));
        assert!(out.contains("（注：技能内容超出字符预算"));

        // 首段即超预算 → 退化为仅保留截断说明
        let tiny = agent_config::apply_skill_budget(&full, 1);
        assert!(!tiny.contains("### 技能："));
        assert!(tiny.contains("（注：技能内容超出字符预算 1"));
    }

    /// 预算默认值与 env 覆盖（单测进程内设置环境变量）
    #[test]
    fn skill_budget_env_override() {
        std::env::remove_var("RG_SKILL_BUDGET_CHARS");
        assert_eq!(agent_config::skill_budget_chars(), 3000);
        std::env::set_var("RG_SKILL_BUDGET_CHARS", "128");
        assert_eq!(agent_config::skill_budget_chars(), 128);
        // 非法值回退默认
        std::env::set_var("RG_SKILL_BUDGET_CHARS", "abc");
        assert_eq!(agent_config::skill_budget_chars(), 3000);
        std::env::remove_var("RG_SKILL_BUDGET_CHARS");
    }

    // === 用户画像常驻技能（get_profile_doc / build_profile_skill_prompt / apply_profile_budget） ===

    /// get_profile_doc 降级路径：用户不存在 / 画像未完成 / 文档空白均返回 None，
    /// 已完成且非空时返回原文（不携带 password_hash 等其他字段）
    #[test]
    fn get_profile_doc_gate_and_degradation() {
        let conn = in_memory_db();
        // 用户不存在 → None
        assert_eq!(user_db::get_profile_doc(&conn, "no-such-user").unwrap(), None);

        // 画像未完成（profile_completed=0，即使带文档）→ None
        let user = user_db::create_user(
            &conn,
            CreateUserRequest {
                username: "profile_user".to_string(),
                password_hash: "hash".to_string(),
                display_name: None,
                role: None,
                profile_doc: Some("草稿画像".to_string()),
            },
        )
        .expect("create user");
        assert_eq!(user_db::get_profile_doc(&conn, &user.id).unwrap(), None);

        // 完成但文档为空白 → None
        conn.execute(
            "UPDATE users SET profile_doc = '   ', profile_completed = 1 WHERE id = ?1",
            [&user.id],
        )
        .unwrap();
        assert_eq!(user_db::get_profile_doc(&conn, &user.id).unwrap(), None);

        // 完成且非空 → 返回原文
        conn.execute(
            "UPDATE users SET profile_doc = '# 画像\n重视长期主义' WHERE id = ?1",
            [&user.id],
        )
        .unwrap();
        assert_eq!(
            user_db::get_profile_doc(&conn, &user.id).unwrap().as_deref(),
            Some("# 画像\n重视长期主义")
        );
    }

    /// build_profile_skill_prompt：空/空白返回空串（此时注入链路 skills 为空，
    /// general_chat_prompt 与无画像现状逐字节一致——llm.rs 既有基准单测锁定）；
    /// 非空包装为 `### 技能：用户画像` 段并剥离意外携带的 frontmatter
    #[test]
    fn build_profile_skill_prompt_wraps_and_strips() {
        // 空/空白/仅剩 frontmatter → 空串（三种降级路径）
        assert_eq!(agent_config::build_profile_skill_prompt(""), "");
        assert_eq!(agent_config::build_profile_skill_prompt("   \n"), "");
        assert_eq!(agent_config::build_profile_skill_prompt("---\nname: p\ndescription: d\n---"), "");

        // 正常包装
        let section = agent_config::build_profile_skill_prompt("# 我的画像\n重视长期主义");
        assert_eq!(section, "### 技能：用户画像\n# 我的画像\n重视长期主义\n\n");

        // 意外携带 frontmatter → 剥离后包装
        let with_fm = "---\nname: p\ndescription: d\n---\n正文内容";
        assert_eq!(
            agent_config::build_profile_skill_prompt(with_fm),
            "### 技能：用户画像\n正文内容\n\n"
        );
    }

    /// apply_profile_budget：未超预算原文返回；超预算按字符截断并追加说明
    #[test]
    fn apply_profile_budget_truncates_by_chars() {
        let section = "### 技能：用户画像\n画像正文\n\n";
        let total = section.chars().count();
        // 未超 / 恰好等于预算 → 原文
        assert_eq!(agent_config::apply_profile_budget(section, total), section);
        assert_eq!(agent_config::apply_profile_budget(section, total + 10), section);
        // 超预算 → 截到预算字符数 + 说明行（多字节字符不被剖半）
        let out = agent_config::apply_profile_budget(section, 6);
        assert!(out.starts_with("### 技能"));
        assert!(out.contains("（注：用户画像超出字符预算 6，已截断）"));
        assert!(!out.contains("画像正文"));
    }

    /// 画像预算默认值与 env 覆盖（单测进程内设置环境变量）
    #[test]
    fn profile_skill_budget_env_override() {
        std::env::remove_var("RG_PROFILE_SKILL_BUDGET_CHARS");
        assert_eq!(agent_config::profile_skill_budget_chars(), 4000);
        std::env::set_var("RG_PROFILE_SKILL_BUDGET_CHARS", "256");
        assert_eq!(agent_config::profile_skill_budget_chars(), 256);
        // 非法值回退默认
        std::env::set_var("RG_PROFILE_SKILL_BUDGET_CHARS", "abc");
        assert_eq!(agent_config::profile_skill_budget_chars(), 4000);
        std::env::remove_var("RG_PROFILE_SKILL_BUDGET_CHARS");
    }

    /// 合并注入顺序与共享预算：画像段在前（最高优先级）、数字人技能段在后；
    /// 共享预算不足时截断从尾部按段边界砍，画像段优先保留
    #[test]
    fn profile_and_agent_skills_merge_order_and_budget() {
        let conn = in_memory_db();
        let agent_id = create_test_agent(&conn, "@测试画像");
        let md = "---\nname: s1\ndescription: d1\n---\n技能正文";
        agent_config::create_agent_skill(&conn, skill_request(&agent_id, "演示技能", Some(md), true)).unwrap();

        let profile_section = agent_config::build_profile_skill_prompt("用户画像正文");
        let agent_section = agent_config::build_skills_prompt(&conn, &agent_id).unwrap();
        let merged = format!("{}{}", profile_section, agent_section);

        // 画像段在最前，数字人技能段在后
        assert!(merged.starts_with("### 技能：用户画像\n用户画像正文\n\n"));
        assert!(merged.find("技能：用户画像").unwrap() < merged.find("技能：演示技能").unwrap());

        // 预算容纳全文 → 不截断
        assert_eq!(agent_config::apply_skill_budget(&merged, merged.chars().count()), merged);

        // 预算仅容画像段 → 数字人技能段被砍，画像保留（画像在前天然优先）
        let budget = profile_section.chars().count() + 1;
        let out = agent_config::apply_skill_budget(&merged, budget);
        assert!(out.starts_with(&profile_section));
        assert!(!out.contains("技能：演示技能"));
        assert!(out.contains("（注：技能内容超出字符预算"));
    }
}
