//! HTTP API 层：对应原 Tauri Commands，统一 JSON 错误响应与鉴权。
//! 日志遵循脱敏原则：不记录密码、token、姓名、电话及内容原文。

use crate::db::{crypto::{derive_key, generate_salt, open_encrypted_db, validate_encrypted_db},
    get_conn, interaction, person, relationship, schema, user as user_db};
use crate::nlq::{self, NlqRequest, NlqResult};
use crate::security::auth;
use crate::security::sensitivity;
use crate::state::SharedState;
use crate::types::{
    ChatRequest, ChatResponse, CreateUserRequest,
    CreateEntityMentionRequest, CreateInteractionRequest, CreatePersonRequest,
    CreateRelationshipRequest, EntityMention, GraphData, GraphEdge, GraphNode, Interaction,
    NlqConfirmRequest, NlqMultiRequest, NlqResponse, Person, Relationship,
};
use axum::extract::{DefaultBodyLimit, Path, Query, Request, State};
use axum::http::StatusCode;
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use std::time::Instant;

mod admin;
mod import;
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
        .route("/api/nlq/confirm", post(nlq_confirm_handler))
        .route("/api/import/preview", post(import::preview))
        .route("/api/import/persons", post(import::commit))
        .route(
            "/api/voice/transcribe",
            // 默认 body 上限 2MB，放宽到音频上限 + multipart 编码开销；音频本体超 10MB 由 handler 返回 413
            post(voice::transcribe).layer(DefaultBodyLimit::max(voice::MAX_AUDIO_BYTES + 1024 * 1024)),
        )
        .route("/api/auth/lock", post(lock_database))
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
        // QA 指令模块管理
        .route(
            "/api/admin/qa-modules",
            get(admin::list_qa_modules).post(admin::create_qa_module),
        )
        .route(
            "/api/admin/qa-modules/:id",
            put(admin::update_qa_module).delete(admin::delete_qa_module),
        )
        .route_layer(middleware::from_fn_with_state(state.clone(), require_admin));

    Router::new()
        .route("/api/health", get(health))
        .route("/api/auth/state", get(auth_state))
        .route("/api/auth/setup", post(setup_database))
        .route("/api/auth/unlock", post(unlock_database))
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
        Self::internal(error.to_string())
    }
}

async fn require_auth(State(state): State<SharedState>, req: Request, next: Next) -> Response {
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
        Some(_) => next.run(req).await,
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
    unlocked: bool,
}

async fn auth_state(State(state): State<SharedState>) -> Result<Json<AuthState>, ApiError> {
    let initialized = state.db_path().exists() && state.salt_path().exists();
    let unlocked = state
        .db
        .lock()
        .map_err(|e| ApiError::internal(e.to_string()))?
        .is_some();
    log::info!(target: "security", "auth_state initialized={} unlocked={}", initialized, unlocked);
    Ok(Json(AuthState { initialized, unlocked }))
}

#[derive(Deserialize)]
struct PasswordRequest {
    password: String,
}

#[derive(Serialize)]
struct TokenResponse {
    token: String,
}

async fn setup_database(
    State(state): State<SharedState>,
    Json(req): Json<PasswordRequest>,
) -> Result<Json<TokenResponse>, ApiError> {
    let started = Instant::now();
    log::info!(target: "security", "setup_database_start");

    if req.password.trim().len() < 8 {
        log::warn!(target: "security", "setup_database_rejected reason=short_password");
        return Err(ApiError::bad_request("主密码至少需要 8 个字符"));
    }
    if state.db_path().exists() && state.salt_path().exists() {
        return Err(ApiError::bad_request("数据库已初始化，请直接解锁"));
    }

    std::fs::create_dir_all(&state.data_dir).map_err(|e| ApiError::internal(e.to_string()))?;
    let salt = generate_salt();
    let key_hex = derive_key(&req.password, &salt).map_err(|e| ApiError::internal(e.to_string()))?;
    std::fs::write(state.salt_path(), hex::encode(salt))
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let conn = open_encrypted_db(state.db_path(), &key_hex)
        .map_err(|e| ApiError::internal(e.to_string()))?;
    schema::migrate(&conn)?;

    // 首次 setup 时自动创建 admin 用户（主密码同时作为 admin 登录密码）
    let existing_users = user_db::list_users(&conn)?;
    if existing_users.is_empty() {
        let password_hash = auth::hash_password(&req.password)
            .map_err(ApiError::internal)?;
        user_db::create_user(&conn, CreateUserRequest {
            username: "admin".to_string(),
            password_hash,
            display_name: Some("管理员".to_string()),
            role: Some("admin".to_string()),
            profile_doc: None,
        })?;
        log::info!(target: "security", "setup_database admin_user_created");
    }

    let mut guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    *guard = Some(conn);
    drop(guard);

    let token = issue_token(&state)?;
    log::info!(
        target: "security",
        "setup_database_success elapsed_ms={}",
        started.elapsed().as_millis()
    );
    Ok(Json(TokenResponse { token }))
}

async fn unlock_database(
    State(state): State<SharedState>,
    Json(req): Json<PasswordRequest>,
) -> Result<Json<TokenResponse>, ApiError> {
    let started = Instant::now();
    log::info!(target: "security", "unlock_database_start source=manual_password");

    let salt_hex = std::fs::read_to_string(state.salt_path())
        .map_err(|_| ApiError::bad_request("数据库尚未初始化"))?;
    let salt = hex::decode(salt_hex.trim()).map_err(|e| ApiError::internal(e.to_string()))?;
    let key_hex = derive_key(&req.password, &salt).map_err(|e| ApiError::internal(e.to_string()))?;

    let conn = open_encrypted_db(state.db_path(), &key_hex)
        .map_err(|e| ApiError::internal(e.to_string()))?;
    validate_encrypted_db(&conn)
        .map_err(|_| ApiError::bad_request("主密码不正确"))?;
    schema::migrate(&conn)?;

    let mut guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    *guard = Some(conn);
    drop(guard);

    let token = issue_token(&state)?;
    log::info!(
        target: "security",
        "unlock_database_success source=manual_password elapsed_ms={}",
        started.elapsed().as_millis()
    );
    Ok(Json(TokenResponse { token }))
}

async fn lock_database(State(state): State<SharedState>) -> Result<Json<serde_json::Value>, ApiError> {
    let mut guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    *guard = None;
    drop(guard);
    state
        .tokens
        .lock()
        .map_err(|e| ApiError::internal(e.to_string()))?
        .revoke_all();
    log::info!(target: "security", "lock_database_success");
    Ok(Json(serde_json::json!({ "locked": true })))
}

fn issue_token(state: &SharedState) -> Result<String, ApiError> {
    Ok(state
        .tokens
        .lock()
        .map_err(|e| ApiError::internal(e.to_string()))?
        .issue())
}

// ---------- Person ----------

async fn create_person(
    State(state): State<SharedState>,
    Json(req): Json<CreatePersonRequest>,
) -> Result<Json<Person>, ApiError> {
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    let created = person::create(conn, req)?;
    log::info!(target: "person_cmd", "create_person_success sensitivity={}", created.sensitivity_level);
    Ok(Json(created))
}

async fn update_person(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    Json(req): Json<CreatePersonRequest>,
) -> Result<Json<Person>, ApiError> {
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    let updated = person::update(conn, &id, req)?;
    log::info!(target: "person_cmd", "update_person_success");
    Ok(Json(updated))
}

async fn list_persons(State(state): State<SharedState>) -> Result<Json<Vec<Person>>, ApiError> {
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    let persons = person::list_all(conn)?;
    log::info!(target: "person_cmd", "list_persons_success count={}", persons.len());
    Ok(Json(persons))
}

async fn get_person(
    State(state): State<SharedState>,
    Path(id): Path<String>,
) -> Result<Json<Option<Person>>, ApiError> {
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    Ok(Json(person::get_by_id(conn, &id)?))
}

async fn delete_person(
    State(state): State<SharedState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    person::delete(conn, &id)?;
    log::info!(target: "person_cmd", "delete_person_success");
    Ok(Json(serde_json::json!({ "deleted": true })))
}

#[derive(Deserialize)]
struct MentionQuery {
    mention: String,
}

async fn search_person_candidates(
    State(state): State<SharedState>,
    Query(query): Query<MentionQuery>,
) -> Result<Json<Vec<Person>>, ApiError> {
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    let persons = person::search_by_mention(conn, &query.mention)?;
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
    Json(req): Json<CreateRelationshipRequest>,
) -> Result<Json<Relationship>, ApiError> {
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    let created = relationship::create(conn, req)?;
    log::info!(target: "relationship_cmd", "create_relationship_success");
    Ok(Json(created))
}

async fn list_relationships(
    State(state): State<SharedState>,
) -> Result<Json<Vec<Relationship>>, ApiError> {
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    Ok(Json(relationship::list_all(conn)?))
}

async fn list_relationships_by_person(
    State(state): State<SharedState>,
    Path(id): Path<String>,
) -> Result<Json<Vec<Relationship>>, ApiError> {
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    Ok(Json(relationship::list_by_person(conn, &id)?))
}

async fn infer_relationships(State(state): State<SharedState>) -> Result<Json<serde_json::Value>, ApiError> {
    let started = Instant::now();
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    let created = crate::infer::run(conn)?;
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
) -> Result<Json<Vec<Relationship>>, ApiError> {
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    Ok(Json(relationship::list_pending(conn)?))
}

#[derive(Deserialize)]
struct ConfirmationRequest {
    status: String,
}

async fn set_relationship_confirmation(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    Json(req): Json<ConfirmationRequest>,
) -> Result<Json<Relationship>, ApiError> {
    if !["confirmed", "rejected", "pending"].contains(&req.status.as_str()) {
        return Err(ApiError::bad_request("确认状态只能是 confirmed / rejected / pending"));
    }
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    let updated = relationship::set_confirmation(conn, &id, &req.status)?;
    log::info!(target: "relationship_cmd", "set_relationship_confirmation_success status={}", req.status);
    Ok(Json(updated))
}

// ---------- Interaction ----------

async fn create_interaction(
    State(state): State<SharedState>,
    Json(req): Json<CreateInteractionRequest>,
) -> Result<Json<Interaction>, ApiError> {
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    let created = interaction::create(conn, req)?;
    log::info!(target: "interaction_cmd", "create_interaction_success");
    Ok(Json(created))
}

async fn list_interactions_by_person(
    State(state): State<SharedState>,
    Path(id): Path<String>,
) -> Result<Json<Vec<Interaction>>, ApiError> {
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    Ok(Json(interaction::list_by_person(conn, &id)?))
}

async fn create_entity_mention(
    State(state): State<SharedState>,
    Json(req): Json<CreateEntityMentionRequest>,
) -> Result<Json<EntityMention>, ApiError> {
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    Ok(Json(interaction::create_mention(conn, req)?))
}

// ---------- Graph ----------

async fn get_graph_data(State(state): State<SharedState>) -> Result<Json<GraphData>, ApiError> {
    let started = Instant::now();
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;

    let persons = person::list_all(conn)?;
    let relationships = relationship::list_all(conn)?;

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
    Json(req): Json<NlqRequest>,
) -> Result<Json<Vec<NlqResult>>, ApiError> {
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    let results = nlq::natural_language_query(conn, req)?;
    Ok(Json(results))
}

async fn chat_handler(Json(req): Json<ChatRequest>) -> Result<Json<ChatResponse>, ApiError> {
    let query = req.query.trim();
    if query.is_empty() {
        return Err(ApiError::bad_request("问题不能为空"));
    }

    log::info!(
        target: "chat",
        "chat_handler query_len={}",
        query.chars().count()
    );
    let reply = crate::llm::general_chat(query).await.map_err(ApiError::internal)?;
    Ok(Json(ChatResponse { reply }))
}

// ---------- NLQ Multi-Intent ----------

async fn nlq_multi_handler(
    State(state): State<SharedState>,
    Json(req): Json<NlqMultiRequest>,
) -> Result<Json<NlqResponse>, ApiError> {
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
                NlqRequest {
                    query: req.query,
                    reveal_sensitive: req.reveal_sensitive,
                },
            )?;
            Ok(Json(NlqResponse::SearchPeople { results }))
        }
        "create_person" => {
            let draft = crate::llm::extract_person_fields(&req.query).await;
            Ok(Json(NlqResponse::CreatePersonDraft { draft }))
        }
        "update_person" => {
            let (target_name, changes) = crate::llm::extract_update_fields(&req.query).await;
            let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
            let conn = get_conn(&guard)?;
            let response = nlq::handle_update_person_sync(conn, &target_name, changes)?;
            Ok(Json(response))
        }
        "add_interaction" => {
            let draft = crate::llm::extract_interaction_data(&req.query).await;
            let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
            let conn = get_conn(&guard)?;
            let response = nlq::handle_add_interaction_sync(conn, draft)?;
            Ok(Json(response))
        }
        "find_path" => {
            let target_name = crate::llm::extract_path_target(&req.query).await;
            let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
            let conn = get_conn(&guard)?;
            let response = nlq::handle_find_path_sync(conn, &target_name)?;
            Ok(Json(response))
        }
        _ => {
            // fallback to search
            let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
            let conn = get_conn(&guard)?;
            let results = nlq::natural_language_query(
                conn,
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
    Json(req): Json<NlqConfirmRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    log::info!(target: "nlq", "nlq_confirm_handler intent_type={}", req.intent_type);

    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;

    match req.intent_type.as_str() {
        "create_person" => {
            let create_req: CreatePersonRequest = serde_json::from_value(req.data)
                .map_err(|e| ApiError::bad_request(format!("Invalid data: {}", e)))?;
            let created = person::create(conn, create_req)?;
            Ok(Json(serde_json::to_value(created).unwrap()))
        }
        "update_person" => {
            let id = req.data["id"]
                .as_str()
                .ok_or_else(|| ApiError::bad_request("Missing person id"))?
                .to_string();
            let update_req: CreatePersonRequest = serde_json::from_value(req.data)
                .map_err(|e| ApiError::bad_request(format!("Invalid data: {}", e)))?;
            let updated = person::update(conn, &id, update_req)?;
            Ok(Json(serde_json::to_value(updated).unwrap()))
        }
        "add_interaction" => {
            let create_req: CreateInteractionRequest = serde_json::from_value(req.data)
                .map_err(|e| ApiError::bad_request(format!("Invalid data: {}", e)))?;
            let created = interaction::create(conn, create_req)?;
            Ok(Json(serde_json::to_value(created).unwrap()))
        }
        _ => Err(ApiError::bad_request(format!(
            "Unknown intent_type: {}",
            req.intent_type
        ))),
    }
}
