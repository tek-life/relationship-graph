use crate::types::{
    AgentSkill, CreateAgentSkillRequest, CreateDigitalAgentRequest, CreateQaInstructionModuleRequest,
    DigitalAgent, QaInstructionModule,
};
use chrono::Utc;
use rusqlite::{params, Connection, Row};
use uuid::Uuid;

// === digital_agents ===

pub fn list_digital_agents(conn: &Connection) -> Result<Vec<DigitalAgent>, rusqlite::Error> {
    let mut stmt = conn.prepare(&(AGENT_SELECT.to_owned() + " ORDER BY sort_order ASC, created_at ASC"))?;
    let rows = stmt.query_map([], map_agent)?;
    rows.collect()
}

pub fn get_digital_agent(conn: &Connection, id: &str) -> Result<Option<DigitalAgent>, rusqlite::Error> {
    let sql = AGENT_SELECT.to_owned() + " WHERE id = ?1";
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query(params![id])?;
    if let Some(row) = rows.next()? {
        Ok(Some(map_agent(row)?))
    } else {
        Ok(None)
    }
}

pub fn create_digital_agent(conn: &Connection, req: CreateDigitalAgentRequest) -> Result<DigitalAgent, rusqlite::Error> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let aliases_json = serde_json::to_string(&req.aliases).unwrap_or_else(|_| "[]".to_string());
    let route_mode = req.route_mode.unwrap_or_else(|| "chat".to_string());
    let is_active: i32 = req.is_active.unwrap_or(true).into();
    let sort_order = req.sort_order.unwrap_or(0);

    conn.execute(
        "INSERT INTO digital_agents (id, display_name, mention, aliases, route_mode, avatar_url, description, skill_description, is_active, sort_order, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![id, req.display_name, req.mention, aliases_json, route_mode, req.avatar_url, req.description, req.skill_description, is_active, sort_order, now, now],
    )?;
    log::info!(target: "db", "create_digital_agent id={} mention={}", id, req.mention);
    get_digital_agent(conn, &id)?.ok_or(rusqlite::Error::QueryReturnedNoRows)
}

pub fn update_digital_agent(conn: &Connection, id: &str, req: CreateDigitalAgentRequest) -> Result<(), rusqlite::Error> {
    let now = Utc::now().to_rfc3339();
    let aliases_json = serde_json::to_string(&req.aliases).unwrap_or_else(|_| "[]".to_string());
    let route_mode = req.route_mode.unwrap_or_else(|| "chat".to_string());
    let is_active: i32 = req.is_active.unwrap_or(true).into();
    let sort_order = req.sort_order.unwrap_or(0);

    conn.execute(
        "UPDATE digital_agents SET display_name=?1, mention=?2, aliases=?3, route_mode=?4, avatar_url=?5, description=?6, skill_description=?7, is_active=?8, sort_order=?9, updated_at=?10 WHERE id=?11",
        params![req.display_name, req.mention, aliases_json, route_mode, req.avatar_url, req.description, req.skill_description, is_active, sort_order, now, id],
    )?;
    log::info!(target: "db", "update_digital_agent id={}", id);
    Ok(())
}

pub fn delete_digital_agent(conn: &Connection, id: &str) -> Result<(), rusqlite::Error> {
    conn.execute("DELETE FROM digital_agents WHERE id = ?1", params![id])?;
    log::info!(target: "db", "delete_digital_agent id={}", id);
    Ok(())
}

// === agent_skills ===

/// 校验 SKILL Markdown 的 frontmatter 格式（轻量手写解析，不依赖 YAML 库）。
///
/// 规则：
/// - 空白/空 Markdown 直接放行（skill_markdown 为可选字段）；
/// - 必须以 `---` 开头的 frontmatter 起始行；
/// - 必须存在闭合的 `---` 行；
/// - frontmatter 必须包含非空值的 `name` 与 `description` 顶层键。
/// 校验失败时 Err 携带中文原因，供 API 层转为 400 返回。
pub fn validate_skill_markdown(markdown: &str) -> Result<(), String> {
    let trimmed = markdown.trim_start().trim_start_matches('\u{FEFF}').trim_start();
    // 空白内容视为未填写，直接放行（skill_markdown 为可选字段）
    if trimmed.is_empty() {
        return Ok(());
    }
    let mut lines = trimmed.lines();

    // frontmatter 起始行必须为 ---
    match lines.next() {
        Some(line) if line.trim() == "---" => {}
        _ => return Err("SKILL 文档缺少 frontmatter 头部（以 --- 开始）".to_string()),
    }

    // 寻找闭合的 ---，收集 frontmatter 内容
    let mut fm_lines: Vec<&str> = Vec::new();
    let mut closed = false;
    for line in lines {
        if line.trim() == "---" {
            closed = true;
            break;
        }
        fm_lines.push(line);
    }
    if !closed {
        return Err("技能 Markdown 的 frontmatter 缺少闭合的 ---".to_string());
    }

    // 解析顶层键（与前端 parseFrontmatter 语义一致：首个出现的键生效，忽略注释行），
    // 要求 name/description 存在且非空
    let mut name_value = String::new();
    let mut description_value = String::new();
    let mut has_name = false;
    let mut has_description = false;
    for line in fm_lines {
        if line.trim_start().starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once(':') {
            match key.trim() {
                "name" if !has_name => {
                    has_name = true;
                    name_value = value.trim().to_string();
                }
                "description" if !has_description => {
                    has_description = true;
                    description_value = value.trim().to_string();
                }
                _ => {}
            }
        }
    }
    if name_value.is_empty() {
        return Err("frontmatter 缺少必填字段 name".to_string());
    }
    if description_value.is_empty() {
        return Err("frontmatter 缺少必填字段 description".to_string());
    }
    Ok(())
}

pub fn list_agent_skills(conn: &Connection, agent_id: &str) -> Result<Vec<AgentSkill>, rusqlite::Error> {
    let sql = SKILL_SELECT.to_owned() + " WHERE agent_id = ?1 ORDER BY created_at ASC";
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![agent_id], map_skill)?;
    rows.collect()
}

pub fn create_agent_skill(conn: &Connection, req: CreateAgentSkillRequest) -> Result<AgentSkill, rusqlite::Error> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let is_active: i32 = req.is_active.unwrap_or(true).into();
    let skill_config_json = req.skill_config_json.unwrap_or_else(|| "{}".to_string());

    conn.execute(
        "INSERT INTO agent_skills (id, agent_id, skill_name, skill_config_json, skill_markdown, trigger_scenario, is_active, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![id, req.agent_id, req.skill_name, skill_config_json, req.skill_markdown, req.trigger_scenario, is_active, now, now],
    )?;
    log::info!(target: "db", "create_agent_skill id={} agent_id={}", id, req.agent_id);
    get_agent_skill(conn, &id)?.ok_or(rusqlite::Error::QueryReturnedNoRows)
}

pub fn update_agent_skill(conn: &Connection, id: &str, req: CreateAgentSkillRequest) -> Result<(), rusqlite::Error> {
    let now = Utc::now().to_rfc3339();
    let is_active: i32 = req.is_active.unwrap_or(true).into();
    let skill_config_json = req.skill_config_json.unwrap_or_else(|| "{}".to_string());
    conn.execute(
        "UPDATE agent_skills SET agent_id=?1, skill_name=?2, skill_config_json=?3, skill_markdown=?4, trigger_scenario=?5, is_active=?6, updated_at=?7 WHERE id=?8",
        params![req.agent_id, req.skill_name, skill_config_json, req.skill_markdown, req.trigger_scenario, is_active, now, id],
    )?;
    log::info!(target: "db", "update_agent_skill id={}", id);
    Ok(())
}

pub fn delete_agent_skill(conn: &Connection, id: &str) -> Result<(), rusqlite::Error> {
    conn.execute("DELETE FROM agent_skills WHERE id = ?1", params![id])?;
    log::info!(target: "db", "delete_agent_skill id={}", id);
    Ok(())
}

/// 构建指定数字人的技能 prompt（注入锚点，本期不接线）。
///
/// 取该 agent 下 is_active=1 的技能，按 created_at 升序排列，
/// 每条拼为 `### 技能：<skill_name>\n<skill_markdown 全文>\n\n`；
/// skill_markdown 为空/空白的技能跳过；无可用技能时返回空串。
// 接线点在后续步骤的 system prompt 组装中，当前仅单测引用
#[allow(dead_code)]
pub fn build_skills_prompt(conn: &Connection, agent_id: &str) -> Result<String, rusqlite::Error> {
    let sql = "SELECT skill_name, skill_markdown FROM agent_skills
               WHERE agent_id = ?1 AND is_active = 1
               ORDER BY created_at ASC";
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map(params![agent_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, Option<String>>(1)?,
        ))
    })?;

    let mut prompt = String::new();
    let mut count = 0usize;
    for row in rows {
        let (skill_name, skill_markdown) = row?;
        let markdown = match skill_markdown {
            Some(md) if !md.trim().is_empty() => md,
            _ => continue,
        };
        prompt.push_str(&format!("### 技能：{}\n{}\n\n", skill_name, markdown));
        count += 1;
    }
    log::info!(target: "db", "build_skills_prompt agent_id={} count={}", agent_id, count);
    Ok(prompt)
}

// === qa_instruction_modules ===

pub fn list_qa_modules(conn: &Connection) -> Result<Vec<QaInstructionModule>, rusqlite::Error> {
    let mut stmt = conn.prepare(&(QA_SELECT.to_owned() + " ORDER BY sort_order ASC, created_at ASC"))?;
    let rows = stmt.query_map([], map_qa)?;
    rows.collect()
}

pub fn get_qa_module(conn: &Connection, id: &str) -> Result<Option<QaInstructionModule>, rusqlite::Error> {
    let sql = QA_SELECT.to_owned() + " WHERE id = ?1";
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query(params![id])?;
    if let Some(row) = rows.next()? {
        Ok(Some(map_qa(row)?))
    } else {
        Ok(None)
    }
}

pub fn create_qa_module(conn: &Connection, req: CreateQaInstructionModuleRequest) -> Result<QaInstructionModule, rusqlite::Error> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let sort_order = req.sort_order.unwrap_or(0);
    let trigger_scenario = req.trigger_scenario.unwrap_or_else(|| "new_user".to_string());
    let is_active: i32 = req.is_active.unwrap_or(true).into();

    conn.execute(
        "INSERT INTO qa_instruction_modules (id, name, description, system_prompt, guidance_text, sort_order, trigger_scenario, is_active, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![id, req.name, req.description, req.system_prompt, req.guidance_text, sort_order, trigger_scenario, is_active, now, now],
    )?;
    log::info!(target: "db", "create_qa_module id={} name={}", id, req.name);
    get_qa_module(conn, &id)?.ok_or(rusqlite::Error::QueryReturnedNoRows)
}

pub fn update_qa_module(conn: &Connection, id: &str, req: CreateQaInstructionModuleRequest) -> Result<(), rusqlite::Error> {
    let now = Utc::now().to_rfc3339();
    let sort_order = req.sort_order.unwrap_or(0);
    let trigger_scenario = req.trigger_scenario.unwrap_or_else(|| "new_user".to_string());
    let is_active: i32 = req.is_active.unwrap_or(true).into();

    conn.execute(
        "UPDATE qa_instruction_modules SET name=?1, description=?2, system_prompt=?3, guidance_text=?4, sort_order=?5, trigger_scenario=?6, is_active=?7, updated_at=?8 WHERE id=?9",
        params![req.name, req.description, req.system_prompt, req.guidance_text, sort_order, trigger_scenario, is_active, now, id],
    )?;
    log::info!(target: "db", "update_qa_module id={}", id);
    Ok(())
}

pub fn delete_qa_module(conn: &Connection, id: &str) -> Result<(), rusqlite::Error> {
    conn.execute("DELETE FROM qa_instruction_modules WHERE id = ?1", params![id])?;
    log::info!(target: "db", "delete_qa_module id={}", id);
    Ok(())
}

// === helpers ===

fn get_agent_skill(conn: &Connection, id: &str) -> Result<Option<AgentSkill>, rusqlite::Error> {
    let sql = SKILL_SELECT.to_owned() + " WHERE id = ?1";
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query(params![id])?;
    if let Some(row) = rows.next()? {
        Ok(Some(map_skill(row)?))
    } else {
        Ok(None)
    }
}

const AGENT_SELECT: &str =
    "SELECT id, display_name, mention, aliases, route_mode, avatar_url, description, skill_description, is_active, sort_order, created_at, updated_at FROM digital_agents";

fn map_agent(row: &Row) -> Result<DigitalAgent, rusqlite::Error> {
    let aliases_json: String = row.get(3)?;
    let is_active_int: i32 = row.get(8)?;
    Ok(DigitalAgent {
        id: row.get(0)?,
        display_name: row.get(1)?,
        mention: row.get(2)?,
        aliases: serde_json::from_str(&aliases_json).unwrap_or_default(),
        route_mode: row.get(4)?,
        avatar_url: row.get(5)?,
        description: row.get(6)?,
        skill_description: row.get(7)?,
        is_active: is_active_int != 0,
        sort_order: row.get(9)?,
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
    })
}

const SKILL_SELECT: &str =
    "SELECT id, agent_id, skill_name, skill_config_json, skill_markdown, trigger_scenario, is_active, created_at, updated_at FROM agent_skills";

fn map_skill(row: &Row) -> Result<AgentSkill, rusqlite::Error> {
    let is_active_int: i32 = row.get(6)?;
    Ok(AgentSkill {
        id: row.get(0)?,
        agent_id: row.get(1)?,
        skill_name: row.get(2)?,
        skill_config_json: row.get(3)?,
        skill_markdown: row.get(4)?,
        trigger_scenario: row.get(5)?,
        is_active: is_active_int != 0,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
    })
}

const QA_SELECT: &str =
    "SELECT id, name, description, system_prompt, guidance_text, sort_order, trigger_scenario, is_active, created_at, updated_at FROM qa_instruction_modules";

fn map_qa(row: &Row) -> Result<QaInstructionModule, rusqlite::Error> {
    let is_active_int: i32 = row.get(7)?;
    Ok(QaInstructionModule {
        id: row.get(0)?,
        name: row.get(1)?,
        description: row.get(2)?,
        system_prompt: row.get(3)?,
        guidance_text: row.get(4)?,
        sort_order: row.get(5)?,
        trigger_scenario: row.get(6)?,
        is_active: is_active_int != 0,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
    })
}

// === 默认数据初始化 ===

/// 当 digital_agents 表为空时，自动插入默认数字人和 QA 指令模块。
/// 在 schema::migrate 完成后调用。
pub fn seed_defaults(conn: &Connection) -> Result<(), rusqlite::Error> {
    // 检查 digital_agents 表是否已有数据
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM digital_agents",
        [],
        |row| row.get(0),
    )?;

    if count > 0 {
        log::info!(target: "db", "seed_defaults_skip reason=already_populated count={}", count);
        return Ok(());
    }

    log::info!(target: "db", "seed_defaults_start");

    // 默认数字人：联系人管家
    conn.execute(
        "INSERT OR IGNORE INTO digital_agents (id, display_name, mention, aliases, route_mode, description, skill_description, is_active, sort_order, created_at, updated_at)
         VALUES ('contact_manager', '联系人管家', '@联系人管家', '[\"@数字管家\",\"@contact-manager\"]', 'relationship', '管理联系人的增删改查，维护关系网络', '联系人查询、新增、更新、路径规划', 1, 0, datetime('now'), datetime('now'))",
        [],
    )?;
    log::info!(target: "db", "seed_defaults contact_manager inserted");

    // 默认 QA 指令模块
    conn.execute(
        "INSERT OR IGNORE INTO qa_instruction_modules (id, name, description, system_prompt, guidance_text, sort_order, trigger_scenario, is_active, created_at, updated_at)
         VALUES ('hero_journey', '英雄之旅复盘', '深入的人生复盘引导', '你是专业的人生教练，请带领我完成一次深入的英雄之旅复盘。请一步一步来，每次只进行一个步骤的引导，等我详细回答或反馈完成后，再像专业教练一样引导我进入下一步。', '英雄之旅复盘引导', 0, 'new_user', 1, datetime('now'), datetime('now'))",
        [],
    )?;

    conn.execute(
        "INSERT OR IGNORE INTO qa_instruction_modules (id, name, description, system_prompt, guidance_text, sort_order, trigger_scenario, is_active, created_at, updated_at)
         VALUES ('munger_thinking', '芒格多元思维', '运用查理·芒格的多元思维模式进行批判性人生梳理', '接下来请你运用查理·芒格的多元思维模式，对我进行批判性的人生梳理：首先运用逆向思维分析到底是什么在毁掉我、消耗我的精力；然后从第一性原理出发帮我打破应该的束缚找到真正的想要；接着运用概率思维和复利思维帮我接受不确定性并找到值得长期投入的方向；最后运用系统思维来设计我的人生系统启动个人成长的飞轮效应。请继续采用一对一的对话模式一个问题一个问题地询问根据我的回答深入思考后再提出下一个问题。', '芒格多元思维模式梳理', 1, 'new_user', 1, datetime('now'), datetime('now'))",
        [],
    )?;

    conn.execute(
        "INSERT OR IGNORE INTO qa_instruction_modules (id, name, description, system_prompt, guidance_text, sort_order, trigger_scenario, is_active, created_at, updated_at)
         VALUES ('profile_generate', '个人画像生成', '根据对话生成完整的个人画像文档', '根据以上所有对话内容，保留我的原始语言表达方式和个性化表述，生成一份完整的个人画像文档，确保能够准确反映我的价值观、思维方式和人生目标。', '个人画像文档生成', 2, 'new_user', 1, datetime('now'), datetime('now'))",
        [],
    )?;

    log::info!(target: "db", "seed_defaults_complete qa_modules=3");
    Ok(())
}
