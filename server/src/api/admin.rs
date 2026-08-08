//! Admin 管理 API：用户列表、角色变更、邀请令牌管理、数字人/技能/QA 模块 CRUD。
//! Admin 路由由 require_admin 中间件保护；list_active_digital_agents 为公开接口。

use crate::db::{agent_config, get_conn, setting, skill_package, user as user_db};
use crate::state::SharedState;
use crate::types::{
    AgentSkill, CreateAgentSkillRequest, CreateDigitalAgentRequest,
    CreateInviteTokenRequest, CreateQaInstructionModuleRequest, CreateSkillPackageRequest,
    DigitalAgent, ImportSkillPackageReport, ImportSkillPackageRequest, ImportSkillPackageResponse,
    InviteToken, QaInstructionModule, SkillBinding, SkillPackage, SkillPackageFile,
    UpdateRoleRequest, UpdateSkillBindingsRequest, User,
};
use axum::extract::{Path, Query, State};
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
/// （写入 agent_skills 后由数据层同步 legacy 技能包，与新注入视图保持一致）
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

/// PUT /api/admin/agent-skills/:id — 更新技能（同步 legacy 技能包内容）
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

/// DELETE /api/admin/agent-skills/:id — 删除技能（同步删除 legacy 技能包与绑定）
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
// 技能包管理 (Admin CRUD + 数字人绑定)
// ============================================================

/// 导入/创建硬性限制：文件数 ≤ 50、单文件 ≤ 200KB（按字符）、总字符 ≤ 1_000_000
const IMPORT_MAX_FILES: usize = 50;
const IMPORT_MAX_FILE_CHARS: usize = 200_000;
const IMPORT_MAX_TOTAL_CHARS: usize = 1_000_000;

/// 技能包三项限额校验（导入与 inline 创建共用）：路径规范化 +
/// 文件数/单文件/总字符上限，失败 400 携带中文原因。
fn check_package_limits(files: &[(String, String)]) -> Result<(), ApiError> {
    if files.len() > IMPORT_MAX_FILES {
        return Err(ApiError::bad_request(format!(
            "导入文件数超限（最多 {} 个，当前 {} 个）",
            IMPORT_MAX_FILES,
            files.len()
        )));
    }
    let mut total_chars = 0usize;
    for (rel_path, content) in files {
        skill_package::normalize_rel_path(rel_path).map_err(ApiError::bad_request)?;
        let chars = content.chars().count();
        if chars > IMPORT_MAX_FILE_CHARS {
            return Err(ApiError::bad_request(format!(
                "单文件超出大小限制（{}，当前 {} 字符，上限 {}）",
                rel_path, chars, IMPORT_MAX_FILE_CHARS
            )));
        }
        total_chars += chars;
    }
    if total_chars > IMPORT_MAX_TOTAL_CHARS {
        return Err(ApiError::bad_request(format!(
            "技能包总字符超限（当前 {}，上限 {}）",
            total_chars, IMPORT_MAX_TOTAL_CHARS
        )));
    }
    Ok(())
}

/// GET /api/admin/skill-packages — 列出全部技能包（不含文件内容）
pub async fn list_skill_packages(
    State(state): State<SharedState>,
) -> Result<Json<Vec<SkillPackage>>, ApiError> {
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    let packages = skill_package::list_skill_packages(conn)?;
    log::info!(target: "admin", "list_skill_packages count={}", packages.len());
    Ok(Json(packages))
}

/// POST /api/admin/skill-packages — 创建技能包（inline）
pub async fn create_skill_package(
    State(state): State<SharedState>,
    Json(req): Json<CreateSkillPackageRequest>,
) -> Result<Json<SkillPackage>, ApiError> {
    let display_name = req.display_name.trim().to_string();
    if display_name.is_empty() {
        return Err(ApiError::bad_request("displayName 不能为空"));
    }
    let files: Vec<(String, String)> = req
        .files
        .iter()
        .map(|f| (f.rel_path.clone(), f.content.clone()))
        .collect();
    // 与导入同款的三项限额校验（文件数/单文件/总字符）
    check_package_limits(&files)?;
    // 硬校验：路径规范化 + 定位 SKILL.md 入口 + frontmatter 合法（失败 400 带中文原因）
    skill_package::parse_skill_package(&files).map_err(ApiError::bad_request)?;

    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    let package = skill_package::create_skill_package(conn, &display_name, req.description, "inline", &files)?;
    log::info!(target: "admin", "create_skill_package id={} files={}", package.id, files.len());
    Ok(Json(package))
}

/// POST /api/admin/skill-packages/import — 从多文件导入技能包（imported）
pub async fn import_skill_package(
    State(state): State<SharedState>,
    Json(req): Json<ImportSkillPackageRequest>,
) -> Result<Json<ImportSkillPackageResponse>, ApiError> {
    if req.files.is_empty() {
        return Err(ApiError::bad_request("技能包不能为空（至少需要 SKILL.md）"));
    }
    let mut files: Vec<(String, String)> = Vec::with_capacity(req.files.len());
    for (rel_path, content) in &req.files {
        files.push((rel_path.clone(), content.clone()));
    }
    // 三项限额校验（与 inline 创建共用）
    check_package_limits(&files)?;
    // 必须能 parse 出根 SKILL.md 且 frontmatter 合法（失败 400 带中文原因）
    let manifest = skill_package::parse_skill_package(&files).map_err(ApiError::bad_request)?;

    // displayName 优先取请求 name，缺省取 frontmatter name
    let display_name = req
        .name
        .as_deref()
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .map(str::to_string)
        .unwrap_or(manifest.name.clone());

    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    let package = skill_package::create_skill_package(conn, &display_name, None, "imported", &files)?;
    drop(guard);

    // overBudget 对照全局技能注入预算（RG_SKILL_BUDGET_CHARS 当前值）
    let budget = agent_config::skill_budget_chars();
    let report = ImportSkillPackageReport {
        file_count: package.files.as_ref().map(|fs| fs.len()).unwrap_or(files.len()),
        total_chars: package.total_chars,
        over_budget: package.total_chars > budget,
    };
    log::info!(
        target: "admin",
        "import_skill_package id={} files={} total_chars={} over_budget={}",
        package.id,
        report.file_count,
        report.total_chars,
        report.over_budget
    );
    Ok(Json(ImportSkillPackageResponse { package, report }))
}

/// GET /api/admin/skill-packages/:id — 技能包详情（含文件内容）
pub async fn get_skill_package(
    State(state): State<SharedState>,
    Path(id): Path<String>,
) -> Result<Json<SkillPackage>, ApiError> {
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    let package = skill_package::get_skill_package(conn, &id)?
        .ok_or_else(|| ApiError::not_found("技能包不存在"))?;
    log::info!(target: "admin", "get_skill_package id={}", id);
    Ok(Json(package))
}

/// DELETE /api/admin/skill-packages/:id — 删除技能包（级联删文件与绑定）；
/// legacy 包（slug 形如 legacy-%）拒绝经此端点删除，引导回数字人技能面板
pub async fn delete_skill_package(
    State(state): State<SharedState>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    let package = skill_package::get_skill_package(conn, &id)?
        .ok_or_else(|| ApiError::not_found("技能包不存在"))?;
    if skill_package::is_legacy_package_slug(&package.slug) {
        return Err(ApiError::bad_request("内联技能包请在数字人技能面板中删除对应技能"));
    }
    // 先删绑定（显式删除，不依赖外键级联）再删包与文件
    conn.execute("DELETE FROM agent_skill_bindings WHERE package_id = ?1", rusqlite::params![id])
        .map_err(ApiError::from)?;
    skill_package::delete_skill_package(conn, &id)?;
    log::info!(target: "admin", "delete_skill_package id={}", id);
    Ok(StatusCode::NO_CONTENT)
}

/// GET /api/admin/skill-packages/:id/files — 技能包文件列表（含内容）
pub async fn list_skill_package_files(
    State(state): State<SharedState>,
    Path(id): Path<String>,
) -> Result<Json<Vec<SkillPackageFile>>, ApiError> {
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    if skill_package::get_skill_package(conn, &id)?.is_none() {
        return Err(ApiError::not_found("技能包不存在"));
    }
    let files = skill_package::list_package_files(conn, &id)?;
    log::info!(target: "admin", "list_skill_package_files id={} count={}", id, files.len());
    Ok(Json(files))
}

/// GET /api/admin/digital-agents/:id/skill-bindings — 数字人的技能包绑定列表
pub async fn list_skill_bindings(
    State(state): State<SharedState>,
    Path(agent_id): Path<String>,
) -> Result<Json<Vec<SkillBinding>>, ApiError> {
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    if agent_config::get_digital_agent(conn, &agent_id)?.is_none() {
        return Err(ApiError::not_found("数字人不存在"));
    }
    let bindings = skill_package::list_bindings(conn, &agent_id)?;
    log::info!(target: "admin", "list_skill_bindings agent_id={} count={}", agent_id, bindings.len());
    Ok(Json(bindings))
}

/// PUT /api/admin/digital-agents/:id/skill-bindings — 全量替换技能包绑定
pub async fn update_skill_bindings(
    State(state): State<SharedState>,
    Path(agent_id): Path<String>,
    Json(req): Json<UpdateSkillBindingsRequest>,
) -> Result<Json<Vec<SkillBinding>>, ApiError> {
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    if agent_config::get_digital_agent(conn, &agent_id)?.is_none() {
        return Err(ApiError::not_found("数字人不存在"));
    }
    // 先校验引用的技能包存在，避免外键约束错误退化为 500
    for binding in &req.bindings {
        if skill_package::get_skill_package(conn, &binding.package_id)?.is_none() {
            return Err(ApiError::bad_request(format!(
                "技能包不存在：{}",
                binding.package_id
            )));
        }
    }
    let bindings = req
        .bindings
        .into_iter()
        .map(|b| (b.package_id, b.sort_order))
        .collect();
    skill_package::replace_bindings(conn, &agent_id, bindings)?;
    let result = skill_package::list_bindings(conn, &agent_id)?;
    log::info!(target: "admin", "update_skill_bindings agent_id={} count={}", agent_id, result.len());
    Ok(Json(result))
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

// ============================================================
// 系统设置（settings 表：云端 API Key 等，P0-2）
// ============================================================

/// PUT /api/admin/config/cloud-api-key 请求体
#[derive(serde::Deserialize)]
pub struct UpdateCloudApiKeyRequest {
    #[serde(rename = "apiKey", default)]
    pub api_key: String,
}

/// 组装云端 API Key 配置摘要（掩码 + 是否已设置 + 生效来源）；
/// 绝不回传明文。
///
/// 死锁防护红线：本函数是 `db_value` 的纯函数，不读 DB、不持 db 锁；
/// 端点层必须先在锁内读出 settings 值并 drop 锁后再调用。
/// （若在持锁路径中触发 llm 层 DB 读取器闭包，会对同一不可重入
/// Mutex（AppState.db）二次加锁 → 确定性死锁。）
fn cloud_api_key_summary(db_value: Option<String>) -> serde_json::Value {
    let db_configured = db_value
        .as_deref()
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false);
    let (source, mask) = crate::llm::cloud_api_key_status(db_value);
    serde_json::json!({
        "configured": source.is_some(),
        "source": source.map(|s| s.as_str()),
        "mask": mask,
        "dbConfigured": db_configured,
    })
}

/// GET /api/admin/config — 系统配置摘要（敏感值仅回掩码，绝不回传明文）
pub async fn get_config(State(state): State<SharedState>) -> Result<Json<serde_json::Value>, ApiError> {
    // 锁内只做 DB 读取，summary 构建在 drop(guard) 之后（防死锁，见
    // cloud_api_key_summary 注释）
    let db_value = {
        let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
        let conn = get_conn(&guard)?;
        setting::get_setting_value::<String>(conn, setting::KEY_CLOUD_API_KEY)?
    };
    let summary = cloud_api_key_summary(db_value);
    log::info!(target: "admin", "get_config");
    Ok(Json(serde_json::json!({ "cloudApiKey": summary })))
}

/// PUT /api/admin/config/cloud-api-key — 保存云端 API Key 到 settings
///（SQLCipher 整库加密，详见 db/setting.rs 加密决策注释）。
/// 保存后失效已缓存的云端客户端，下次调用即用新 Key。
/// 日志只记掩码，不落明文。
pub async fn update_cloud_api_key(
    State(state): State<SharedState>,
    Json(req): Json<UpdateCloudApiKeyRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let api_key = req.api_key.trim().to_string();
    if api_key.is_empty() {
        return Err(ApiError::bad_request("API Key 不能为空，如需清除请使用 DELETE"));
    }
    // 锁内完成写入与回读，summary 构建在 drop(guard) 之后（防死锁）
    let db_value = {
        let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
        let conn = get_conn(&guard)?;
        setting::set_setting(conn, setting::KEY_CLOUD_API_KEY, &api_key)?;
        setting::get_setting_value::<String>(conn, setting::KEY_CLOUD_API_KEY)?
    };
    let summary = cloud_api_key_summary(db_value);

    crate::llm::invalidate_cloud_client();
    // 日志脱敏：只记掩码与生效来源（env/file 优先级更高时 DB 值不会生效）
    log::info!(
        target: "admin",
        "update_cloud_api_key mask={} effective_source={}",
        setting::mask_secret(&api_key),
        summary["source"].as_str().unwrap_or("none")
    );
    Ok(Json(serde_json::json!({ "updated": true, "cloudApiKey": summary })))
}

/// DELETE /api/admin/config/cloud-api-key — 清除 settings 中的云端 API Key
///（幂等）。注意：env / 文件来源优先级更高，清除 DB 值后生效 Key 可能
/// 仍来自前两层，响应中 source 字段如实反映。
pub async fn delete_cloud_api_key(
    State(state): State<SharedState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    // 锁内只做删除，summary 构建在 drop(guard) 之后（防死锁）；
    // DB 值已清除，直接以 None 构建摘要
    {
        let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
        let conn = get_conn(&guard)?;
        setting::delete_setting(conn, setting::KEY_CLOUD_API_KEY)?;
    }
    let summary = cloud_api_key_summary(None);

    crate::llm::invalidate_cloud_client();
    log::info!(
        target: "admin",
        "delete_cloud_api_key effective_source={}",
        summary["source"].as_str().unwrap_or("none")
    );
    Ok(Json(serde_json::json!({ "deleted": true, "cloudApiKey": summary })))
}

// ---------- 按场景模型配置（P1-7：model_configs 表） ----------
//
// 日志只记场景键与模型名（非敏感），不落对话内容。

/// PUT /api/admin/model-configs/:scenario 请求体
#[derive(serde::Deserialize)]
pub struct UpdateModelConfigRequest {
    pub model: String,
}

/// GET /api/admin/llm-usages 查询参数（limit 缺省 50，上限 500）
#[derive(serde::Deserialize)]
pub struct ListUsagesQuery {
    pub limit: Option<i64>,
}

/// 单场景配置视图：默认值 / DB 配置值 / env 覆盖值 / 实际生效值
fn scenario_config_view(
    scenario: &str,
    default_model: &str,
    description: &str,
    configured: Option<&str>,
) -> serde_json::Value {
    let env_override = crate::llm::env_override_for_scenario(scenario);
    serde_json::json!({
        "scenario": scenario,
        "description": description,
        "defaultModel": default_model,
        "model": configured,
        "envOverride": env_override,
        "effectiveModel": crate::llm::effective_model_for(scenario),
    })
}

/// GET /api/admin/model-configs — 列出全部场景的模型配置（含默认值/
/// env 覆盖/实际生效模型），按 SCENARIO_METAS 定义顺序
pub async fn list_model_configs(
    State(state): State<SharedState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    let configs = crate::db::model_config::list_configs(conn)?;
    drop(guard);

    let items: Vec<serde_json::Value> = crate::db::model_config::SCENARIO_METAS
        .iter()
        .map(|(scenario, default_model, description)| {
            let configured = configs
                .iter()
                .find(|(key, _, _)| key == scenario)
                .map(|(_, model, _)| model.as_str());
            scenario_config_view(scenario, default_model, description, configured)
        })
        .collect();
    log::info!(target: "admin", "list_model_configs count={}", items.len());
    Ok(Json(serde_json::json!({ "configs": items })))
}

/// PUT /api/admin/model-configs/:scenario — 更新场景模型（未知场景/空白
/// 模型名拒绝）。注意：env 覆盖层优先级更高，设置了 RG_*_MODEL 时 DB
/// 值不会生效，响应中 envOverride/effectiveModel 如实反映。
pub async fn update_model_config(
    State(state): State<SharedState>,
    Path(scenario): Path<String>,
    Json(req): Json<UpdateModelConfigRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if !crate::db::model_config::is_known_scenario(&scenario) {
        return Err(ApiError::bad_request(format!("未知场景：{}", scenario)));
    }
    let model = req.model.trim().to_string();
    if model.is_empty() {
        return Err(ApiError::bad_request("模型名不能为空，如需清除请使用 DELETE"));
    }
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    crate::db::model_config::set_model(conn, &scenario, &model)?;
    drop(guard);
    log::info!(target: "admin", "update_model_config scenario={} model={}", scenario, model);
    Ok(Json(serde_json::json!({ "updated": true, "scenario": scenario, "model": model })))
}

/// DELETE /api/admin/model-configs/:scenario — 清除场景配置行（幂等），
/// 解析回退 env/硬编码默认
pub async fn delete_model_config(
    State(state): State<SharedState>,
    Path(scenario): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if !crate::db::model_config::is_known_scenario(&scenario) {
        return Err(ApiError::bad_request(format!("未知场景：{}", scenario)));
    }
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    crate::db::model_config::delete_model(conn, &scenario)?;
    drop(guard);
    log::info!(target: "admin", "delete_model_config scenario={}", scenario);
    Ok(Json(serde_json::json!({ "deleted": true, "scenario": scenario })))
}

/// GET /api/admin/llm-usages?limit=N — 最近 N 条 LLM 用量元数据
///（默认 50，上限 500）。只含 token 数/耗时等元数据，不含对话内容。
pub async fn list_llm_usages(
    State(state): State<SharedState>,
    Query(query): Query<ListUsagesQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let limit = query.limit.unwrap_or(50).clamp(1, 500);
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    let usages = crate::db::model_config::recent_usages(conn, limit)?;
    drop(guard);
    log::info!(target: "admin", "list_llm_usages limit={} count={}", limit, usages.len());
    Ok(Json(serde_json::json!({ "usages": usages })))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn files_of(count: usize, content: &str) -> Vec<(String, String)> {
        (0..count)
            .map(|i| (format!("f{}.md", i), content.to_string()))
            .collect()
    }

    /// 三项限额：正常包放行；文件数/单文件/总字符超限分别拒绝（携带中文原因）
    #[test]
    fn check_package_limits_enforces_three_caps() {
        // 正常包放行
        assert!(check_package_limits(&files_of(2, "内容")).is_ok());

        // 文件数超限
        let err = check_package_limits(&files_of(IMPORT_MAX_FILES + 1, "x")).unwrap_err();
        assert_eq!(err.status, StatusCode::BAD_REQUEST);
        assert!(err.message.contains("文件数超限"));

        // 恰好等于上限不算超限
        assert!(check_package_limits(&files_of(IMPORT_MAX_FILES, "x")).is_ok());

        // 单文件超限
        let big = vec![("SKILL.md".to_string(), "字".repeat(IMPORT_MAX_FILE_CHARS + 1))];
        let err = check_package_limits(&big).unwrap_err();
        assert!(err.message.contains("单文件超出大小限制"));
        assert!(err.message.contains("SKILL.md"));

        // 总字符超限（单文件不超、合计超）
        let many_big: Vec<(String, String)> = (0..6)
            .map(|i| (format!("f{}.md", i), "字".repeat(180_000)))
            .collect();
        let err = check_package_limits(&many_big).unwrap_err();
        assert!(err.message.contains("总字符超限"));

        // 非法路径拒绝（携带中文原因）
        let bad_path = vec![("../etc/passwd".to_string(), "x".to_string())];
        let err = check_package_limits(&bad_path).unwrap_err();
        assert!(err.message.contains(".."));
    }

    /// legacy 包 slug 识别：legacy- 前缀命中，其余不命中
    #[test]
    fn legacy_slug_detection() {
        assert!(skill_package::is_legacy_package_slug("legacy-abc"));
        assert!(!skill_package::is_legacy_package_slug("my-skill-1a2b"));
        assert!(!skill_package::is_legacy_package_slug(""));
    }

    /// 死锁回归（Critical）：summary 构建绝不触发 llm 层 DB 读取器闭包。
    ///
    /// 修复前：get/update/delete config 在持有 state.db 锁期间调用
    /// cloud_api_key_summary(conn) → llm::cloud_api_key_status() →
    /// read_db_cloud_api_key() → main.rs 注册的读取器闭包对同一个
    /// 不可重入 Mutex（AppState.db）二次加锁 → 确定性死锁。
    /// 修复后：summary 是传入 db_value 的纯函数，端点层先短持锁读出
    /// settings 值、drop 锁后再构建。本测试注册一个「探针读取器」：
    /// 若 summary 构建路径重入 DB 层，探针必被触发 → 断言失败。
    #[test]
    fn cloud_api_key_summary_does_not_invoke_db_reader() {
        use std::sync::atomic::{AtomicBool, Ordering};
        static READER_INVOKED: AtomicBool = AtomicBool::new(false);
        // OnceLock 仅首次注册生效；探针返回 None，与未注册时语义一致，
        // 不影响其它测试
        crate::llm::register_cloud_key_db_reader(Box::new(|| {
            READER_INVOKED.store(true, Ordering::SeqCst);
            None
        }));
        READER_INVOKED.store(false, Ordering::SeqCst);

        let summary = cloud_api_key_summary(Some("sk-test-key-1234567890".to_string()));

        assert!(
            !READER_INVOKED.load(Ordering::SeqCst),
            "summary 构建触发了 DB 读取器闭包：在持锁路径中调用会二次加锁导致死锁"
        );
        // db_value 非空 → dbConfigured=true，证明摘要确实基于传入值构建
        assert_eq!(summary["dbConfigured"], serde_json::json!(true));

        // None → dbConfigured=false（清除场景）
        let empty = cloud_api_key_summary(None);
        assert_eq!(empty["dbConfigured"], serde_json::json!(false));
        assert!(
            !READER_INVOKED.load(Ordering::SeqCst),
            "None 分支同样不得触发 DB 读取器闭包"
        );
    }
}
