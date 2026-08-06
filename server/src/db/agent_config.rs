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

    conn.execute(
        "INSERT INTO agent_skills (id, agent_id, skill_name, skill_config_json, trigger_scenario, is_active, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![id, req.agent_id, req.skill_name, req.skill_config_json, req.trigger_scenario, is_active, now, now],
    )?;
    log::info!(target: "db", "create_agent_skill id={} agent_id={}", id, req.agent_id);
    get_agent_skill(conn, &id)?.ok_or(rusqlite::Error::QueryReturnedNoRows)
}

pub fn update_agent_skill(conn: &Connection, id: &str, req: CreateAgentSkillRequest) -> Result<(), rusqlite::Error> {
    let now = Utc::now().to_rfc3339();
    let is_active: i32 = req.is_active.unwrap_or(true).into();
    conn.execute(
        "UPDATE agent_skills SET agent_id=?1, skill_name=?2, skill_config_json=?3, trigger_scenario=?4, is_active=?5, updated_at=?6 WHERE id=?7",
        params![req.agent_id, req.skill_name, req.skill_config_json, req.trigger_scenario, is_active, now, id],
    )?;
    log::info!(target: "db", "update_agent_skill id={}", id);
    Ok(())
}

pub fn delete_agent_skill(conn: &Connection, id: &str) -> Result<(), rusqlite::Error> {
    conn.execute("DELETE FROM agent_skills WHERE id = ?1", params![id])?;
    log::info!(target: "db", "delete_agent_skill id={}", id);
    Ok(())
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
    "SELECT id, agent_id, skill_name, skill_config_json, trigger_scenario, is_active, created_at, updated_at FROM agent_skills";

fn map_skill(row: &Row) -> Result<AgentSkill, rusqlite::Error> {
    let is_active_int: i32 = row.get(5)?;
    Ok(AgentSkill {
        id: row.get(0)?,
        agent_id: row.get(1)?,
        skill_name: row.get(2)?,
        skill_config_json: row.get(3)?,
        trigger_scenario: row.get(4)?,
        is_active: is_active_int != 0,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
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
