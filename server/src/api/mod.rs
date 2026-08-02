//! HTTP API 层：对应原 Tauri Commands，统一 JSON 错误响应与鉴权。
//! 日志遵循脱敏原则：不记录密码、token、姓名、电话及内容原文。

use crate::db::crypto::{derive_key, generate_salt, open_encrypted_db, validate_encrypted_db};
use crate::db::{get_conn, interaction, person, relationship, schema, user};
use crate::nlq::{self, NlqRequest, NlqResult};
use crate::security::sensitivity;
use crate::state::SharedState;
use crate::types::{
    AuthResponse, CreateEntityMentionRequest, CreateInteractionRequest, CreatePersonRequest,
    CreateRelationshipRequest, EntityMention, GraphData, GraphEdge, GraphNode, Interaction,
    LoginRequest, NlqConfirmRequest, NlqMultiRequest, NlqResponse, OAuthRequest, Person,
    RefreshRequest, RegisterRequest, Relationship, User,
};
use axum::extract::{DefaultBodyLimit, Extension, Path, Query, Request, State};
use axum::http::StatusCode;
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use std::time::Instant;

mod import;
mod voice;

// ---------- UserId 提取器 ----------

#[derive(Clone, Debug)]
pub struct UserId(pub String);

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
        .route("/api/nlq", post(natural_language_query))
        .route("/api/nlq/multi", post(nlq_multi_handler))
        .route("/api/nlq/confirm", post(nlq_confirm_handler))
        .route("/api/import/preview", post(import::preview))
        .route("/api/import/persons", post(import::commit))
        .route(
            "/api/voice/transcribe",
            post(voice::transcribe).layer(DefaultBodyLimit::max(voice::MAX_AUDIO_BYTES + 1024 * 1024)),
        )
        .route("/api/auth/lock", post(lock_database))
        .route("/api/auth/me", get(get_current_user))
        .route_layer(middleware::from_fn_with_state(state.clone(), require_auth));

    Router::new()
        .route("/api/health", get(health))
        .route("/api/auth/state", get(auth_state))
        .route("/api/auth/setup", post(setup_database))
        .route("/api/auth/unlock", post(unlock_database))
        .route("/api/auth/register", post(register_user))
        .route("/api/auth/login", post(login_user))
        .route("/api/auth/refresh", post(refresh_token))
        .route("/api/auth/oauth/:provider", post(oauth_callback))
        .merge(protected)
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

async fn require_auth(
    State(state): State<SharedState>,
    mut req: Request,
    next: Next,
) -> Response {
    let token = req
        .headers()
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .unwrap_or_default()
        .to_string();

    if token.is_empty() {
        log::warn!(target: "auth", "request_rejected reason=missing_token path={}", req.uri().path());
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "未登录或会话已过期" })),
        )
            .into_response();
    }

    // 优先尝试 JWT 验证
    if let Some(user_id) = state.jwt.validate_access_token(&token) {
        req.extensions_mut().insert(UserId(user_id));
        return next.run(req).await;
    }

    // 降级：旧 TokenStore 验证（向后兼容 unlock）
    let valid = state
        .tokens
        .lock()
        .map(|mut store| store.validate(&token))
        .unwrap_or(false);

    if valid {
        req.extensions_mut().insert(UserId("legacy".to_string()));
        return next.run(req).await;
    }

    log::warn!(target: "auth", "request_rejected reason=invalid_token path={}", req.uri().path());
    (
        StatusCode::UNAUTHORIZED,
        Json(serde_json::json!({ "error": "未登录或会话已过期" })),
    )
        .into_response()
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

    let mut guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    *guard = Some(conn);
    drop(guard);

    // unlock 成功后签发 JWT（user_id="legacy"）
    let (access_token, _refresh) = state.jwt.issue_tokens("legacy");
    // 同时兼容旧前端：也签发一个旧 token
    let _legacy_token = state
        .tokens
        .lock()
        .map_err(|e| ApiError::internal(e.to_string()))?
        .issue();

    log::info!(
        target: "security",
        "setup_database_success elapsed_ms={}",
        started.elapsed().as_millis()
    );
    Ok(Json(TokenResponse { token: access_token }))
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

    // unlock 成功后签发 JWT（user_id="legacy"）
    let (access_token, _refresh) = state.jwt.issue_tokens("legacy");
    // 同时签发旧 token（兼容旧前端）
    let _legacy_token = state
        .tokens
        .lock()
        .map_err(|e| ApiError::internal(e.to_string()))?
        .issue();

    log::info!(
        target: "security",
        "unlock_database_success source=manual_password elapsed_ms={}",
        started.elapsed().as_millis()
    );
    Ok(Json(TokenResponse { token: access_token }))
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

// ---------- 用户注册/登录/刷新 ----------

async fn register_user(
    State(state): State<SharedState>,
    Json(req): Json<RegisterRequest>,
) -> Result<Json<AuthResponse>, ApiError> {
    if req.username.trim().len() < 2 {
        return Err(ApiError::bad_request("用户名至少需要 2 个字符"));
    }
    if req.password.len() < 8 {
        return Err(ApiError::bad_request("密码至少需要 8 个字符"));
    }

    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;

    let created_user = user::create_user(
        conn,
        req.username.trim(),
        req.email.as_deref(),
        req.phone.as_deref(),
        &req.password,
    )
    .map_err(|e| ApiError::bad_request(e))?;

    let (access_token, refresh_token) = state.jwt.issue_tokens(&created_user.id);
    log::info!(target: "auth", "register_user_success user_id={}", created_user.id);

    Ok(Json(AuthResponse {
        access_token,
        refresh_token,
        user: created_user,
    }))
}

async fn login_user(
    State(state): State<SharedState>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<AuthResponse>, ApiError> {
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;

    let verified_user = user::verify_user(conn, &req.login, &req.password)
        .map_err(|e| ApiError { status: StatusCode::UNAUTHORIZED, message: e })?;

    let (access_token, refresh_token) = state.jwt.issue_tokens(&verified_user.id);
    log::info!(target: "auth", "login_user_success user_id={}", verified_user.id);

    Ok(Json(AuthResponse {
        access_token,
        refresh_token,
        user: verified_user,
    }))
}

async fn refresh_token(
    State(state): State<SharedState>,
    Json(req): Json<RefreshRequest>,
) -> Result<Json<AuthResponse>, ApiError> {
    let user_id = state
        .jwt
        .validate_refresh_token(&req.refresh_token)
        .ok_or_else(|| ApiError { status: StatusCode::UNAUTHORIZED, message: "刷新令牌无效或已过期".to_string() })?;

    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;

    let found_user = user::get_user_by_id(conn, &user_id)
        .map_err(|e| ApiError { status: StatusCode::UNAUTHORIZED, message: e })?;

    let (access_token, new_refresh_token) = state.jwt.issue_tokens(&found_user.id);
    log::info!(target: "auth", "refresh_token_success user_id={}", found_user.id);

    Ok(Json(AuthResponse {
        access_token,
        refresh_token: new_refresh_token,
        user: found_user,
    }))
}

async fn oauth_callback(
    State(state): State<SharedState>,
    Path(provider): Path<String>,
    Json(req): Json<OAuthRequest>,
) -> Result<Json<AuthResponse>, ApiError> {
    // Mock 模式：接收任何 code，创建/查找测试用户并返回 JWT
    log::info!(target: "auth", "oauth_callback provider={} code_len={}", provider, req.code.len());

    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;

    // 使用 code 作为 oauth_id（mock），用 provider_code 作为 username
    let oauth_id = &req.code;
    let username = format!("{}_{}", provider, &req.code[..req.code.len().min(8)]);

    let oauth_user = user::find_or_create_oauth_user(conn, &provider, oauth_id, &username)
        .map_err(|e| ApiError::internal(e))?;

    let (access_token, refresh_token) = state.jwt.issue_tokens(&oauth_user.id);
    log::info!(target: "auth", "oauth_callback_success provider={} user_id={}", provider, oauth_user.id);

    Ok(Json(AuthResponse {
        access_token,
        refresh_token,
        user: oauth_user,
    }))
}

async fn get_current_user(
    State(state): State<SharedState>,
    Extension(user_id): Extension<UserId>,
) -> Result<Json<User>, ApiError> {
    if user_id.0 == "legacy" {
        // legacy 用户返回一个虚拟用户
        return Ok(Json(User {
            id: "legacy".to_string(),
            username: "legacy".to_string(),
            email: None,
            phone: None,
            display_name: Some("旧版用户".to_string()),
            created_at: "2024-01-01T00:00:00Z".to_string(),
        }));
    }

    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;

    let found_user = user::get_user_by_id(conn, &user_id.0)
        .map_err(|e| ApiError { status: StatusCode::NOT_FOUND, message: e })?;

    Ok(Json(found_user))
}

// ---------- Person ----------

async fn create_person(
    State(state): State<SharedState>,
    Extension(user_id): Extension<UserId>,
    Json(req): Json<CreatePersonRequest>,
) -> Result<Json<Person>, ApiError> {
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    let created = person::create(conn, &user_id.0, req)?;
    log::info!(target: "person_cmd", "create_person_success sensitivity={}", created.sensitivity_level);
    Ok(Json(created))
}

async fn update_person(
    State(state): State<SharedState>,
    Extension(user_id): Extension<UserId>,
    Path(id): Path<String>,
    Json(req): Json<CreatePersonRequest>,
) -> Result<Json<Person>, ApiError> {
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    let updated = person::update(conn, &user_id.0, &id, req)?;
    log::info!(target: "person_cmd", "update_person_success");
    Ok(Json(updated))
}

async fn list_persons(
    State(state): State<SharedState>,
    Extension(user_id): Extension<UserId>,
) -> Result<Json<Vec<Person>>, ApiError> {
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    let persons = person::list_all(conn, &user_id.0)?;
    log::info!(target: "person_cmd", "list_persons_success count={}", persons.len());
    Ok(Json(persons))
}

async fn get_person(
    State(state): State<SharedState>,
    Extension(user_id): Extension<UserId>,
    Path(id): Path<String>,
) -> Result<Json<Option<Person>>, ApiError> {
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    Ok(Json(person::get_by_id(conn, &user_id.0, &id)?))
}

async fn delete_person(
    State(state): State<SharedState>,
    Extension(user_id): Extension<UserId>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    person::delete(conn, &user_id.0, &id)?;
    log::info!(target: "person_cmd", "delete_person_success");
    Ok(Json(serde_json::json!({ "deleted": true })))
}

#[derive(Deserialize)]
struct MentionQuery {
    mention: String,
}

async fn search_person_candidates(
    State(state): State<SharedState>,
    Extension(user_id): Extension<UserId>,
    Query(query): Query<MentionQuery>,
) -> Result<Json<Vec<Person>>, ApiError> {
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    let persons = person::search_by_mention(conn, &user_id.0, &query.mention)?;
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
    Extension(user_id): Extension<UserId>,
    Json(req): Json<CreateRelationshipRequest>,
) -> Result<Json<Relationship>, ApiError> {
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    let created = relationship::create(conn, &user_id.0, req)?;
    log::info!(target: "relationship_cmd", "create_relationship_success");
    Ok(Json(created))
}

async fn list_relationships(
    State(state): State<SharedState>,
    Extension(user_id): Extension<UserId>,
) -> Result<Json<Vec<Relationship>>, ApiError> {
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    Ok(Json(relationship::list_all(conn, &user_id.0)?))
}

async fn list_relationships_by_person(
    State(state): State<SharedState>,
    Extension(user_id): Extension<UserId>,
    Path(id): Path<String>,
) -> Result<Json<Vec<Relationship>>, ApiError> {
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    Ok(Json(relationship::list_by_person(conn, &user_id.0, &id)?))
}

async fn infer_relationships(
    State(state): State<SharedState>,
    Extension(user_id): Extension<UserId>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let started = Instant::now();
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    let created = crate::infer::run(conn, &user_id.0)?;
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
    Extension(user_id): Extension<UserId>,
) -> Result<Json<Vec<Relationship>>, ApiError> {
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    Ok(Json(relationship::list_pending(conn, &user_id.0)?))
}

#[derive(Deserialize)]
struct ConfirmationRequest {
    status: String,
}

async fn set_relationship_confirmation(
    State(state): State<SharedState>,
    Extension(user_id): Extension<UserId>,
    Path(id): Path<String>,
    Json(req): Json<ConfirmationRequest>,
) -> Result<Json<Relationship>, ApiError> {
    if !["confirmed", "rejected", "pending"].contains(&req.status.as_str()) {
        return Err(ApiError::bad_request("确认状态只能是 confirmed / rejected / pending"));
    }
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    let updated = relationship::set_confirmation(conn, &user_id.0, &id, &req.status)?;
    log::info!(target: "relationship_cmd", "set_relationship_confirmation_success status={}", req.status);
    Ok(Json(updated))
}

// ---------- Interaction ----------

async fn create_interaction(
    State(state): State<SharedState>,
    Extension(user_id): Extension<UserId>,
    Json(req): Json<CreateInteractionRequest>,
) -> Result<Json<Interaction>, ApiError> {
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    let created = interaction::create(conn, &user_id.0, req)?;
    log::info!(target: "interaction_cmd", "create_interaction_success");
    Ok(Json(created))
}

async fn list_interactions_by_person(
    State(state): State<SharedState>,
    Extension(user_id): Extension<UserId>,
    Path(id): Path<String>,
) -> Result<Json<Vec<Interaction>>, ApiError> {
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    Ok(Json(interaction::list_by_person(conn, &user_id.0, &id)?))
}

async fn create_entity_mention(
    State(state): State<SharedState>,
    Extension(user_id): Extension<UserId>,
    Json(req): Json<CreateEntityMentionRequest>,
) -> Result<Json<EntityMention>, ApiError> {
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    Ok(Json(interaction::create_mention(conn, &user_id.0, req)?))
}

// ---------- Graph ----------

async fn get_graph_data(
    State(state): State<SharedState>,
    Extension(user_id): Extension<UserId>,
) -> Result<Json<GraphData>, ApiError> {
    let started = Instant::now();
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;

    let persons = person::list_all(conn, &user_id.0)?;
    let relationships = relationship::list_all(conn, &user_id.0)?;

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
    Extension(user_id): Extension<UserId>,
    Json(req): Json<NlqRequest>,
) -> Result<Json<Vec<NlqResult>>, ApiError> {
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    let results = nlq::natural_language_query(conn, &user_id.0, req)?;
    Ok(Json(results))
}

// ---------- NLQ Multi-Intent ----------

async fn nlq_multi_handler(
    State(state): State<SharedState>,
    Extension(user_id): Extension<UserId>,
    Json(req): Json<NlqMultiRequest>,
) -> Result<Json<NlqResponse>, ApiError> {
    let intent = nlq::classify_intent(&req.query);
    log::info!(target: "nlq", "nlq_multi_handler intent={} query_len={}", intent, req.query.chars().count());

    match intent {
        "search_people" => {
            let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
            let conn = get_conn(&guard)?;
            let results = nlq::natural_language_query(
                conn,
                &user_id.0,
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
            let response = nlq::handle_update_person_sync(conn, &user_id.0, &target_name, changes)?;
            Ok(Json(response))
        }
        "add_interaction" => {
            let draft = crate::llm::extract_interaction_data(&req.query).await;
            let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
            let conn = get_conn(&guard)?;
            let response = nlq::handle_add_interaction_sync(conn, &user_id.0, draft)?;
            Ok(Json(response))
        }
        "find_path" => {
            let target_name = crate::llm::extract_path_target(&req.query).await;
            let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
            let conn = get_conn(&guard)?;
            let response = nlq::handle_find_path_sync(conn, &user_id.0, &target_name)?;
            Ok(Json(response))
        }
        _ => {
            // fallback to search
            let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
            let conn = get_conn(&guard)?;
            let results = nlq::natural_language_query(
                conn,
                &user_id.0,
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
    Extension(user_id): Extension<UserId>,
    Json(req): Json<NlqConfirmRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    log::info!(target: "nlq", "nlq_confirm_handler intent_type={}", req.intent_type);

    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;

    match req.intent_type.as_str() {
        "create_person" => {
            let create_req: CreatePersonRequest = serde_json::from_value(req.data)
                .map_err(|e| ApiError::bad_request(format!("Invalid data: {}", e)))?;
            let created = person::create(conn, &user_id.0, create_req)?;
            Ok(Json(serde_json::to_value(created).unwrap()))
        }
        "update_person" => {
            let id = req.data["id"]
                .as_str()
                .ok_or_else(|| ApiError::bad_request("Missing person id"))?
                .to_string();
            let update_req: CreatePersonRequest = serde_json::from_value(req.data)
                .map_err(|e| ApiError::bad_request(format!("Invalid data: {}", e)))?;
            let updated = person::update(conn, &user_id.0, &id, update_req)?;
            Ok(Json(serde_json::to_value(updated).unwrap()))
        }
        "add_interaction" => {
            let create_req: CreateInteractionRequest = serde_json::from_value(req.data)
                .map_err(|e| ApiError::bad_request(format!("Invalid data: {}", e)))?;
            let created = interaction::create(conn, &user_id.0, create_req)?;
            Ok(Json(serde_json::to_value(created).unwrap()))
        }
        _ => Err(ApiError::bad_request(format!(
            "Unknown intent_type: {}",
            req.intent_type
        ))),
    }
}
