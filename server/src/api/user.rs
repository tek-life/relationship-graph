//! 用户认证 API：注册（邀请制）、登录、获取当前用户、更新个人画像。
//! 日志遵循脱敏原则：不记录密码、token 明文。

use crate::db::{get_conn, user as user_db};
use crate::security::auth;
use crate::state::SharedState;
use crate::types::{
    CreateUserRequest, LoginRequest, LoginResponse, RegisterRequest, RegisterResponse,
    UpdateProfileRequest, User,
};
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use chrono::Utc;

use super::ApiError;

/// 从请求头中提取 token，再从 TokenStore 获取关联的 user_id
fn extract_user_id(
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
            status: StatusCode::UNAUTHORIZED,
            message: "未登录或会话已过期".to_string(),
        })?;

    token_info.user_id.ok_or_else(|| ApiError {
        status: StatusCode::FORBIDDEN,
        message: "当前会话未关联用户，请通过 /api/auth/login 登录".to_string(),
    })
}

/// POST /api/auth/register — 公开路由（需邀请令牌）
pub async fn register(
    State(state): State<SharedState>,
    Json(req): Json<RegisterRequest>,
) -> Result<Json<RegisterResponse>, ApiError> {
    // 基本输入校验
    let username = req.username.trim();
    if username.is_empty() {
        return Err(ApiError::bad_request("用户名不能为空"));
    }
    if req.password.len() < 8 {
        return Err(ApiError::bad_request("密码至少需要 8 个字符"));
    }

    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;

    // 1. 验证邀请令牌：存在、未使用、未过期
    let invite = user_db::get_invite_token(conn, &req.invite_token)?
        .ok_or_else(|| ApiError::bad_request("无效的邀请令牌"))?;

    if invite.used_by.is_some() {
        return Err(ApiError::bad_request("邀请令牌已被使用"));
    }

    let expires = chrono::DateTime::parse_from_rfc3339(&invite.expires_at)
        .map_err(|e| ApiError::internal(format!("邀请令牌过期时间格式错误: {}", e)))?;
    if expires.with_timezone(&Utc) < Utc::now() {
        return Err(ApiError::bad_request("邀请令牌已过期"));
    }

    // 2. 检查用户名是否重复
    if user_db::get_user_by_username(conn, username)?.is_some() {
        return Err(ApiError::bad_request("用户名已被占用"));
    }

    // 3. 哈希密码并创建用户（默认角色 user）
    let password_hash =
        auth::hash_password(&req.password).map_err(ApiError::internal)?;

    let mut user = user_db::create_user(
        conn,
        CreateUserRequest {
            username: username.to_string(),
            password_hash,
            display_name: None,
            role: Some("user".to_string()),
            profile_doc: None,
        },
    )?;

    // 4. 标记邀请令牌为已使用
    user_db::mark_invite_used(conn, &invite.id, &user.id)?;

    // 5. 签发关联用户信息的 token（自动登录）
    let token = state
        .tokens
        .lock()
        .map_err(|e| ApiError::internal(e.to_string()))?
        .issue_with_user(Some(user.id.clone()), Some(user.role.clone()));

    log::info!(
        target: "auth",
        "register_success username={} role={}",
        user.username,
        user.role
    );

    // 响应中不携带密码哈希
    user.password_hash = String::new();
    Ok(Json(RegisterResponse { token, user }))
}

/// POST /api/auth/login — 公开路由
pub async fn login(
    State(state): State<SharedState>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, ApiError> {
    let username = req.username.trim();
    if username.is_empty() {
        return Err(ApiError::bad_request("用户名不能为空"));
    }

    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;

    // 1. 按用户名查找用户
    let mut user = user_db::get_user_by_username(conn, username)?.ok_or_else(|| ApiError {
        status: StatusCode::UNAUTHORIZED,
        message: "用户名或密码错误".to_string(),
    })?;

    // 2. 验证密码
    if !auth::verify_password(&req.password, &user.password_hash) {
        log::warn!(
            target: "auth",
            "login_failed username={} reason=invalid_password",
            username
        );
        return Err(ApiError {
            status: StatusCode::UNAUTHORIZED,
            message: "用户名或密码错误".to_string(),
        });
    }

    // 3. 签发关联用户信息的 token
    let token = state
        .tokens
        .lock()
        .map_err(|e| ApiError::internal(e.to_string()))?
        .issue_with_user(Some(user.id.clone()), Some(user.role.clone()));

    log::info!(
        target: "auth",
        "login_success username={} role={}",
        user.username,
        user.role
    );

    // 响应中不携带密码哈希
    user.password_hash = String::new();
    Ok(Json(LoginResponse { token, user }))
}

/// GET /api/auth/me — 受保护路由，返回当前登录用户信息
pub async fn get_current_user(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Result<Json<User>, ApiError> {
    let user_id = extract_user_id(&state, &headers)?;

    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;

    let mut user = user_db::get_user_by_id(conn, &user_id)?.ok_or_else(|| ApiError {
        status: StatusCode::NOT_FOUND,
        message: "用户不存在".to_string(),
    })?;

    // 响应中不携带密码哈希
    user.password_hash = String::new();
    Ok(Json(user))
}

/// PUT /api/users/profile — 受保护路由，更新个人画像
pub async fn update_profile(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<UpdateProfileRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let user_id = extract_user_id(&state, &headers)?;

    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;

    user_db::update_user_profile(conn, &user_id, &req.profile_doc)?;
    user_db::update_user_profile_completed(conn, &user_id)?;

    log::info!(target: "user", "update_profile_success user_id={}", user_id);

    Ok(Json(serde_json::json!({ "updated": true })))
}
