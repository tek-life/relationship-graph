//! Admin 管理 API：用户列表、角色变更、邀请令牌管理。
//! 所有路由需要 admin 角色（由 require_admin 中间件保护）。

use crate::db::{get_conn, user as user_db};
use crate::state::SharedState;
use crate::types::{CreateInviteTokenRequest, InviteToken, UpdateRoleRequest, User};
use axum::extract::{Path, State};
use axum::http::HeaderMap;
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
    let users = user_db::list_users(conn)?;
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
