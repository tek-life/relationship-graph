use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use std::time::Duration;
use log::{info, warn};

use rig_core::client::{CompletionClient, Nothing};
use rig_core::completion::{AssistantContent, CompletionModel as _};
use rig_core::providers::ollama;
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

async fn call_ollama(prompt: &str, format: Option<&str>, timeout: Duration, fn_name: &str) -> Result<String, String> {
    // 三值通道分发：RG_LLM_BACKEND=rig/cloud 时全量走对应通道；legacy 模式下
    // 命中 RG_LLM_CLOUD_FNS / RG_LLM_RIG_FNS 白名单的函数分别走 cloud / rig
    //（函数级灰度，cloud 优先），其余走原 reqwest legacy 实现（默认行为与
    // 改造前完全一致）。详见 llm_channel_for 注释。
    match llm_channel(fn_name) {
        Channel::Rig => return call_rig(prompt, format, timeout).await,
        // Cloud 通道在下一步接线，暂回落 legacy
        Channel::Cloud | Channel::Legacy => {}
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

const DEFAULT_CLOUD_TIMEOUT_SECS: u64 = 60;

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
            let trimmed = content.trim();
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
            "你是关系图谱应用中的通用助理。请直接回答用户问题，默认使用简体中文。\
当用户问题涉及实时互联网信息时，明确说明你无法联网，并给出可行替代方案。\
\n\n用户问题：{}",
            query
        )
    }

    #[test]
    fn empty_skills_prompt_matches_legacy_format_byte_for_byte() {
        let query = "帮我总结一下最近的关系维护情况";
        assert_eq!(general_chat_prompt(query, ""), legacy_prompt(query));
        // 空 query 同样一致
        assert_eq!(general_chat_prompt("", ""), legacy_prompt(""));
    }

    #[test]
    fn non_empty_skills_insert_section_between_role_and_question() {
        let skills = "### 技能：演示\n技能正文\n\n";
        let prompt = general_chat_prompt("你好", skills);
        // 角色设定在前、技能段居中、用户问题殿后
        assert!(prompt.starts_with("你是关系图谱应用中的通用助理。"));
        assert!(prompt.contains("\n\n你当前具备以下技能，请在适用时遵循：\n### 技能：演示\n技能正文\n\n用户问题：你好"));
        assert!(prompt.ends_with("用户问题：你好"));
        // 技能尾部空白被归一，与“用户问题：”之间恰有一个空行
        assert!(!prompt.contains("\n\n\n\n用户问题"));
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

/// general_chat 的系统语（流式与非流式保持一致）。
/// skills 为空时输出与无技能注入的旧格式逐字节一致；非空时在角色设定与
/// “用户问题：”之间插入技能段（技能内容由 db::agent_config::build_skills_prompt
/// 构建，已剥离 frontmatter 并按字符预算截断）。
fn general_chat_prompt(query: &str, skills: &str) -> String {
    let base = "你是关系图谱应用中的通用助理。请直接回答用户问题，默认使用简体中文。\
当用户问题涉及实时互联网信息时，明确说明你无法联网，并给出可行替代方案。";
    if skills.is_empty() {
        format!("{}\n\n用户问题：{}", base, query)
    } else {
        format!(
            "{}\n\n你当前具备以下技能，请在适用时遵循：\n{}\n\n用户问题：{}",
            base,
            skills.trim_end(),
            query
        )
    }
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

pub async fn general_chat(query: &str, skills: &str) -> Result<String, String> {
    let prompt = general_chat_prompt(query, skills);

    call_ollama(
        &prompt,
        None,
        ollama_timeout("RG_OLLAMA_CHAT_TIMEOUT_SECS", DEFAULT_OLLAMA_CHAT_TIMEOUT_SECS),
        "general_chat",
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
) -> Result<
    std::pin::Pin<Box<dyn futures::Stream<Item = Result<ChatStreamEvent, String>> + Send>>,
    String,
> {
    let client = rig_client()?;
    let model_name = ollama_model();
    let timeout = ollama_timeout("RG_OLLAMA_CHAT_TIMEOUT_SECS", DEFAULT_OLLAMA_CHAT_TIMEOUT_SECS);
    let prompt = general_chat_prompt(query, skills);
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
    )
    .await
}
