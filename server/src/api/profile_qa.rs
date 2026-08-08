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

/// 画像对话历史字符预算默认值（P1-5）：generate_profile 把全部 QA 历史拼入
/// prompt，需防超窗；默认 12000 字符（中文约 12-15k token）+ system + 输出
/// 可被 RG_OLLAMA_NUM_CTX 默认 16384 的窗口容纳。RG_PROFILE_HISTORY_CHARS 可覆盖。
const DEFAULT_PROFILE_HISTORY_CHARS: usize = 12000;

/// 解析画像历史预算（纯函数，可单测）：输入为 Some 且 trim 后为正整数时取其值，
/// 否则（未设置/空串/非数字/0/溢出）回退 default。
fn parse_profile_budget(env_value: Option<&str>, default: usize) -> usize {
    env_value
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(default)
}

/// 读取画像历史字符预算（RG_PROFILE_HISTORY_CHARS，默认 12000）
fn profile_history_budget() -> usize {
    parse_profile_budget(
        std::env::var("RG_PROFILE_HISTORY_CHARS").ok().as_deref(),
        DEFAULT_PROFILE_HISTORY_CHARS,
    )
}

/// 画像对话历史预算截断（纯函数，可单测）：entries 按时间正序（尾部为最新），
/// 从新到旧按字符预算累加保留最近轮次（与 resolve_chat_history 的从新到旧
/// 累加惯例一致）；最近一条即使单独超预算也保留；被丢弃的早期轮次在头部
/// 附一行截断说明。未超预算时逐字节原文拼接，空历史返回空串。
fn truncate_profile_history(entries: &[String], budget_chars: usize) -> String {
    if entries.is_empty() {
        return String::new();
    }
    // 轮次间分隔符 "\n\n" 占 2 字符
    let mut kept: Vec<&str> = Vec::new();
    let mut total = 0usize;
    for entry in entries.iter().rev() {
        let added = entry.chars().count() + if kept.is_empty() { 0 } else { 2 };
        if total + added > budget_chars && !kept.is_empty() {
            break;
        }
        total += added;
        kept.push(entry.as_str());
    }
    kept.reverse();
    let body = kept.join("\n\n");
    if kept.len() == entries.len() {
        body
    } else {
        format!(
            "[注：对话记录超长，已按字符预算 {} 截断早期内容，仅保留最近轮次]\n\n{}",
            budget_chars, body
        )
    }
}

#[cfg(test)]
mod profile_history_tests {
    use super::*;

    #[test]
    fn parse_profile_budget_valid_and_invalid() {
        // 合法值（含首尾空白 trim）
        assert_eq!(parse_profile_budget(Some("20000"), 100), 20000);
        assert_eq!(parse_profile_budget(Some(" 300 "), 100), 300);
        // 未设置 / 空串 / 空白 / 非数字 / 0 / 溢出 → 回退默认
        assert_eq!(parse_profile_budget(None, 100), 100);
        assert_eq!(parse_profile_budget(Some(""), 100), 100);
        assert_eq!(parse_profile_budget(Some("   "), 100), 100);
        assert_eq!(parse_profile_budget(Some("abc"), 100), 100);
        assert_eq!(parse_profile_budget(Some("0"), 100), 100);
        assert_eq!(parse_profile_budget(Some("-3"), 100), 100);
        assert_eq!(parse_profile_budget(Some("999999999999999999999999"), 100), 100);
    }

    #[test]
    fn profile_budget_default_baseline() {
        if std::env::var("RG_PROFILE_HISTORY_CHARS").is_ok() {
            return;
        }
        assert_eq!(profile_history_budget(), DEFAULT_PROFILE_HISTORY_CHARS);
        assert_eq!(DEFAULT_PROFILE_HISTORY_CHARS, 12000);
    }

    #[test]
    fn truncate_empty_history() {
        assert_eq!(truncate_profile_history(&[], 100), "");
    }

    #[test]
    fn truncate_within_budget_byte_for_byte() {
        let entries = vec!["第一轮".to_string(), "第二轮".to_string()];
        // 未超预算 → 逐字节原文拼接，无截断说明
        assert_eq!(truncate_profile_history(&entries, 100), "第一轮\n\n第二轮");
    }

    #[test]
    fn truncate_exact_budget_boundary() {
        // "ab\n\ncd" 恰 6 字符，预算 6 → 全保留无截断；预算 5 → 仅保最新
        let entries = vec!["ab".to_string(), "cd".to_string()];
        assert_eq!(truncate_profile_history(&entries, 6), "ab\n\ncd");
        let truncated = truncate_profile_history(&entries, 5);
        assert!(truncated.starts_with("[注：对话记录超长"));
        assert!(truncated.ends_with("cd"));
        assert!(!truncated.contains("ab"));
    }

    #[test]
    fn truncate_over_budget_keeps_newest_order() {
        let entries: Vec<String> = (1..=10).map(|i| format!("轮次{}内容", i)).collect();
        // 轮次1-9 各 5 字符、轮次10 为 6 字符，分隔符各占 2：
        // 预算 13 → 轮次10(6) + 轮次9(5+2)=13 恰满；再加轮次8 到 20 超限
        let out = truncate_profile_history(&entries, 13);
        assert!(out.starts_with("[注：对话记录超长，已按字符预算 13 截断早期内容，仅保留最近轮次]\n\n"));
        assert!(out.contains("轮次9内容\n\n轮次10内容"));
        assert!(!out.contains("轮次8"));
        // 保留部分仍按时间正序
        let pos9 = out.find("轮次9").unwrap();
        let pos10 = out.find("轮次10").unwrap();
        assert!(pos9 < pos10);
    }

    #[test]
    fn truncate_single_entry_over_budget_kept() {
        // 最近一条即使单独超预算也保留（与 resolve_chat_history 惯例一致）
        let entries = vec!["x".repeat(500)];
        assert_eq!(truncate_profile_history(&entries, 10), "x".repeat(500));
    }
}

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

    // 构建完整对话历史文本，并按字符预算截断（P1-5：防全部历史拼入超窗，
    // 从新到旧保留最近轮次，截断方式与 resolve_chat_history 惯例一致）
    let entries: Vec<String> = req
        .history
        .iter()
        .map(|h| {
            format!(
                "[{}] 教练: {}\n[{}] 我: {}",
                h.module_id, h.question, h.module_id, h.answer
            )
        })
        .collect();
    let total_entries = entries.len();
    let budget = profile_history_budget();
    let conversation = truncate_profile_history(&entries, budget);

    log::info!(
        target: "profile_qa",
        "generate_profile history_count={} conversation_len={} budget={} truncated={}",
        total_entries,
        conversation.chars().count(),
        budget,
        conversation.starts_with("[注：对话记录超长")
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
