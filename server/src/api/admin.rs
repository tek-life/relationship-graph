//! Admin 管理 API：用户列表、角色变更、邀请令牌管理、数字人/技能/QA 模块 CRUD。
//! Admin 路由由 require_admin 中间件保护；list_active_digital_agents 为公开接口。

use crate::db::{agent_config, get_conn, user as user_db};
use crate::state::SharedState;
use crate::types::{
    AgentSkill, CreateAgentSkillRequest, CreateDigitalAgentRequest,
    CreateInviteTokenRequest, CreateQaInstructionModuleRequest, DigitalAgent,
    InviteToken, QaInstructionModule, UpdateRoleRequest, User,
};
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use chrono::Utc;
use rand::RngCore;

use super::ApiError;

/// 从请求头中提取 token 并获取关联的 user_id（admin 路由辅助函数）
fn extract_admin_user_id(
    state: &SharedState,
    headers: &HeaderMap,
) -> Result<String, ApiError> {
    let token = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or_default()
        .to_string();

    let token_info = state
        .tokens
        .lock()
        .map_err(|e| ApiError::internal(e.to_string()))?
        .get_token_info(&token)
        .ok_or_else(|| ApiError {
            status: axum::http::StatusCode::UNAUTHORIZED,
            message: "未登录或会话已过期".to_string(),
        })?;

    token_info.user_id.ok_or_else(|| ApiError {
        status: axum::http::StatusCode::FORBIDDEN,
        message: "当前会话未关联用户".to_string(),
    })
}

/// GET /api/admin/users — 列出所有用户
pub async fn list_users(State(state): State<SharedState>) -> Result<Json<Vec<User>>, ApiError> {
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    let mut users = user_db::list_users(conn)?;
    // 响应中不携带密码哈希
    for u in users.iter_mut() {
        u.password_hash = String::new();
    }
    log::info!(target: "admin", "list_users count={}", users.len());
    Ok(Json(users))
}

/// PUT /api/admin/users/:id/role — 更新用户角色
pub async fn update_user_role(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    Json(req): Json<UpdateRoleRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if !["admin", "user"].contains(&req.role.as_str()) {
        return Err(ApiError::bad_request("角色只能是 admin 或 user"));
    }
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    user_db::update_user_role(conn, &id, &req.role)?;
    drop(guard);

    // 同步刷新该用户已签发 token 的角色快照，升/降权立即生效，无需重新登录
    state
        .tokens
        .lock()
        .map_err(|e| ApiError::internal(e.to_string()))?
        .update_user_role(&id, &req.role);

    log::info!(target: "admin", "update_user_role id={} role={}", id, req.role);
    Ok(Json(serde_json::json!({ "updated": true })))
}

/// POST /api/admin/invite — 创建邀请令牌（有效期 7 天）
pub async fn create_invite(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let created_by = extract_admin_user_id(&state, &headers)?;

    // 生成随机邀请令牌
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    let token = hex::encode(bytes);

    let expires_at = (Utc::now() + chrono::Duration::days(7)).to_rfc3339();

    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    user_db::create_invite_token(
        conn,
        CreateInviteTokenRequest {
            token: token.clone(),
            created_by,
            expires_at: expires_at.clone(),
        },
    )?;

    log::info!(target: "admin", "create_invite_success");

    Ok(Json(serde_json::json!({
        "token": token,
        "expiresAt": expires_at,
    })))
}

/// GET /api/admin/invites — 列出所有邀请令牌
pub async fn list_invites(
    State(state): State<SharedState>,
) -> Result<Json<Vec<InviteToken>>, ApiError> {
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    Ok(Json(user_db::list_invites(conn)?))
}

// ============================================================
// 数字人管理 (Admin CRUD)
// ============================================================

/// 校验 mention 必须以 @ 开头：前端 @ 提及解析（正则 `^@`）与 SKILL 注入均依赖该约定，
/// 漏填 @ 会导致头像点击插入缺前缀、发送解析失败、技能不注入。
fn validate_mention(mention: &str) -> Result<(), ApiError> {
    if !mention.trim_start().starts_with('@') {
        return Err(ApiError::bad_request("mention 必须以 @ 开头，例如 @xxx"));
    }
    Ok(())
}

/// GET /api/admin/digital-agents — 列出所有数字人（含已禁用）
pub async fn list_digital_agents(
    State(state): State<SharedState>,
) -> Result<Json<Vec<DigitalAgent>>, ApiError> {
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    let agents = agent_config::list_digital_agents(conn)?;
    log::info!(target: "admin", "list_digital_agents count={}", agents.len());
    Ok(Json(agents))
}

/// POST /api/admin/digital-agents — 创建数字人
pub async fn create_digital_agent(
    State(state): State<SharedState>,
    Json(req): Json<CreateDigitalAgentRequest>,
) -> Result<Json<DigitalAgent>, ApiError> {
    validate_mention(&req.mention)?;
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    let agent = agent_config::create_digital_agent(conn, req)?;
    log::info!(target: "admin", "create_digital_agent id={}", agent.id);
    Ok(Json(agent))
}

/// PUT /api/admin/digital-agents/:id — 更新数字人
pub async fn update_digital_agent(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    Json(req): Json<CreateDigitalAgentRequest>,
) -> Result<Json<()>, ApiError> {
    validate_mention(&req.mention)?;
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    agent_config::update_digital_agent(conn, &id, req)?;
    log::info!(target: "admin", "update_digital_agent id={}", id);
    Ok(Json(()))
}

/// DELETE /api/admin/digital-agents/:id — 删除数字人
pub async fn delete_digital_agent(
    State(state): State<SharedState>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    agent_config::delete_digital_agent(conn, &id)?;
    log::info!(target: "admin", "delete_digital_agent id={}", id);
    Ok(StatusCode::NO_CONTENT)
}

// ============================================================
// 技能管理 (Admin CRUD)
// ============================================================

/// GET /api/admin/digital-agents/:id/skills — 列出指定数字人的技能
pub async fn list_agent_skills(
    State(state): State<SharedState>,
    Path(agent_id): Path<String>,
) -> Result<Json<Vec<AgentSkill>>, ApiError> {
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    let skills = agent_config::list_agent_skills(conn, &agent_id)?;
    log::info!(target: "admin", "list_agent_skills agent_id={} count={}", agent_id, skills.len());
    Ok(Json(skills))
}

/// POST /api/admin/digital-agents/:id/skills — 为数字人创建技能
pub async fn create_agent_skill(
    State(state): State<SharedState>,
    Path(agent_id): Path<String>,
    Json(mut req): Json<CreateAgentSkillRequest>,
) -> Result<Json<AgentSkill>, ApiError> {
    // 强制使用路径中的 agent_id，覆盖请求体中的值
    req.agent_id = agent_id;
    if let Some(md) = &req.skill_markdown {
        agent_config::validate_skill_markdown(md).map_err(ApiError::bad_request)?;
    }
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    let skill = agent_config::create_agent_skill(conn, req)?;
    log::info!(target: "admin", "create_agent_skill id={}", skill.id);
    Ok(Json(skill))
}

/// PUT /api/admin/agent-skills/:id — 更新技能
pub async fn update_agent_skill(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    Json(req): Json<CreateAgentSkillRequest>,
) -> Result<Json<()>, ApiError> {
    if let Some(md) = &req.skill_markdown {
        agent_config::validate_skill_markdown(md).map_err(ApiError::bad_request)?;
    }
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    agent_config::update_agent_skill(conn, &id, req)?;
    log::info!(target: "admin", "update_agent_skill id={}", id);
    Ok(Json(()))
}

/// DELETE /api/admin/agent-skills/:id — 删除技能
pub async fn delete_agent_skill(
    State(state): State<SharedState>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    agent_config::delete_agent_skill(conn, &id)?;
    log::info!(target: "admin", "delete_agent_skill id={}", id);
    Ok(StatusCode::NO_CONTENT)
}

// ============================================================
// QA 指令模块管理 (Admin CRUD)
// ============================================================

/// GET /api/admin/qa-modules — 列出所有 QA 模块
pub async fn list_qa_modules(
    State(state): State<SharedState>,
) -> Result<Json<Vec<QaInstructionModule>>, ApiError> {
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    let modules = agent_config::list_qa_modules(conn)?;
    log::info!(target: "admin", "list_qa_modules count={}", modules.len());
    Ok(Json(modules))
}

/// POST /api/admin/qa-modules — 创建 QA 模块
pub async fn create_qa_module(
    State(state): State<SharedState>,
    Json(req): Json<CreateQaInstructionModuleRequest>,
) -> Result<Json<QaInstructionModule>, ApiError> {
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    let module = agent_config::create_qa_module(conn, req)?;
    log::info!(target: "admin", "create_qa_module id={}", module.id);
    Ok(Json(module))
}

/// PUT /api/admin/qa-modules/:id — 更新 QA 模块
pub async fn update_qa_module(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    Json(req): Json<CreateQaInstructionModuleRequest>,
) -> Result<Json<()>, ApiError> {
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    agent_config::update_qa_module(conn, &id, req)?;
    log::info!(target: "admin", "update_qa_module id={}", id);
    Ok(Json(()))
}

/// DELETE /api/admin/qa-modules/:id — 删除 QA 模块
pub async fn delete_qa_module(
    State(state): State<SharedState>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    agent_config::delete_qa_module(conn, &id)?;
    log::info!(target: "admin", "delete_qa_module id={}", id);
    Ok(StatusCode::NO_CONTENT)
}

// ============================================================
// 公开接口：前端获取已激活的数字人列表（需要登录，但不需要 admin）
// ============================================================

/// GET /api/digital-agents — 仅返回 is_active=true 的数字人
pub async fn list_active_digital_agents(
    State(state): State<SharedState>,
) -> Result<Json<Vec<DigitalAgent>>, ApiError> {
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    let agents = agent_config::list_digital_agents(conn)?;
    let active: Vec<_> = agents.into_iter().filter(|a| a.is_active).collect();
    log::info!(target: "agent_config", "list_active_digital_agents count={}", active.len());
    Ok(Json(active))
}
