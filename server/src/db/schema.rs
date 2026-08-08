use crate::db::{agent_config, model_config, skill_package};
use rusqlite::Connection;
use std::time::Instant;

pub fn migrate(conn: &Connection) -> Result<(), rusqlite::Error> {
    let started = Instant::now();
    log::info!(target: "db", "schema_migrate_start");
    // 老库升级必须在 CREATE INDEX 之前完成：
    // 老 users 表缺少 role 列会导致 idx_users_role 创建失败，
    // 老 persons 表缺少 owner_id 列会导致 idx_persons_owner 创建失败，
    // 任一项都会让整个迁移中止
    ensure_user_columns(conn)?;
    ensure_person_columns(conn)?;
    let result = conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS persons (
            id TEXT PRIMARY KEY,
            owner_id TEXT,
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
        CREATE INDEX IF NOT EXISTS idx_persons_owner ON persons(owner_id);
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
            skill_markdown TEXT,
            trigger_scenario TEXT,
            is_active INTEGER NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            FOREIGN KEY (agent_id) REFERENCES digital_agents(id) ON DELETE CASCADE
        );

        -- 技能包（多文件技能）与数字人多对多绑定
        CREATE TABLE IF NOT EXISTS skill_packages (
            id TEXT PRIMARY KEY,
            slug TEXT UNIQUE NOT NULL,
            display_name TEXT NOT NULL,
            description TEXT,
            source_kind TEXT NOT NULL DEFAULT 'inline',
            manifest_json TEXT,
            total_chars INTEGER NOT NULL DEFAULT 0,
            is_active INTEGER NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS skill_package_files (
            id TEXT PRIMARY KEY,
            package_id TEXT NOT NULL,
            rel_path TEXT NOT NULL,
            content TEXT NOT NULL,
            size_chars INTEGER NOT NULL,
            created_at TEXT NOT NULL,
            FOREIGN KEY (package_id) REFERENCES skill_packages(id) ON DELETE CASCADE,
            UNIQUE(package_id, rel_path)
        );
        CREATE TABLE IF NOT EXISTS agent_skill_bindings (
            agent_id TEXT NOT NULL,
            package_id TEXT NOT NULL,
            sort_order INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL,
            PRIMARY KEY(agent_id, package_id),
            FOREIGN KEY (agent_id) REFERENCES digital_agents(id) ON DELETE CASCADE,
            FOREIGN KEY (package_id) REFERENCES skill_packages(id) ON DELETE CASCADE
        );

        -- 模型配置与 LLM 用量（P1-7：按场景模型配置表 + token usage 落库）
        -- model_configs：场景 → 模型映射（env 覆盖层 > 本表 > 硬编码默认，
        -- 解析优先级决策见 db/model_config.rs 模块注释）；
        -- llm_usages：只追加的调用元数据遥测（不落对话内容）。
        CREATE TABLE IF NOT EXISTS model_configs (
            scenario TEXT PRIMARY KEY,
            model TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS llm_usages (
            id TEXT PRIMARY KEY,
            scenario TEXT NOT NULL,
            channel TEXT NOT NULL,
            model TEXT NOT NULL,
            fn_name TEXT NOT NULL DEFAULT '',
            prompt_tokens INTEGER,
            completion_tokens INTEGER,
            elapsed_ms INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL
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
        CREATE INDEX IF NOT EXISTS idx_skill_files_package ON skill_package_files(package_id);
        CREATE INDEX IF NOT EXISTS idx_skill_bindings_package ON agent_skill_bindings(package_id);
        CREATE INDEX IF NOT EXISTS idx_qa_modules_active ON qa_instruction_modules(is_active);
        CREATE INDEX IF NOT EXISTS idx_llm_usages_created ON llm_usages(created_at);
        CREATE INDEX IF NOT EXISTS idx_llm_usages_scenario ON llm_usages(scenario);
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
    ensure_agent_skills_columns(conn)?;
    repair_orphan_sessions(conn)?;
    // 存量技能迁移失败不得阻断服务启动：记日志后继续，
    // 事务回滚保证下次启动可整体重跑
    if let Err(error) = migrate_legacy_skills(conn) {
        log::warn!(
            target: "db",
            "migrate_legacy_skills_failed error={}（下次启动重试）",
            error
        );
    }
    // 模型配置种子（P1-7）：INSERT OR IGNORE 幂等，已有配置零覆盖；
    // 失败不阻断启动（解析层有 env/硬编码默认兜底，下次启动可重跑）
    if let Err(error) = model_config::seed_default_models(conn) {
        log::warn!(
            target: "db",
            "seed_default_models_failed error={}（下次启动重试）",
            error
        );
    }
    agent_config::seed_defaults(conn)
}

/// 存量单文档技能迁移：将 agent_skills 中 skill_markdown 非空（trim 后）的行
/// 转为 source_kind='inline' 的技能包（slug = `legacy-<原 skill id>`）+
/// 单文件（rel_path='SKILL.md'，content=原 skill_markdown）+ 绑定
/// （sort_order 按 created_at 顺序递增）。
///
/// 幂等：以 slug 存在性查重，重复执行零副作用；skill_config_json 旧 JSON
/// 形态（skill_markdown 为空）不迁移；agent_skills 旧表保留不删。
///
/// 健壮性：整体用事务包裹（整段失败回滚、下次重跑），但行级失败降级：
/// 单行 upsert 失败记 log::warn（仅记 skill_id 与错误，不落内容）后跳过，
/// 不阻断其余行迁移；孤儿 agent_id（数字人已不存在）在 PRAGMA foreign_keys=ON
/// 下会触发绑定 FK 错误，预检跳过化解；sort_order 取当前绑定计数，
/// 重入（部分成功后重跑）不会产生重复序号。
fn migrate_legacy_skills(conn: &Connection) -> Result<(), rusqlite::Error> {
    // 先把待迁移行收集完（释放 statement）再开事务
    let rows: Vec<(String, String, String, String, i32)> = {
        let mut stmt = conn.prepare(
            "SELECT id, agent_id, skill_name, skill_markdown, is_active FROM agent_skills
             WHERE skill_markdown IS NOT NULL
             ORDER BY created_at ASC, id ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, i32>(4)?,
            ))
        })?;
        rows.collect::<Result<Vec<_>, _>>()?
    };

    let tx = conn.unchecked_transaction()?;
    let mut migrated = 0usize;
    for (skill_id, agent_id, skill_name, skill_markdown, is_active_int) in rows {
        if skill_markdown.trim().is_empty() {
            continue;
        }
        let slug = format!("legacy-{}", skill_id);
        let exists: bool = tx.query_row(
            "SELECT EXISTS (SELECT 1 FROM skill_packages WHERE slug = ?1)",
            [&slug],
            |row| row.get(0),
        )?;
        if exists {
            continue;
        }
        // 孤儿 agent_id 预检：foreign_keys=ON 下绑定插入会触发 FK 错误
        // （INSERT OR IGNORE 不豁免 FK），提前跳过并告警
        let agent_exists: bool = tx.query_row(
            "SELECT EXISTS (SELECT 1 FROM digital_agents WHERE id = ?1)",
            [&agent_id],
            |row| row.get(0),
        )?;
        if !agent_exists {
            log::warn!(
                target: "db",
                "migrate_legacy_skills_skip_orphan_agent skill_id={} agent_id={}",
                skill_id,
                agent_id
            );
            continue;
        }
        // sort_order 取当前绑定计数：事务内逐行插入自然递增，
        // 部分成功后重跑也不会与已有绑定撞号
        let sort_order: i32 = tx.query_row(
            "SELECT COUNT(*) FROM agent_skill_bindings WHERE agent_id = ?1",
            [&agent_id],
            |row| row.get(0),
        )?;
        let row_result = skill_package::upsert_legacy_package(
            &tx,
            &slug,
            &agent_id,
            &skill_name,
            &skill_markdown,
            is_active_int != 0,
        )
        .and_then(|()| {
            // upsert 默认 sort_order=0，存量迁移需按 created_at 顺序递增
            tx.execute(
                "UPDATE agent_skill_bindings SET sort_order = ?1 WHERE agent_id = ?2 AND package_id = (SELECT id FROM skill_packages WHERE slug = ?3)",
                rusqlite::params![sort_order, agent_id, slug],
            )
            .map(|_| ())
        });
        match row_result {
            Ok(()) => migrated += 1,
            Err(error) => {
                // 行级降级：清理本行可能残留的包/文件/绑定（保持事务可用），
                // 仅记 skill_id 与错误（不落内容）后继续
                let _ = skill_package::delete_legacy_package(&tx, &slug);
                log::warn!(
                    target: "db",
                    "migrate_legacy_skills_row_failed skill_id={} error={}",
                    skill_id,
                    error
                );
                continue;
            }
        }
    }
    tx.commit()?;
    if migrated > 0 {
        log::info!(target: "db", "migrate_legacy_skills count={}", migrated);
    }
    Ok(())
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
        // 更早期版本的 users 表连时间戳列都没有，登录查询会报 no such column: updated_at
        ("created_at", "TEXT NOT NULL DEFAULT ''"),
        ("updated_at", "TEXT NOT NULL DEFAULT ''"),
    ];
    for (name, ddl) in columns {
        if !existing.iter().any(|col| col == name) {
            conn.execute(&format!("ALTER TABLE users ADD COLUMN {} {}", name, ddl), [])?;
            log::info!(target: "db", "schema_migrate_add_column table=users column={}", name);
        }
    }

    // 回填空时间戳（新增列的存量行为空字符串，老列可能为 NULL）
    let now = chrono::Utc::now().to_rfc3339();
    let backfilled = conn.execute(
        "UPDATE users SET created_at = ?1, updated_at = ?1
         WHERE COALESCE(created_at, '') = '' OR COALESCE(updated_at, '') = ''",
        [&now],
    )?;
    if backfilled > 0 {
        log::info!(target: "db", "schema_migrate_backfill_user_timestamps count={}", backfilled);
    }
    Ok(())
}

/// 老库升级：为 persons 表补充学校/项目列（v1.4 推断规则扩展）
/// 与 owner_id 列（用户数据隔离）：owner_id 为空的存量联系人
/// 归属到首个用户（通常是 admin，与 repair_orphan_sessions 先例一致）。
/// 必须在 CREATE INDEX 之前调用（老库无 owner_id 列时 idx_persons_owner
/// 创建会失败）；全新库 persons 尚未创建，直接跳过。
fn ensure_person_columns(conn: &Connection) -> Result<(), rusqlite::Error> {
    let persons_exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'persons')",
        [],
        |row| row.get(0),
    )?;
    if !persons_exists {
        return Ok(());
    }

    let mut stmt = conn.prepare("PRAGMA table_info(persons)")?;
    let existing: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<Result<_, _>>()?;

    let columns = [
        ("school", "TEXT"),
        ("projects", "TEXT NOT NULL DEFAULT '[]'"),
        ("owner_id", "TEXT"),
    ];
    for (name, ddl) in columns {
        if !existing.iter().any(|col| col == name) {
            conn.execute(&format!("ALTER TABLE persons ADD COLUMN {} {}", name, ddl), [])?;
            log::info!(target: "db", "schema_migrate_add_column table=persons column={}", name);
        }
    }

    // 存量联系人归属回填：无主数据归到首个用户，不删除任何数据；
    // 幂等：仅处理 owner_id 仍为空的行；无 users 表的极早期库跳过
    let users_exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'users')",
        [],
        |row| row.get(0),
    )?;
    if users_exists {
        let backfilled = conn.execute(
            "UPDATE persons SET owner_id = (
                 SELECT id FROM users ORDER BY created_at ASC LIMIT 1
             )
             WHERE owner_id IS NULL",
            [],
        )?;
        if backfilled > 0 {
            log::info!(target: "db", "schema_migrate_backfill_person_owner count={}", backfilled);
        }
    }
    Ok(())
}

/// 老库升级：为 agent_skills 表补充 SKILL Markdown 列（数字人 Markdown 技能管理）
fn ensure_agent_skills_columns(conn: &Connection) -> Result<(), rusqlite::Error> {
    let mut stmt = conn.prepare("PRAGMA table_info(agent_skills)")?;
    let existing: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<Result<_, _>>()?;

    let columns = [
        ("skill_markdown", "TEXT"),
    ];
    for (name, ddl) in columns {
        if !existing.iter().any(|col| col == name) {
            conn.execute(&format!("ALTER TABLE agent_skills ADD COLUMN {} {}", name, ddl), [])?;
            log::info!(target: "db", "schema_migrate_add_column table=agent_skills column={}", name);
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
