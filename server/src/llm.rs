use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use log::{info, warn};

use rig_core::client::{CompletionClient, Nothing};
use rig_core::completion::{AssistantContent, CompletionModel as _};
use rig_core::providers::{ollama, openai};
use rig_core::streaming::StreamedAssistantContent;

use crate::types::{PersonDraft, FieldChange, InteractionDraft, ChatMessage};

const DEFAULT_OLLAMA_TIMEOUT_SECS: u64 = 45;
const DEFAULT_OLLAMA_CHAT_TIMEOUT_SECS: u64 = 120;

fn ollama_url() -> String {
    std::env::var("RG_OLLAMA_URL").unwrap_or_else(|_| "http://localhost:11434".to_string())
}

fn ollama_model() -> String {
    std::env::var("RG_OLLAMA_MODEL").unwrap_or_else(|_| "qwen2.5:7b".to_string())
}

fn ollama_timeout(env_key: &str, default_secs: u64) -> Duration {
    std::env::var(env_key)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|secs| *secs > 0)
        .map(Duration::from_secs)
        .unwrap_or_else(|| Duration::from_secs(default_secs))
}

#[derive(Serialize)]
struct OllamaRequest {
    model: String,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    format: Option<String>,
    prompt: String,
}

#[derive(Deserialize)]
struct OllamaResponse {
    response: Option<String>,
}

async fn call_ollama(prompt: &str, format: Option<&str>, timeout: Duration, fn_name: &str, web_search: bool) -> Result<String, String> {
    // 三值通道分发：RG_LLM_BACKEND=rig/cloud 时全量走对应通道；legacy 模式下
    // 命中 RG_LLM_CLOUD_FNS / RG_LLM_RIG_FNS 白名单的函数分别走 cloud / rig
    //（函数级灰度，cloud 优先），其余走原 reqwest legacy 实现（默认行为与
    // 改造前完全一致）。详见 llm_channel_for 注释。
    // web_search 仅 cloud 通道生效（百炼 enable_search），legacy/rig 忽略。
    match llm_channel(fn_name) {
        Channel::Rig => return call_rig(prompt, format, timeout).await,
        // Cloud 尊重调用方传入的 timeout（聊天类 120s、抽取类 45s），
        // 避免长调用（generate_profile_document / compress_context）被 60s 截断
        Channel::Cloud => return call_cloud(prompt, format, timeout, fn_name, web_search).await,
        Channel::Legacy => {}
    }

    let client = reqwest::Client::builder()
        .timeout(timeout)
        .build()
        .map_err(|e| format!("HTTP client error: {}", e))?;

    let req = OllamaRequest {
        model: ollama_model(),
        stream: false,
        format: format.map(str::to_string),
        prompt: prompt.to_string(),
    };

    let url = format!("{}/api/generate", ollama_url());
    info!(target: "llm", "ollama_request url={} model={} prompt_len={}", url, req.model, prompt.len());

    let resp = client
        .post(&url)
        .json(&req)
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                format!(
                    "Ollama 请求超时：模型 {} 在 {} 秒内未返回结果。你可以稍后重试，或提高环境变量 RG_OLLAMA_CHAT_TIMEOUT_SECS / RG_OLLAMA_TIMEOUT_SECS。",
                    req.model,
                    timeout.as_secs()
                )
            } else {
                format!("Ollama request failed: {}", e)
            }
        })?;

    if !resp.status().is_success() {
        return Err(format!("Ollama returned status {}", resp.status()));
    }

    let data: OllamaResponse = resp.json().await.map_err(|e| format!("Parse error: {}", e))?;
    data.response.ok_or_else(|| "Empty response from Ollama".to_string())
}

// ---------- 通道分发（legacy | rig | cloud 三值） ----------

const DEFAULT_CLOUD_TIMEOUT_SECS: u64 = 120;

/// 云端（阿里云百炼）兼容端点
fn cloud_base_url() -> String {
    std::env::var("RG_CLOUD_BASE_URL")
        .unwrap_or_else(|_| "https://dashscope.aliyuncs.com/compatible-mode/v1".to_string())
}

/// 云端聊天主力模型（开思考）
fn cloud_chat_model() -> String {
    std::env::var("RG_CLOUD_CHAT_MODEL").unwrap_or_else(|_| "qwen3.7-plus".to_string())
}

/// 云端抽取首选模型（无思考开销，json_object 正常）
fn cloud_extract_model() -> String {
    std::env::var("RG_CLOUD_EXTRACT_MODEL").unwrap_or_else(|_| "qwen-flash".to_string())
}

/// 云端默认超时（RG_CLOUD_TIMEOUT_SECS 可覆盖）：仅作为 cloud 流式
/// （cloud_chat_stream）建连超时的默认值，默认值对齐聊天超时语义
///（与 DEFAULT_OLLAMA_CHAT_TIMEOUT_SECS 同档）；非流式 call_cloud 由
/// call_ollama 分发时尊重调用方传入的 timeout，不受此 env 覆盖。
fn cloud_timeout() -> Duration {
    ollama_timeout("RG_CLOUD_TIMEOUT_SECS", DEFAULT_CLOUD_TIMEOUT_SECS)
}

/// 云端 API Key 默认文件路径：~/.config/rg-cloud-api-key
fn cloud_api_key_file() -> std::path::PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from(".config"))
        .join("rg-cloud-api-key")
}

/// 解析云端 API Key：优先 env RG_CLOUD_API_KEY，缺省读文件（均 trim）；
/// 两者皆无/皆空时报错。Key 不落日志与代码。
fn cloud_api_key() -> Result<String, String> {
    cloud_api_key_from(std::env::var("RG_CLOUD_API_KEY").ok(), &cloud_api_key_file())
}

/// Key 解析纯函数版本（便于单测）
fn cloud_api_key_from(env_value: Option<String>, file_path: &std::path::Path) -> Result<String, String> {
    const MISSING_HINT: &str = "未配置云端 API Key：请设置环境变量 RG_CLOUD_API_KEY 或文件 ~/.config/rg-cloud-api-key";
    if let Some(value) = env_value {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return Ok(trimmed.to_string());
        }
    }
    match std::fs::read_to_string(file_path) {
        Ok(content) => {
            // 剥离 UTF-8 BOM（U+FEFF，Windows 记事本保存的 Key 文件会携带），
            // 否则 BOM 混入 Key 导致云端 401 且报错不直观
            let trimmed = content.trim().trim_start_matches('\u{FEFF}').trim();
            if trimmed.is_empty() {
                Err(MISSING_HINT.to_string())
            } else {
                Ok(trimmed.to_string())
            }
        }
        Err(_) => Err(MISSING_HINT.to_string()),
    }
}

/// 云端模型类别：聊天类函数用聊天模型，其余（extract_* / compress_context）用抽取模型
fn is_cloud_chat_fn(fn_name: &str) -> bool {
    matches!(fn_name, "general_chat" | "profile_qa_chat" | "generate_profile_document")
}

fn cloud_model_for(fn_name: &str) -> String {
    if is_cloud_chat_fn(fn_name) {
        cloud_chat_model()
    } else {
        cloud_extract_model()
    }
}

/// LLM 通道（三值）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Channel {
    Legacy,
    Rig,
    Cloud,
}

/// 读取通道开关：RG_LLM_BACKEND = "legacy" | "rig" | "cloud"，默认 legacy。
/// trim 处理与白名单解析保持对称，避免环境变量含首尾空白时静默回退 legacy。
fn llm_backend() -> String {
    std::env::var("RG_LLM_BACKEND")
        .unwrap_or_else(|_| "legacy".to_string())
        .trim()
        .to_lowercase()
}

/// 读取函数白名单（逗号分隔），仅在 RG_LLM_BACKEND=legacy 时生效。
/// 未设置返回 None；设置了但为空（或全为空白）视为无函数启用灰度。
fn env_fns_whitelist(env_key: &str) -> Option<Vec<String>> {
    std::env::var(env_key).ok().map(|value| {
        value
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    })
}

fn rig_fns_whitelist() -> Option<Vec<String>> {
    env_fns_whitelist("RG_LLM_RIG_FNS")
}

fn cloud_fns_whitelist() -> Option<Vec<String>> {
    env_fns_whitelist("RG_LLM_CLOUD_FNS")
}

/// 三值通道决策（纯函数，可单测）。语义：
/// - backend=rig / cloud → 全量走对应通道（两个白名单均忽略）；
/// - legacy（默认）：命中 RG_LLM_CLOUD_FNS → Cloud；否则命中
///   RG_LLM_RIG_FNS → Rig；否则 Legacy（函数级灰度发布）。
/// 同一函数同时命中两个白名单时 Cloud 优先。
/// 与原双轨开关（should_use_rig）语义等价：rig 白名单未设置/为空时
/// legacy 模式下无函数走 rig，cloud 白名单同理。
fn llm_channel_for(
    backend: &str,
    rig_whitelist: Option<&[String]>,
    cloud_whitelist: Option<&[String]>,
    fn_name: &str,
) -> Channel {
    match backend {
        "rig" => return Channel::Rig,
        "cloud" => return Channel::Cloud,
        _ => {}
    }
    if let Some(list) = cloud_whitelist {
        if list.iter().any(|f| f == fn_name) {
            return Channel::Cloud;
        }
    }
    if let Some(list) = rig_whitelist {
        if list.iter().any(|f| f == fn_name) {
            return Channel::Rig;
        }
    }
    Channel::Legacy
}

fn llm_channel(fn_name: &str) -> Channel {
    llm_channel_for(
        &llm_backend(),
        rig_fns_whitelist().as_deref(),
        cloud_fns_whitelist().as_deref(),
        fn_name,
    )
}

#[cfg(test)]
mod channel_tests {
    use super::*;

    /// 与原双轨开关 6 断言等价的 rig 语义（以 Channel 表达）
    #[test]
    fn legacy_rig_semantics_preserved() {
        // backend=rig → 全量走 rig，即使白名单不含该函数
        assert_eq!(llm_channel_for("rig", None, None, "general_chat"), Channel::Rig);
        assert_eq!(
            llm_channel_for("rig", Some(&["other_fn".to_string()]), None, "general_chat"),
            Channel::Rig
        );
        // legacy + rig 白名单命中 → 函数级灰度走 rig
        assert_eq!(
            llm_channel_for("legacy", Some(&["general_chat".to_string()]), None, "general_chat"),
            Channel::Rig
        );
        // legacy + 白名单未命中 → legacy
        assert_eq!(
            llm_channel_for("legacy", Some(&["other_fn".to_string()]), None, "general_chat"),
            Channel::Legacy
        );
        // legacy + 未设置/空白名单 → legacy
        assert_eq!(llm_channel_for("legacy", None, None, "general_chat"), Channel::Legacy);
        assert_eq!(llm_channel_for("legacy", Some(&[]), None, "general_chat"), Channel::Legacy);
    }

    #[test]
    fn cloud_channel_semantics() {
        // backend=cloud → 全量走 cloud，两个白名单均忽略
        assert_eq!(llm_channel_for("cloud", None, None, "general_chat"), Channel::Cloud);
        assert_eq!(
            llm_channel_for(
                "cloud",
                Some(&["general_chat".to_string()]),
                Some(&["other".to_string()]),
                "extract_person_fields"
            ),
            Channel::Cloud
        );
        // legacy + cloud 白名单命中 → cloud
        assert_eq!(
            llm_channel_for("legacy", None, Some(&["general_chat".to_string()]), "general_chat"),
            Channel::Cloud
        );
        // legacy + cloud 未命中、rig 命中 → rig
        assert_eq!(
            llm_channel_for(
                "legacy",
                Some(&["general_chat".to_string()]),
                Some(&["other".to_string()]),
                "general_chat"
            ),
            Channel::Rig
        );
        // 两个白名单同时命中 → cloud 优先
        assert_eq!(
            llm_channel_for(
                "legacy",
                Some(&["general_chat".to_string()]),
                Some(&["general_chat".to_string()]),
                "general_chat"
            ),
            Channel::Cloud
        );
        // 空 cloud 白名单 → legacy
        assert_eq!(
            llm_channel_for("legacy", None, Some(&[]), "general_chat"),
            Channel::Legacy
        );
    }

    #[test]
    fn cloud_api_key_resolution() {
        let missing = std::env::temp_dir().join("rg-test-nonexistent-key-file");
        let _ = std::fs::remove_file(&missing);
        // env 与文件均缺失 → 报错
        let err = cloud_api_key_from(None, &missing).unwrap_err();
        assert!(err.contains("未配置云端 API Key"));
        // env 优先且 trim
        assert_eq!(cloud_api_key_from(Some("  sk-abc  ".to_string()), &missing).unwrap(), "sk-abc");
        // env 为空白 → 回退文件（trim）
        let file = std::env::temp_dir().join("rg-test-key-file");
        std::fs::write(&file, " sk-file \n").unwrap();
        assert_eq!(cloud_api_key_from(Some("   ".to_string()), &file).unwrap(), "sk-file");
        assert_eq!(cloud_api_key_from(None, &file).unwrap(), "sk-file");
        // 文件内容为空白 → 报错
        std::fs::write(&file, "  \n").unwrap();
        assert!(cloud_api_key_from(None, &file).is_err());
        // Windows 记事本保存的 Key 文件携带 UTF-8 BOM（U+FEFF）→ 剥离后正常解析
        std::fs::write(&file, "\u{FEFF}sk-bom\n").unwrap();
        assert_eq!(cloud_api_key_from(None, &file).unwrap(), "sk-bom");
        // BOM + 空白包裹同样剥离
        std::fs::write(&file, "\u{FEFF}  sk-bom2  \n").unwrap();
        assert_eq!(cloud_api_key_from(None, &file).unwrap(), "sk-bom2");
        let _ = std::fs::remove_file(&file);
    }

    #[test]
    fn cloud_model_category_selection() {
        // 聊天类函数
        assert!(is_cloud_chat_fn("general_chat"));
        assert!(is_cloud_chat_fn("profile_qa_chat"));
        assert!(is_cloud_chat_fn("generate_profile_document"));
        // 抽取/压缩类函数
        assert!(!is_cloud_chat_fn("extract_person_fields"));
        assert!(!is_cloud_chat_fn("extract_update_fields"));
        assert!(!is_cloud_chat_fn("extract_interaction_data"));
        assert!(!is_cloud_chat_fn("extract_path_target"));
        assert!(!is_cloud_chat_fn("compress_context"));
        // 默认模型名（env 未设置时）
        assert_eq!(cloud_model_for("general_chat"), "qwen3.7-plus");
        assert_eq!(cloud_model_for("compress_context"), "qwen-flash");
    }
}

#[cfg(test)]
mod general_chat_prompt_tests {
    use super::*;

    /// 旧格式基准（技能注入改造前的 prompt 结构）：空技能时必须逐字节一致
    fn legacy_prompt(query: &str) -> String {
        format!(
            "你是您的个人 AI 平台的通用助理。请直接回答用户问题，默认使用简体中文。\
当用户问题涉及实时互联网信息时，明确说明你无法联网，并给出可行替代方案。\
\n\n用户问题：{}",
            query
        )
    }

    #[test]
    fn empty_skills_prompt_matches_legacy_format_byte_for_byte() {
        let query = "帮我总结一下最近的关系维护情况";
        assert_eq!(general_chat_prompt(query, "", false, ""), legacy_prompt(query));
        // 空 query 同样一致
        assert_eq!(general_chat_prompt("", "", false, ""), legacy_prompt(""));
    }

    #[test]
    fn non_empty_skills_insert_section_between_role_and_question() {
        let skills = "### 技能：演示\n技能正文\n\n";
        let prompt = general_chat_prompt("你好", skills, false, "");
        // 角色设定在前、技能段居中、用户问题殿后
        assert!(prompt.starts_with("你是您的个人 AI 平台的通用助理。"));
        assert!(prompt.contains("\n\n你当前具备以下技能，请在适用时遵循：\n### 技能：演示\n技能正文\n\n用户问题：你好"));
        assert!(prompt.ends_with("用户问题：你好"));
        // 技能尾部空白被归一，与“用户问题：”之间恰有一个空行
        assert!(!prompt.contains("\n\n\n\n用户问题"));
    }

    /// 画像常驻技能：空/空白画像经 build_profile_skill_prompt 产出空串，
    /// 注入后 prompt 与无画像现状逐字节一致（legacy_prompt 基准同源锁定）；
    /// 画像段作为技能段注入时位置与数字人技能一致（角色设定在前、问题殿后）
    #[test]
    fn profile_skill_section_injection_format() {
        let query = "帮我分析一下关系维护情况";
        // 空画像 → 注入串为空 → 与现状逐字节一致
        let empty_skills = crate::db::agent_config::build_profile_skill_prompt("   ");
        assert_eq!(empty_skills, "");
        assert_eq!(general_chat_prompt(query, &empty_skills, false, ""), legacy_prompt(query));

        // 画像段注入：与 build_profile_skill_prompt 产出格式对齐
        let skills = crate::db::agent_config::build_profile_skill_prompt("画像正文");
        assert_eq!(skills, "### 技能：用户画像\n画像正文\n\n");
        let prompt = general_chat_prompt(query, &skills, false, "");
        assert!(prompt.starts_with("你是您的个人 AI 平台的通用助理。"));
        assert!(prompt.contains("\n\n你当前具备以下技能，请在适用时遵循：\n### 技能：用户画像\n画像正文\n\n用户问题："));
        assert!(prompt.ends_with(&format!("用户问题：{}", query)));
    }

    /// 联网搜索开启态：“无法联网”句被替换为联网说明，其余结构与关闭态一致
    #[test]
    fn web_search_enabled_replaces_offline_sentence() {
        let prompt = general_chat_prompt("今天有什么新闻", "", true, "");
        assert!(prompt.starts_with(
            "你是您的个人 AI 平台的通用助理。请直接回答用户问题，默认使用简体中文。本次对话已启用联网搜索，可参考最新网络信息作答。"
        ));
        assert!(!prompt.contains("无法联网"));
        assert!(prompt.ends_with("用户问题：今天有什么新闻"));
    }

    /// 文档段注入位置：角色设定 → 技能 → 文档段 → 用户问题；
    /// 文档为空时与无文档现状逐字节一致
    #[test]
    fn documents_section_between_skills_and_question() {
        let query = "总结这份报告";
        let skills = "### 技能：演示\n技能正文\n\n";
        let docs = "### 用户上传文档《报告.pdf》正文\n抽取文本";
        // 空文档 → 与无文档现状（旧格式）逐字节一致
        let legacy = format!(
            "你是您的个人 AI 平台的通用助理。请直接回答用户问题，默认使用简体中文。当用户问题涉及实时互联网信息时，明确说明你无法联网，并给出可行替代方案。\n\n你当前具备以下技能，请在适用时遵循：\n{}\n\n用户问题：{}",
            skills.trim_end(),
            query
        );
        assert_eq!(general_chat_prompt(query, skills, false, ""), legacy);
        let prompt = general_chat_prompt(query, skills, false, docs);
        assert!(prompt.contains("\n\n你当前具备以下技能，请在适用时遵循：\n### 技能：演示\n技能正文\n\n### 用户上传文档《报告.pdf》正文\n抽取文本\n\n用户问题：总结这份报告"));
        // 无技能仅有文档：文档段紧跟角色设定
        let prompt_no_skills = general_chat_prompt(query, "", false, docs);
        assert!(prompt_no_skills.contains("替代方案。\n\n### 用户上传文档《报告.pdf》正文\n抽取文本\n\n用户问题：总结这份报告"));
    }
}

/// 缓存 rig 的 Ollama client（顺带修复每次新建连接问题）。
/// 构建失败不缓存，下次调用可重试（client Clone 廉价，构建也廉价）。
/// 不注入自定义 http_client，超时完全由 tokio::time::timeout 控制。
fn rig_client() -> Result<ollama::Client, String> {
    static CLIENT: Mutex<Option<ollama::Client>> = Mutex::new(None);
    let mut guard = CLIENT
        .lock()
        .map_err(|e| format!("Rig client lock failed: {}", e))?;
    if let Some(client) = guard.as_ref() {
        return Ok(client.clone());
    }
    let client = ollama::Client::builder()
        .api_key(Nothing)
        .base_url(ollama_url())
        .build()
        .map_err(|e| format!("Rig client build failed: {}", e))?;
    *guard = Some(client.clone());
    Ok(client)
}

/// 通过 rig-core 调用 Ollama /api/chat，与 legacy 的 call_ollama 平行。
/// 超时与错误文案与 legacy 通道保持一致。
/// format 为 Some 时设置宽松 JSON Schema 约束（Ollama provider 会将
/// output_schema 映射为请求体的 format 字段），等价于 legacy 的 format:"json"。
async fn call_rig(prompt: &str, format: Option<&str>, timeout: Duration) -> Result<String, String> {
    let client = rig_client()?;
    let model_name = ollama_model();
    info!(
        target: "llm",
        "rig_request url={} model={} prompt_len={} format={:?}",
        ollama_url(),
        model_name,
        prompt.len(),
        format
    );

    let model = client.completion_model(&model_name);
    let request_builder = model.completion_request(prompt);
    let request = if format.is_some() {
        let schema: schemars::Schema = serde_json::from_value(serde_json::json!({"type": "object"}))
            .map_err(|e| format!("Schema build error: {}", e))?;
        request_builder.output_schema(schema).build()
    } else {
        request_builder.build()
    };

    match tokio::time::timeout(timeout, model.completion(request)).await {
        Ok(Ok(response)) => {
            // 取 choices 中的 Text 内容拼接
            let text: String = response
                .choice
                .iter()
                .filter_map(|item| match item {
                    AssistantContent::Text(t) => Some(t.text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("");
            if text.is_empty() {
                Err("Empty response from Ollama".to_string())
            } else {
                Ok(text)
            }
        }
        Ok(Err(e)) => Err(format!("Ollama request failed: {}", e)),
        Err(_) => Err(format!(
            "Ollama 请求超时：模型 {} 在 {} 秒内未返回结果。你可以稍后重试，或提高环境变量 RG_OLLAMA_CHAT_TIMEOUT_SECS / RG_OLLAMA_TIMEOUT_SECS。",
            model_name,
            timeout.as_secs()
        )),
    }
}

// ---------- cloud 通道（阿里云百炼，OpenAI 兼容端点） ----------

/// 缓存云端 CompletionsClient（必须用 CompletionsClient：默认 Client 走
/// Responses API，百炼兼容端点会 404）。构建失败/Key 缺失不缓存，下次调用可重试。
fn cloud_client() -> Result<openai::CompletionsClient, String> {
    static CLIENT: Mutex<Option<openai::CompletionsClient>> = Mutex::new(None);
    let mut guard = CLIENT
        .lock()
        .map_err(|e| format!("Cloud client lock failed: {}", e))?;
    if let Some(client) = guard.as_ref() {
        return Ok(client.clone());
    }
    let api_key = cloud_api_key()?;
    let client = openai::CompletionsClient::builder()
        .api_key(api_key)
        .base_url(cloud_base_url())
        .build()
        .map_err(|e| format!("Cloud client build failed: {}", e))?;
    *guard = Some(client.clone());
    Ok(client)
}

/// 通过 rig openai CompletionsClient 调用百炼兼容端点（非流式）。
/// timeout 尊重调用方传入值（聊天类 120s / 抽取类 45s）；RG_CLOUD_TIMEOUT_SECS
/// 仅作 cloud 流式建连默认值，不覆盖此处。
/// 模型按 fn 类别选择：聊天类 → RG_CLOUD_CHAT_MODEL（默认 qwen3.7-plus），
/// 抽取/压缩类 → RG_CLOUD_EXTRACT_MODEL（默认 qwen-flash）。
/// format=Some 时经 additional_params 注入 response_format=json_object
///（不用 output_schema 的 json_schema strict 模式，宽松 schema 可能被拒）。
/// web_search=true 时额外合入百炼 enable_search + search_strategy=turbo
///（思考开关保持现状：非流式聊天类仍显式 enable_thinking=false）。
/// 日志仅记元数据（model/fn/prompt_len/elapsed/web_search），不落内容。
async fn call_cloud(
    prompt: &str,
    format: Option<&str>,
    timeout: Duration,
    fn_name: &str,
    web_search: bool,
) -> Result<String, String> {
    let client = cloud_client()?;
    let model_name = cloud_model_for(fn_name);
    let started = Instant::now();
    info!(
        target: "llm",
        "cloud_request url={} model={} fn={} prompt_len={} format={:?} web_search={}",
        cloud_base_url(),
        model_name,
        fn_name,
        prompt.len(),
        format,
        web_search
    );

    let model = client.completion_model(&model_name);
    let mut params = serde_json::Map::new();
    if format.is_some() {
        params.insert(
            "response_format".to_string(),
            serde_json::json!({"type": "json_object"}),
        );
    }
    if is_cloud_chat_fn(fn_name) {
        // 百炼思考模型开思考时要求 stream=true；非流式请求显式关闭思考避免 400
        params.insert("enable_thinking".to_string(), serde_json::json!(false));
    }
    if web_search {
        // 百炼联网搜索（实测对 qwen3.7-plus 生效：prompt_tokens 注入搜索上下文）
        params.insert("enable_search".to_string(), serde_json::json!(true));
        params.insert(
            "search_options".to_string(),
            serde_json::json!({"search_strategy": "turbo"}),
        );
    }
    let request_builder = model.completion_request(prompt);
    let request_builder = if params.is_empty() {
        request_builder
    } else {
        request_builder.additional_params(serde_json::Value::Object(params))
    };
    let request = request_builder.build();

    match tokio::time::timeout(timeout, model.completion(request)).await {
        Ok(Ok(response)) => {
            let text: String = response
                .choice
                .iter()
                .filter_map(|item| match item {
                    AssistantContent::Text(t) => Some(t.text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("");
            info!(
                target: "llm",
                "cloud_response model={} fn={} elapsed_ms={} text_chars={}",
                model_name,
                fn_name,
                started.elapsed().as_millis(),
                text.chars().count()
            );
            if text.is_empty() {
                Err("云端模型返回内容为空".to_string())
            } else {
                Ok(text)
            }
        }
        Ok(Err(e)) => {
            warn!(
                target: "llm",
                "cloud_request_failed model={} fn={} elapsed_ms={}",
                model_name,
                fn_name,
                started.elapsed().as_millis()
            );
            Err(format!("云端请求失败（模型 {}）：{}", model_name, e))
        }
        Err(_) => Err(format!(
            "云端请求超时：模型 {} 在 {} 秒内未返回结果。你可以稍后重试，或提高环境变量 RG_CLOUD_TIMEOUT_SECS。",
            model_name,
            timeout.as_secs()
        )),
    }
}

/// general_chat 的云端流式版本：百炼聊天模型开思考（enable_thinking=true），
/// reasoning_content 增量由 rig openai provider 映射为 ReasoningDelta，
/// include_usage 由 provider 自动注入，usage 随末尾独立 chunk 的 Final 到达。
/// 事件映射与 rig ollama 路径一致（Reasoning→thinking_delta 等由上层转换）。
async fn cloud_chat_stream(
    prompt: &str,
    web_search: bool,
) -> Result<
    std::pin::Pin<Box<dyn futures::Stream<Item = Result<ChatStreamEvent, String>> + Send>>,
    String,
> {
    let client = cloud_client()?;
    let model_name = cloud_chat_model();
    // 超时对齐聊天超时语义（默认 120s，RG_CLOUD_TIMEOUT_SECS 可覆盖）：
    // 同时覆盖建流阶段与流消费阶段每次 stream.next() 的兑底，
    // 避免云端公网链路中途 stall 时 SSE 无限挂起
    let timeout = cloud_timeout();
    info!(
        target: "llm",
        "cloud_stream_request url={} model={} prompt_len={} web_search={}",
        cloud_base_url(),
        model_name,
        prompt.len(),
        web_search
    );

    let model = client.completion_model(&model_name);
    // 联网搜索开启时在思考参数基础上合入百炼 enable_search 私有参数
    let additional = if web_search {
        serde_json::json!({
            "enable_thinking": true,
            "enable_search": true,
            "search_options": {"search_strategy": "turbo"}
        })
    } else {
        serde_json::json!({"enable_thinking": true})
    };
    let request = model
        .completion_request(prompt)
        .additional_params(additional)
        .build();

    let stream = match tokio::time::timeout(timeout, model.stream(request)).await {
        Ok(Ok(stream)) => stream,
        Ok(Err(e)) => return Err(format!("云端请求失败（模型 {}）：{}", model_name, e)),
        Err(_) => {
            return Err(format!(
                "云端请求超时：模型 {} 在 {} 秒内未返回结果。你可以稍后重试，或提高环境变量 RG_CLOUD_TIMEOUT_SECS。",
                model_name,
                timeout.as_secs()
            ))
        }
    };

    let stream = Box::pin(stream);
    // 流消费阶段兜底：每次 stream.next() 包超时（公网链路中途 stall 时
    // 终止流而非无限挂起）；错误文案预构造，闭包内每轮克隆供 async 块使用
    let stall_error = format!(
        "云端流式响应超时：模型 {} 在 {} 秒内未返回新内容。你可以稍后重试，或提高环境变量 RG_CLOUD_TIMEOUT_SECS。",
        model_name,
        timeout.as_secs()
    );
    let mapped = futures::stream::try_unfold(stream, move |mut stream| {
        let stall_error = stall_error.clone();
        async move {
            use futures::StreamExt;
            loop {
                let next = match tokio::time::timeout(timeout, stream.next()).await {
                    Ok(item) => item,
                    Err(_) => return Err(stall_error),
                };
                match next {
                    None => return Ok(None),
                    Some(Ok(StreamedAssistantContent::Reasoning(reasoning))) => {
                        let text: String = reasoning
                            .content
                            .iter()
                            .filter_map(|item| match item {
                                rig_core::completion::message::ReasoningContent::Text { text, .. } => {
                                    Some(text.as_str())
                                }
                                _ => None,
                            })
                            .collect();
                        if text.is_empty() {
                            continue;
                        }
                        return Ok(Some((ChatStreamEvent::Reasoning(text), stream)));
                    }
                    Some(Ok(StreamedAssistantContent::ReasoningDelta { reasoning, .. })) => {
                        if reasoning.is_empty() {
                            continue;
                        }
                        return Ok(Some((ChatStreamEvent::Reasoning(reasoning), stream)));
                    }
                    Some(Ok(StreamedAssistantContent::Text(text))) => {
                        if text.text.is_empty() {
                            continue;
                        }
                        return Ok(Some((ChatStreamEvent::Text(text.text), stream)));
                    }
                    Some(Ok(StreamedAssistantContent::Final(final_response))) => {
                        // openai provider 的 Final 携带 Usage（prompt_tokens/completion_tokens）；
                        // provider 未回 usage 时 Usage::default() 全 0，此时映射为
                        // Done(None)，与 ollama 路径 usage=null 语义一致
                        let input = final_response.usage.prompt_tokens;
                        let output = final_response.usage.completion_tokens.unwrap_or(0);
                        let usage = if input == 0 && output == 0 { None } else { Some((input, output)) };
                        return Ok(Some((ChatStreamEvent::Done(usage), stream)));
                    }
                    // ToolCall / ToolCallDelta / Unknown 等与通用聊天无关，忽略
                    Some(Ok(_)) => continue,
                    Some(Err(e)) => return Err(format!("云端流式响应失败：{}", e)),
                }
            }
        }
    });
    Ok(Box::pin(mapped))
}

// ---------- Agent 工具循环（cloud 通道 Function Calling） ----------
//
// 为「基于我的数据生成报告」类请求提供真实联系人数据：模型通过
// data_tools 定义的只读工具自主查库（脱敏在工具层完成）。仅 cloud
// 通道启用；rig / legacy 通道不带 tools，行为与改造前一致。
// 循环模型：每轮流式调用携 tools；Final 携带 ToolCall → 执行工具并
// 追加消息进入下一轮；否则视为最终回答。DB 锁仅在工具执行瞬间持有。

/// Agent 循环中模型请求的单个工具调用
#[derive(Debug, Clone)]
pub struct CloudToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

/// Agent 流事件（对外契约，api 层映射为 SSE）
pub enum AgentStreamEvent {
    Reasoning(String),
    Text(String),
    ToolCall(CloudToolCall),
    Done(Option<(usize, usize)>),
}

/// 单轮 rig 原始流（错误已映射为 String）。
/// StreamedAssistantContent 泛型参数为 Final 携带的 raw 响应类型：
/// openai CompletionsClient 的 StreamingResponse = StreamingCompletionResponse
///（仅含 usage: Usage 字段；工具调用不在 Final 里，而在流式 ToolCall 事件中）。
type AgentRoundStream = std::pin::Pin<
    Box<
        dyn futures::Stream<
                Item = Result<StreamedAssistantContent<openai::StreamingCompletionResponse>, String>,
            > + Send,
    >,
>;

/// Agent 对外流
pub type AgentEventStream =
    std::pin::Pin<Box<dyn futures::Stream<Item = Result<AgentStreamEvent, String>> + Send>>;

/// 工具循环最大轮次（防模型反复调工具不收敛；超限后不再开启新轮）
pub const AGENT_MAX_TOOL_TURNS: usize = 4;

/// Agent 循环的系统语：复用 general_chat 的角色设定与技能/文档/联网段，
/// 在技能段与文档段之间追加数据工具使用指令（禁止编造联系人数据）。
pub fn tool_loop_system_prompt(query: &str, skills: &str, web_search: bool, documents: &str) -> String {
    let base = if web_search {
        "你是您的个人 AI 平台的通用助理。请直接回答用户问题，默认使用简体中文。\
本次对话已启用联网搜索，可参考最新网络信息作答。"
    } else {
        "你是您的个人 AI 平台的通用助理。请直接回答用户问题，默认使用简体中文。\
当用户问题涉及实时互联网信息时，明确说明你无法联网，并给出可行替代方案。"
    };
    let mut prompt = base.to_string();
    if !skills.is_empty() {
        prompt.push_str(&format!(
            "\n\n你当前具备以下技能，请在适用时遵循：\n{}",
            skills.trim_end()
        ));
    }
    prompt.push_str(
        "\n\n你可以通过工具访问用户的联系人数据库（联系人、互动记录、关系链）。\
当用户的问题涉及其联系人、关系、互动记录，或要求基于这些数据做统计、盘点、分析、生成报告时，\
必须先调用工具获取真实数据后再作答；严禁在未查询真实数据的情况下编造或模拟联系人数据。\
工具返回的姓名可能是隐私保护代称，请照用代称，不要猜测真名。",
    );
    if !documents.is_empty() {
        prompt.push_str(&format!("\n\n{}", documents.trim_end()));
    }
    prompt.push_str(&format!("\n\n用户问题：{}", query));
    prompt
}

/// 单轮携 tools 的流式调用（cloud 通道）。思考保持开启（已实测与 tools 共存）；
/// web_search 时合入百炼 enable_search。messages 为完整有序消息序列。
async fn cloud_agent_round_stream(
    messages: Vec<rig_core::completion::Message>,
    web_search: bool,
) -> Result<AgentRoundStream, String> {
    let client = cloud_client()?;
    let model_name = cloud_chat_model();
    let timeout = cloud_timeout();
    info!(
        target: "llm",
        "agent_round_stream model={} messages={} web_search={}",
        model_name,
        messages.len(),
        web_search
    );

    let model = client.completion_model(&model_name);
    let mut additional = serde_json::json!({"enable_thinking": true});
    if web_search {
        additional["enable_search"] = serde_json::json!(true);
        additional["search_options"] = serde_json::json!({"search_strategy": "turbo"});
    }
    // builder 语义：chat_history + prompt；messages 尾元素作为 prompt，
    // 其余按序放入 chat_history，重建后与原始序列一致
    let mut messages = messages;
    let last = messages.pop().ok_or_else(|| "Agent 消息序列为空".to_string())?;
    let request = model
        .completion_request(last)
        .messages(messages)
        .tools(crate::data_tools::definitions())
        .additional_params(additional)
        .build();

    let stream = match tokio::time::timeout(timeout, model.stream(request)).await {
        Ok(Ok(stream)) => stream,
        Ok(Err(e)) => return Err(format!("云端请求失败（模型 {}）：{}", model_name, e)),
        Err(_) => {
            return Err(format!(
                "云端请求超时：模型 {} 在 {} 秒内未建连。你可以稍后重试，或提高环境变量 RG_CLOUD_TIMEOUT_SECS。",
                model_name,
                timeout.as_secs()
            ))
        }
    };
    use futures::StreamExt;
    Ok(Box::pin(stream.map(move |item| {
        item.map_err(|e| format!("云端流式响应失败（模型 {}）：{}", model_name, e))
    })))
}

struct AgentCtx {
    state: crate::state::SharedState,
    /// 工具数据查询的归属用户（用户隔离：工具只能读到该用户的数据）
    owner_id: String,
    messages: Vec<rig_core::completion::Message>,
    web_search: bool,
    max_turns: usize,
    turn: usize,
    current: Option<AgentRoundStream>,
    pending_calls: Vec<CloudToolCall>,
    /// 最近一轮 Final 携带的 usage（流结束时随 Done 事件对外输出）
    last_usage: Option<(usize, usize)>,
}

/// 工具循环的流式版本（/api/chat/stream）：Reasoning/Text 增量透传，
/// ToolCall 事件由 api 层映射为 SSE step 并进入下一轮。
/// 工具执行锁约定：锁内查询 → 立即 drop guard，LLM 调用期间不持锁。
pub async fn cloud_agent_stream(
    system_prompt: String,
    query: String,
    web_search: bool,
    state: crate::state::SharedState,
    owner_id: String,
    max_turns: usize,
) -> Result<AgentEventStream, String> {
    let messages = vec![
        rig_core::completion::Message::system(system_prompt),
        rig_core::completion::Message::user(query),
    ];
    let stream = cloud_agent_round_stream(messages.clone(), web_search).await?;
    let ctx = AgentCtx {
        state,
        owner_id,
        messages,
        web_search,
        max_turns,
        turn: 1,
        current: Some(stream),
        pending_calls: Vec::new(),
        last_usage: None,
    };
    let timeout = cloud_timeout();
    let stall_error = "云端流式响应超时：长时间未返回新内容，请稍后重试。".to_string();

    let stream = futures::stream::try_unfold(ctx, move |mut ctx| {
        let stall_error = stall_error.clone();
        async move {
            use futures::StreamExt;
            use rig_core::completion::AssistantContent;
            loop {
                // 上一轮以工具调用结束 → 执行工具（每个工具独立加锁/释放，
                // 查完立即 drop guard）→ 组装消息 → 开启新一轮
                if ctx.current.is_none() {
                    if ctx.pending_calls.is_empty() {
                        return Ok(None);
                    }
                    let calls = std::mem::take(&mut ctx.pending_calls);
                    ctx.messages.push(rig_core::completion::Message::Assistant {
                        id: None,
                        content: rig_core::OneOrMany::many(
                            calls
                                .iter()
                                .map(|c| {
                                    AssistantContent::ToolCall(
                                        rig_core::completion::message::ToolCall::new(
                                            c.id.clone(),
                                            rig_core::completion::message::ToolFunction::new(
                                                c.name.clone(),
                                                c.arguments.clone(),
                                            ),
                                        ),
                                    )
                                })
                                .collect::<Vec<_>>(),
                        )
                        .map_err(|_| "工具调用消息为空".to_string())?,
                    });
                    for call in &calls {
                        let output = match ctx.state.db.lock() {
                            Ok(guard) => match crate::db::get_conn(&guard) {
                                Ok(conn) => {
                                    crate::data_tools::execute_tool(conn, &ctx.owner_id, &call.name, &call.arguments)
                                }
                                Err(e) => {
                                    serde_json::json!({"error": format!("数据库未解锁: {}", e)}).to_string()
                                }
                            },
                            Err(e) => {
                                serde_json::json!({"error": format!("数据库锁获取失败: {}", e)}).to_string()
                            }
                        }; // guard 在此释放
                        ctx.messages
                            .push(rig_core::completion::Message::tool_result(call.id.clone(), output));
                    }
                    let next = cloud_agent_round_stream(ctx.messages.clone(), ctx.web_search).await?;
                    ctx.turn += 1;
                    ctx.current = Some(next);
                }

                let stream = ctx.current.as_mut().expect("current stream");
                let next = match tokio::time::timeout(timeout, stream.next()).await {
                    Ok(item) => item,
                    Err(_) => return Err(stall_error.clone()),
                };
                match next {
                    None => {
                        // 本轮流结束：有待处理的工具调用且未超轮次上限 → 进入
                        // 工具执行分支；否则发 Done 终止（pending 需清空，
                        // 防止下一轮 poll 误入工具执行分支）
                        ctx.current = None;
                        if ctx.pending_calls.is_empty() {
                            return Ok(Some((AgentStreamEvent::Done(ctx.last_usage), ctx)));
                        }
                        if ctx.turn >= ctx.max_turns {
                            log::warn!(
                                target: "llm",
                                "agent_tool_budget_exhausted turn={} pending={}",
                                ctx.turn,
                                ctx.pending_calls.len()
                            );
                            ctx.pending_calls.clear();
                            return Ok(Some((AgentStreamEvent::Done(ctx.last_usage), ctx)));
                        }
                        continue;
                    }
                    Some(Err(e)) => return Err(e),
                    Some(Ok(StreamedAssistantContent::Reasoning(reasoning))) => {
                        let text: String = reasoning
                            .content
                            .iter()
                            .filter_map(|item| match item {
                                rig_core::completion::message::ReasoningContent::Text { text, .. } => {
                                    Some(text.as_str())
                                }
                                _ => None,
                            })
                            .collect();
                        if text.is_empty() {
                            continue;
                        }
                        return Ok(Some((AgentStreamEvent::Reasoning(text), ctx)));
                    }
                    Some(Ok(StreamedAssistantContent::ReasoningDelta { reasoning, .. })) => {
                        if reasoning.is_empty() {
                            continue;
                        }
                        return Ok(Some((AgentStreamEvent::Reasoning(reasoning), ctx)));
                    }
                    Some(Ok(StreamedAssistantContent::Text(text))) => {
                        if text.text.is_empty() {
                            continue;
                        }
                        return Ok(Some((AgentStreamEvent::Text(text.text), ctx)));
                    }
                    Some(Ok(StreamedAssistantContent::ToolCall { tool_call, .. })) => {
                        // 完整工具调用事件（provider 已将 delta 累积为完整参数）：
                        // 记入 pending_calls 待本轮流结束后执行，同时对外发进度事件
                        let call = CloudToolCall {
                            id: tool_call
                                .call_id
                                .clone()
                                .unwrap_or_else(|| tool_call.id.clone()),
                            name: tool_call.function.name.clone(),
                            arguments: tool_call.function.arguments.clone(),
                        };
                        ctx.pending_calls.push(call.clone());
                        return Ok(Some((AgentStreamEvent::ToolCall(call), ctx)));
                    }
                    Some(Ok(StreamedAssistantContent::Final(final_response))) => {
                        // Final 仅携带 usage（openai StreamingCompletionResponse）；
                        // 工具调用以流式 ToolCall 事件为准。provider 未回 usage
                        // 时全 0 → None，与 ollama 路径 usage=null 语义一致
                        let input = final_response.usage.prompt_tokens;
                        let output = final_response.usage.completion_tokens.unwrap_or(0);
                        ctx.last_usage = if input == 0 && output == 0 {
                            None
                        } else {
                            Some((input, output))
                        };
                        continue;
                    }
                    // ToolCallDelta / Unknown 等：完整 ToolCall 事件会随后到达，忽略
                    Some(Ok(_)) => continue,
                }
            }
        }
    });
    Ok(Box::pin(stream))
}

/// 工具循环的非流式版本（/api/chat）：消费 cloud_agent_stream 收集全文；
/// 工具执行由引擎内部完成，此处仅需透传文本。
pub async fn cloud_chat_with_tools(
    system_prompt: String,
    query: String,
    web_search: bool,
    state: crate::state::SharedState,
    owner_id: String,
    max_turns: usize,
) -> Result<String, String> {
    use futures::StreamExt;
    let stream = cloud_agent_stream(system_prompt, query, web_search, state, owner_id, max_turns).await?;
    futures::pin_mut!(stream);
    let mut text = String::new();
    let mut tool_rounds = 0usize;
    while let Some(event) = stream.next().await {
        match event {
            Err(e) => return Err(e),
            Ok(AgentStreamEvent::Text(t)) => text.push_str(&t),
            Ok(AgentStreamEvent::ToolCall(c)) => {
                tool_rounds += 1;
                info!(target: "llm", "agent_tool_call tool={}", c.name);
            }
            Ok(AgentStreamEvent::Reasoning(_)) | Ok(AgentStreamEvent::Done(_)) => {}
        }
    }
    info!(target: "llm", "agent_chat_done tool_rounds={} text_chars={}", tool_rounds, text.chars().count());
    if text.trim().is_empty() {
        return Err("云端模型未返回有效内容".to_string());
    }
    Ok(text)
}

/// general_chat 的系统语（流式与非流式保持一致）。
/// skills/documents 为空且 web_search=false 时输出与无技能注入的旧格式
/// 逐字节一致；非空时在角色设定与“用户问题：”之间依次插入技能段、文档段
///（技能内容由 db::agent_config::build_skills_prompt 构建，已剥离
/// frontmatter 并按字符预算截断；文档段由 document::build_documents_prompt
/// 构建并按 RG_DOC_CONTEXT_CHARS 预算截断）。
/// web_search=true 时将“无法联网”句替换为联网搜索已启用说明。
fn general_chat_prompt(query: &str, skills: &str, web_search: bool, documents: &str) -> String {
    let base = if web_search {
        "你是您的个人 AI 平台的通用助理。请直接回答用户问题，默认使用简体中文。\
本次对话已启用联网搜索，可参考最新网络信息作答。"
    } else {
        "你是您的个人 AI 平台的通用助理。请直接回答用户问题，默认使用简体中文。\
当用户问题涉及实时互联网信息时，明确说明你无法联网，并给出可行替代方案。"
    };
    let mut prompt = base.to_string();
    if !skills.is_empty() {
        prompt.push_str(&format!(
            "\n\n你当前具备以下技能，请在适用时遵循：\n{}",
            skills.trim_end()
        ));
    }
    if !documents.is_empty() {
        prompt.push_str(&format!("\n\n{}", documents.trim_end()));
    }
    prompt.push_str(&format!("\n\n用户问题：{}", query));
    prompt
}

/// 当前 general_chat 实际走的通道（"rig" / "cloud" / "legacy"），供 SSE 端点发 routing 事件
pub fn general_chat_backend() -> &'static str {
    match llm_channel("general_chat") {
        Channel::Rig => "rig",
        Channel::Cloud => "cloud",
        Channel::Legacy => "legacy",
    }
}

/// 当前对话模型名，供 SSE 端点发 llm_call 事件（cloud 通道返回云端聊天模型名）
pub fn general_chat_model() -> String {
    if llm_channel("general_chat") == Channel::Cloud {
        cloud_chat_model()
    } else {
        ollama_model()
    }
}

pub async fn general_chat(query: &str, skills: &str, web_search: bool, documents: &str) -> Result<String, String> {
    let prompt = general_chat_prompt(query, skills, web_search, documents);

    call_ollama(
        &prompt,
        None,
        ollama_timeout("RG_OLLAMA_CHAT_TIMEOUT_SECS", DEFAULT_OLLAMA_CHAT_TIMEOUT_SECS),
        "general_chat",
        web_search,
    )
    .await
}

/// general_chat_stream 的流式输出事件：
/// Reasoning → thinking_delta；Text → text_delta；Done → done（usage 无则 None）
pub enum ChatStreamEvent {
    Reasoning(String),
    Text(String),
    Done(Option<(usize, usize)>),
}

/// general_chat 的 rig 流式版本：通过 rig `model.stream()` 调用 Ollama /api/chat，
/// 将 StreamedAssistantContent::Reasoning / ReasoningDelta 映射为 Reasoning 事件、
/// Text 映射为 Text 事件、Final 映射为 Done（usage 取 prompt_eval_count / eval_count，
/// 缺失时为 None）。复用 rig client 缓存、ollama_url()/ollama_model() 与聊天超时
/// （RG_OLLAMA_CHAT_TIMEOUT_SECS），超时文案与 call_rig 一致。
/// 客户端断开时流自然 drop（rig stream 支持 cancel）。
pub async fn general_chat_stream(
    query: &str,
    skills: &str,
    web_search: bool,
    documents: &str,
) -> Result<
    std::pin::Pin<Box<dyn futures::Stream<Item = Result<ChatStreamEvent, String>> + Send>>,
    String,
> {
    let prompt = general_chat_prompt(query, skills, web_search, documents);

    // cloud 通道：百炼聊天模型（开思考，web_search 时合入 enable_search），
    // SSE 事件契约与 rig ollama 路径一致
    if llm_channel("general_chat") == Channel::Cloud {
        return cloud_chat_stream(&prompt, web_search).await;
    }

    let client = rig_client()?;
    let model_name = ollama_model();
    let timeout = ollama_timeout("RG_OLLAMA_CHAT_TIMEOUT_SECS", DEFAULT_OLLAMA_CHAT_TIMEOUT_SECS);
    info!(
        target: "llm",
        "rig_stream_request url={} model={} prompt_len={}",
        ollama_url(),
        model_name,
        prompt.len()
    );

    let model = client.completion_model(&model_name);
    let request = model.completion_request(prompt).build();

    // 超时覆盖连接建立阶段；流消费阶段由 rig stream 自身驱动，
    // 客户端断开时 Abortable 包装使流取消。
    let rig_stream = match tokio::time::timeout(timeout, model.stream(request)).await {
        Ok(Ok(stream)) => stream,
        Ok(Err(e)) => {
            return Err(format!(
                "Ollama 请求失败（请确认 Ollama 服务正在运行且地址 {} 可达）：{}",
                ollama_url(),
                e
            ))
        }
        Err(_) => return Err(format!(
            "Ollama 请求超时：模型 {} 在 {} 秒内未返回结果。你可以稍后重试，或提高环境变量 RG_OLLAMA_CHAT_TIMEOUT_SECS / RG_OLLAMA_TIMEOUT_SECS。",
            model_name,
            timeout.as_secs()
        )),
    };

    let stream = Box::pin(rig_stream);
    let mapped = futures::stream::try_unfold(stream, |mut stream| async move {
        use futures::StreamExt;
        loop {
            match stream.next().await {
                None => return Ok(None),
                Some(Ok(StreamedAssistantContent::Reasoning(reasoning))) => {
                    let text: String = reasoning
                        .content
                        .iter()
                        .filter_map(|item| match item {
                            rig_core::completion::message::ReasoningContent::Text { text, .. } => {
                                Some(text.as_str())
                            }
                            _ => None,
                        })
                        .collect();
                    if text.is_empty() {
                        continue;
                    }
                    return Ok(Some((ChatStreamEvent::Reasoning(text), stream)));
                }
                Some(Ok(StreamedAssistantContent::ReasoningDelta { reasoning, .. })) => {
                    if reasoning.is_empty() {
                        continue;
                    }
                    return Ok(Some((ChatStreamEvent::Reasoning(reasoning), stream)));
                }
                Some(Ok(StreamedAssistantContent::Text(text))) => {
                    if text.text.is_empty() {
                        continue;
                    }
                    return Ok(Some((ChatStreamEvent::Text(text.text), stream)));
                }
                Some(Ok(StreamedAssistantContent::Final(final_response))) => {
                    let usage = match (final_response.prompt_eval_count, final_response.eval_count) {
                        (None, None) => None,
                        (input, output) => {
                            Some((input.unwrap_or(0) as usize, output.unwrap_or(0) as usize))
                        }
                    };
                    return Ok(Some((ChatStreamEvent::Done(usage), stream)));
                }
                // ToolCall / ToolCallDelta / Unknown 等与通用聊天无关，忽略
                Some(Ok(_)) => continue,
                Some(Err(e)) => return Err(format!("Ollama 流式响应失败：{}", e)),
            }
        }
    });
    Ok(Box::pin(mapped))
}

/// 画像构建 QA 流程：根据系统提示词和对话历史生成下一个引导问题
pub async fn profile_qa_chat(system_prompt: &str, user_message: &str) -> Result<String, String> {
    let prompt = format!(
        "{}\n\n{}",
        system_prompt, user_message
    );

    call_ollama(
        &prompt,
        None,
        ollama_timeout("RG_OLLAMA_CHAT_TIMEOUT_SECS", DEFAULT_OLLAMA_CHAT_TIMEOUT_SECS),
        "profile_qa_chat",
        false,
    )
    .await
}

/// 画像构建最终步骤：根据完整对话历史生成个人画像文档
pub async fn generate_profile_document(system_prompt: &str, conversation: &str) -> Result<String, String> {
    let prompt = format!(
        "{}\n\n以下是完整的对话记录：\n{}\n\n请根据以上所有对话内容，保留我的原始语言表达方式和个性化表述，生成一份完整的个人画像文档（Markdown 格式），包括：价值观、思维方式、人生目标、优势与挑战、长期规划等部分。",
        system_prompt, conversation
    );

    call_ollama(
        &prompt,
        None,
        ollama_timeout("RG_OLLAMA_CHAT_TIMEOUT_SECS", DEFAULT_OLLAMA_CHAT_TIMEOUT_SECS),
        "generate_profile_document",
        false,
    )
    .await
}

/// 从自然语言中提取联系人字段（用于 create_person 意图）
pub async fn extract_person_fields(query: &str) -> PersonDraft {
    let prompt = format!(
        r#"请从以下文字中提取联系人信息，只输出JSON：
{{
  "name": "姓名（必填）",
  "company": "公司名或null",
  "location": "城市或null",
  "title": "职位或null",
  "resource_tags": ["行业标签"],
  "background": "背景描述或null",
  "school": "学校或null"
}}

文字：{}"#,
        query
    );

    match call_ollama(
        &prompt,
        Some("json"),
        ollama_timeout("RG_OLLAMA_TIMEOUT_SECS", DEFAULT_OLLAMA_TIMEOUT_SECS),
        "extract_person_fields",
        false,
    ).await {
        Ok(json_str) => {
            match serde_json::from_str::<serde_json::Value>(&json_str) {
                Ok(v) => PersonDraft {
                    name: v["name"].as_str().unwrap_or("").to_string(),
                    company: v["company"].as_str().map(|s| s.to_string()),
                    location: v["location"].as_str().map(|s| s.to_string()),
                    title: v["title"].as_str().map(|s| s.to_string()),
                    resource_tags: v["resource_tags"]
                        .as_array()
                        .map(|arr| arr.iter().filter_map(|x| x.as_str().map(String::from)).collect())
                        .unwrap_or_default(),
                    background: v["background"].as_str().map(|s| s.to_string()),
                    school: v["school"].as_str().map(|s| s.to_string()),
                    confidence: 75,
                },
                Err(_) => empty_person_draft(),
            }
        }
        Err(e) => {
            warn!(target: "llm", "extract_person_fields failed: {}", e);
            empty_person_draft()
        }
    }
}

/// 从自然语言中提取更新字段（用于 update_person 意图）
pub async fn extract_update_fields(query: &str) -> (String, Vec<FieldChange>) {
    let prompt = format!(
        r#"请从以下文字中提取联系人更新信息，只输出JSON：
{{
  "target_name": "要更新的人名",
  "changes": [
    {{"field": "字段名(company/location/title/school)", "new_value": "新值"}}
  ]
}}

文字：{}"#,
        query
    );

    match call_ollama(
        &prompt,
        Some("json"),
        ollama_timeout("RG_OLLAMA_TIMEOUT_SECS", DEFAULT_OLLAMA_TIMEOUT_SECS),
        "extract_update_fields",
        false,
    ).await {
        Ok(json_str) => {
            match serde_json::from_str::<serde_json::Value>(&json_str) {
                Ok(v) => {
                    let name = v["target_name"].as_str().unwrap_or("").to_string();
                    let changes = v["changes"]
                        .as_array()
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|c| {
                                    Some(FieldChange {
                                        field: c["field"].as_str()?.to_string(),
                                        old_value: None,
                                        new_value: c["new_value"].as_str()?.to_string(),
                                    })
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    (name, changes)
                }
                Err(_) => (String::new(), vec![]),
            }
        }
        Err(e) => {
            warn!(target: "llm", "extract_update_fields failed: {}", e);
            (String::new(), vec![])
        }
    }
}

/// 从自然语言中提取互动信息（用于 add_interaction 意图）
pub async fn extract_interaction_data(query: &str) -> InteractionDraft {
    let prompt = format!(
        r#"请从以下文字中提取互动记录信息，只输出JSON：
{{
  "person_mention": "与谁互动",
  "topic": "话题或null",
  "summary": "一句话摘要",
  "action_items": ["待办事项"]
}}

文字：{}"#,
        query
    );

    match call_ollama(
        &prompt,
        Some("json"),
        ollama_timeout("RG_OLLAMA_TIMEOUT_SECS", DEFAULT_OLLAMA_TIMEOUT_SECS),
        "extract_interaction_data",
        false,
    ).await {
        Ok(json_str) => {
            match serde_json::from_str::<serde_json::Value>(&json_str) {
                Ok(v) => InteractionDraft {
                    person_mention: v["person_mention"].as_str().unwrap_or("").to_string(),
                    resolved_person: None,
                    candidates: vec![],
                    topic: v["topic"].as_str().map(|s| s.to_string()),
                    summary: v["summary"].as_str().map(|s| s.to_string()),
                    action_items: v["action_items"]
                        .as_array()
                        .map(|arr| arr.iter().filter_map(|x| x.as_str().map(String::from)).collect())
                        .unwrap_or_default(),
                    confidence: 70,
                },
                Err(_) => empty_interaction_draft(),
            }
        }
        Err(e) => {
            warn!(target: "llm", "extract_interaction_data failed: {}", e);
            empty_interaction_draft()
        }
    }
}

/// 从路径查询中提取目标人名
pub async fn extract_path_target(query: &str) -> String {
    let prompt = format!(
        r#"请从以下文字中提取目标人名（用户想查找与谁的关系路径），只输出JSON：
{{"target_name": "目标人名"}}

文字：{}"#,
        query
    );

    match call_ollama(
        &prompt,
        Some("json"),
        ollama_timeout("RG_OLLAMA_TIMEOUT_SECS", DEFAULT_OLLAMA_TIMEOUT_SECS),
        "extract_path_target",
        false,
    ).await {
        Ok(json_str) => {
            serde_json::from_str::<serde_json::Value>(&json_str)
                .ok()
                .and_then(|v| v["target_name"].as_str().map(String::from))
                .unwrap_or_default()
        }
        Err(e) => {
            warn!(target: "llm", "extract_path_target failed: {}", e);
            String::new()
        }
    }
}

/// 从自然语言中提取要删除的联系人名（用于 delete_person 意图）
pub async fn extract_delete_target(query: &str) -> String {
    let prompt = format!(
        r#"请从以下文字中提取要删除的联系人姓名，只输出JSON：
{{"target_name": "人名"}}

文字：{}"#,
        query
    );

    match call_ollama(
        &prompt,
        Some("json"),
        ollama_timeout("RG_OLLAMA_TIMEOUT_SECS", DEFAULT_OLLAMA_TIMEOUT_SECS),
        "extract_delete_target",
        false,
    ).await {
        Ok(json_str) => {
            serde_json::from_str::<serde_json::Value>(&json_str)
                .ok()
                .and_then(|v| v["target_name"].as_str().map(String::from))
                .unwrap_or_default()
        }
        Err(e) => {
            warn!(target: "llm", "extract_delete_target failed: {}", e);
            String::new()
        }
    }
}

fn empty_person_draft() -> PersonDraft {
    PersonDraft {
        name: String::new(),
        company: None,
        location: None,
        title: None,
        resource_tags: vec![],
        background: None,
        school: None,
        confidence: 0,
    }
}

fn empty_interaction_draft() -> InteractionDraft {
    InteractionDraft {
        person_mention: String::new(),
        resolved_person: None,
        candidates: vec![],
        topic: None,
        summary: None,
        action_items: vec![],
        confidence: 0,
    }
}

/// 将多条消息压缩为一段摘要文本
pub async fn compress_context(
    messages: &[ChatMessage],
    max_tokens: usize,
) -> Result<String, String> {
    let system_prompt = "你是一个对话摘要助手。请将以下对话历史压缩为简洁的摘要，保留关键信息、决策和上下文。用中文输出。";

    let conversation_text: String = messages
        .iter()
        .map(|m| {
            format!(
                "{}: {}",
                if m.role == "user" { "用户" } else { "助手" },
                m.content
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let prompt = format!(
        "{}\n\n请将以下对话压缩为不超过 {} token 的摘要：\n\n{}",
        system_prompt, max_tokens, conversation_text
    );

    info!(target: "llm", "compress_context message_count={} total_chars={}", messages.len(), conversation_text.len());

    call_ollama(
        &prompt,
        None,
        ollama_timeout("RG_OLLAMA_CHAT_TIMEOUT_SECS", DEFAULT_OLLAMA_CHAT_TIMEOUT_SECS),
        "compress_context",
        false,
    )
    .await
}
