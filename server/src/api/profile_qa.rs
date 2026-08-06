//! Profile QA（个人画像构建）API：逐问逐答流程、画像生成、保存。
//! 三个核心端点：next（获取下一个引导问题）、generate（生成画像文档）、save（保存画像）。

use crate::db::{agent_config, get_conn, user as user_db};
use crate::llm;
use crate::state::SharedState;
use crate::types::{
    GenerateProfileRequest, NextQuestionRequest, QaModuleInfo, QaModulesResponse,
    SaveProfileRequest,
};
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;

use super::ApiError;

/// 从请求头中提取 token 并获取关联的 user_id
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

/// GET /api/profile-qa/modules — 获取 QA 模块列表（仅活跃模块，用于前端展示阶段名称）
pub async fn list_modules(
    State(state): State<SharedState>,
) -> Result<Json<QaModulesResponse>, ApiError> {
    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;

    let modules = agent_config::list_qa_modules(conn)?;
    let active_modules: Vec<QaModuleInfo> = modules
        .into_iter()
        .filter(|m| m.is_active)
        .map(|m| QaModuleInfo {
            id: m.id,
            name: m.name,
            description: m.description,
        })
        .collect();

    Ok(Json(QaModulesResponse {
        modules: active_modules,
    }))
}

/// POST /api/profile-qa/next — 生成下一个引导问题
pub async fn next_question(
    State(state): State<SharedState>,
    Json(req): Json<NextQuestionRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    // 从数据库加载模块配置，完成后立即释放锁
    let (system, module_name, module_id, total_active) = {
        let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
        let conn = get_conn(&guard)?;
        let modules = agent_config::list_qa_modules(conn)?;
        let active: Vec<_> = modules.into_iter().filter(|m| m.is_active).collect();

        if req.module_index >= active.len() {
            return Ok(Json(serde_json::json!({
                "question": "",
                "moduleName": "",
                "moduleIndex": req.module_index,
                "isModuleComplete": true,
                "isFlowComplete": true
            })));
        }

        let module = &active[req.module_index];

        let guidance = module.guidance_text.clone().unwrap_or_default();
        let system = if guidance.is_empty() {
            module.system_prompt.clone()
        } else {
            format!("{}\n\n引导要点：{}", module.system_prompt, guidance)
        };

        (system, module.name.clone(), module.id.clone(), active.len())
    }; // guard 在此处释放

    // 构建对话上下文：只取当前模块的历史问答
    let history_text: String = req
        .history
        .iter()
        .filter(|h| h.module_id == module_id)
        .map(|h| format!("教练: {}\n我: {}", h.question, h.answer))
        .collect::<Vec<_>>()
        .join("\n\n");

    let user_message = if history_text.is_empty() {
        "请开始第一步的引导。".to_string()
    } else {
        format!(
            "以下是已有的对话：\n{}\n\n请根据以上对话内容，继续引导下一步。如果当前阶段已经充分探讨，请在回复末尾添加 [阶段完成] 标记并准备进入下一阶段。",
            history_text
        )
    };

    // 调用 LLM 生成引导问题（不持有数据库锁）
    let question = llm::profile_qa_chat(&system, &user_message)
        .await
        .map_err(|e| ApiError::internal(format!("LLM 调用失败: {}", e)))?;

    // 检测模块是否完成（LLM 回复中包含特定标记）
    let is_complete = question.contains("[阶段完成]") || question.contains("[MODULE_COMPLETE]");

    log::info!(
        target: "profile_qa",
        "next_question module={} module_index={} history_len={} is_complete={}",
        module_id,
        req.module_index,
        req.history.len(),
        is_complete
    );

    Ok(Json(serde_json::json!({
        "question": question,
        "moduleName": module_name,
        "moduleIndex": req.module_index,
        "isModuleComplete": is_complete,
        "isFlowComplete": is_complete && req.module_index >= total_active - 1
    })))
}

/// POST /api/profile-qa/generate — 根据完整对话生成画像文档
pub async fn generate_profile(
    State(state): State<SharedState>,
    Json(req): Json<GenerateProfileRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    // 从数据库加载模块配置，完成后立即释放锁
    let system_prompt = {
        let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
        let conn = get_conn(&guard)?;
        let modules = agent_config::list_qa_modules(conn)?;

        modules
            .iter()
            .find(|m| m.id == "profile_generate")
            .or_else(|| modules.last())
            .map(|m| m.system_prompt.clone())
            .unwrap_or_else(|| "根据对话生成个人画像文档".to_string())
    }; // guard 在此处释放

    // 构建完整对话历史文本
    let conversation: String = req
        .history
        .iter()
        .map(|h| {
            format!(
                "[{}] 教练: {}\n[{}] 我: {}",
                h.module_id, h.question, h.module_id, h.answer
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    log::info!(
        target: "profile_qa",
        "generate_profile history_count={} conversation_len={}",
        req.history.len(),
        conversation.len()
    );

    // 调用 LLM 生成画像文档（不持有数据库锁）
    let profile_doc = llm::generate_profile_document(&system_prompt, &conversation)
        .await
        .map_err(|e| ApiError::internal(format!("LLM 调用失败: {}", e)))?;

    Ok(Json(serde_json::json!({
        "profileDoc": profile_doc
    })))
}

/// POST /api/profile-qa/save — 保存画像文档到当前用户记录
pub async fn save_profile(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<SaveProfileRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let user_id = extract_user_id(&state, &headers)?;

    let guard = state.db.lock().map_err(|e| ApiError::internal(e.to_string()))?;
    let conn = get_conn(&guard)?;

    user_db::update_user_profile(conn, &user_id, &req.profile_doc)?;
    user_db::update_user_profile_completed(conn, &user_id)?;

    log::info!(
        target: "profile_qa",
        "save_profile_success user_id={} doc_len={}",
        user_id,
        req.profile_doc.len()
    );

    Ok(Json(serde_json::json!({ "saved": true })))
}
