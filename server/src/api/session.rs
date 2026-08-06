//! 会话管理 API：会话 CRUD 与消息追加，支持自动命名。
//! 日志遵循脱敏原则：不记录消息原文。

use crate::db::{get_conn, session as db_session};
use crate::llm;
use crate::state::SharedState;
use crate::types::{
    ChatMessage, CreateChatMessageRequest, CreateSessionBody, CreateSessionRequest,
    MessageQuery, Session, UpdateSessionRequest,
};
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;

use super::ApiError;

/// 从请求头中的 token 提取关联的 user_id；
/// setup/unlock 签发的 token 无 user_id，回退到首个用户（通常是 admin）。
/// 不能回退到不存在的占位 ID：sessions.user_id 有外键约束，会导致插入失败。
fn extract_user_id(state: &SharedState, headers: &HeaderMap) -> Result<String, ApiError> {
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

    if let Some(user_id) = token_info.user_id {
        return Ok(user_id);
    }

    // unlock token 兜底：解析数据库中真实存在的首个用户，保证外键约束成立
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    let users = crate::db::user::list_users(conn).map_err(|e| ApiError::internal(e.to_string()))?;
    users
        .first()
        .map(|u| u.id.clone())
        .ok_or_else(|| ApiError::internal("系统中尚无用户，请先完成注册"))
}

/// GET /api/sessions — 当前用户的会话列表（按 updated_at 降序）
pub async fn list_sessions(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Result<Json<Vec<Session>>, ApiError> {
    let user_id = extract_user_id(&state, &headers)?;
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    let sessions = db_session::list_sessions_by_user(conn, &user_id)?;
    log::info!(target: "session", "list_sessions_success user_id={} count={}", user_id, sessions.len());
    Ok(Json(sessions))
}

/// POST /api/sessions — 创建新会话
pub async fn create_session(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<CreateSessionBody>,
) -> Result<Json<Session>, ApiError> {
    let user_id = extract_user_id(&state, &headers)?;
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    let session = db_session::create_session(
        conn,
        CreateSessionRequest {
            user_id: user_id.clone(),
            title: req.title,
        },
    )?;
    log::info!(target: "session", "create_session_success user_id={} session_id={}", user_id, session.id);
    Ok(Json(session))
}

/// GET /api/sessions/:id/messages — 获取会话消息（支持分页）
pub async fn list_messages(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    Query(params): Query<MessageQuery>,
) -> Result<Json<Vec<ChatMessage>>, ApiError> {
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    let limit = params.limit.unwrap_or(100);
    let offset = params.offset.unwrap_or(0);
    let messages = db_session::list_messages_by_session(conn, &id, limit, offset)?;
    log::info!(target: "session", "list_messages_success session_id={} count={}", id, messages.len());
    Ok(Json(messages))
}

/// POST /api/sessions/:id/messages — 追加消息
/// 自动命名：如果是第一条用户消息且会话尚无标题，以前 20 字为默认标题。
/// 上下文压缩：消息数超过阈值时自动调用 LLM 压缩旧消息为摘要。
const COMPRESSION_THRESHOLD: usize = 50;
const KEEP_RECENT: usize = 10;
const MAX_SUMMARY_TOKENS: usize = 1000;

pub async fn add_message(
    State(state): State<SharedState>,
    Path(session_id): Path<String>,
    Json(req): Json<CreateChatMessageRequest>,
) -> Result<Json<ChatMessage>, ApiError> {
    // 第一阶段：创建消息、自动命名、检查是否需要压缩
    let old_messages = {
        let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
        let conn = get_conn(&guard)?;

        let message = db_session::create_message(
            conn,
            CreateChatMessageRequest {
                session_id: session_id.clone(),
                role: req.role.clone(),
                content: req.content.clone(),
                metadata_json: req.metadata_json.clone(),
            },
        )?;

        // 自动命名：第一条用户消息且 session 没有标题时，取前 20 字
        if req.role == "user" {
            let count = db_session::count_messages_by_session(conn, &session_id)?;
            if count <= 1 {
                let title: String = req.content.chars().take(20).collect();
                db_session::update_session_title(conn, &session_id, &title)?;
                log::info!(target: "session", "auto_title_set session_id={} title_len={}", session_id, title.chars().count());
            }
        }

        log::info!(target: "session", "add_message_success session_id={} role={}", session_id, req.role);

        // 检查是否需要压缩
        let count = db_session::count_messages_by_session(conn, &session_id)?;
        if count > COMPRESSION_THRESHOLD {
            let fetch_limit = (count - KEEP_RECENT) as i64;
            let old = db_session::list_messages_by_session(conn, &session_id, fetch_limit, 0)?;
            if old.is_empty() {
                None
            } else {
                Some((message, old))
            }
        } else {
            return Ok(Json(message));
        }
    }; // guard 在此处释放，避免跨 .await 持有锁

    // 第二阶段：异步调用 LLM 压缩（不持有 DB 锁）
    if let Some((message, old_msgs)) = old_messages {
        match llm::compress_context(&old_msgs, MAX_SUMMARY_TOKENS).await {
            Ok(summary) => {
                // 第三阶段：重新获取锁，删除旧消息并插入摘要
                let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
                let conn = get_conn(&guard)?;

                if let Err(e) = db_session::delete_old_messages(conn, &session_id, KEEP_RECENT as i64) {
                    log::warn!(target: "session", "delete_old_messages_failed session={} error={}", session_id, e);
                } else {
                    let summary_req = CreateChatMessageRequest {
                        session_id: session_id.clone(),
                        role: "system".to_string(),
                        content: format!("[对话摘要] {}", summary),
                        metadata_json: Some(r#"{"type":"context_summary"}"#.to_string()),
                    };
                    if let Err(e) = db_session::create_message(conn, summary_req) {
                        log::warn!(target: "session", "insert_summary_failed session={} error={}", session_id, e);
                    } else {
                        log::info!(target: "session", "context_compressed session={} old_messages={}", session_id, old_msgs.len());
                    }
                }
            }
            Err(e) => {
                log::warn!(target: "session", "context_compression_failed session={} error={}", session_id, e);
            }
        }
        return Ok(Json(message));
    }

    unreachable!()
}

/// PUT /api/sessions/:id — 更新会话标题
pub async fn update_session(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    Json(req): Json<UpdateSessionRequest>,
) -> Result<Json<Session>, ApiError> {
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    db_session::update_session_title(conn, &id, &req.title)?;
    let session = db_session::get_session(conn, &id)?
        .ok_or_else(|| ApiError::bad_request("会话不存在"))?;
    log::info!(target: "session", "update_session_success session_id={}", id);
    Ok(Json(session))
}

/// DELETE /api/sessions/:id — 删除会话及其消息
pub async fn delete_session(
    State(state): State<SharedState>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;
    db_session::delete_session(conn, &id)?;
    log::info!(target: "session", "delete_session_success session_id={}", id);
    Ok(StatusCode::NO_CONTENT)
}
