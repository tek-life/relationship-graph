#[cfg(test)]
mod tests {
    use crate::db::{agent_config, person, schema, skill_package, user as user_db};
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

    // ============================================================
    // 技能包（多文件）：纯函数 + 迁移 + 注入 + 绑定
    // ============================================================

    // === normalize_rel_path ===

    #[test]
    fn normalize_rel_path_accepts_valid_relative_paths() {
        assert_eq!(skill_package::normalize_rel_path("SKILL.md").unwrap(), "SKILL.md");
        assert_eq!(skill_package::normalize_rel_path("docs/guide.md").unwrap(), "docs/guide.md");
        // 忽略 `.` 分量与重复分隔符
        assert_eq!(skill_package::normalize_rel_path("./a/b.txt").unwrap(), "a/b.txt");
        assert_eq!(skill_package::normalize_rel_path("a//b.md").unwrap(), "a/b.md");
        assert_eq!(skill_package::normalize_rel_path("a/./b/c.txt").unwrap(), "a/b/c.txt");
        // 首尾空白先 trim
        assert_eq!(skill_package::normalize_rel_path("  docs/x.md  ").unwrap(), "docs/x.md");
    }

    #[test]
    fn normalize_rel_path_rejects_invalid_paths() {
        // 空路径
        assert!(skill_package::normalize_rel_path("").is_err());
        assert!(skill_package::normalize_rel_path("   ").is_err());
        assert!(skill_package::normalize_rel_path("/").is_err());
        assert!(skill_package::normalize_rel_path("./.").is_err());
        // 绝对路径
        assert!(skill_package::normalize_rel_path("/etc/passwd").is_err());
        // 含 .. 分量
        assert!(skill_package::normalize_rel_path("../x").is_err());
        assert!(skill_package::normalize_rel_path("a/../b").is_err());
        assert!(skill_package::normalize_rel_path("a/..").is_err());
        // 反斜杠（Windows 风格路径不接受）
        assert!(skill_package::normalize_rel_path("a\\b.md").is_err());
    }

    // === parse_skill_package ===

    fn sample_skill_md() -> &'static str {
        "---\nname: 示例技能包\ndescription: 演示用途\n---\n# 正文标题\n正文内容"
    }

    #[test]
    fn parse_skill_package_normal_package() {
        let files = vec![
            ("SKILL.md".to_string(), sample_skill_md().to_string()),
            ("docs/a.md".to_string(), "附属文档一\n详细说明".to_string()),
            ("notes.txt".to_string(), "纯文本备注".to_string()),
            ("scripts/run.sh".to_string(), "#!/bin/bash\necho hi".to_string()),
        ];
        let manifest = skill_package::parse_skill_package(&files).expect("parse package");
        assert_eq!(manifest.name, "示例技能包");
        assert_eq!(manifest.description, "演示用途");
        assert_eq!(manifest.body, "# 正文标题\n正文内容");
        assert_eq!(manifest.entry_path, "SKILL.md");

        // 附属文档按 rel_path 升序；非文本文件标记不注入
        assert_eq!(manifest.sub_docs.len(), 3);
        assert_eq!(manifest.sub_docs[0].rel_path, "docs/a.md");
        assert_eq!(manifest.sub_docs[0].summary, "附属文档一");
        assert!(manifest.sub_docs[0].injectable);
        assert_eq!(manifest.sub_docs[1].rel_path, "notes.txt");
        assert!(manifest.sub_docs[1].injectable);
        assert_eq!(manifest.sub_docs[2].rel_path, "scripts/run.sh");
        assert!(!manifest.sub_docs[2].injectable);
        assert_eq!(manifest.sub_docs[2].summary, "");
    }

    #[test]
    fn parse_skill_package_summary_truncated_to_80_chars() {
        let long_line = "长".repeat(120);
        let files = vec![
            ("SKILL.md".to_string(), sample_skill_md().to_string()),
            ("long.md".to_string(), format!("\n\n{}尾部", long_line)),
        ];
        let manifest = skill_package::parse_skill_package(&files).expect("parse");
        // 首行非空（跳过前导空行）且截断到 80 字符
        assert_eq!(manifest.sub_docs[0].summary.chars().count(), 80);
        assert!(manifest.sub_docs[0].summary.chars().all(|c| c == '长'));
    }

    #[test]
    fn parse_skill_package_errors() {
        // 空列表
        let err = skill_package::parse_skill_package(&[]).unwrap_err();
        assert!(err.contains("不能为空"));
        // 无 SKILL.md
        let files = vec![("docs/a.md".to_string(), "内容".to_string())];
        let err = skill_package::parse_skill_package(&files).unwrap_err();
        assert!(err.contains("SKILL.md"));
        // frontmatter 不合法
        let files = vec![("SKILL.md".to_string(), "# 无头部\n正文".to_string())];
        let err = skill_package::parse_skill_package(&files).unwrap_err();
        assert!(err.contains("SKILL.md 校验失败"));
        // 重复路径
        let files = vec![
            ("SKILL.md".to_string(), sample_skill_md().to_string()),
            ("a.md".to_string(), "一".to_string()),
            ("./a.md".to_string(), "二".to_string()),
        ];
        let err = skill_package::parse_skill_package(&files).unwrap_err();
        assert!(err.contains("重复"));
    }

    #[test]
    fn parse_skill_package_nested_root_and_case_insensitive() {
        // 嵌套一层目录的仓库风格：包根为 pkg/，路径去前缀
        let files = vec![
            ("pkg/SKILL.md".to_string(), sample_skill_md().to_string()),
            ("pkg/docs/x.md".to_string(), "嵌套附属文档".to_string()),
        ];
        let manifest = skill_package::parse_skill_package(&files).expect("parse nested");
        assert_eq!(manifest.entry_path, "SKILL.md");
        assert_eq!(manifest.sub_docs.len(), 1);
        assert_eq!(manifest.sub_docs[0].rel_path, "docs/x.md");

        // 文件名大小写不敏感
        let files = vec![("Skill.MD".to_string(), sample_skill_md().to_string())];
        let manifest = skill_package::parse_skill_package(&files).expect("parse case-insensitive");
        assert_eq!(manifest.name, "示例技能包");

        // 多个同级 SKILL.md → 报歧义错误
        let files = vec![
            ("a/skill.md".to_string(), sample_skill_md().to_string()),
            ("b/SKILL.md".to_string(), sample_skill_md().to_string()),
        ];
        let err = skill_package::parse_skill_package(&files).unwrap_err();
        assert!(err.contains("多个"));
    }

    // === assemble_package_section ===

    fn sample_manifest(sub_docs: Vec<skill_package::SubDocInfo>) -> skill_package::PackageManifest {
        skill_package::PackageManifest {
            name: "演示包".to_string(),
            description: "描述".to_string(),
            body: "正文内容".to_string(),
            entry_prefix: String::new(),
            entry_path: "SKILL.md".to_string(),
            sub_docs,
        }
    }

    #[test]
    fn assemble_package_section_normal_and_empty_subdocs() {
        // 无附属文档 → 仅标题 + 正文
        let manifest = sample_manifest(vec![]);
        let section = skill_package::assemble_package_section(&manifest, 2000);
        assert_eq!(section, "### 技能：演示包\n正文内容\n\n");

        // 含附属文档 → 附 `#### 附属文档` 索引；不注入的文件不出现
        let manifest = sample_manifest(vec![
            skill_package::SubDocInfo { rel_path: "docs/a.md".into(), summary: "摘要A".into(), injectable: true },
            skill_package::SubDocInfo { rel_path: "run.sh".into(), summary: "".into(), injectable: false },
        ]);
        let section = skill_package::assemble_package_section(&manifest, 2000);
        assert!(section.starts_with("### 技能：演示包\n正文内容\n\n#### 附属文档\n"));
        assert!(section.contains("- docs/a.md: 摘要A\n"));
        assert!(!section.contains("run.sh"));
        assert!(section.ends_with("\n\n"));
        assert!(!section.contains("已截断"));
    }

    #[test]
    fn assemble_package_section_truncates_index_over_budget() {
        let manifest = sample_manifest(vec![
            skill_package::SubDocInfo { rel_path: "a.md".into(), summary: "摘要A".into(), injectable: true },
            skill_package::SubDocInfo { rel_path: "b.md".into(), summary: "摘要B".into(), injectable: true },
        ]);
        let header = "### 技能：演示包\n正文内容\n\n#### 附属文档\n";
        let line1 = "- a.md: 摘要A\n";
        // 预算恰好容纳头部 + 第一行 → 第二行被截断
        let budget = header.chars().count() + line1.chars().count();
        let section = skill_package::assemble_package_section(&manifest, budget);
        assert!(section.contains(line1));
        assert!(!section.contains("b.md"));
        assert!(section.contains("（注：附属文档索引超出单包预算，已截断）"));

        // 预算充足 → 不截断
        let full = skill_package::assemble_package_section(&manifest, 2000);
        assert!(full.contains("b.md"));
        assert!(!full.contains("已截断"));
    }

    /// 单包预算约束正文：SKILL.md 正文超预算时按字符截断并附截断说明，
    /// 截断后仍拼附属文档索引；整段不超预算
    #[test]
    fn assemble_package_section_truncates_body_over_budget() {
        let long_body = "长".repeat(5000);
        let manifest = skill_package::PackageManifest {
            name: "演示包".to_string(),
            description: "描述".to_string(),
            body: long_body.clone(),
            entry_prefix: String::new(),
            entry_path: "SKILL.md".to_string(),
            sub_docs: vec![skill_package::SubDocInfo {
                rel_path: "a.md".into(),
                summary: "摘要A".into(),
                injectable: true,
            }],
        };

        // 正文未超预算：逐字节保持原格式（不引入截断说明）
        let section = skill_package::assemble_package_section(&manifest, 10000);
        assert!(section.starts_with(&format!("### 技能：演示包\n{}\n\n", long_body)));
        assert!(section.contains("- a.md: 摘要A\n"));
        assert!(!section.contains("正文超出单包预算"));

        // 正文超预算：按字符截断 + 截断说明，整段（含附属文档索引）不超预算
        let budget = 200;
        let section = skill_package::assemble_package_section(&manifest, budget);
        assert!(section.starts_with("### 技能：演示包\n"));
        assert!(section.contains("（注：SKILL.md 正文超出单包预算，已截断）"));
        assert!(section.chars().count() <= budget);
        assert!(!section.contains(&long_body));

        // 无附属文档时同样受正文预算约束
        let no_docs = skill_package::PackageManifest {
            sub_docs: vec![],
            ..manifest.clone()
        };
        let section = skill_package::assemble_package_section(&no_docs, budget);
        assert!(section.contains("（注：SKILL.md 正文超出单包预算，已截断）"));
        assert!(section.chars().count() <= budget);
    }

    // === skill_package_budget_chars ===

    #[test]
    fn skill_package_budget_env_override() {
        std::env::remove_var("RG_SKILL_PACKAGE_BUDGET_CHARS");
        assert_eq!(skill_package::skill_package_budget_chars(), 2000);
        std::env::set_var("RG_SKILL_PACKAGE_BUDGET_CHARS", "512");
        assert_eq!(skill_package::skill_package_budget_chars(), 512);
        // 非法值回退默认
        std::env::set_var("RG_SKILL_PACKAGE_BUDGET_CHARS", "abc");
        assert_eq!(skill_package::skill_package_budget_chars(), 2000);
        std::env::remove_var("RG_SKILL_PACKAGE_BUDGET_CHARS");
    }

    // === 技能包 CRUD 与绑定 ===

    fn package_files(entries: &[(&str, &str)]) -> Vec<(String, String)> {
        entries.iter().map(|(p, c)| (p.to_string(), c.to_string())).collect()
    }

    #[test]
    fn create_get_list_delete_skill_package_roundtrip() {
        let conn = in_memory_db();
        let files = package_files(&[
            ("SKILL.md", sample_skill_md()),
            ("docs/a.md", "附属文档"),
        ]);
        let pkg = skill_package::create_skill_package(&conn, "演示包", Some("描述".into()), "imported", &files)
            .expect("create package");
        assert_eq!(pkg.display_name, "演示包");
        assert_eq!(pkg.source_kind, "imported");
        assert!(pkg.slug.starts_with("演示包-") || pkg.slug.starts_with("skill-"));
        assert_eq!(pkg.total_chars, sample_skill_md().chars().count() + "附属文档".chars().count());
        assert!(pkg.is_active);

        // 列表不含文件内容；详情含文件
        let list = skill_package::list_skill_packages(&conn).unwrap();
        assert_eq!(list.len(), 1);
        assert!(list[0].files.is_none());
        let detail = skill_package::get_skill_package(&conn, &pkg.id).unwrap().expect("package exists");
        let files = detail.files.expect("files loaded");
        assert_eq!(files.len(), 2);
        assert!(files.iter().any(|f| f.rel_path == "SKILL.md"));

        // 删除后列表为空，重复删除不报错
        skill_package::delete_skill_package(&conn, &pkg.id).unwrap();
        assert!(skill_package::list_skill_packages(&conn).unwrap().is_empty());
        assert!(skill_package::get_skill_package(&conn, &pkg.id).unwrap().is_none());
        skill_package::delete_skill_package(&conn, &pkg.id).unwrap();
    }

    #[test]
    fn replace_bindings_full_replacement_semantics() {
        let conn = in_memory_db();
        let agent_id = create_test_agent(&conn, "@绑定测试");
        let p1 = skill_package::create_skill_package(&conn, "包一", None, "inline", &package_files(&[("SKILL.md", sample_skill_md())])).unwrap();
        let p2 = skill_package::create_skill_package(&conn, "包二", None, "inline", &package_files(&[("SKILL.md", sample_skill_md())])).unwrap();

        // 初始绑定两个（JOIN 取到 display_name）
        skill_package::replace_bindings(&conn, &agent_id, vec![(p1.id.clone(), 0), (p2.id.clone(), 1)]).unwrap();
        let bindings = skill_package::list_bindings(&conn, &agent_id).unwrap();
        assert_eq!(bindings.len(), 2);
        assert_eq!(bindings[0].package_display_name, "包一");
        assert_eq!(bindings[1].package_display_name, "包二");

        // 全量替换为单个 → 旧绑定被删
        skill_package::replace_bindings(&conn, &agent_id, vec![(p2.id.clone(), 5)]).unwrap();
        let bindings = skill_package::list_bindings(&conn, &agent_id).unwrap();
        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings[0].package_id, p2.id);
        assert_eq!(bindings[0].sort_order, 5);

        // 重复 package_id 去重（以最后一次为准）
        skill_package::replace_bindings(&conn, &agent_id, vec![(p1.id.clone(), 1), (p1.id.clone(), 9)]).unwrap();
        let bindings = skill_package::list_bindings(&conn, &agent_id).unwrap();
        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings[0].sort_order, 9);

        // 替换为空 → 清空全部绑定
        skill_package::replace_bindings(&conn, &agent_id, vec![]).unwrap();
        assert!(skill_package::list_bindings(&conn, &agent_id).unwrap().is_empty());
    }

    // === build_skills_prompt（技能包注入） ===

    #[test]
    fn build_skills_prompt_package_binding_section_format() {
        let conn = in_memory_db();
        let agent_id = create_test_agent(&conn, "@包注入");

        // 无绑定 → 空串
        assert_eq!(agent_config::build_skills_prompt(&conn, &agent_id).unwrap(), "");

        let files = package_files(&[
            ("SKILL.md", sample_skill_md()),
            ("docs/a.md", "附属文档一"),
        ]);
        let pkg = skill_package::create_skill_package(&conn, "展示名", None, "imported", &files).unwrap();
        skill_package::replace_bindings(&conn, &agent_id, vec![(pkg.id.clone(), 0)]).unwrap();

        let prompt = agent_config::build_skills_prompt(&conn, &agent_id).unwrap();
        // 段标题用包展示名；正文剥离 frontmatter；附属文档索引来自 manifest
        assert!(prompt.starts_with("### 技能：展示名\n# 正文标题\n正文内容\n\n"));
        assert!(prompt.contains("#### 附属文档\n- docs/a.md: 附属文档一\n"));
        assert!(!prompt.contains("name: 示例技能包"));
        assert!(prompt.ends_with("\n\n"));

        // is_active=0 的包不注入
        conn.execute("UPDATE skill_packages SET is_active = 0 WHERE id = ?1", [&pkg.id]).unwrap();
        assert_eq!(agent_config::build_skills_prompt(&conn, &agent_id).unwrap(), "");
        conn.execute("UPDATE skill_packages SET is_active = 1 WHERE id = ?1", [&pkg.id]).unwrap();

        // 入口正文仅剩 frontmatter（剥离后为空）的包跳过
        let only_fm = package_files(&[("SKILL.md", "---\nname: x\ndescription: d\n---")]);
        let empty_pkg = skill_package::create_skill_package(&conn, "空正文包", None, "inline", &only_fm).unwrap();
        skill_package::replace_bindings(&conn, &agent_id, vec![(pkg.id.clone(), 0), (empty_pkg.id, 1)]).unwrap();
        let prompt = agent_config::build_skills_prompt(&conn, &agent_id).unwrap();
        assert!(prompt.contains("技能：展示名"));
        assert!(!prompt.contains("空正文包"));
    }

    #[test]
    fn build_skills_prompt_multi_package_sort_order() {
        let conn = in_memory_db();
        let agent_id = create_test_agent(&conn, "@多包排序");
        let p1 = skill_package::create_skill_package(&conn, "甲包", None, "inline", &package_files(&[("SKILL.md", "---\nname: a\ndescription: d\n---\n甲正文")])).unwrap();
        let p2 = skill_package::create_skill_package(&conn, "乙包", None, "inline", &package_files(&[("SKILL.md", "---\nname: b\ndescription: d\n---\n乙正文")])).unwrap();

        // 绑定顺序与创建顺序相反：sort_order 小者在前
        skill_package::replace_bindings(&conn, &agent_id, vec![(p1.id, 2), (p2.id, 1)]).unwrap();
        let prompt = agent_config::build_skills_prompt(&conn, &agent_id).unwrap();
        assert!(prompt.contains("### 技能：甲包"));
        assert!(prompt.contains("### 技能：乙包"));
        assert!(prompt.find("技能：乙包").unwrap() < prompt.find("技能：甲包").unwrap());
    }

    // === migrate_legacy_skills ===

    #[test]
    fn migrate_legacy_skills_idempotent_and_skips_json_form() {
        let conn = in_memory_db();
        let agent_id = create_test_agent(&conn, "@存量迁移");
        // 手工插入存量技能（绕过数据层同步），模拟老库数据：
        // 两条 markdown 技能 + 一条 JSON 形态（skill_markdown NULL）+ 一条空白 markdown
        conn.execute_batch(&format!(
            "INSERT INTO agent_skills (id, agent_id, skill_name, skill_config_json, skill_markdown, is_active, created_at, updated_at)
             VALUES ('ls1', '{a}', '存量一', '{{}}', '---\nname: l1\ndescription: d\n---\n正文一', 1, '2026-01-01', '2026-01-01');
             INSERT INTO agent_skills (id, agent_id, skill_name, skill_config_json, skill_markdown, is_active, created_at, updated_at)
             VALUES ('ls2', '{a}', '存量二', '{{}}', '---\nname: l2\ndescription: d\n---\n正文二', 1, '2026-01-02', '2026-01-02');
             INSERT INTO agent_skills (id, agent_id, skill_name, skill_config_json, skill_markdown, is_active, created_at, updated_at)
             VALUES ('ls3', '{a}', 'JSON形态', '{{\"prompt\":\"x\"}}', NULL, 1, '2026-01-03', '2026-01-03');
             INSERT INTO agent_skills (id, agent_id, skill_name, skill_config_json, skill_markdown, is_active, created_at, updated_at)
             VALUES ('ls4', '{a}', '空白', '{{}}', '   ', 1, '2026-01-04', '2026-01-04');",
            a = agent_id
        )).expect("insert legacy skills");

        // 首次迁移：仅两条 markdown 技能转包
        schema::migrate(&conn).expect("migrate with legacy skills");
        let packages = skill_package::list_skill_packages(&conn).unwrap();
        assert_eq!(packages.len(), 2);
        assert!(packages.iter().all(|p| p.source_kind == "inline"));
        assert_eq!(packages[0].slug, "legacy-ls1");
        assert_eq!(packages[1].slug, "legacy-ls2");

        // 单文件 SKILL.md，内容为原 skill_markdown
        let files = skill_package::list_package_files(&conn, &packages[0].id).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].rel_path, "SKILL.md");
        assert!(files[0].content.contains("正文一"));

        // 绑定：sort_order 按 created_at 顺序递增
        let bindings = skill_package::list_bindings(&conn, &agent_id).unwrap();
        assert_eq!(bindings.len(), 2);
        assert_eq!(bindings[0].package_display_name, "存量一");
        assert_eq!(bindings[0].sort_order, 0);
        assert_eq!(bindings[1].package_display_name, "存量二");
        assert_eq!(bindings[1].sort_order, 1);

        // 幂等：重复执行零副作用（无重复行）
        schema::migrate(&conn).expect("second migrate");
        schema::migrate(&conn).expect("third migrate");
        assert_eq!(skill_package::list_skill_packages(&conn).unwrap().len(), 2);
        assert_eq!(skill_package::list_bindings(&conn, &agent_id).unwrap().len(), 2);
        let file_count: i64 = conn.query_row("SELECT COUNT(*) FROM skill_package_files", [], |r| r.get(0)).unwrap();
        assert_eq!(file_count, 2);

        // 迁移后注入生效：两个存量技能按序拼接
        let prompt = agent_config::build_skills_prompt(&conn, &agent_id).unwrap();
        assert!(prompt.contains("### 技能：存量一"));
        assert!(prompt.contains("### 技能：存量二"));
        assert!(!prompt.contains("JSON形态"));
        assert!(!prompt.contains("空白"));
        assert!(prompt.find("技能：存量一").unwrap() < prompt.find("技能：存量二").unwrap());
    }

    // === 旧技能写入路径的 legacy 包同步 ===

    #[test]
    fn legacy_skill_write_path_syncs_package_and_binding() {
        let conn = in_memory_db();
        let agent_id = create_test_agent(&conn, "@旧技能同步");

        // 创建带 markdown 的技能 → 同步出 legacy 包 + 绑定
        let skill = agent_config::create_agent_skill(&conn, skill_request(&agent_id, "同步技能", Some(sample_skill_md()), true)).unwrap();
        let slug = format!("legacy-{}", skill.id);
        let package_id: String = conn.query_row("SELECT id FROM skill_packages WHERE slug = ?1", [&slug], |r| r.get(0)).unwrap();
        assert_eq!(skill_package::list_bindings(&conn, &agent_id).unwrap().len(), 1);

        // 更新内容 → 包文件同步更新
        let updated_md = "---\nname: n2\ndescription: d2\n---\n更新后正文";
        agent_config::update_agent_skill(&conn, &skill.id, skill_request(&agent_id, "同步技能", Some(updated_md), true)).unwrap();
        let files = skill_package::list_package_files(&conn, &package_id).unwrap();
        assert_eq!(files.len(), 1);
        assert!(files[0].content.contains("更新后正文"));

        // 停用技能 → 包 is_active=0，不再注入
        agent_config::update_agent_skill(&conn, &skill.id, skill_request(&agent_id, "同步技能", Some(updated_md), false)).unwrap();
        assert_eq!(agent_config::build_skills_prompt(&conn, &agent_id).unwrap(), "");

        // 删除技能 → legacy 包与绑定同步删除
        agent_config::delete_agent_skill(&conn, &skill.id).unwrap();
        assert!(skill_package::get_skill_package(&conn, &package_id).unwrap().is_none());
        assert!(skill_package::list_bindings(&conn, &agent_id).unwrap().is_empty());

        // 创建无 markdown 的技能 → 不产生包
        agent_config::create_agent_skill(&conn, skill_request(&agent_id, "JSON技能", None, true)).unwrap();
        assert!(skill_package::list_skill_packages(&conn).unwrap().is_empty());
    }

    // === M4：嵌套入口包落库归一化 + 注入 e2e ===

    /// 嵌套入口包（pkg/SKILL.md 风格）create 落库时按 manifest 归一化路径
    /// （rel_path 以 SKILL.md 为根），build_skills_prompt 能正常注入
    #[test]
    fn nested_package_files_normalized_and_injected() {
        let conn = in_memory_db();
        let agent_id = create_test_agent(&conn, "@嵌套包");
        let files = package_files(&[
            ("pkg/SKILL.md", sample_skill_md()),
            ("pkg/docs/x.md", "嵌套附属文档"),
            ("pkg/scripts/run.sh", "#!/bin/bash"),
        ]);

        // parse 阶段：entry_prefix 带目录前缀，清单内路径已归一
        let manifest = skill_package::parse_skill_package(&files).unwrap();
        assert_eq!(manifest.entry_prefix, "pkg/");
        assert_eq!(manifest.entry_path, "SKILL.md");

        let pkg = skill_package::create_skill_package(&conn, "嵌套包", None, "imported", &files).unwrap();
        // 落库文件路径已去前缀：根级 SKILL.md（注入 JOIN 命中）+ 附属文档
        let stored = skill_package::list_package_files(&conn, &pkg.id).unwrap();
        let paths: Vec<&str> = stored.iter().map(|f| f.rel_path.as_str()).collect();
        assert!(paths.contains(&"SKILL.md"));
        assert!(paths.contains(&"docs/x.md"));
        assert!(paths.contains(&"scripts/run.sh"));
        assert!(!paths.iter().any(|p| p.starts_with("pkg/")));

        // 端到端：绑定后 build_skills_prompt 正常注入（正文 + 附属文档索引）
        skill_package::replace_bindings(&conn, &agent_id, vec![(pkg.id.clone(), 0)]).unwrap();
        let prompt = agent_config::build_skills_prompt(&conn, &agent_id).unwrap();
        assert!(prompt.contains("### 技能：嵌套包"));
        assert!(prompt.contains("# 正文标题\n正文内容"));
        assert!(prompt.contains("#### 附属文档\n- docs/x.md: 嵌套附属文档\n"));
    }

    /// slug 冲突重试：同名展示名重复创建均成功且 slug 互不相同
    #[test]
    fn create_skill_package_generates_unique_slugs_with_retry() {
        let conn = in_memory_db();
        let files = package_files(&[("SKILL.md", sample_skill_md())]);
        let mut slugs = std::collections::HashSet::new();
        for _ in 0..10 {
            let pkg = skill_package::create_skill_package(&conn, "同名包", None, "inline", &files).unwrap();
            assert!(slugs.insert(pkg.slug.clone()), "slug 冲突：{}", pkg.slug);
        }
        assert_eq!(skill_package::list_skill_packages(&conn).unwrap().len(), 10);
    }

    /// create 层 parse 失败错误携带中文原因（不吞原因）
    #[test]
    fn create_skill_package_parse_error_carries_reason() {
        let conn = in_memory_db();
        // 缺 SKILL.md
        let err = skill_package::create_skill_package(
            &conn,
            "坏包",
            None,
            "inline",
            &package_files(&[("docs/a.md", "内容")]),
        )
        .unwrap_err();
        assert!(err.to_string().contains("SKILL.md"), "错误应携带中文原因：{}", err);
        // frontmatter 不合法
        let err = skill_package::create_skill_package(
            &conn,
            "坏包",
            None,
            "inline",
            &package_files(&[("SKILL.md", "# 无头部\n正文")]),
        )
        .unwrap_err();
        assert!(err.to_string().contains("SKILL.md 校验失败"), "错误应携带中文原因：{}", err);
    }

    // === M2：legacy 包绑定严格跟随技能的 agent_id ===

    /// PUT 移动技能 agent_id 后：绑定跟随迁移，旧数字人不再注入（无双份）
    #[test]
    fn legacy_binding_follows_agent_move() {
        let conn = in_memory_db();
        let a1 = create_test_agent(&conn, "@移动源");
        let a2 = create_test_agent(&conn, "@移动目标");
        let skill = agent_config::create_agent_skill(&conn, skill_request(&a1, "移动技能", Some(sample_skill_md()), true)).unwrap();
        assert_eq!(skill_package::list_bindings(&conn, &a1).unwrap().len(), 1);

        // 移动到 a2 → 绑定只剩 a2，a1 注入为空
        agent_config::update_agent_skill(&conn, &skill.id, skill_request(&a2, "移动技能", Some(sample_skill_md()), true)).unwrap();
        assert!(skill_package::list_bindings(&conn, &a1).unwrap().is_empty());
        let b2 = skill_package::list_bindings(&conn, &a2).unwrap();
        assert_eq!(b2.len(), 1);
        assert_eq!(agent_config::build_skills_prompt(&conn, &a1).unwrap(), "");
        assert!(agent_config::build_skills_prompt(&conn, &a2).unwrap().contains("### 技能：移动技能"));

        // 再次编辑保存 → 不产生双份绑定
        agent_config::update_agent_skill(&conn, &skill.id, skill_request(&a2, "移动技能", Some(sample_skill_md()), true)).unwrap();
        assert_eq!(skill_package::list_bindings(&conn, &a2).unwrap().len(), 1);
    }

    /// 管理员主动解绑（包无任何绑定）后，旧技能编辑保存不得复活绑定
    #[test]
    fn legacy_edit_does_not_resurrect_admin_unbound_package() {
        let conn = in_memory_db();
        let agent_id = create_test_agent(&conn, "@解绑复活");
        let skill = agent_config::create_agent_skill(&conn, skill_request(&agent_id, "复活测试", Some(sample_skill_md()), true)).unwrap();

        // 管理员通过全量替换清空绑定
        skill_package::replace_bindings(&conn, &agent_id, vec![]).unwrap();
        assert!(skill_package::list_bindings(&conn, &agent_id).unwrap().is_empty());

        // 编辑保存同一技能 → 包内容更新但绑定不复活
        agent_config::update_agent_skill(&conn, &skill.id, skill_request(&agent_id, "复活测试", Some(sample_skill_md()), true)).unwrap();
        assert!(skill_package::list_bindings(&conn, &agent_id).unwrap().is_empty());
        let package_id: String = conn
            .query_row("SELECT id FROM skill_packages WHERE slug = ?1", [format!("legacy-{}", skill.id)], |r| r.get(0))
            .unwrap();
        assert!(skill_package::get_skill_package(&conn, &package_id).unwrap().is_some());
    }

    // === M1：存量迁移孤儿 agent_id 不阻断启动 ===

    /// 孤儿 agent_id（数字人已不存在）的存量技能在 foreign_keys=ON 下
    /// 被跳过并告警，不阻断其余行迁移，也不阻断 migrate（重跑幂等）
    #[test]
    fn migrate_legacy_skills_skips_orphan_agent_without_blocking() {
        let conn = in_memory_db();
        let agent_id = create_test_agent(&conn, "@孤儿迁移");
        // 模拟历史脏数据：暂时关闭 FK 插入孤儿技能行（旧版本连接未强制外键）
        conn.execute_batch("PRAGMA foreign_keys = OFF;").unwrap();
        conn.execute_batch(&format!(
            "INSERT INTO agent_skills (id, agent_id, skill_name, skill_config_json, skill_markdown, is_active, created_at, updated_at)
             VALUES ('ok1', '{a}', '正常技能', '{{}}', '---\nname: ok\ndescription: d\n---\n正文', 1, '2026-01-01', '2026-01-01');
             INSERT INTO agent_skills (id, agent_id, skill_name, skill_config_json, skill_markdown, is_active, created_at, updated_at)
             VALUES ('orphan1', 'ghost-agent', '孤儿技能', '{{}}', '---\nname: o\ndescription: d\n---\n孤儿正文', 1, '2026-01-02', '2026-01-02');",
            a = agent_id
        )).expect("insert legacy rows");
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();

        // migrate 不得报错（不阻断启动）；正常行迁移成功，孤儿行跳过
        schema::migrate(&conn).expect("migrate must not fail on orphan agent");
        let packages = skill_package::list_skill_packages(&conn).unwrap();
        assert_eq!(packages.len(), 1);
        assert_eq!(packages[0].slug, "legacy-ok1");
        let bindings = skill_package::list_bindings(&conn, &agent_id).unwrap();
        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings[0].sort_order, 0);
        assert!(agent_config::build_skills_prompt(&conn, &agent_id).unwrap().contains("### 技能：正常技能"));

        // 重跑幂等：孤儿行仍被跳过，无重复包
        schema::migrate(&conn).expect("second migrate");
        assert_eq!(skill_package::list_skill_packages(&conn).unwrap().len(), 1);
    }

    // === m4：删除数字人后清理孤儿 legacy 包 ===

    #[test]
    fn delete_digital_agent_cleans_orphan_legacy_packages() {
        let conn = in_memory_db();
        let a1 = create_test_agent(&conn, "@待删除");
        let a2 = create_test_agent(&conn, "@保留");
        let skill = agent_config::create_agent_skill(&conn, skill_request(&a1, "随主技能", Some(sample_skill_md()), true)).unwrap();
        // imported 包也绑定到 a1（非 legacy，不应被孤儿清理误删）
        let imported = skill_package::create_skill_package(&conn, "导入包", None, "imported", &package_files(&[("SKILL.md", sample_skill_md())])).unwrap();
        skill_package::replace_bindings(&conn, &a1, vec![(imported.id.clone(), 1)]).unwrap();

        agent_config::delete_digital_agent(&conn, &a1).unwrap();

        // legacy 包已无绑定 → 被清理（包与文件一并）
        let packages = skill_package::list_skill_packages(&conn).unwrap();
        assert!(packages.iter().all(|p| p.slug != format!("legacy-{}", skill.id)));
        let file_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM skill_package_files WHERE package_id NOT IN (SELECT id FROM skill_packages)", [], |r| r.get(0))
            .unwrap();
        assert_eq!(file_count, 0);
        // imported 包不是 legacy，即便无绑定也保留
        assert!(packages.iter().any(|p| p.id == imported.id));
        // a2 不受影响
        assert_eq!(skill_package::list_bindings(&conn, &a2).unwrap().len(), 0);
        assert!(agent_config::get_digital_agent(&conn, &a1).unwrap().is_none());
    }

    // ---------- settings 读写 ----------

    /// 键不存在时返回 None（get_setting 与类型化读取均如此）
    #[test]
    fn settings_missing_key_returns_none() {
        use crate::db::setting;
        let conn = in_memory_db();
        assert_eq!(setting::get_setting(&conn, "nope").unwrap(), None);
        assert_eq!(setting::get_setting_value::<String>(&conn, "nope").unwrap(), None);
    }

    /// 字符串值写入后以 JSON 文本存储，可原样读回
    #[test]
    fn settings_set_get_string_roundtrip() {
        use crate::db::setting;
        let conn = in_memory_db();
        setting::set_setting(&conn, "cloud_api_key", &"sk-sp-test1234").unwrap();
        // 原始文本是 JSON 序列化形态（带引号），而非明文直存
        assert_eq!(
            setting::get_setting(&conn, "cloud_api_key").unwrap().as_deref(),
            Some("\"sk-sp-test1234\"")
        );
        assert_eq!(
            setting::get_setting_value::<String>(&conn, "cloud_api_key").unwrap().as_deref(),
            Some("sk-sp-test1234")
        );
    }

    /// 同键重复写入为覆盖（upsert），不产生重复行
    #[test]
    fn settings_upsert_overwrites_value() {
        use crate::db::setting;
        let conn = in_memory_db();
        setting::set_setting(&conn, "k", &"v1").unwrap();
        setting::set_setting(&conn, "k", &"v2").unwrap();
        assert_eq!(setting::get_setting_value::<String>(&conn, "k").unwrap().as_deref(), Some("v2"));
        assert_eq!(setting::list_settings(&conn).unwrap().len(), 1);
    }

    /// 支持结构化值（JSON 对象）与列表；list_settings 按 key 排序
    #[test]
    fn settings_structured_values_and_listing() {
        use crate::db::setting;
        let conn = in_memory_db();
        setting::set_setting(&conn, "models", &serde_json::json!({ "chat": "qwen3.7-plus" })).unwrap();
        setting::set_setting(&conn, "flags", &vec!["a", "b"]).unwrap();

        let models: serde_json::Value = setting::get_setting_value(&conn, "models").unwrap().unwrap();
        assert_eq!(models["chat"], "qwen3.7-plus");
        let flags: Vec<String> = setting::get_setting_value(&conn, "flags").unwrap().unwrap();
        assert_eq!(flags, vec!["a", "b"]);

        let all = setting::list_settings(&conn).unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].0, "flags");
        assert_eq!(all[1].0, "models");
    }

    /// 删除幂等：删存在的键生效，删不存在的键不报错
    #[test]
    fn settings_delete_is_idempotent() {
        use crate::db::setting;
        let conn = in_memory_db();
        setting::set_setting(&conn, "k", &"v").unwrap();
        setting::delete_setting(&conn, "k").unwrap();
        assert_eq!(setting::get_setting(&conn, "k").unwrap(), None);
        setting::delete_setting(&conn, "k").unwrap(); // 再删一次不报错
    }

    /// 脏数据降级：value 非合法 JSON 时类型化读取返回 None 而非报错
    #[test]
    fn settings_invalid_json_degrades_to_none() {
        use crate::db::setting;
        let conn = in_memory_db();
        conn.execute(
            "INSERT INTO settings (key, value) VALUES ('bad', 'not-json')",
            [],
        )
        .unwrap();
        assert_eq!(setting::get_setting_value::<String>(&conn, "bad").unwrap(), None);
        // 原始文本仍可读出（便于排查）
        assert_eq!(setting::get_setting(&conn, "bad").unwrap().as_deref(), Some("not-json"));
    }

    // ---------- 敏感值掩码 ----------

    /// 掩码只保留末 4 位；短值全隐藏；首尾空白先 trim；绝不包含原值中段
    #[test]
    fn mask_secret_keeps_only_last_four_chars() {
        use crate::db::setting;
        assert_eq!(setting::mask_secret("sk-sp-abcdefghijklmnop"), "sk-…mnop");
        assert_eq!(setting::mask_secret("  sk-sp-1234abcd  "), "sk-…abcd");
        // 恰好 4 位 → 可见末 4 位
        assert_eq!(setting::mask_secret("1234"), "sk-…1234");
        // 不足 4 位 → 全隐藏
        assert_eq!(setting::mask_secret("abc"), "****");
        assert_eq!(setting::mask_secret(""), "****");
        assert_eq!(setting::mask_secret("   "), "****");
        // 掩码结果不含中段明文
        let masked = setting::mask_secret("sk-sp-secretsecretsecret9876");
        assert!(!masked.contains("secret"));
        assert!(masked.ends_with("9876"));
    }
}
