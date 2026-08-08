//! HTTP API 层：对应原 Tauri Commands，统一 JSON 错误响应与鉴权。
//! 日志遵循脱敏原则：不记录密码、token、姓名、电话及内容原文。

use crate::db::{crypto::{derive_key, generate_db_key, open_encrypted_db, rekey_db, validate_encrypted_db},
    agent_config, get_conn, interaction, person, relationship, schema, session as db_session, user as user_db};
use crate::nlq::{self, NlqRequest, NlqResult};
use crate::security::auth;
use crate::security::sensitivity;
use crate::state::SharedState;
use crate::types::{
    ChatRequest, ChatResponse, CreateUserRequest,
    CreateEntityMentionRequest, CreateInteractionRequest, CreatePersonRequest,
    CreateRelationshipRequest, EntityMention, FieldChange, GraphData, GraphEdge, GraphNode, Interaction,
    LoginResponse, NlqConfirmRequest, NlqMultiRequest, NlqResponse, Person, Relationship, User,
};
use axum::extract::{DefaultBodyLimit, Extension, Path, Query, Request, State};
use axum::http::StatusCode;
use axum::middleware::{self, Next};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use chrono::Utc;
use futures::Stream;
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;

mod admin;
mod import;
mod profile_qa;
mod session;
mod user;
mod voice;

pub fn router(state: SharedState) -> Router {
    let protected = Router::new()
        .route("/api/persons", get(list_persons).post(create_person))
        .route(
            "/api/persons/:id",
            get(get_person).put(update_person).delete(delete_person),
        )
        .route("/api/persons/:id/relationships", get(list_relationships_by_person))
        .route("/api/persons/:id/interactions", get(list_interactions_by_person))
        .route("/api/persons-search", get(search_person_candidates))
        .route("/api/relationships", get(list_relationships).post(create_relationship))
        .route("/api/relationships/infer", post(infer_relationships))
        .route("/api/relationships/pending", get(list_pending_relationships))
        .route("/api/relationships/:id/confirmation", post(set_relationship_confirmation))
        .route("/api/interactions", post(create_interaction))
        .route("/api/entity-mentions", post(create_entity_mention))
        .route("/api/graph", get(get_graph_data))
        // 前端获取已激活的数字人列表（需要登录，不需要 admin）
        .route("/api/digital-agents", get(admin::list_active_digital_agents))
        .route("/api/nlq", post(natural_language_query))
        .route("/api/nlq/multi", post(nlq_multi_handler))
        .route("/api/chat", post(chat_handler))
        .route("/api/chat/stream", post(chat_stream_handler))
        .route("/api/nlq/confirm", post(nlq_confirm_handler))
        .route("/api/import/preview", post(import::preview))
        .route("/api/import/persons", post(import::commit))
        .route(
            "/api/voice/transcribe",
            // 默认 body 上限 2MB，放宽到音频上限 + multipart 编码开销；音频本体超 10MB 由 handler 返回 413
            post(voice::transcribe).layer(DefaultBodyLimit::max(voice::MAX_AUDIO_BYTES + 1024 * 1024)),
        )
        // 用户认证相关受保护路由
        .route("/api/auth/me", get(user::get_current_user))
        .route("/api/users/profile", put(user::update_profile))
        // 会话管理相关受保护路由
        .route(
            "/api/sessions",
            get(session::list_sessions).post(session::create_session),
        )
        .route(
            "/api/sessions/:id",
            put(session::update_session).delete(session::delete_session),
        )
        .route(
            "/api/sessions/:id/messages",
            get(session::list_messages).post(session::add_message),
        )
        // Profile QA（个人画像构建）受保护路由
        .route("/api/profile-qa/modules", get(profile_qa::list_modules))
        .route("/api/profile-qa/next", post(profile_qa::next_question))
        .route("/api/profile-qa/generate", post(profile_qa::generate_profile))
        .route("/api/profile-qa/save", post(profile_qa::save_profile))
        .route_layer(middleware::from_fn_with_state(state.clone(), require_auth));

    // Admin 路由（需要 admin 角色）
    let admin_router = Router::new()
        .route("/api/admin/users", get(admin::list_users))
        .route("/api/admin/users/:id/role", put(admin::update_user_role))
        .route("/api/admin/invite", post(admin::create_invite))
        .route("/api/admin/invites", get(admin::list_invites))
        // 数字人管理
        .route(
            "/api/admin/digital-agents",
            get(admin::list_digital_agents).post(admin::create_digital_agent),
        )
        .route(
            "/api/admin/digital-agents/:id",
            put(admin::update_digital_agent).delete(admin::delete_digital_agent),
        )
        // 技能管理
        .route(
            "/api/admin/digital-agents/:id/skills",
            get(admin::list_agent_skills).post(admin::create_agent_skill),
        )
        .route(
            "/api/admin/agent-skills/:id",
            put(admin::update_agent_skill).delete(admin::delete_agent_skill),
        )
        // 技能包管理（多文件技能 + 数字人绑定）
        .route(
            "/api/admin/skill-packages",
            get(admin::list_skill_packages).post(admin::create_skill_package),
        )
        .route(
            "/api/admin/skill-packages/import",
            // 导入上限：总字符 1_000_000（中文 UTF-8 至多 3MB）+ JSON 转义开销
            post(admin::import_skill_package).layer(DefaultBodyLimit::max(8 * 1024 * 1024)),
        )
        .route(
            "/api/admin/skill-packages/:id",
            get(admin::get_skill_package).delete(admin::delete_skill_package),
        )
        .route(
            "/api/admin/skill-packages/:id/files",
            get(admin::list_skill_package_files),
        )
        // 数字人↔技能包绑定
        .route(
            "/api/admin/digital-agents/:id/skill-bindings",
            get(admin::list_skill_bindings).put(admin::update_skill_bindings),
        )
        // QA 指令模块管理
        .route(
            "/api/admin/qa-modules",
            get(admin::list_qa_modules).post(admin::create_qa_module),
        )
        .route(
            "/api/admin/qa-modules/:id",
            put(admin::update_qa_module).delete(admin::delete_qa_module),
        )
        // 系统设置（P0-2：云端 API Key 等）
        .route("/api/admin/config", get(admin::get_config))
        .route(
            "/api/admin/config/cloud-api-key",
            put(admin::update_cloud_api_key).delete(admin::delete_cloud_api_key),
        )
        // 按场景模型配置 + LLM 用量（P1-7）
        .route("/api/admin/model-configs", get(admin::list_model_configs))
        .route(
            "/api/admin/model-configs/:scenario",
            put(admin::update_model_config).delete(admin::delete_model_config),
        )
        .route("/api/admin/llm-usages", get(admin::list_llm_usages))
        .route_layer(middleware::from_fn_with_state(state.clone(), require_admin));

    Router::new()
        .route("/api/health", get(health))
        .route("/api/auth/state", get(auth_state))
        .route("/api/auth/setup", post(setup_admin))
        .route("/api/auth/migrate", post(migrate_legacy))
        .route("/api/auth/recover-admin", post(recover_admin))
        // 公开认证路由（不需要 auth，register 需要邀请令牌）
        .route("/api/auth/register", post(user::register))
        .route("/api/auth/login", post(user::login))
        .merge(protected)
        .merge(admin_router)
        .with_state(state)
}

// ---------- 错误与鉴权 ----------

pub struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn bad_request(message: impl Into<String>) -> Self {
        Self { status: StatusCode::BAD_REQUEST, message: message.into() }
    }

    fn unauthorized(message: impl Into<String>) -> Self {
        Self { status: StatusCode::UNAUTHORIZED, message: message.into() }
    }

    fn not_found(message: impl Into<String>) -> Self {
        Self { status: StatusCode::NOT_FOUND, message: message.into() }
    }

    fn internal(message: impl Into<String>) -> Self {
        Self { status: StatusCode::INTERNAL_SERVER_ERROR, message: message.into() }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(serde_json::json!({ "error": self.message }))).into_response()
    }
}

impl From<String> for ApiError {
    fn from(message: String) -> Self {
        // db 未解锁属于客户端可恢复错误
        if message.contains("尚未初始化或解锁") {
            Self { status: StatusCode::CONFLICT, message }
        } else {
            Self::internal(message)
        }
    }
}

impl From<rusqlite::Error> for ApiError {
    fn from(error: rusqlite::Error) -> Self {
        // InvalidQuery 由数据层归属校验抛出（越权读写），
        // QueryReturnedNoRows 由归属过滤后的回读失败抛出：统一表现为
        // 「查无此数据」，不泄露目标数据存在性
        if matches!(error, rusqlite::Error::InvalidQuery | rusqlite::Error::QueryReturnedNoRows) {
            Self { status: StatusCode::NOT_FOUND, message: "数据不存在或无权访问".to_string() }
        } else {
            Self::internal(error.to_string())
        }
    }
}

/// 从认证中间件注入的 AuthUser 提取 user_id；未关联用户的会话
/// （如 setup/unlock 签发的 legacy token）访问联系人数据时一律 401。
fn require_user_id(user: Option<Extension<AuthUser>>) -> Result<String, ApiError> {
    user.map(|u| (u.0).0)
        .ok_or_else(|| ApiError::unauthorized("当前会话未关联用户，请重新登录"))
}

/// 认证中间件注入的身份标记：token 关联的用户 ID。
/// chat 端点以 Option<Extension<AuthUser>> 提取，提取不到时降级为
/// “不注入画像”，永不阻断聊天。
#[derive(Clone, Debug)]
pub struct AuthUser(pub String);

async fn require_auth(State(state): State<SharedState>, mut req: Request, next: Next) -> Response {
    let token = req
        .headers()
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .unwrap_or_default()
        .to_string();

    let token_info = state
        .tokens
        .lock()
        .map(|mut store| store.get_token_info(&token))
        .unwrap_or(None);

    match token_info {
        Some(info) => {
            // 纯加法：将 token 关联的用户 ID 注入 extensions，供 chat 端点
            // 提取用于画像注入；setup/unlock 签发的未关联用户 token 不注入，
            // 其余受保护路由不消费该扩展，行为不变
            if let Some(user_id) = info.user_id {
                req.extensions_mut().insert(AuthUser(user_id));
            }
            next.run(req).await
        }
        None => {
            log::warn!(target: "auth", "request_rejected reason=invalid_token path={}", req.uri().path());
            (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({ "error": "未登录或会话已过期" })),
            )
                .into_response()
        }
    }
}

/// Admin 权限中间件：验证 token 且 role 必须为 admin
async fn require_admin(State(state): State<SharedState>, req: Request, next: Next) -> Response {
    let token = req
        .headers()
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .unwrap_or_default()
        .to_string();

    let token_info = state
        .tokens
        .lock()
        .map(|mut store| store.get_token_info(&token))
        .unwrap_or(None);

    match token_info {
        Some(info) if info.role.as_deref() == Some("admin") => next.run(req).await,
        Some(_) => {
            log::warn!(target: "auth", "admin_request_rejected reason=forbidden path={}", req.uri().path());
            (
                StatusCode::FORBIDDEN,
                Json(serde_json::json!({ "error": "需要管理员权限" })),
            )
                .into_response()
        }
        None => {
            log::warn!(target: "auth", "admin_request_rejected reason=invalid_token path={}", req.uri().path());
            (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({ "error": "未登录或会话已过期" })),
            )
                .into_response()
        }
    }
}

// ---------- 健康检查与认证 ----------

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ok" }))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AuthState {
    initialized: bool,
    /// 老库（主密码派生密钥）尚未迁移到密钥文件机制，需走 /api/auth/migrate
    needs_migration: bool,
}

async fn auth_state(State(state): State<SharedState>) -> Result<Json<AuthState>, ApiError> {
    let initialized = state.key_file_path().exists() && state.db_path().exists();
    let needs_migration = state.db_path().exists() && !state.key_file_path().exists();
    log::info!(
        target: "security",
        "auth_state initialized={} needs_migration={}",
        initialized,
        needs_migration
    );
    Ok(Json(AuthState { initialized, needs_migration }))
}

#[derive(Deserialize)]
struct PasswordRequest {
    password: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetupAdminRequest {
    username: Option<String>,
    password: String,
}

/// 密钥文件写入（仅属主可读写，0600）
fn write_key_file(path: &std::path::Path, key_hex: &str) -> Result<(), ApiError> {
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)
        .map_err(|e| ApiError::internal(e.to_string()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
            .map_err(|e| ApiError::internal(e.to_string()))?;
    }
    file.write_all(key_hex.as_bytes())
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(())
}

/// 确保存在 admin 账号且密码为给定值：优先 role=admin，其次 username=admin，
/// 均无则按 preferred_username 创建。老设计中主密码即 admin 密码，迁移/恢复时用此函数对齐。
fn ensure_admin_account(
    conn: &rusqlite::Connection,
    password: &str,
    preferred_username: &str,
) -> Result<User, ApiError> {
    let password_hash = auth::hash_password(password).map_err(ApiError::internal)?;
    let users = user_db::list_users(conn).map_err(|e| ApiError::internal(e.to_string()))?;
    let mut admin = match users
        .iter()
        .find(|u| u.role == "admin")
        .or_else(|| users.iter().find(|u| u.username == "admin"))
        .or_else(|| users.iter().find(|u| u.username == preferred_username))
    {
        Some(existing) => existing.clone(),
        None => user_db::create_user(conn, CreateUserRequest {
            username: preferred_username.to_string(),
            password_hash: password_hash.clone(),
            display_name: Some("管理员".to_string()),
            role: Some("admin".to_string()),
            profile_doc: None,
        })
        .map_err(|e| ApiError::internal(e.to_string()))?,
    };

    user_db::update_user_password(conn, &admin.id, &password_hash)
        .map_err(|e| ApiError::internal(e.to_string()))?;
    if admin.role != "admin" {
        user_db::update_user_role(conn, &admin.id, "admin")
            .map_err(|e| ApiError::internal(e.to_string()))?;
        admin.role = "admin".to_string();
    }
    // 响应中不携带密码哈希
    admin.password_hash = String::new();
    Ok(admin)
}

fn issue_admin_token(state: &SharedState, user: &User) -> Result<String, ApiError> {
    Ok(state
        .tokens
        .lock()
        .map_err(|e| ApiError::internal(e.to_string()))?
        .issue_with_user(Some(user.id.clone()), Some(user.role.clone())))
}

/// POST /api/auth/setup — 全新部署初始化：生成密钥文件、创建加密库与 admin 账号。
/// 此后服务端启动即自动解锁，不再存在任何人工解锁步骤。
async fn setup_admin(
    State(state): State<SharedState>,
    Json(req): Json<SetupAdminRequest>,
) -> Result<Json<LoginResponse>, ApiError> {
    let started = Instant::now();
    log::info!(target: "security", "setup_admin_start");

    if req.password.trim().len() < 8 {
        log::warn!(target: "security", "setup_admin_rejected reason=short_password");
        return Err(ApiError::bad_request("密码至少需要 8 个字符"));
    }
    let username = req
        .username
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("admin")
        .to_string();
    if state.db_path().exists() || state.key_file_path().exists() {
        return Err(ApiError::bad_request("系统已初始化，请直接登录"));
    }

    std::fs::create_dir_all(&state.data_dir).map_err(|e| ApiError::internal(e.to_string()))?;
    let key_hex = generate_db_key();
    let conn = open_encrypted_db(state.db_path(), &key_hex)
        .map_err(|e| ApiError::internal(e.to_string()))?;
    schema::migrate(&conn)?;

    let admin = ensure_admin_account(&conn, &req.password, &username)?;
    write_key_file(&state.key_file_path(), &key_hex)?;

    let mut guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    *guard = Some(conn);
    drop(guard);

    let token = issue_admin_token(&state, &admin)?;
    log::info!(
        target: "security",
        "setup_admin_success username={} elapsed_ms={}",
        admin.username,
        started.elapsed().as_millis()
    );
    Ok(Json(LoginResponse { token, user: admin }))
}

/// POST /api/auth/migrate — 老库一次性迁移：用旧主密码打开库，rekey 为随机密钥文件。
/// 迁移后主密码概念消失，admin 密码对齐为该主密码；该端点随后无存在意义。
async fn migrate_legacy(
    State(state): State<SharedState>,
    Json(req): Json<PasswordRequest>,
) -> Result<Json<LoginResponse>, ApiError> {
    let started = Instant::now();
    log::info!(target: "security", "migrate_legacy_start");

    if !state.db_path().exists() {
        return Err(ApiError::bad_request("数据库不存在，无需迁移"));
    }
    if state.key_file_path().exists() {
        return Err(ApiError::bad_request("已完成迁移，请直接登录"));
    }

    // 1. 用旧主密码派生密钥并验证
    let salt_hex = std::fs::read_to_string(state.salt_path())
        .map_err(|_| ApiError::bad_request("缺少 salt 文件，无法迁移"))?;
    let salt = hex::decode(salt_hex.trim()).map_err(|e| ApiError::internal(e.to_string()))?;
    let old_key = derive_key(&req.password, &salt).map_err(|e| ApiError::internal(e.to_string()))?;

    let conn = open_encrypted_db(state.db_path(), &old_key)
        .map_err(|e| ApiError::internal(e.to_string()))?;
    validate_encrypted_db(&conn)
        .map_err(|_| ApiError::bad_request("主密码不正确"))?;

    // 2. rekey 为随机密钥并落盘密钥文件
    let new_key = generate_db_key();
    rekey_db(&conn, &new_key).map_err(|e| ApiError::internal(e.to_string()))?;
    write_key_file(&state.key_file_path(), &new_key)?;

    schema::migrate(&conn)?;
    let admin = ensure_admin_account(&conn, &req.password, "admin")?;

    let mut guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    *guard = Some(conn);
    drop(guard);

    let token = issue_admin_token(&state, &admin)?;
    log::info!(
        target: "security",
        "migrate_legacy_success username={} elapsed_ms={}",
        admin.username,
        started.elapsed().as_millis()
    );
    Ok(Json(LoginResponse { token, user: admin }))
}

/// POST /api/auth/recover-admin — 老库（未迁移）用主密码恢复管理员账号。
/// 迁移到密钥文件机制后该端点自动失效（主密码不再能打开库）。
async fn recover_admin(
    State(state): State<SharedState>,
    Json(req): Json<PasswordRequest>,
) -> Result<Json<LoginResponse>, ApiError> {
    if state.key_file_path().exists() {
        return Err(ApiError::bad_request("系统已迁移到账号登录，请使用用户名密码登录"));
    }

    // 1. 验证主密码：派生密钥必须能打开加密库
    let salt_hex = std::fs::read_to_string(state.salt_path())
        .map_err(|_| ApiError::bad_request("数据库尚未初始化"))?;
    let salt = hex::decode(salt_hex.trim()).map_err(|e| ApiError::internal(e.to_string()))?;
    let key_hex = derive_key(&req.password, &salt).map_err(|e| ApiError::internal(e.to_string()))?;

    let probe = open_encrypted_db(state.db_path(), &key_hex)
        .map_err(|e| ApiError::internal(e.to_string()))?;
    validate_encrypted_db(&probe)
        .map_err(|_| ApiError::bad_request("主密码不正确"))?;

    // 2. 库未解锁时顺带完成解锁（复用已验证的连接）
    let mut guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    if guard.is_none() {
        schema::migrate(&probe)?;
        *guard = Some(probe);
        log::info!(target: "security", "recover_admin_unlocked_db");
    } else {
        drop(probe);
    }

    // 3. 保障 admin 账号存在且密码与主密码一致
    let conn = get_conn(&guard)?;
    let admin = ensure_admin_account(conn, &req.password, "admin")?;
    drop(guard);

    let token = issue_admin_token(&state, &admin)?;
    log::info!(target: "security", "recover_admin_success username={}", admin.username);
    Ok(Json(LoginResponse { token, user: admin }))
}

// ---------- Person ----------

async fn create_person(
    State(state): State<SharedState>,
    user: Option<Extension<AuthUser>>,
    Json(req): Json<CreatePersonRequest>,
) -> Result<Json<Person>, ApiError> {
    let owner_id = require_user_id(user)?;
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    let created = person::create(conn, &owner_id, req)?;
    log::info!(target: "person_cmd", "create_person_success sensitivity={}", created.sensitivity_level);
    Ok(Json(created))
}

async fn update_person(
    State(state): State<SharedState>,
    user: Option<Extension<AuthUser>>,
    Path(id): Path<String>,
    Json(req): Json<CreatePersonRequest>,
) -> Result<Json<Person>, ApiError> {
    let owner_id = require_user_id(user)?;
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    let updated = person::update(conn, &owner_id, &id, req)?;
    log::info!(target: "person_cmd", "update_person_success");
    Ok(Json(updated))
}

async fn list_persons(
    State(state): State<SharedState>,
    user: Option<Extension<AuthUser>>,
) -> Result<Json<Vec<Person>>, ApiError> {
    let owner_id = require_user_id(user)?;
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    let persons = person::list_all(conn, &owner_id)?;
    log::info!(target: "person_cmd", "list_persons_success count={}", persons.len());
    Ok(Json(persons))
}

async fn get_person(
    State(state): State<SharedState>,
    user: Option<Extension<AuthUser>>,
    Path(id): Path<String>,
) -> Result<Json<Person>, ApiError> {
    let owner_id = require_user_id(user)?;
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    let person = person::get_by_id(conn, &owner_id, &id)?
        .ok_or_else(|| ApiError::not_found("数据不存在或无权访问"))?;
    Ok(Json(person))
}

async fn delete_person(
    State(state): State<SharedState>,
    user: Option<Extension<AuthUser>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let owner_id = require_user_id(user)?;
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    // 先校验存在与归属，避免删除不存在的 id 仍返回 deleted:true
    person::get_by_id(conn, &owner_id, &id)?
        .ok_or_else(|| ApiError::not_found("数据不存在或无权访问"))?;
    person::delete(conn, &owner_id, &id)?;
    log::info!(target: "person_cmd", "delete_person_success");
    Ok(Json(serde_json::json!({ "deleted": true })))
}

#[derive(Deserialize)]
struct MentionQuery {
    mention: String,
}

async fn search_person_candidates(
    State(state): State<SharedState>,
    user: Option<Extension<AuthUser>>,
    Query(query): Query<MentionQuery>,
) -> Result<Json<Vec<Person>>, ApiError> {
    let owner_id = require_user_id(user)?;
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    let persons = person::search_by_mention(conn, &owner_id, &query.mention)?;
    log::info!(
        target: "person_cmd",
        "search_person_candidates_success mention_len={} count={}",
        query.mention.chars().count(),
        persons.len()
    );
    Ok(Json(persons))
}

// ---------- Relationship ----------

async fn create_relationship(
    State(state): State<SharedState>,
    user: Option<Extension<AuthUser>>,
    Json(req): Json<CreateRelationshipRequest>,
) -> Result<Json<Relationship>, ApiError> {
    let owner_id = require_user_id(user)?;
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    let created = relationship::create(conn, &owner_id, req)?;
    log::info!(target: "relationship_cmd", "create_relationship_success");
    Ok(Json(created))
}

async fn list_relationships(
    State(state): State<SharedState>,
    user: Option<Extension<AuthUser>>,
) -> Result<Json<Vec<Relationship>>, ApiError> {
    let owner_id = require_user_id(user)?;
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    Ok(Json(relationship::list_all(conn, &owner_id)?))
}

async fn list_relationships_by_person(
    State(state): State<SharedState>,
    user: Option<Extension<AuthUser>>,
    Path(id): Path<String>,
) -> Result<Json<Vec<Relationship>>, ApiError> {
    let owner_id = require_user_id(user)?;
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    Ok(Json(relationship::list_by_person(conn, &owner_id, &id)?))
}

async fn infer_relationships(
    State(state): State<SharedState>,
    user: Option<Extension<AuthUser>>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let owner_id = require_user_id(user)?;
    let started = Instant::now();
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    let created = crate::infer::run(conn, &owner_id)?;
    log::info!(
        target: "relationship_cmd",
        "infer_relationships_success created={} elapsed_ms={}",
        created,
        started.elapsed().as_millis()
    );
    Ok(Json(serde_json::json!({ "created": created })))
}

async fn list_pending_relationships(
    State(state): State<SharedState>,
    user: Option<Extension<AuthUser>>,
) -> Result<Json<Vec<Relationship>>, ApiError> {
    let owner_id = require_user_id(user)?;
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    Ok(Json(relationship::list_pending(conn, &owner_id)?))
}

#[derive(Deserialize)]
struct ConfirmationRequest {
    status: String,
}

async fn set_relationship_confirmation(
    State(state): State<SharedState>,
    user: Option<Extension<AuthUser>>,
    Path(id): Path<String>,
    Json(req): Json<ConfirmationRequest>,
) -> Result<Json<Relationship>, ApiError> {
    if !["confirmed", "rejected", "pending"].contains(&req.status.as_str()) {
        return Err(ApiError::bad_request("确认状态只能是 confirmed / rejected / pending"));
    }
    let owner_id = require_user_id(user)?;
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    let updated = relationship::set_confirmation(conn, &owner_id, &id, &req.status)?;
    log::info!(target: "relationship_cmd", "set_relationship_confirmation_success status={}", req.status);
    Ok(Json(updated))
}

// ---------- Interaction ----------

async fn create_interaction(
    State(state): State<SharedState>,
    user: Option<Extension<AuthUser>>,
    Json(req): Json<CreateInteractionRequest>,
) -> Result<Json<Interaction>, ApiError> {
    let owner_id = require_user_id(user)?;
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    let created = interaction::create(conn, &owner_id, req)?;
    log::info!(target: "interaction_cmd", "create_interaction_success");
    Ok(Json(created))
}

async fn list_interactions_by_person(
    State(state): State<SharedState>,
    user: Option<Extension<AuthUser>>,
    Path(id): Path<String>,
) -> Result<Json<Vec<Interaction>>, ApiError> {
    let owner_id = require_user_id(user)?;
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    Ok(Json(interaction::list_by_person(conn, &owner_id, &id)?))
}

async fn create_entity_mention(
    State(state): State<SharedState>,
    user: Option<Extension<AuthUser>>,
    Json(req): Json<CreateEntityMentionRequest>,
) -> Result<Json<EntityMention>, ApiError> {
    let owner_id = require_user_id(user)?;
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    Ok(Json(interaction::create_mention(conn, &owner_id, req)?))
}

// ---------- Graph ----------

async fn get_graph_data(
    State(state): State<SharedState>,
    user: Option<Extension<AuthUser>>,
) -> Result<Json<GraphData>, ApiError> {
    let owner_id = require_user_id(user)?;
    let started = Instant::now();
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;

    let persons = person::list_all(conn, &owner_id)?;
    let relationships = relationship::list_all(conn, &owner_id)?;

    let nodes: Vec<GraphNode> = persons
        .into_iter()
        .map(|p| GraphNode {
            id: p.id,
            label: sensitivity::display_name(&p.name, &p.aliases, &p.sensitivity_level, false),
            sensitivity_level: p.sensitivity_level,
            status: p.status,
        })
        .collect();

    let edges: Vec<GraphEdge> = relationships
        .into_iter()
        .filter(|r| r.confirmation_status != "rejected")
        .map(|r| GraphEdge {
            id: r.id,
            source: r.from_person_id,
            target: r.to_person_id,
            label: r.relationship_type,
            strength: r.strength,
            edge_source: r.source,
            confirmation_status: r.confirmation_status,
            confidence: r.confidence,
            inference_reason: r.inference_reason,
        })
        .collect();

    log::info!(
        target: "graph_cmd",
        "get_graph_data_success nodes={} edges={} elapsed_ms={}",
        nodes.len(),
        edges.len(),
        started.elapsed().as_millis()
    );
    Ok(Json(GraphData { nodes, edges }))
}

// ---------- NLQ ----------

async fn natural_language_query(
    State(state): State<SharedState>,
    user: Option<Extension<AuthUser>>,
    Json(req): Json<NlqRequest>,
) -> Result<Json<Vec<NlqResult>>, ApiError> {
    let owner_id = require_user_id(user)?;
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    let results = nlq::natural_language_query(conn, &owner_id, req)?;
    Ok(Json(results))
}

async fn chat_handler(
    State(state): State<SharedState>,
    user: Option<Extension<AuthUser>>,
    Json(req): Json<ChatRequest>,
) -> Result<Json<ChatResponse>, ApiError> {
    let query = req.query.trim();
    if query.is_empty() {
        return Err(ApiError::bad_request("问题不能为空"));
    }

    let user_id = user.map(|u| (u.0).0);
    // 归属校验前置：携带 sessionId 时必须属于当前用户，否则 404（不泄露存在性）
    verify_chat_session_owned(&state, req.session_id.as_deref(), user_id.as_deref())?;
    // 聊天历史组装：request_at 捕获请求时刻，严格排除本轮刚落库的 user 消息；
    // 读取失败降级为空历史，永不阻断聊天
    let request_at = Utc::now().to_rfc3339();
    let history = load_chat_history(&state, req.session_id.as_deref(), &request_at);
    let skills = resolve_skills_prompt(&state, req.agent_id.as_deref(), user_id.as_deref());
    // 联网搜索：请求开关 AND env 总闸 AND 云端通道；非 cloud 静默置 false 不阻断
    let backend = crate::llm::general_chat_backend();
    let (web_search, _) = resolve_web_search(req.web_search.unwrap_or(false), backend);
    // 文档上下文注入（预算 RG_DOC_CONTEXT_CHARS）
    let (documents_prompt, doc_count, doc_chars, doc_truncated) =
        resolve_documents_prompt(req.documents);
    log::info!(
        target: "chat",
        "chat_handler query_len={} skills_chars={} web_search={} doc_count={} doc_chars={} truncated={}",
        query.chars().count(),
        skills.chars().count(),
        web_search,
        doc_count,
        doc_chars,
        doc_truncated
    );
    // Agent 工具循环（仅 cloud 通道）：模型通过工具自主查询联系人数据，
    // 支撑「基于我的数据生成报告」类请求；失败时降级为普通聊天，永不阻断。
    // 未关联用户的会话禁用工具（无归属可查），降级普通聊天
    let (tools_enabled, _) = resolve_data_tools_enabled(backend);
    let tools_enabled = tools_enabled && user_id.is_some();
    if tools_enabled {
        let owner_id = user_id.clone().unwrap_or_default();
        let system = crate::llm::tool_loop_system_prompt(query, &skills, web_search, &documents_prompt);
        match crate::llm::cloud_chat_with_tools(
            system,
            query.to_string(),
            web_search,
            state.clone(),
            owner_id,
            crate::llm::AGENT_MAX_TOOL_TURNS,
            &history,
        )
        .await
        {
            Ok(reply) => return Ok(Json(ChatResponse { reply })),
            Err(e) => {
                log::warn!(target: "chat", "agent_chat_failed fallback=general_chat err={}", e);
            }
        }
    }
    let reply = crate::llm::general_chat(query, &skills, web_search, &documents_prompt, &history)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(ChatResponse { reply }))
}

/// 会话归属判定（纯函数，可单测）：会话存在且属于当前用户。
/// 跨用户访问统一表现为「查无此数据」，不泄露存在性。
fn session_owned_by(session: Option<&crate::types::Session>, user_id: &str) -> bool {
    matches!(session, Some(s) if s.user_id == user_id)
}

/// 聊天请求携带 sessionId 时的归属校验（前置）：不匹配/不存在一律
/// 404「数据不存在或无权访问」；未携带 sessionId 保持旧行为（单轮）；
/// 未关联用户的会话（legacy token）无法主张任何会话，一律 401。
/// DB 锁约定：锁内查数 → 立即 drop guard → 再做后续 LLM 调用。
fn verify_chat_session_owned(
    state: &SharedState,
    session_id: Option<&str>,
    user_id: Option<&str>,
) -> Result<(), ApiError> {
    let session_id = match session_id {
        Some(id) if !id.trim().is_empty() => id,
        _ => return Ok(()),
    };
    let user_id = user_id
        .ok_or_else(|| ApiError::unauthorized("当前会话未关联用户，请重新登录"))?;
    let session = {
        let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
        let conn = get_conn(&guard)?;
        db_session::get_session(conn, session_id).map_err(|e| ApiError::internal(e.to_string()))?
    }; // guard 在此释放
    if session_owned_by(session.as_ref(), user_id) {
        Ok(())
    } else {
        Err(ApiError::not_found("数据不存在或无权访问"))
    }
}

/// 联网搜索全局总闸：env `RG_WEB_SEARCH` 未设置或 `on` = 允许，`off` = 关闭
fn web_search_env_allowed() -> bool {
    std::env::var("RG_WEB_SEARCH")
        .map(|v| v.trim().to_lowercase() != "off")
        .unwrap_or(true)
}

/// Agent 联系人数据工具开关（方案 B）：env `RG_CHAT_TOOLS` 未设置或 `on` =
/// 允许，`off` = 关闭；仅 cloud 通道生效，非 cloud 返回降级文案，永不阻断。
fn resolve_data_tools_enabled(backend: &str) -> (bool, Option<&'static str>) {
    let enabled = std::env::var("RG_CHAT_TOOLS")
        .map(|v| v.trim().to_lowercase() != "off")
        .unwrap_or(true);
    if !enabled {
        return (false, None);
    }
    if backend == "cloud" {
        (true, Some("已启用联系人数据工具"))
    } else {
        (false, Some("联系人数据工具仅云端通道可用，已按普通对话处理"))
    }
}

/// 解析生效的联网搜索开关与 SSE step 文案（纯函数，可单测）：
/// 请求开关 AND env 总闸 AND backend==cloud 才生效；未请求/总闸关闭返回
/// (false, None)；请求但非 cloud 返回 (false, Some(降级文案))，永不阻断。
fn resolve_web_search(requested: bool, backend: &str) -> (bool, Option<&'static str>) {
    if !requested || !web_search_env_allowed() {
        return (false, None);
    }
    if backend == "cloud" {
        (true, Some("已启用联网搜索"))
    } else {
        (false, Some("联网搜索仅云端通道可用，已按普通对话处理"))
    }
}

/// 构建文档段 prompt 与日志元数据（文档内容不落日志）：
/// 返回 (documents_prompt, doc_count, doc_chars, truncated)。
/// doc_chars 统计全部文档正文总字符数（截断前）；truncated 由产物尾部
/// 截断标记判定。空/None 输入返回全零元数据与空串（prompt 逐字节不变）。
fn resolve_documents_prompt(
    documents: Option<Vec<crate::types::ChatDocument>>,
) -> (String, usize, usize, bool) {
    let docs = documents.unwrap_or_default();
    if docs.is_empty() {
        return (String::new(), 0, 0, false);
    }
    let doc_count = docs.len();
    let doc_chars: usize = docs.iter().map(|d| d.content.chars().count()).sum();
    let pairs: Vec<(String, String)> = docs
        .into_iter()
        .map(|d| (d.file_name, d.content))
        .collect();
    let budget = crate::document::doc_context_budget_chars();
    let prompt = crate::document::build_documents_prompt(&pairs, budget);
    let truncated = prompt.ends_with("[文档内容超长已截断]");
    (prompt, doc_count, doc_chars, truncated)
}

// ---------- 聊天历史组装（多轮对话） ----------

/// 压缩摘要消息前缀（与 api/session.rs 压缩写入格式一致）
const SUMMARY_PREFIX: &str = "[对话摘要]";

/// 历史窗口一次取数上限（足够覆盖字符预算；压缩后每会话保留最近 10 条 + 摘要）
const HISTORY_FETCH_LIMIT: i64 = 200;

/// 聊天历史字符预算：env `RG_CHAT_HISTORY_CHARS`（默认 8000），非法/0 回退默认
fn chat_history_budget_chars() -> usize {
    std::env::var("RG_CHAT_HISTORY_CHARS")
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .filter(|n| *n > 0)
        .unwrap_or(8000)
}

fn strip_summary_prefix(content: &str) -> String {
    content
        .trim()
        .strip_prefix(SUMMARY_PREFIX)
        .unwrap_or(content.trim())
        .trim()
        .to_string()
}

/// 历史窗口选择（纯函数，可单测）：输入按时间升序的消息序列。
/// 1. 提取最近一条 [对话摘要] system 消息（剥离前缀）作为 summary，不进入 turns；
/// 2. user/assistant 轮次从新到旧按字符数累加，超出预算即止（不含溢出消息）；
///    最近一条即使单独超预算也保留，避免长消息导致历史恒空；
/// 3. 输出 turns 维持时间升序。
fn select_history_window(
    messages: &[crate::types::ChatMessage],
    budget: usize,
) -> (Option<String>, Vec<(String, String)>) {
    let mut summary: Option<String> = None;
    for m in messages.iter().rev() {
        if m.role == "system" && m.content.starts_with(SUMMARY_PREFIX) {
            summary = Some(strip_summary_prefix(&m.content));
            break;
        }
    }
    let mut picked: Vec<(String, String)> = Vec::new();
    let mut used = 0usize;
    for m in messages.iter().rev() {
        if m.role != "user" && m.role != "assistant" {
            continue;
        }
        let chars = m.content.chars().count();
        if used + chars > budget && !picked.is_empty() {
            break;
        }
        used += chars;
        picked.push((m.role.clone(), m.content.clone()));
    }
    picked.reverse();
    (summary, picked)
}

/// 聊天历史组装（调用方已完成会话归属校验）：
/// 取严格早于 `before` 的最近消息（排除本轮刚落库的 user 消息）→
/// 摘要提取（窗口内优先，否则补查 DB 最近摘要）→ 预算截取。
fn resolve_chat_history(
    conn: &rusqlite::Connection,
    session_id: &str,
    budget: usize,
    before: &str,
) -> Result<crate::types::ChatHistory, rusqlite::Error> {
    let messages =
        db_session::list_recent_messages_before(conn, session_id, before, HISTORY_FETCH_LIMIT)?;
    let (summary, turns) = select_history_window(&messages, budget);
    let summary = match summary {
        Some(s) => Some(s),
        None => db_session::get_latest_summary(conn, session_id)?
            .map(|m| strip_summary_prefix(&m.content)),
    };
    Ok(crate::types::ChatHistory { summary, turns })
}

/// 读取并组装聊天历史（DB 锁约定：锁内取数 → 立即 drop guard → 再 await LLM）。
/// 无 sessionId / 锁失败 / 库未解锁 / 查询失败一律降级为空历史（warn 日志
/// 仅记元数据），历史读取故障永不阻断聊天。
fn load_chat_history(
    state: &SharedState,
    session_id: Option<&str>,
    before: &str,
) -> crate::types::ChatHistory {
    let session_id = match session_id {
        Some(id) if !id.trim().is_empty() => id,
        _ => return crate::types::ChatHistory::default(),
    };
    let budget = chat_history_budget_chars();
    let guard = match state.db.lock() {
        Ok(guard) => guard,
        Err(e) => {
            log::warn!(target: "chat", "load_chat_history lock_failed err={}", e);
            return crate::types::ChatHistory::default();
        }
    };
    let outcome = match get_conn(&guard) {
        Ok(conn) => {
            resolve_chat_history(conn, session_id, budget, before).map_err(|e| e.to_string())
        }
        Err(e) => Err(e),
    };
    drop(guard);
    match outcome {
        Ok(history) => {
            log::info!(
                target: "chat",
                "load_chat_history session_id={} turns={} history_chars={} summary_chars={} budget={}",
                session_id,
                history.turns.len(),
                history.turns.iter().map(|(_, c)| c.chars().count()).sum::<usize>(),
                history.summary.as_ref().map(|s| s.chars().count()).unwrap_or(0),
                budget
            );
            history
        }
        Err(e) => {
            log::warn!(
                target: "chat",
                "load_chat_history_failed session_id={} err={} fallback=no_history",
                session_id,
                e
            );
            crate::types::ChatHistory::default()
        }
    }
}

/// 解析聊天注入的技能 prompt（注入 /api/chat 与 /api/chat/stream 两条链路）：
/// 用户画像常驻段（user_id 有值且画像已完成时，永远注入、无需 agentId）
/// 在前（最高优先级），数字人技能段（agent_id 有值时）在后；合并后整体走
/// apply_skill_budget 共享预算（截断从尾部按段边界砍，画像在前天然优先保留）。
/// 画像段先经 apply_profile_budget（RG_PROFILE_SKILL_BUDGET_CHARS 默认 4000）
/// 预裁剪，防止超长画像挤掉全部数字人技能。
/// NLQ 链路（nlq_multi）不注入画像与技能（agents.md §11.5 约定）。
/// DB 锁约定：锁内取数据 → 立即 drop guard → 再由调用方 await LLM；
/// 任何错误（锁失败/库未解锁/查询失败）均 log::warn 后降级——画像/技能
/// 故障永不阻断聊天。日志仅记元数据，不落内容。
fn resolve_skills_prompt(state: &SharedState, agent_id: Option<&str>, user_id: Option<&str>) -> String {
    if agent_id.is_none() && user_id.is_none() {
        return String::new();
    }
    let guard = match state.db.lock() {
        Ok(guard) => guard,
        Err(e) => {
            log::warn!(target: "chat", "resolve_skills_prompt lock_failed err={}", e);
            return String::new();
        }
    };
    // 一次加锁内先取画像后取数字人技能，各自失败独立降级互不影响
    let (profile_section, agent_section) = match get_conn(&guard) {
        Ok(conn) => {
            let profile_section = match user_id {
                Some(user_id) => match user_db::get_profile_doc(conn, user_id) {
                    Ok(Some(doc)) => {
                        let section = agent_config::build_profile_skill_prompt(&doc);
                        agent_config::apply_profile_budget(
                            &section,
                            agent_config::profile_skill_budget_chars(),
                        )
                    }
                    Ok(None) => String::new(),
                    Err(e) => {
                        log::warn!(target: "chat", "resolve_skills_prompt profile_failed err={}", e);
                        String::new()
                    }
                },
                None => String::new(),
            };
            let agent_section = match agent_id {
                Some(agent_id) => match agent_config::build_skills_prompt(conn, agent_id) {
                    Ok(prompt) => prompt,
                    Err(e) => {
                        log::warn!(
                            target: "chat",
                            "resolve_skills_prompt skills_failed agent_id={} err={}",
                            agent_id,
                            e
                        );
                        String::new()
                    }
                },
                None => String::new(),
            };
            (profile_section, agent_section)
        }
        Err(e) => {
            log::warn!(target: "chat", "resolve_skills_prompt conn_failed err={}", e);
            drop(guard);
            return String::new();
        }
    };
    drop(guard);

    // 画像段在前（最高优先级），数字人技能段在后；合并后整体走共享预算，
    // 截断从尾部按段边界砍，画像在前天然优先保留
    let merged = format!("{}{}", profile_section, agent_section);
    let budget = agent_config::skill_budget_chars();
    let total_chars = merged.chars().count();
    let truncated = total_chars > budget;
    let merged = agent_config::apply_skill_budget(&merged, budget);
    log::info!(
        target: "chat",
        "resolve_skills_prompt profile_chars={} agent_chars={} merged_chars={} budget={} truncated={}",
        profile_section.chars().count(),
        agent_section.chars().count(),
        total_chars,
        budget,
        truncated
    );
    merged
}

// ---------- SSE 流式聊天 ----------

/// SSE 事件构建：事件契约见前端约定（step / thinking_delta / text_delta / done / error）。
/// axum 0.7 的 Event::data 要求 AsRef<str>，统一先序列化为 JSON 字符串。
fn sse_step(stage: &str, detail: &str) -> Event {
    Event::default().event("step").data(
        serde_json::json!({ "stage": stage, "detail": detail }).to_string(),
    )
}

fn sse_delta(event_name: &str, text: String) -> Event {
    Event::default()
        .event(event_name)
        .data(serde_json::json!({ "text": text }).to_string())
}

fn sse_done(usage: Option<(usize, usize)>, backend: &str) -> Event {
    Event::default().event("done").data(
        serde_json::json!({
            "usage": usage.map(|(input, output)| serde_json::json!({ "input": input, "output": output })),
            "backend": backend,
        })
        .to_string(),
    )
}

fn sse_error(message: String) -> Event {
    Event::default()
        .event("error")
        .data(serde_json::json!({ "message": message }).to_string())
}

type RigEventStream = Pin<Box<dyn Stream<Item = Result<crate::llm::ChatStreamEvent, String>> + Send>>;

type AgentEventStream = crate::llm::AgentEventStream;

/// Agent 工具循环事件 → SSE 映射：Reasoning/Text 透传为增量，ToolCall 发
/// step(tool_call) 供前端展示进度，Done 携带末轮 usage。
async fn poll_agent_event(
    mut stream: AgentEventStream,
    thinking_count: &AtomicUsize,
    text_count: &AtomicUsize,
) -> Option<(Result<Event, Infallible>, ChatStreamPhase)> {
    use futures::StreamExt;
    match stream.next().await {
        Some(Ok(crate::llm::AgentStreamEvent::Reasoning(text))) => {
            thinking_count.fetch_add(1, Ordering::SeqCst);
            Some((
                Ok(sse_delta("thinking_delta", text)),
                ChatStreamPhase::Agent(stream),
            ))
        }
        Some(Ok(crate::llm::AgentStreamEvent::Text(text))) => {
            text_count.fetch_add(1, Ordering::SeqCst);
            Some((
                Ok(sse_delta("text_delta", text)),
                ChatStreamPhase::Agent(stream),
            ))
        }
        Some(Ok(crate::llm::AgentStreamEvent::ToolCall(call))) => Some((
            Ok(sse_step(
                "tool_call",
                &format!("正在调用工具 {} 查询联系人数据", call.name),
            )),
            ChatStreamPhase::Agent(stream),
        )),
        Some(Ok(crate::llm::AgentStreamEvent::Done(usage))) => {
            Some((Ok(sse_done(usage, "cloud")), ChatStreamPhase::End))
        }
        Some(Err(e)) => Some((Ok(sse_error(e)), ChatStreamPhase::End)),
        None => Some((Ok(sse_done(None, "cloud")), ChatStreamPhase::End)),
    }
}

/// 流式聊天状态机：保证 step(llm_call) 在模型调用之前发出，
/// 每个 unfold 轮次恰产出一条 SSE 事件。
/// WebSearchStep：仅当请求了联网搜索时发 step(web_search)（复用现有
/// step 事件，stage="web_search"），未请求时直接跳到 llm_call，永不阻断。
enum ChatStreamPhase {
    Routing,
    WebSearchStep,
    ToolLoopStep,
    LlmCallStep,
    Init,
    Rig(RigEventStream),
    Agent(AgentEventStream),
    LegacyDone,
    End,
}

/// SSE 阶段机前奏 step 转移（纯函数，回归单测见 chat_stream_prelude_tests）：
/// 顺序固定 routing → web_search? → tool_loop? → llm_call → Init。
/// 回归防护（d9fe0de）：llm_call 恒发展示真实路由模型，
/// 不得被 web_search / tool_loop 吞掉（前端依赖此 step 展示模型名）。
/// 非前奏阶段返回 None（由调用方兑底）。
fn prelude_step_transition(
    phase: &ChatStreamPhase,
    backend: &str,
    web_search_step: Option<&str>,
    tools_step: Option<&str>,
    model_name: &str,
) -> Option<(&'static str, String, ChatStreamPhase)> {
    match phase {
        ChatStreamPhase::Routing => Some((
            "routing",
            format!("backend={}", backend),
            ChatStreamPhase::WebSearchStep,
        )),
        ChatStreamPhase::WebSearchStep => match web_search_step {
            // 有联网搜索 step 先发，工具/llm step 留给后续轮次
            Some(detail) => Some((
                "web_search",
                detail.to_string(),
                ChatStreamPhase::ToolLoopStep,
            )),
            None => tool_or_llm_step(tools_step, model_name),
        },
        ChatStreamPhase::ToolLoopStep => tool_or_llm_step(tools_step, model_name),
        ChatStreamPhase::LlmCallStep => Some((
            "llm_call",
            format!("model={}", model_name),
            ChatStreamPhase::Init,
        )),
        _ => None,
    }
}

/// 前奏转移共用分支：有工具 step 先发 tool_loop 再进 llm_call，
/// 否则直接发 llm_call（llm_call 恒发）
fn tool_or_llm_step(
    tools_step: Option<&str>,
    model_name: &str,
) -> Option<(&'static str, String, ChatStreamPhase)> {
    match tools_step {
        Some(detail) => Some((
            "tool_loop",
            detail.to_string(),
            ChatStreamPhase::LlmCallStep,
        )),
        None => Some((
            "llm_call",
            format!("model={}", model_name),
            ChatStreamPhase::Init,
        )),
    }
}

/// 从 rig/cloud 事件流取下一条 SSE 事件；流耗尽时补发 done（usage=null）。
/// done 的 backend 字段随流传入（"rig" / "cloud"），不再硬编码。
/// 顺带累加事件计数（仅元数据，不落内容）。
async fn poll_rig_event(
    mut stream: RigEventStream,
    backend: &str,
    thinking_count: &AtomicUsize,
    text_count: &AtomicUsize,
) -> Option<(Result<Event, Infallible>, ChatStreamPhase)> {
    use futures::StreamExt;
    match stream.next().await {
        Some(Ok(crate::llm::ChatStreamEvent::Reasoning(text))) => {
            thinking_count.fetch_add(1, Ordering::SeqCst);
            Some((
                Ok(sse_delta("thinking_delta", text)),
                ChatStreamPhase::Rig(stream),
            ))
        }
        Some(Ok(crate::llm::ChatStreamEvent::Text(text))) => {
            text_count.fetch_add(1, Ordering::SeqCst);
            Some((
                Ok(sse_delta("text_delta", text)),
                ChatStreamPhase::Rig(stream),
            ))
        }
        Some(Ok(crate::llm::ChatStreamEvent::Done(usage))) => {
            Some((Ok(sse_done(usage, backend)), ChatStreamPhase::End))
        }
        Some(Err(e)) => Some((Ok(sse_error(e)), ChatStreamPhase::End)),
        None => Some((Ok(sse_done(None, backend)), ChatStreamPhase::End)),
    }
}

/// POST /api/chat/stream — SSE 流式聊天。
/// rig / cloud 路径：general_chat_stream 内部按通道分流（rig ollama 或百炼
/// CompletionsClient），增量推送 thinking_delta / text_delta，Final 携带 usage；
/// legacy 降级路径：step 事件后同步调用 general_chat，完整回复作为单条 text_delta。
async fn chat_stream_handler(
    State(state): State<SharedState>,
    user: Option<Extension<AuthUser>>,
    Json(req): Json<ChatRequest>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, ApiError> {
    let query = req.query.trim().to_string();
    if query.is_empty() {
        return Err(ApiError::bad_request("问题不能为空"));
    }

    // 技能 prompt 在 unfold 之前同步解析（锁内取数 → drop guard），
    // clone 进闭包后 rig / cloud / legacy 分支共用
    let user_id = user.map(|u| (u.0).0);
    // 归属校验前置：携带 sessionId 时必须属于当前用户，否则 404（不泄露存在性）
    verify_chat_session_owned(&state, req.session_id.as_deref(), user_id.as_deref())?;
    // 聊天历史组装：request_at 捕获请求时刻，严格排除本轮刚落库的 user 消息；
    // 读取失败降级为空历史，永不阻断聊天
    let request_at = Utc::now().to_rfc3339();
    let history = load_chat_history(&state, req.session_id.as_deref(), &request_at);
    let skills = resolve_skills_prompt(&state, req.agent_id.as_deref(), user_id.as_deref());

    let backend = crate::llm::general_chat_backend();
    // 联网搜索：请求开关 AND env 总闸 AND 云端通道；非 cloud 发降级
    // step 后置 false 继续，永不阻断
    let (web_search, web_search_step) =
        resolve_web_search(req.web_search.unwrap_or(false), backend);
    // 模型名在 web_search 决议后计算：联网请求实际路由到搜索模型，llm_call step 展示真实模型
    let model_name = crate::llm::general_chat_model_for(web_search);
    // 文档上下文注入（预算 RG_DOC_CONTEXT_CHARS）
    let (documents_prompt, doc_count, doc_chars, doc_truncated) =
        resolve_documents_prompt(req.documents);
    // Agent 工具循环（仅 cloud 通道）：建流失败降级为普通流式聊天，永不阻断；
    // 未关联用户的会话禁用工具（无归属可查）
    let (tools_enabled, tools_step) = resolve_data_tools_enabled(backend);
    let tools_enabled = tools_enabled && user_id.is_some();
    let agent_stream: Option<AgentEventStream> = if tools_enabled {
        let owner_id = user_id.clone().unwrap_or_default();
        let system = crate::llm::tool_loop_system_prompt(&query, &skills, web_search, &documents_prompt);
        match crate::llm::cloud_agent_stream(
            system,
            query.clone(),
            web_search,
            state.clone(),
            owner_id,
            crate::llm::AGENT_MAX_TOOL_TURNS,
            &history,
        )
        .await
        {
            Ok(s) => Some(s),
            Err(e) => {
                log::warn!(target: "llm", "agent_stream_init_failed fallback=general_chat err={}", e);
                None
            }
        }
    } else {
        None
    };
    // 建流失败时同步降级 step 文案，避免前端收到误导性的工具提示
    let tools_step = if agent_stream.is_some() { tools_step } else { None };
    log::info!(
        target: "llm",
        "chat_stream_start backend={} model={} query_len={} skills_chars={} web_search={} doc_count={} doc_chars={} truncated={} agent_tools={}",
        backend,
        model_name,
        query.chars().count(),
        skills.chars().count(),
        web_search,
        doc_count,
        doc_chars,
        doc_truncated,
        agent_stream.is_some()
    );

    // 仅记元数据的事件计数（不落内容）
    let thinking_count = Arc::new(AtomicUsize::new(0));
    let text_count = Arc::new(AtomicUsize::new(0));

    let stream = futures::stream::unfold(
        (ChatStreamPhase::Routing, agent_stream),
        move |(phase, agent_stream)| {
            let query = query.clone();
            let skills = skills.clone();
            let documents_prompt = documents_prompt.clone();
            let history = history.clone();
            let model_name = model_name.clone();
            let thinking_count = thinking_count.clone();
            let text_count = text_count.clone();
            async move {
                match phase {
                    // 前奏 step 统一走纯函数转移（含回归单测）：
                    // routing → web_search? → tool_loop? → llm_call（恒发）
                    p @ (ChatStreamPhase::Routing
                    | ChatStreamPhase::WebSearchStep
                    | ChatStreamPhase::ToolLoopStep
                    | ChatStreamPhase::LlmCallStep) => {
                        let (stage, detail, next) = prelude_step_transition(
                            &p,
                            backend,
                            web_search_step,
                            tools_step,
                            &model_name,
                        )
                        .expect("prelude phase transition is total");
                        Some((Ok(sse_step(stage, &detail)), (next, agent_stream)))
                    }
                    ChatStreamPhase::Init => {
                        // Agent 工具循环优先（仅 cloud 且建流成功时）；
                        // 其余 rig / cloud 走普通流式，legacy 降级非流式一次性调用
                        if let Some(stream) = agent_stream {
                            return poll_agent_event(stream, &thinking_count, &text_count)
                                .await
                                .map(|(event, phase)| (event, (phase, None)));
                        }
                        if backend == "rig" || backend == "cloud" {
                            match crate::llm::general_chat_stream(&query, &skills, web_search, &documents_prompt, &history).await {
                                Ok(stream) => {
                                    poll_rig_event(stream, backend, &thinking_count, &text_count)
                                        .await
                                        .map(|(event, phase)| (event, (phase, None)))
                                }
                                Err(e) => Some((Ok(sse_error(e)), (ChatStreamPhase::End, None))),
                            }
                        } else {
                            match crate::llm::general_chat(&query, &skills, web_search, &documents_prompt, &history).await {
                                Ok(reply) => {
                                    text_count.fetch_add(1, Ordering::SeqCst);
                                    Some((
                                        Ok(sse_delta("text_delta", reply)),
                                        (ChatStreamPhase::LegacyDone, None),
                                    ))
                                }
                                Err(e) => Some((Ok(sse_error(e)), (ChatStreamPhase::End, None))),
                            }
                        }
                    }
                    ChatStreamPhase::Rig(stream) => {
                        poll_rig_event(stream, backend, &thinking_count, &text_count)
                            .await
                            .map(|(event, phase)| (event, (phase, None)))
                    }
                    ChatStreamPhase::Agent(stream) => {
                        poll_agent_event(stream, &thinking_count, &text_count)
                            .await
                            .map(|(event, phase)| (event, (phase, None)))
                    }
                    // 仅 legacy 非流式降级路径可达，done 固定报 "legacy"
                    ChatStreamPhase::LegacyDone => {
                        Some((Ok(sse_done(None, "legacy")), (ChatStreamPhase::End, None)))
                    }
                    ChatStreamPhase::End => {
                        log::info!(
                            target: "llm",
                            "chat_stream_finish backend={} model={} thinking_events={} text_events={}",
                            backend,
                            model_name,
                            thinking_count.load(Ordering::SeqCst),
                            text_count.load(Ordering::SeqCst)
                        );
                        None
                    }
                }
            }
        },
    );

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

// ---------- NLQ Multi-Intent ----------

async fn nlq_multi_handler(
    State(state): State<SharedState>,
    user: Option<Extension<AuthUser>>,
    Json(req): Json<NlqMultiRequest>,
) -> Result<Json<NlqResponse>, ApiError> {
    let owner_id = require_user_id(user)?;
    let route_mode = req.route_mode.as_deref().unwrap_or("auto");
    let intent = nlq::classify_intent(&req.query);
    log::info!(
        target: "nlq",
        "nlq_multi_handler route_mode={} intent={} query_len={}",
        route_mode,
        intent,
        req.query.chars().count()
    );

    match intent {
        "search_people" => {
            let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
            let conn = get_conn(&guard)?;
            let results = nlq::natural_language_query(
                conn,
                &owner_id,
                NlqRequest {
                    query: req.query,
                    reveal_sensitive: req.reveal_sensitive,
                },
            )?;
            Ok(Json(NlqResponse::SearchPeople { results }))
        }
        "create_person" => {
            // P1-6：抽取失败显性报错（不再静默降级为空草稿）
            let draft = crate::llm::extract_person_fields(&req.query)
                .await
                .map_err(ApiError::internal)?;
            Ok(Json(NlqResponse::CreatePersonDraft { draft }))
        }
        "delete_person" => {
            // 人名提取：LLM 优先，失败/未识别时规则兜底（既有设计）；
            // 空名由 handle_delete_person_sync 返回提示草稿
            let target_name = match crate::llm::extract_delete_target(&req.query).await {
                Ok(name) if !name.trim().is_empty() => name,
                _ => nlq::extract_delete_name_fallback(&req.query),
            };
            let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
            let conn = get_conn(&guard)?;
            let response = nlq::handle_delete_person_sync(conn, &owner_id, &target_name)?;
            Ok(Json(response))
        }
        "update_person" => {
            // P1-6：抽取失败显性报错
            let (target_name, changes) = crate::llm::extract_update_fields(&req.query)
                .await
                .map_err(ApiError::internal)?;
            let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
            let conn = get_conn(&guard)?;
            let response = nlq::handle_update_person_sync(conn, &owner_id, &target_name, changes)?;
            Ok(Json(response))
        }
        "add_interaction" => {
            // P1-6：抽取失败显性报错（不再静默降级为空草稿）
            let draft = crate::llm::extract_interaction_data(&req.query)
                .await
                .map_err(ApiError::internal)?;
            let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
            let conn = get_conn(&guard)?;
            let response = nlq::handle_add_interaction_sync(conn, &owner_id, draft)?;
            Ok(Json(response))
        }
        "find_path" => {
            // P1-6：未识别出目标人名时显性报错
            let target_name = crate::llm::extract_path_target(&req.query)
                .await
                .map_err(ApiError::internal)?;
            let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
            let conn = get_conn(&guard)?;
            let response = nlq::handle_find_path_sync(conn, &owner_id, &target_name)?;
            Ok(Json(response))
        }
        _ => {
            // fallback to search
            let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
            let conn = get_conn(&guard)?;
            let results = nlq::natural_language_query(
                conn,
                &owner_id,
                NlqRequest {
                    query: req.query,
                    reveal_sensitive: req.reveal_sensitive,
                },
            )?;
            Ok(Json(NlqResponse::SearchPeople { results }))
        }
    }
}

async fn nlq_confirm_handler(
    State(state): State<SharedState>,
    user: Option<Extension<AuthUser>>,
    Json(req): Json<NlqConfirmRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let owner_id = require_user_id(user)?;
    log::info!(target: "nlq", "nlq_confirm_handler intent_type={}", req.intent_type);

    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;

    match req.intent_type.as_str() {
        // 前端草稿确认使用 camelCase（createPersonDraft 等），旧调用方使用 snake_case，两者均接受
        "create_person" | "createPersonDraft" => {
            let create_req: CreatePersonRequest = serde_json::from_value(req.data)
                .map_err(|e| ApiError::bad_request(format!("Invalid data: {}", e)))?;
            let created = person::create(conn, &owner_id, create_req)?;
            Ok(Json(serde_json::to_value(created).unwrap()))
        }
        "delete_person" | "deletePersonDraft" => {
            // 前端草稿确认契约：{personId}；删除为不可逆操作，先校验存在与归属
            let id = req.data["personId"]
                .as_str()
                .or_else(|| req.data["id"].as_str())
                .ok_or_else(|| ApiError::bad_request("Missing person id"))?
                .to_string();
            person::get_by_id(conn, &owner_id, &id)?
                .ok_or_else(|| ApiError::not_found("数据不存在或无权访问"))?;
            person::delete(conn, &owner_id, &id)?;
            log::info!(target: "nlq", "delete_person_confirmed");
            Ok(Json(serde_json::json!({ "deleted": true })))
        }
        "update_person" | "updatePersonDraft" => {
            if req.data.get("changes").is_some() {
                // 前端草稿确认契约：{personId, changes:[{field, oldValue, newValue}]}
                // 在现有联系人全量字段上叠加 changes 后走 person::update
                let id = req.data["personId"]
                    .as_str()
                    .ok_or_else(|| ApiError::bad_request("Missing person id"))?
                    .to_string();
                let existing = person::get_by_id(conn, &owner_id, &id)?
                    .ok_or_else(|| ApiError::not_found("数据不存在或无权访问"))?;
                let changes: Vec<FieldChange> = serde_json::from_value(req.data["changes"].clone())
                    .map_err(|e| ApiError::bad_request(format!("Invalid changes: {}", e)))?;
                let mut update_req = CreatePersonRequest {
                    name: existing.name.clone(),
                    aliases: existing.aliases.clone(),
                    avatar: existing.avatar.clone(),
                    phone: existing.phone.clone(),
                    email: existing.email.clone(),
                    company: existing.company.clone(),
                    title: existing.title.clone(),
                    location: existing.location.clone(),
                    background: existing.background.clone(),
                    relationship_strength: existing.relationship_strength.clone(),
                    resource_tags: existing.resource_tags.clone(),
                    sensitivity_level: existing.sensitivity_level.clone(),
                    status: Some(existing.status.clone()),
                    next_step: existing.next_step.clone(),
                    notes: existing.notes.clone(),
                    school: existing.school.clone(),
                    projects: existing.projects.clone(),
                };
                for c in &changes {
                    let v = match c.new_value.trim() {
                        "" => None,
                        s => Some(s.to_string()),
                    };
                    match c.field.as_str() {
                        "name" => {
                            if let Some(v) = v {
                                update_req.name = v;
                            }
                        }
                        "company" => update_req.company = v,
                        "title" => update_req.title = v,
                        "location" => update_req.location = v,
                        "school" => update_req.school = v,
                        "background" => update_req.background = v,
                        "phone" => update_req.phone = v,
                        "email" => update_req.email = v,
                        "status" => update_req.status = v,
                        "next_step" | "nextStep" => update_req.next_step = v,
                        "notes" => update_req.notes = v,
                        _ => {}
                    }
                }
                let updated = person::update(conn, &owner_id, &id, update_req)?;
                Ok(Json(serde_json::to_value(updated).unwrap()))
            } else {
                // 旧契约：{id, ...完整字段}
                let id = req.data["id"]
                    .as_str()
                    .ok_or_else(|| ApiError::bad_request("Missing person id"))?
                    .to_string();
                let update_req: CreatePersonRequest = serde_json::from_value(req.data)
                    .map_err(|e| ApiError::bad_request(format!("Invalid data: {}", e)))?;
                let updated = person::update(conn, &owner_id, &id, update_req)?;
                Ok(Json(serde_json::to_value(updated).unwrap()))
            }
        }
        "add_interaction" | "addInteractionDraft" => {
            let create_req = if req.data.get("content").is_none() {
                // 前端草稿确认契约：{personId, topic, summary, actionItems}
                let person_id = req.data["personId"]
                    .as_str()
                    .ok_or_else(|| ApiError::bad_request("Missing person id"))?
                    .to_string();
                let topic = req.data["topic"]
                    .as_str()
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty());
                let summary = req.data["summary"]
                    .as_str()
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty());
                let action_items: Vec<String> =
                    serde_json::from_value(req.data["actionItems"].clone()).unwrap_or_default();
                let content = summary.clone().or_else(|| topic.clone()).unwrap_or_default();
                CreateInteractionRequest {
                    person_id,
                    timestamp: Utc::now().to_rfc3339(),
                    content,
                    summary,
                    topics: topic.into_iter().collect(),
                    action_items,
                }
            } else {
                // 旧契约：完整 CreateInteractionRequest
                serde_json::from_value(req.data)
                    .map_err(|e| ApiError::bad_request(format!("Invalid data: {}", e)))?
            };
            let created = interaction::create(conn, &owner_id, create_req)?;
            Ok(Json(serde_json::to_value(created).unwrap()))
        }
        _ => Err(ApiError::bad_request(format!(
            "Unknown intent_type: {}",
            req.intent_type
        ))),
    }
}

#[cfg(test)]
mod session_ownership_tests {
    use super::*;
    use crate::types::Session;

    fn session(user_id: &str) -> Session {
        Session {
            id: "s-1".to_string(),
            user_id: user_id.to_string(),
            title: None,
            created_at: "t".to_string(),
            updated_at: "t".to_string(),
        }
    }

    /// 归属判定语义：本人通过；跨用户与不存在一律拒绝（handler 层映射为
    /// 404「数据不存在或无权访问」，不泄露存在性）
    #[test]
    fn session_owned_by_semantics() {
        assert!(session_owned_by(Some(&session("u1")), "u1"));
        assert!(!session_owned_by(Some(&session("u2")), "u1"));
        assert!(!session_owned_by(None, "u1"));
    }
}

#[cfg(test)]
mod chat_history_tests {
    use super::*;
    use crate::types::ChatMessage;
    use rusqlite::{params, Connection};

    fn msg(role: &str, content: &str, created_at: &str) -> ChatMessage {
        ChatMessage {
            id: String::new(),
            session_id: "s-1".to_string(),
            role: role.to_string(),
            content: content.to_string(),
            metadata_json: None,
            created_at: created_at.to_string(),
        }
    }

    fn in_memory_db() -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory db");
        schema::migrate(&conn).expect("schema migration");
        conn
    }

    fn seed_session(conn: &Connection, session_id: &str) {
        // sessions.user_id 有外键约束（migrate 已启用 foreign_keys），先造用户
        let _ = conn.execute(
            "INSERT OR IGNORE INTO users (id, username, password_hash, role, created_at, updated_at)
             VALUES ('u1', 'u1', 'x', 'user', 't', 't')",
            [],
        );
        conn.execute(
            "INSERT INTO sessions (id, user_id, title, created_at, updated_at) VALUES (?1, 'u1', NULL, 't', 't')",
            params![session_id],
        )
        .expect("insert session");
    }

    fn seed_message(conn: &Connection, id: &str, session_id: &str, role: &str, content: &str, created_at: &str) {
        conn.execute(
            "INSERT INTO chat_messages (id, session_id, role, content, metadata_json, created_at)
             VALUES (?1, ?2, ?3, ?4, NULL, ?5)",
            params![id, session_id, role, content, created_at],
        )
        .expect("insert message");
    }

    /// 预算默认基线（仅在 env 未设置时断言，避免并发测试改 env 竞态）
    #[test]
    fn budget_default_baseline() {
        if std::env::var("RG_CHAT_HISTORY_CHARS").is_ok() {
            return;
        }
        assert_eq!(chat_history_budget_chars(), 8000);
    }

    /// 空消息序列 → 空窗口无摘要
    #[test]
    fn empty_window() {
        let (summary, turns) = select_history_window(&[], 8000);
        assert!(summary.is_none());
        assert!(turns.is_empty());
    }

    /// 预算截取：从新到旧累加，超出即止，输出维持时间升序
    #[test]
    fn budget_truncates_newest_to_oldest() {
        let messages = vec![
            msg("user", "旧问题一", "t1"),       // 4 字
            msg("assistant", "旧回答一", "t2"),   // 4 字
            msg("user", "新问题二", "t3"),       // 4 字
            msg("assistant", "新回答二", "t4"),   // 4 字
        ];
        // 预算 8：仅容纳最新两条（新回答二 + 新问题二）
        let (summary, turns) = select_history_window(&messages, 8);
        assert!(summary.is_none());
        assert_eq!(
            turns,
            vec![
                ("user".to_string(), "新问题二".to_string()),
                ("assistant".to_string(), "新回答二".to_string()),
            ]
        );
        // 预算恰容纳全部 → 全量保留且升序
        let (_, turns) = select_history_window(&messages, 16);
        assert_eq!(turns.len(), 4);
        assert_eq!(turns[0].1, "旧问题一");
        assert_eq!(turns[3].1, "新回答二");
    }

    /// 最近一条单独超预算也保留（避免长消息导致历史恒空）
    #[test]
    fn single_oversize_recent_message_kept() {
        let messages = vec![
            msg("user", "旧", "t1"),
            msg("assistant", &"长".repeat(100), "t2"),
        ];
        let (_, turns) = select_history_window(&messages, 10);
        assert_eq!(turns.len(), 1);
        assert_eq!(turns[0].0, "assistant");
    }

    /// 摘要提取：最近一条 [对话摘要] system 消息剥离前缀后作为 summary，
    /// 不进入 turns；多条取最近
    #[test]
    fn summary_extracted_and_excluded_from_turns() {
        let messages = vec![
            msg("system", "[对话摘要] 旧摘要", "t1"),
            msg("user", "问题", "t2"),
            msg("assistant", "回答", "t3"),
            msg("system", "[对话摘要] 新摘要", "t4"),
        ];
        let (summary, turns) = select_history_window(&messages, 8000);
        assert_eq!(summary.as_deref(), Some("新摘要"));
        assert_eq!(turns.len(), 2);
        assert!(turns.iter().all(|(role, _)| role != "system"));
    }

    /// 排除本轮消息：created_at 不早于 before 的消息不进入历史
    ///（前端先 addMessage 落库本轮 user 消息再发聊天请求）
    #[test]
    fn resolve_excludes_current_turn_message() {
        let conn = in_memory_db();
        seed_session(&conn, "s-1");
        seed_message(&conn, "m1", "s-1", "user", "上一轮问题", "2026-08-08T01:00:00+00:00");
        seed_message(&conn, "m2", "s-1", "assistant", "上一轮回答", "2026-08-08T01:00:01+00:00");
        // 本轮刚落库的 user 消息（时间戳晚于请求时刻 before）
        seed_message(&conn, "m3", "s-1", "user", "本轮问题", "2026-08-08T01:00:05+00:00");

        let history = resolve_chat_history(&conn, "s-1", 8000, "2026-08-08T01:00:04+00:00").unwrap();
        assert_eq!(history.turns.len(), 2);
        assert_eq!(history.turns[0].1, "上一轮问题");
        assert_eq!(history.turns[1].1, "上一轮回答");
        assert!(history.turns.iter().all(|(_, c)| c != "本轮问题"));
    }

    /// 空会话 → 空历史无摘要
    #[test]
    fn resolve_empty_session() {
        let conn = in_memory_db();
        seed_session(&conn, "s-empty");
        let history = resolve_chat_history(&conn, "s-empty", 8000, "2026-08-08T23:59:59+00:00").unwrap();
        assert!(history.is_empty());
    }

    /// 摘要落在取数窗口外（被更多新消息挤出）时补查 DB 最近摘要
    #[test]
    fn resolve_summary_outside_fetch_window() {
        let conn = in_memory_db();
        seed_session(&conn, "s-2");
        seed_message(&conn, "m0", "s-2", "system", "[对话摘要] 历史摘要", "2026-08-08T00:00:00+00:00");
        // 窗口预算极小：摘要虽在取数窗口内但被排除出 turns，仍应提取
        seed_message(&conn, "m1", "s-2", "user", "问题", "2026-08-08T01:00:00+00:00");
        seed_message(&conn, "m2", "s-2", "assistant", "回答", "2026-08-08T01:00:01+00:00");
        let history = resolve_chat_history(&conn, "s-2", 8000, "2026-08-08T23:59:59+00:00").unwrap();
        assert_eq!(history.summary.as_deref(), Some("历史摘要"));
        assert_eq!(history.turns.len(), 2);

        // 无窗口内摘要且 DB 也无 → summary=None
        seed_session(&conn, "s-3");
        seed_message(&conn, "m3", "s-3", "user", "问题", "2026-08-08T01:00:00+00:00");
        let history = resolve_chat_history(&conn, "s-3", 8000, "2026-08-08T23:59:59+00:00").unwrap();
        assert!(history.summary.is_none());
    }

    /// 摘要被超出取数窗口（HISTORY_FETCH_LIMIT 条更新消息挤出）时，
    /// 回退补查 DB 最近摘要仍能注入
    #[test]
    fn resolve_summary_fallback_when_outside_fetch_window() {
        let conn = in_memory_db();
        seed_session(&conn, "s-4");
        seed_message(&conn, "m0", "s-4", "system", "[对话摘要] 早期摘要", "2026-08-08T00:00:00+00:00");
        // 在摘要后插入超出取数窗口上限的消息，把摘要挤出窗口
        for i in 1..=(HISTORY_FETCH_LIMIT + 1) {
            seed_message(
                &conn,
                &format!("w-{}", i),
                "s-4",
                if i % 2 == 1 { "user" } else { "assistant" },
                "水",
                &format!("2026-08-08T01:{:02}:00+00:00", (i % 60)),
            );
        }
        let history = resolve_chat_history(&conn, "s-4", 8000, "2026-08-09T00:00:00+00:00").unwrap();
        assert_eq!(history.summary.as_deref(), Some("早期摘要"));
    }
}

#[cfg(test)]
mod web_search_resolve_tests {
    use super::*;

    /// 联网搜索开关解析（仅在 env RG_WEB_SEARCH 未设置的测试环境下断言，
    /// 避免与其它用例并发修改 env 的竞态）：cloud 生效、非 cloud 降级
    /// 置 false 且携带文案、未请求静默关闭
    #[test]
    fn resolve_web_search_semantics() {
        if std::env::var("RG_WEB_SEARCH").is_ok() {
            return;
        }
        assert_eq!(
            resolve_web_search(true, "cloud"),
            (true, Some("已启用联网搜索"))
        );
        assert_eq!(
            resolve_web_search(true, "legacy"),
            (false, Some("联网搜索仅云端通道可用，已按普通对话处理"))
        );
        assert_eq!(
            resolve_web_search(true, "rig"),
            (false, Some("联网搜索仅云端通道可用，已按普通对话处理"))
        );
        assert_eq!(resolve_web_search(false, "cloud"), (false, None));
    }

    /// Agent 数据工具开关语义（仅在 env RG_CHAT_TOOLS 未设置的测试环境下断言）：
    /// cloud 生效；非 cloud 降级置 false 且携带文案
    #[test]
    fn resolve_data_tools_enabled_semantics() {
        if std::env::var("RG_CHAT_TOOLS").is_ok() {
            return;
        }
        assert_eq!(
            resolve_data_tools_enabled("cloud"),
            (true, Some("已启用联系人数据工具"))
        );
        let (enabled, step) = resolve_data_tools_enabled("legacy");
        assert!(!enabled);
        assert!(step.is_some());
        let (enabled, step) = resolve_data_tools_enabled("rig");
        assert!(!enabled);
        assert!(step.is_some());
    }

    /// 空/None 文档 → 空串与全零元数据，prompt 逐字节不变前提
    #[test]
    fn resolve_documents_prompt_empty() {
        assert_eq!(resolve_documents_prompt(None), (String::new(), 0, 0, false));
        assert_eq!(
            resolve_documents_prompt(Some(vec![])),
            (String::new(), 0, 0, false)
        );
    }

    /// 单文档注入与元数据统计（文档内容不落日志，仅统计字符数）
    #[test]
    fn resolve_documents_prompt_single_doc() {
        let docs = vec![crate::types::ChatDocument {
            file_name: "报告.pdf".to_string(),
            content: "抽取文本".to_string(),
        }];
        let (prompt, count, chars, truncated) = resolve_documents_prompt(Some(docs));
        assert_eq!(prompt, "### 用户上传文档《报告.pdf》正文\n抽取文本");
        assert_eq!(count, 1);
        assert_eq!(chars, 4);
        assert!(!truncated);
    }
}

#[cfg(test)]
mod chat_stream_prelude_tests {
    use super::*;

    /// 从 Routing 走到 Init，收集前奏 step 序列（stage, detail）
    fn run_prelude(
        web_search_step: Option<&str>,
        tools_step: Option<&str>,
    ) -> Vec<(&'static str, String)> {
        let mut steps = Vec::new();
        let mut phase = ChatStreamPhase::Routing;
        while let Some((stage, detail, next)) =
            prelude_step_transition(&phase, "cloud", web_search_step, tools_step, "test-model")
        {
            steps.push((stage, detail));
            phase = next;
        }
        steps
    }

    fn stages(steps: &[(&'static str, String)]) -> Vec<&'static str> {
        steps.iter().map(|(s, _)| *s).collect()
    }

    /// 回归防护（d9fe0de）：联网搜索 + 工具循环同时存在时，llm_call 必须恒发
    /// 且顺序固定 routing → web_search → tool_loop → llm_call；
    /// regression 场景是 tool_loop 顶替了 llm_call，导致前端看不到模型名
    #[test]
    fn llm_call_emitted_when_web_search_and_tools_both_present() {
        let steps = run_prelude(Some("已启用联网搜索"), Some("已启用联系人数据工具"));
        assert_eq!(
            stages(&steps),
            vec!["routing", "web_search", "tool_loop", "llm_call"]
        );
        // llm_call 的 detail 必须携带真实路由模型名
        assert_eq!(steps.last().unwrap().1, "model=test-model");
    }

    /// 仅工具循环：llm_call 仍恒发
    #[test]
    fn llm_call_emitted_with_tools_only() {
        let steps = run_prelude(None, Some("已启用联系人数据工具"));
        assert_eq!(stages(&steps), vec!["routing", "tool_loop", "llm_call"]);
        assert_eq!(steps.last().unwrap().1, "model=test-model");
    }

    /// 仅联网搜索：llm_call 仍恒发
    #[test]
    fn llm_call_emitted_with_web_search_only() {
        let steps = run_prelude(Some("已启用联网搜索"), None);
        assert_eq!(stages(&steps), vec!["routing", "web_search", "llm_call"]);
        assert_eq!(steps.last().unwrap().1, "model=test-model");
    }

    /// 普通聊天（无联网无工具）：routing 后直接 llm_call
    #[test]
    fn plain_chat_emits_routing_then_llm_call() {
        let steps = run_prelude(None, None);
        assert_eq!(stages(&steps), vec!["routing", "llm_call"]);
        assert_eq!(steps.last().unwrap().1, "model=test-model");
    }

    /// 非前奏阶段返回 None，不会误发 step
    #[test]
    fn non_prelude_phase_returns_none() {
        assert!(prelude_step_transition(
            &ChatStreamPhase::Init,
            "cloud",
            None,
            None,
            "m"
        )
        .is_none());
        assert!(prelude_step_transition(
            &ChatStreamPhase::End,
            "cloud",
            None,
            None,
            "m"
        )
        .is_none());
    }
}
