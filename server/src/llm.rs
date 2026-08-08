use serde::{Deserialize, Serialize};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use log::{info, warn};

use rig_core::client::{CompletionClient, Nothing};
use rig_core::completion::{AssistantContent, CompletionModel as _};
use rig_core::providers::{ollama, openai};
use rig_core::streaming::StreamedAssistantContent;

use crate::types::{PersonDraft, FieldChange, InteractionDraft, ChatMessage, ChatHistory};

const DEFAULT_OLLAMA_TIMEOUT_SECS: u64 = 45;
const DEFAULT_OLLAMA_CHAT_TIMEOUT_SECS: u64 = 120;

/// Ollama 上下文窗口默认值（P1-5）：取建议区间 8192-16384 的上限，
/// 需同时容纳 system（角色+技能+文档）+ 会话历史（RG_CHAT_HISTORY_CHARS
/// 默认 8000 字符，中文约 8-10k token）+ 本轮输出（RG_MAX_OUTPUT_TOKENS），
/// RG_OLLAMA_NUM_CTX 可覆盖，非法值回退本默认值。
const DEFAULT_OLLAMA_NUM_CTX: u64 = 16384;
/// 模型单次输出 token 上限默认值（P1-5）：同时映射为 Ollama 的
/// num_predict 与 OpenAI 兼容端点的 max_tokens，防止模型无限生成占满
/// 上下文窗口；RG_MAX_OUTPUT_TOKENS 可覆盖，非法值回退本默认值。
const DEFAULT_MAX_OUTPUT_TOKENS: u64 = 4096;

/// 解析正整数（纯函数，可单测）：输入为 Some 且 trim 后为正整数时返回其值；
/// 未设置（None）、空串、非数字、0、负数一律回退 default。
fn parse_positive_u64(env_value: Option<&str>, default: u64) -> u64 {
    env_value
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .and_then(|v| v.parse::<u64>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(default)
}

/// 读取正整数型环境变量（非法/未设置回退 default）
fn env_positive(env_key: &str, default: u64) -> u64 {
    parse_positive_u64(std::env::var(env_key).ok().as_deref(), default)
}

/// Ollama 上下文窗口大小（RG_OLLAMA_NUM_CTX，默认 16384）
fn ollama_num_ctx() -> u64 {
    env_positive("RG_OLLAMA_NUM_CTX", DEFAULT_OLLAMA_NUM_CTX)
}

/// 模型输出 token 上限（RG_MAX_OUTPUT_TOKENS，默认 4096）
fn max_output_tokens() -> u64 {
    env_positive("RG_MAX_OUTPUT_TOKENS", DEFAULT_MAX_OUTPUT_TOKENS)
}

/// Ollama 原生 API 的 options 对象（纯函数，可单测）：
/// num_ctx 上下文窗口 + num_predict 输出上限（原生 API 中 max_tokens
/// 对应的参数名）。legacy /api/generate 与 /api/chat 共用。
fn build_ollama_options(num_ctx: u64, num_predict: u64) -> serde_json::Value {
    let mut options = serde_json::Map::new();
    options.insert("num_ctx".to_string(), serde_json::json!(num_ctx));
    options.insert("num_predict".to_string(), serde_json::json!(num_predict));
    serde_json::Value::Object(options)
}

#[cfg(test)]
mod window_param_tests {
    use super::*;

    #[test]
    fn parse_positive_u64_valid_values() {
        assert_eq!(parse_positive_u64(Some("16384"), 42), 16384);
        assert_eq!(parse_positive_u64(Some("  8192 "), 42), 8192);
        assert_eq!(parse_positive_u64(Some("1"), 42), 1);
    }

    #[test]
    fn parse_positive_u64_invalid_fallback() {
        // 未设置 / 空串 / 空白 / 非数字 / 0 / 负数 / 溢出 / 小数 → 全部回退默认
        assert_eq!(parse_positive_u64(None, 42), 42);
        assert_eq!(parse_positive_u64(Some(""), 42), 42);
        assert_eq!(parse_positive_u64(Some("   "), 42), 42);
        assert_eq!(parse_positive_u64(Some("abc"), 42), 42);
        assert_eq!(parse_positive_u64(Some("0"), 42), 42);
        assert_eq!(parse_positive_u64(Some("-5"), 42), 42);
        assert_eq!(parse_positive_u64(Some("999999999999999999999999"), 42), 42);
        assert_eq!(parse_positive_u64(Some("4096.5"), 42), 42);
    }

    /// 默认值基线（仅在对应 env 未设置时断言，避免并发测试改 env 竞态）
    #[test]
    fn ollama_num_ctx_default_baseline() {
        if std::env::var("RG_OLLAMA_NUM_CTX").is_ok() {
            return;
        }
        assert_eq!(ollama_num_ctx(), DEFAULT_OLLAMA_NUM_CTX);
        assert_eq!(DEFAULT_OLLAMA_NUM_CTX, 16384);
    }

    #[test]
    fn max_output_tokens_default_baseline() {
        if std::env::var("RG_MAX_OUTPUT_TOKENS").is_ok() {
            return;
        }
        assert_eq!(max_output_tokens(), DEFAULT_MAX_OUTPUT_TOKENS);
        assert_eq!(DEFAULT_MAX_OUTPUT_TOKENS, 4096);
    }

    #[test]
    fn ollama_options_contains_num_ctx_and_num_predict() {
        let options = build_ollama_options(8192, 2048);
        assert_eq!(options["num_ctx"], serde_json::json!(8192));
        assert_eq!(options["num_predict"], serde_json::json!(2048));
        // 仅含两个窗口参数，无多余字段
        assert_eq!(options.as_object().unwrap().len(), 2);
    }
}

fn ollama_url() -> String {
    std::env::var("RG_OLLAMA_URL").unwrap_or_else(|_| "http://localhost:11434".to_string())
}

// ---------- 按场景模型解析（P1-7：model_configs 表 + env 覆盖层） ----------
//
// 解析优先级（向后兼容决策）：env 覆盖层（实时）> DB model_configs 表 > 硬编码默认。
// 理由：存量部署依赖 RG_OLLAMA_MODEL / RG_CLOUD_*_MODEL 的行为逐字节不变，
// 且运维可随时通过 env 紧急回滚；DB 层为管理后台的持久化配置面（种子值与
// 硬编码默认同源，见 db::model_config::seed_default_models）。未配置时
//（DB 无行且 env 未设置）结果与改造前 env 路由默认逐一致。

/// DB model_configs 读取器注册表：由 main.rs 启动时注入（读 model_configs
/// 表），llm.rs 不直接依赖 AppState（与云端 Key 的 CLOUD_KEY_DB_READER 同模式）。
/// 注册前/注册失败时 DB 层视为无值（回退 env/默认，行为与改造前一致）。
type ModelConfigReader = dyn Fn(&str) -> Option<String> + Send + Sync;
static MODEL_CONFIG_DB_READER: OnceLock<Box<ModelConfigReader>> = OnceLock::new();

/// 注册 DB model_configs 读取器（仅首次生效）。读取器约束：短持锁、
/// 不在持锁期间做 LLM 调用。
pub fn register_model_config_reader(reader: Box<ModelConfigReader>) {
    if MODEL_CONFIG_DB_READER.set(reader).is_err() {
        log::warn!(target: "llm", "model_config_reader_already_registered（忽略重复注册）");
    }
}

fn read_db_model(scenario: &str) -> Option<String> {
    MODEL_CONFIG_DB_READER
        .get()
        .and_then(|reader| reader(scenario))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// 模型调用场景（P1-7）：与 db::model_config::SCENARIO_METAS 的场景键一一对应
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelScenario {
    /// 本地 Ollama 通道（legacy/rig）：聊天/抽取等全部本地调用共用单一模型
    Local,
    /// cloud 聊天主力模型（开思考）
    Chat,
    /// cloud 联网搜索模型
    ChatSearch,
    /// cloud 结构化抽取
    Extract,
    /// 上下文压缩摘要
    Summarize,
}

impl ModelScenario {
    /// 场景键（model_configs / llm_usages 的 scenario 字段取值）
    pub fn as_str(&self) -> &'static str {
        match self {
            ModelScenario::Local => "local",
            ModelScenario::Chat => "chat",
            ModelScenario::ChatSearch => "chat_search",
            ModelScenario::Extract => "extract",
            ModelScenario::Summarize => "summarize",
        }
    }

    fn from_key(key: &str) -> Option<ModelScenario> {
        match key {
            "local" => Some(ModelScenario::Local),
            "chat" => Some(ModelScenario::Chat),
            "chat_search" => Some(ModelScenario::ChatSearch),
            "extract" => Some(ModelScenario::Extract),
            "summarize" => Some(ModelScenario::Summarize),
            _ => None,
        }
    }

    /// 该场景的 env 覆盖键。summarize 与 extract 共用 RG_CLOUD_EXTRACT_MODEL：
    /// 改造前 compress_context 走抽取模型（读同一 env），共用覆盖键保证
    /// 存量 env 部署行为不变（向后兼容决策）。
    fn env_keys(&self) -> &'static [&'static str] {
        match self {
            ModelScenario::Local => &["RG_OLLAMA_MODEL"],
            ModelScenario::Chat => &["RG_CLOUD_CHAT_MODEL"],
            ModelScenario::ChatSearch => &["RG_CLOUD_SEARCH_MODEL"],
            ModelScenario::Extract => &["RG_CLOUD_EXTRACT_MODEL"],
            ModelScenario::Summarize => &["RG_CLOUD_EXTRACT_MODEL"],
        }
    }

    /// 硬编码默认模型（与改造前 env 路由默认同源）
    fn default_model(&self) -> &'static str {
        crate::db::model_config::default_model_for(self.as_str())
    }
}

/// 模型解析纯函数（可单测）：env 值按顺序取第一个 trim 后非空者 >
/// DB 值 trim 后非空取之 > 硬编码默认。
fn resolve_scenario_model(db_value: Option<&str>, env_values: &[Option<String>], default: &str) -> String {
    for env_value in env_values {
        if let Some(value) = env_value.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
            return value.to_string();
        }
    }
    if let Some(value) = db_value.map(str::trim).filter(|v| !v.is_empty()) {
        return value.to_string();
    }
    default.to_string()
}

/// 解析场景生效模型：env 覆盖层 > DB model_configs > 硬编码默认
///（优先级决策见本节顶部注释）
fn resolve_model_for_scenario(scenario: ModelScenario) -> String {
    let env_values: Vec<Option<String>> = scenario
        .env_keys()
        .iter()
        .map(|key| std::env::var(key).ok())
        .collect();
    resolve_scenario_model(
        read_db_model(scenario.as_str()).as_deref(),
        &env_values,
        scenario.default_model(),
    )
}

/// 场景当前设置的 env 覆盖值（按覆盖键顺序取第一个非空；无则 None）。
/// 供 admin 配置页展示覆盖状态；仅回 env 变量名与模型名，无敏感信息。
pub fn env_override_for_scenario(scenario_key: &str) -> Option<String> {
    let scenario = ModelScenario::from_key(scenario_key)?;
    scenario.env_keys().iter().find_map(|key| {
        std::env::var(key)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    })
}

/// 场景当前生效的模型名（env > DB > 默认），供 admin 配置页展示；
/// 未知场景返回 None。仅返回模型名，无敏感信息。
pub fn effective_model_for(scenario_key: &str) -> Option<String> {
    ModelScenario::from_key(scenario_key).map(resolve_model_for_scenario)
}

/// fn → 场景映射（供 usage 落库 scenario 字段）：聊天类函数或开启联网
/// 搜索 → chat；compress_context → summarize；其余抽取类 → extract。
/// 本地通道的 fn 同样按此映射（scenario 语义与 cloud 对齐，model 字段
/// 记录实际生效的本地模型）。
pub fn scenario_for_fn(fn_name: &str, web_search: bool) -> &'static str {
    if is_cloud_chat_fn(fn_name) || web_search {
        ModelScenario::Chat.as_str()
    } else if fn_name == "compress_context" {
        ModelScenario::Summarize.as_str()
    } else {
        ModelScenario::Extract.as_str()
    }
}

// ---------- token usage 落库（P1-7：只落元数据，绝不落对话内容） ----------

/// LLM 调用用量元数据（与 db::model_config::UsageInsert 一一对应）。
/// 全部字段为调用元数据，不含任何 prompt / 回复内容；prompt/completion
/// token 数在 provider 未回传时为 None。
#[derive(Debug, Clone)]
pub struct LlmUsageRecord {
    pub scenario: &'static str,
    pub channel: &'static str,
    pub model: String,
    pub fn_name: String,
    pub prompt_tokens: Option<usize>,
    pub completion_tokens: Option<usize>,
    pub elapsed_ms: u64,
}

/// usage 落库写入器注册表：由 main.rs 启动时注入（短持 DB 锁写入，
/// 失败记 warn 日志不阻断主链路）。未注册（如测试环境）时静默丢弃。
type UsageWriter = dyn Fn(LlmUsageRecord) + Send + Sync;
static USAGE_WRITER: OnceLock<Box<UsageWriter>> = OnceLock::new();

pub fn register_usage_writer(writer: Box<UsageWriter>) {
    if USAGE_WRITER.set(writer).is_err() {
        log::warn!(target: "llm", "usage_writer_already_registered（忽略重复注册）");
    }
}

/// 记录一条用量元数据：fire-and-forget，写入失败不影响主链路；
/// 日志与落库均只含 token 数/耗时等数字元数据，无对话内容。
fn record_usage(record: LlmUsageRecord) {
    if let Some(writer) = USAGE_WRITER.get() {
        writer(record);
    }
}

/// rig 非流式响应的 usage 提取（可单测）：provider 未回传时 rig 置全 0，
/// 此时映射为 None（与流式路径 usage=null 语义一致）
fn usage_from_rig_response<T>(
    response: &rig_core::completion::CompletionResponse<T>,
) -> (Option<usize>, Option<usize>) {
    let input = response.usage.input_tokens;
    let output = response.usage.output_tokens;
    (
        if input == 0 { None } else { Some(input as usize) },
        if output == 0 { None } else { Some(output as usize) },
    )
}

/// Ollama 本地模型（legacy/rig 通道唯一模型）：P1-7 按场景解析，
/// 优先级 env RG_OLLAMA_MODEL > DB model_configs > 默认 qwen2.5:7b
fn ollama_model() -> String {
    resolve_model_for_scenario(ModelScenario::Local)
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
    /// 模型参数（P1-5：num_ctx / num_predict），始终携带
    options: serde_json::Value,
}

#[derive(Deserialize)]
struct OllamaResponse {
    response: Option<String>,
    /// 用量元数据（P1-7）：Ollama /api/generate 回传 token 计数，
    /// 旧版本可能缺失 → None（仅用于 usage 落库，不影响回复内容）
    #[serde(default)]
    prompt_eval_count: Option<u64>,
    #[serde(default)]
    eval_count: Option<u64>,
}

// ---------- 重试退避基础设施（P1-6） ----------

/// LLM 调用错误分类（P1-6）：Transient 可指数退避重试，Permanent 立即返回。
/// 仅用于非流式调用；流式链路一旦开始产出内容不可重试（安全优先，宁可降级）。
#[derive(Debug, Clone, PartialEq, Eq)]
enum LlmError {
    /// 瞬时错误：超时 / 连接失败 / HTTP 5xx / 429
    Transient(String),
    /// 持久错误：其他 4xx、配置缺失（如 API Key）、空响应、解析失败等
    Permanent(String),
}

/// HTTP 状态码可重试判定（纯函数，可单测）：429（限流）与 5xx（服务端错误）
fn is_retryable_status(status: u16) -> bool {
    status == 429 || (500..=599).contains(&status)
}

/// 传输层错误分类（纯函数，可单测）：rig/cloud 通道的错误以字符串呈现，
/// 按常见瞬时错误标记（超时/连接/429/5xx 文案）归类；无法识别一律视为
/// Permanent（宁可少重试，不可重复产生副作用）。
fn classify_transport_error(message: &str) -> LlmError {
    const TRANSIENT_MARKERS: &[&str] = &[
        "timeout",
        "timed out",
        "deadline",
        "connection",
        "error sending request",
        "too many requests",
        "service unavailable",
        "bad gateway",
        "gateway timeout",
        "internal server error",
        "429",
        "500",
        "502",
        "503",
        "504",
    ];
    let lower = message.to_lowercase();
    if TRANSIENT_MARKERS.iter().any(|m| lower.contains(m)) {
        LlmError::Transient(message.to_string())
    } else {
        LlmError::Permanent(message.to_string())
    }
}

/// 退避间隔表（P1-6）：最多重试 2 次，间隔 0.5s → 1.5s
fn retry_backoffs() -> &'static [Duration] {
    static BACKOFFS: [Duration; 2] = [Duration::from_millis(500), Duration::from_millis(1500)];
    &BACKOFFS
}

/// 指数退避重试包装（P1-6）：仅对 Transient 错误按 retry_backoffs() 重试，
/// Permanent 与重试耗尽后的最后一次错误原样返回。日志仅记元数据
///（fn/attempt/elapsed/backoff），不落对话内容。
async fn retry_transient<F, Fut>(fn_name: &str, mut op: F) -> Result<String, String>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<String, LlmError>>,
{
    let backoffs = retry_backoffs();
    let mut attempt = 0usize;
    loop {
        let started = Instant::now();
        match op().await {
            Ok(text) => return Ok(text),
            Err(LlmError::Permanent(msg)) => return Err(msg),
            Err(LlmError::Transient(msg)) => {
                if attempt >= backoffs.len() {
                    return Err(msg);
                }
                warn!(
                    target: "llm",
                    "llm_retry fn={} attempt={}/{} elapsed_ms={} backoff_ms={}",
                    fn_name,
                    attempt + 1,
                    backoffs.len(),
                    started.elapsed().as_millis(),
                    backoffs[attempt].as_millis()
                );
                tokio::time::sleep(backoffs[attempt]).await;
                attempt += 1;
            }
        }
    }
}

#[cfg(test)]
mod retry_policy_tests {
    use super::*;

    #[test]
    fn retryable_status_codes_are_429_and_5xx() {
        assert!(is_retryable_status(429));
        for code in [500u16, 502, 503, 504, 599] {
            assert!(is_retryable_status(code));
        }
    }

    #[test]
    fn non_retryable_status_codes() {
        for code in [200u16, 301, 400, 401, 403, 404, 422, 499] {
            assert!(!is_retryable_status(code));
        }
    }

    #[test]
    fn backoff_schedule_is_half_then_one_and_half_seconds() {
        let backoffs = retry_backoffs();
        assert_eq!(backoffs.len(), 2);
        assert_eq!(backoffs[0], Duration::from_millis(500));
        assert_eq!(backoffs[1], Duration::from_millis(1500));
    }

    #[test]
    fn transport_error_classification_transient_markers() {
        for msg in [
            "request timeout",
            "error sending request for url (http://localhost:11434/api/generate)",
            "connection refused",
            "HTTP status 429 Too Many Requests",
            "HTTP 503 Service Unavailable",
            "upstream 502 Bad Gateway",
        ] {
            assert_eq!(
                classify_transport_error(msg),
                LlmError::Transient(msg.to_string()),
                "应为瞬时错误：{}",
                msg
            );
        }
    }

    #[test]
    fn transport_error_classification_permanent_default() {
        for msg in [
            "model not found",
            "invalid api key",
            "context length exceeded",
        ] {
            assert_eq!(
                classify_transport_error(msg),
                LlmError::Permanent(msg.to_string()),
                "应为持久错误：{}",
                msg
            );
        }
    }

    /// 瞬时错误首次失败后退避重试 1 次即成功：总尝试 2 次
    #[tokio::test]
    async fn retry_transient_retries_then_succeeds() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let attempts = AtomicUsize::new(0);
        let result = retry_transient("test_fn", || {
            let n = attempts.fetch_add(1, Ordering::SeqCst);
            async move {
                if n == 0 {
                    Err(LlmError::Transient("simulated timeout".to_string()))
                } else {
                    Ok("ok".to_string())
                }
            }
        })
        .await;
        assert_eq!(result.unwrap(), "ok");
        assert_eq!(attempts.load(Ordering::SeqCst), 2);
    }

    /// 持久错误立即返回，不重试：总尝试 1 次
    #[tokio::test]
    async fn retry_transient_does_not_retry_permanent() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let attempts = AtomicUsize::new(0);
        let result = retry_transient("test_fn", || {
            attempts.fetch_add(1, Ordering::SeqCst);
            async move { Err(LlmError::Permanent("bad request".to_string())) }
        })
        .await;
        assert_eq!(result.unwrap_err(), "bad request");
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
    }

    /// 瞬时错误重试耗尽（最多 2 次重试）后返回最后一次错误：总尝试 3 次
    #[tokio::test]
    async fn retry_transient_gives_up_after_max_retries() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let attempts = AtomicUsize::new(0);
        let result = retry_transient("test_fn", || {
            attempts.fetch_add(1, Ordering::SeqCst);
            async move { Err(LlmError::Transient("still failing".to_string())) }
        })
        .await;
        assert_eq!(result.unwrap_err(), "still failing");
        assert_eq!(attempts.load(Ordering::SeqCst), 3);
    }
}

// ---------- legacy 通道 HTTP Client（P2-11 全局单例） ----------

/// legacy 通道的 reqwest::Client 全局单例：复用连接池，避免每次调用
/// 新建 TCP/TLS 连接。Client 本身不携带超时，每次调用经
/// RequestBuilder::timeout 按场景指定（抽取类 45s / 聊天类 120s）。
fn legacy_http_client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(reqwest::Client::new)
}

#[cfg(test)]
mod legacy_client_tests {
    use super::*;

    /// 单例性：多次获取返回同一 &'static 实例（指针相等）
    #[test]
    fn legacy_http_client_is_singleton() {
        let first = legacy_http_client();
        let second = legacy_http_client();
        assert!(std::ptr::eq(first, second));
    }
}

async fn call_generate(prompt: &str, format: Option<&str>, timeout: Duration, fn_name: &str, web_search: bool) -> Result<String, String> {
    // P1-6：非流式调用统一套指数退避重试（超时 / 5xx / 429 最多重试 2 次，
    // 0.5s → 1.5s）；流式链路不经过本函数，天然不重试。
    retry_transient(fn_name, || call_generate_once(prompt, format, timeout, fn_name, web_search)).await
}

/// 单次调用（不含重试）：通道分发收敛到 provider_for（P0-1）。
/// 错误按 LlmError 分类供 retry_transient 决策。
async fn call_generate_once(prompt: &str, format: Option<&str>, timeout: Duration, fn_name: &str, web_search: bool) -> Result<String, LlmError> {
    // RG_LLM_BACKEND=rig/cloud 时全量走对应 provider；legacy 模式下命中
    // RG_LLM_CLOUD_FNS / RG_LLM_RIG_FNS 白名单的函数分别走 cloud / rig
    //（函数级灰度，cloud 优先），其余走 OllamaProvider Legacy 模式
    //（reqwest Ollama 原生 API，默认行为与改造前完全一致）。详见
    // llm_channel_for 注释。web_search 仅 cloud 通道生效。
    provider_for(fn_name)
        .generate(fn_name, prompt, format, timeout, web_search)
        .await
}

/// Ollama 原生 API 单 prompt 非流式调用（OllamaProvider Legacy 模式实现体）：
/// /api/generate + format。即 legacy 通道协议，向后兼容保留（P0-1 决策）。
/// 错误按 LlmError 分类（P1-6）；成功时 usage 落库（P1-7，仅元数据）。
async fn ollama_generate_once(fn_name: &str, prompt: &str, format: Option<&str>, timeout: Duration) -> Result<String, LlmError> {
    let client = legacy_http_client();
    let model = ollama_model();
    let started = Instant::now();

    let req = OllamaRequest {
        model: model.clone(),
        stream: false,
        format: format.map(str::to_string),
        prompt: prompt.to_string(),
        options: build_ollama_options(ollama_num_ctx(), max_output_tokens()),
    };

    let url = format!("{}/api/generate", ollama_url());
    info!(target: "llm", "ollama_request url={} model={} prompt_len={}", url, req.model, prompt.len());

    let resp = client
        .post(&url)
        .timeout(timeout)
        .json(&req)
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                LlmError::Transient(format!(
                    "Ollama 请求超时：模型 {} 在 {} 秒内未返回结果。你可以稍后重试，或提高环境变量 RG_OLLAMA_CHAT_TIMEOUT_SECS / RG_OLLAMA_TIMEOUT_SECS。",
                    model,
                    timeout.as_secs()
                ))
            } else if e.is_connect() || e.is_request() {
                LlmError::Transient(format!("Ollama request failed: {}", e))
            } else {
                LlmError::Permanent(format!("Ollama request failed: {}", e))
            }
        })?;

    if !resp.status().is_success() {
        let msg = format!("Ollama returned status {}", resp.status());
        return Err(if is_retryable_status(resp.status().as_u16()) {
            LlmError::Transient(msg)
        } else {
            LlmError::Permanent(msg)
        });
    }

    let data: OllamaResponse = resp.json().await.map_err(|e| LlmError::Permanent(format!("Parse error: {}", e)))?;
    let text = data
        .response
        .ok_or_else(|| LlmError::Permanent("Empty response from Ollama".to_string()))?;
    // usage 落库（P1-7）：Ollama 原生 API 回传 prompt_eval_count / eval_count，
    // 旧版本可能缺失 → None；只落元数据，不落内容
    record_usage(LlmUsageRecord {
        scenario: scenario_for_fn(fn_name, false),
        channel: "legacy",
        model,
        fn_name: fn_name.to_string(),
        prompt_tokens: data.prompt_eval_count.map(|v| v as usize),
        completion_tokens: data.eval_count.map(|v| v as usize),
        elapsed_ms: started.elapsed().as_millis() as u64,
    });
    Ok(text)
}

// ---------- 通道分发（legacy | rig | cloud 三值） ----------

const DEFAULT_CLOUD_TIMEOUT_SECS: u64 = 120;

/// 云端（阿里云百炼）兼容端点：默认走 Token Plan 专属网关
///（sk-sp- 前缀 Key 仅在此网关生效；普通百炼 Key 用 RG_CLOUD_BASE_URL 覆盖回
/// dashscope.aliyuncs.com/compatible-mode/v1）
fn cloud_base_url() -> String {
    std::env::var("RG_CLOUD_BASE_URL")
        .unwrap_or_else(|_| "https://token-plan.cn-beijing.maas.aliyuncs.com/compatible-mode/v1".to_string())
}

/// 云端聊天主力模型（开思考）：P1-7 按场景解析，
/// 优先级 env RG_CLOUD_CHAT_MODEL > DB model_configs > 默认
fn cloud_chat_model() -> String {
    resolve_model_for_scenario(ModelScenario::Chat)
}

/// 云端联网搜索模型：Token Plan 网关上 qwen3.7-plus 已支持 enable_search
///（2026-08-08 实测，流式+思考均正常），故默认与聊天模型统一；保留本
/// 路由机制作为逃生门（平台行为再变时可 env 切换到其它搜索可用模型）。
/// P1-7 按场景解析，优先级 env RG_CLOUD_SEARCH_MODEL > DB > 默认
fn cloud_search_model() -> String {
    resolve_model_for_scenario(ModelScenario::ChatSearch)
}

/// 云端聊天模型路由（纯逻辑入口，回归单测见 model_routing_tests）：
/// web_search 请求路由到搜索模型，否则用聊天模型
fn cloud_chat_model_for(web_search: bool) -> String {
    if web_search {
        cloud_search_model()
    } else {
        cloud_chat_model()
    }
}

/// 按通道路由通用聊天模型（纯逻辑，可单测）：分发收敛到 provider_for_channel
///（P0-1）——cloud 通道按 web_search 选择搜索/聊天模型，其余通道走本地 Ollama
fn chat_model_route(channel: Channel, web_search: bool) -> String {
    provider_for_channel(channel).chat_model(web_search)
}

/// 云端抽取首选模型（无思考开销，json_object 正常）：Token Plan 网关
/// 无 qwen-flash，用其上的轻量模型 qwen3.6-flash（2026-08-08 实测 json_object 正常）。
/// P1-7 按场景解析，优先级 env RG_CLOUD_EXTRACT_MODEL > DB > 默认
fn cloud_extract_model() -> String {
    resolve_model_for_scenario(ModelScenario::Extract)
}

/// 云端压缩摘要模型（P1-7 新场景）：改造前 compress_context 与抽取共用
/// 模型，故默认与 extract 同源（qwen3.6-flash），env 覆盖层同样共用
/// RG_CLOUD_EXTRACT_MODEL（向后兼容）；管理后台可单独配置绑定不同模型
fn cloud_summarize_model() -> String {
    resolve_model_for_scenario(ModelScenario::Summarize)
}

/// 云端默认超时（RG_CLOUD_TIMEOUT_SECS 可覆盖）：仅作为 cloud 流式
/// （cloud_chat_stream）建连超时的默认值，默认值对齐聊天超时语义
///（与 DEFAULT_OLLAMA_CHAT_TIMEOUT_SECS 同档）；非流式 call_cloud 由
/// call_generate 分发时尊重调用方传入的 timeout，不受此 env 覆盖。
fn cloud_timeout() -> Duration {
    ollama_timeout("RG_CLOUD_TIMEOUT_SECS", DEFAULT_CLOUD_TIMEOUT_SECS)
}

/// 云端 API Key 默认文件路径：~/.config/rg-cloud-api-key
fn cloud_api_key_file() -> std::path::PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from(".config"))
        .join("rg-cloud-api-key")
}

/// 云端 API Key 来源标识（优先级从高到低：env > 文件 > DB settings）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloudApiKeySource {
    /// 环境变量 RG_CLOUD_API_KEY
    Env,
    /// 本地文件 ~/.config/rg-cloud-api-key
    File,
    /// 数据库 settings 表（admin 配置页写入，key=cloud_api_key）
    Db,
}

impl CloudApiKeySource {
    pub fn as_str(&self) -> &'static str {
        match self {
            CloudApiKeySource::Env => "env",
            CloudApiKeySource::File => "file",
            CloudApiKeySource::Db => "db",
        }
    }
}

/// DB settings 层 Key 读取器注册表：由 main.rs 启动时注入（读 settings
/// 表 cloud_api_key），llm.rs 不直接依赖 AppState。注册前/注册失败时
/// DB 层视为无值（前两层行为与改造前逐字节一致）。
type DbKeyReader = dyn Fn() -> Option<String> + Send + Sync;
static CLOUD_KEY_DB_READER: OnceLock<Box<DbKeyReader>> = OnceLock::new();

/// 注册 DB settings 层 Key 读取器（仅首次注册生效）。
/// 读取器实现约束：① 短持锁、不在持锁期间做 LLM 调用；② 返回前
/// trim；③ 绝不把 Key 写入日志。
pub fn register_cloud_key_db_reader(reader: Box<DbKeyReader>) {
    if CLOUD_KEY_DB_READER.set(reader).is_err() {
        log::warn!(target: "llm", "cloud_key_db_reader_already_registered（忽略重复注册）");
    }
}

fn read_db_cloud_api_key() -> Option<String> {
    CLOUD_KEY_DB_READER
        .get()
        .and_then(|reader| reader())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// 解析云端 API Key（含来源）：优先级 env RG_CLOUD_API_KEY >
/// 文件 ~/.config/rg-cloud-api-key > DB settings（均 trim）；
/// 三层皆无/皆空时报错。Key 不落日志与代码。
fn resolve_cloud_api_key(
    env_value: Option<String>,
    file_path: &std::path::Path,
    db_value: Option<String>,
) -> Result<(String, CloudApiKeySource), String> {
    const MISSING_HINT: &str = "未配置云端 API Key：请设置环境变量 RG_CLOUD_API_KEY、文件 ~/.config/rg-cloud-api-key，或在管理后台「系统设置」中保存";
    if let Some(value) = env_value {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return Ok((trimmed.to_string(), CloudApiKeySource::Env));
        }
    }
    match std::fs::read_to_string(file_path) {
        Ok(content) => {
            // 剥离 UTF-8 BOM（U+FEFF，Windows 记事本保存的 Key 文件会携带），
            // 否则 BOM 混入 Key 导致云端 401 且报错不直观
            let trimmed = content.trim().trim_start_matches('\u{FEFF}').trim();
            if !trimmed.is_empty() {
                return Ok((trimmed.to_string(), CloudApiKeySource::File));
            }
        }
        Err(_) => {}
    }
    if let Some(value) = db_value {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return Ok((trimmed.to_string(), CloudApiKeySource::Db));
        }
    }
    Err(MISSING_HINT.to_string())
}

/// Key 解析纯函数版本（便于单测）
fn cloud_api_key_from(
    env_value: Option<String>,
    file_path: &std::path::Path,
    db_value: Option<String>,
) -> Result<String, String> {
    resolve_cloud_api_key(env_value, file_path, db_value).map(|(key, _source)| key)
}

/// 解析云端 API Key：优先 env RG_CLOUD_API_KEY，其次文件，最后 DB settings；
/// 三层皆无时报错。Key 不落日志与代码。
fn cloud_api_key() -> Result<String, String> {
    cloud_api_key_from(
        std::env::var("RG_CLOUD_API_KEY").ok(),
        &cloud_api_key_file(),
        read_db_cloud_api_key(),
    )
}

/// 当前生效 Key 的摘要（供 admin 配置页 GET 展示）：只返回来源与
/// 掩码，绝不返回明文。
pub fn cloud_api_key_status() -> (Option<CloudApiKeySource>, Option<String>) {
    match resolve_cloud_api_key(
        std::env::var("RG_CLOUD_API_KEY").ok(),
        &cloud_api_key_file(),
        read_db_cloud_api_key(),
    ) {
        Ok((key, source)) => (Some(source), Some(crate::db::setting::mask_secret(&key))),
        Err(_) => (None, None),
    }
}

/// 云端模型类别：聊天类函数用聊天模型，其余（extract_* / compress_context）用抽取模型
fn is_cloud_chat_fn(fn_name: &str) -> bool {
    matches!(fn_name, "general_chat" | "profile_qa_chat" | "generate_profile_document")
}

/// 云端模型类别（P1-7 场景化）：聊天类函数用 chat 场景模型，
/// compress_context 用 summarize 场景模型（默认与 extract 同源），
/// 其余抽取类用 extract 场景模型
fn cloud_model_for(fn_name: &str) -> String {
    if is_cloud_chat_fn(fn_name) {
        cloud_chat_model()
    } else if fn_name == "compress_context" {
        cloud_summarize_model()
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

// ---------- Provider trait 与实现（P0-1：通道分发结构收敛） ----------
//
// 设计概览：
// - trait LlmProvider 收敛三个通道共性能力：单 prompt 非流式（generate）、
//   messages 非流式（chat）、messages 流式（chat_stream），以及通道标识与
//   聊天模型路由元数据；接口 dyn-safe，经 Box<dyn LlmProvider> 动态分发；
// - OllamaProvider：Ollama 本地实现，覆盖 legacy / rig 两个通道（见下）；
// - OpenAiCompatProvider：OpenAI 兼容端点实现，即 cloud 通道（阿里云百炼
//   兼容端点，经 rig openai CompletionsClient）；
// - provider_for / provider_for_channel 是唯一通道分发入口，替代原散落在
//   call_generate_once / general_chat / general_chat_stream / chat_model_route /
//   general_chat_backend 五处的 Channel match 分发；重试语义不变（非流式
//   调用由 retry_transient 在外层包装，流式链路不重试）。
//
// legacy 通道决策（P0-1）：保留 legacy 通道，将其收编为 OllamaProvider 的
// Legacy 模式——它是 Ollama provider 的 Ollama 私有协议实现体
//（/api/generate + format:"json" 单 prompt，多轮时 /api/chat），向后兼容
// 存量部署的默认行为（RG_LLM_BACKEND=legacy 且未配置白名单时全部函数走
// 原生 API）；rig 通道是同一 Ollama 服务的 rig-core 路径（Rig 模式）。
// 不删除任何通道，不改变 RG_LLM_BACKEND / RG_LLM_RIG_FNS /
// RG_LLM_CLOUD_FNS 开关语义。Agent 工具循环（cloud_agent_stream）是
// cloud 专属 Function Calling 能力，Ollama 通道无工具能力（§8.2-3），
// 不纳入 trait，保留独立实现。

/// 对外流式输出流类型（SSE 契约：Reasoning → thinking_delta；
/// Text → text_delta；Done → done）
pub type ChatEventStream =
    std::pin::Pin<Box<dyn futures::Stream<Item = Result<ChatStreamEvent, String>> + Send>>;

/// 统一 Provider 接口：通道能力抽象层（dyn-safe）。所有方法均不含重试，
/// 重试/不重试语义由调用方按「非流式重试、流式不重试」约定决定。
trait LlmProvider: Send + Sync {
    /// 通道标识（"legacy" / "rig" / "cloud"），供 SSE routing 事件
    fn channel_name(&self) -> &'static str;
    /// 按 web_search 路由的聊天模型名（供 SSE llm_call 事件）
    fn chat_model(&self, web_search: bool) -> String;
    /// 单 prompt 非流式调用
    fn generate<'a>(
        &'a self,
        fn_name: &'a str,
        prompt: &'a str,
        format: Option<&'a str>,
        timeout: Duration,
        web_search: bool,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, LlmError>> + Send + 'a>>;
    /// messages 非流式调用（system → 历史轮次 → 本轮问题）
    fn chat<'a>(
        &'a self,
        system: &'a str,
        history: &'a ChatHistory,
        query: &'a str,
        web_search: bool,
        timeout: Duration,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, LlmError>> + Send + 'a>>;
    /// messages 流式调用（实现体各自保持建连超时语义：Ollama 用
    /// RG_OLLAMA_CHAT_TIMEOUT_SECS，cloud 用 RG_CLOUD_TIMEOUT_SECS）
    fn chat_stream<'a>(
        &'a self,
        system: &'a str,
        history: &'a ChatHistory,
        query: &'a str,
        web_search: bool,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<ChatEventStream, String>> + Send + 'a>>;
}

/// Ollama 本地 Provider：Legacy 模式 = Ollama 原生 HTTP API（legacy 通道），
/// Rig 模式 = 经 rig-core 调用同一 Ollama 服务（rig 通道）
struct OllamaProvider {
    mode: OllamaMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OllamaMode {
    Legacy,
    Rig,
}

impl LlmProvider for OllamaProvider {
    fn channel_name(&self) -> &'static str {
        match self.mode {
            OllamaMode::Legacy => "legacy",
            OllamaMode::Rig => "rig",
        }
    }

    /// Ollama 通道无联网能力，不按 web_search 分流，固定本地模型
    fn chat_model(&self, _web_search: bool) -> String {
        ollama_model()
    }

    fn generate<'a>(
        &'a self,
        fn_name: &'a str,
        prompt: &'a str,
        format: Option<&'a str>,
        timeout: Duration,
        _web_search: bool,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, LlmError>> + Send + 'a>> {
        let mode = self.mode;
        Box::pin(async move {
            match mode {
                // legacy 通道：Ollama 私有 /api/generate + format（向后兼容）
                OllamaMode::Legacy => ollama_generate_once(fn_name, prompt, format, timeout).await,
                OllamaMode::Rig => call_rig(fn_name, prompt, format, timeout).await,
            }
        })
    }

    fn chat<'a>(
        &'a self,
        system: &'a str,
        history: &'a ChatHistory,
        query: &'a str,
        _web_search: bool,
        timeout: Duration,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, LlmError>> + Send + 'a>> {
        let mode = self.mode;
        Box::pin(async move {
            match mode {
                // legacy 通道：reqwest 直连 Ollama /api/chat
                OllamaMode::Legacy => ollama_chat_messages(system, history, query, timeout).await,
                OllamaMode::Rig => rig_chat_messages(system, history, query, timeout).await,
            }
        })
    }

    fn chat_stream<'a>(
        &'a self,
        system: &'a str,
        history: &'a ChatHistory,
        query: &'a str,
        _web_search: bool,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<ChatEventStream, String>> + Send + 'a>> {
        // 两种模式共用同一实现：改造前 legacy/rig 通道的流式即统一经
        // rig-core（legacy 通道未集成 Ollama 原生 API 流式），行为原样保留；
        // 通道名透传给流内 usage 落库（P1-7）区分 channel 字段
        Box::pin(ollama_chat_stream(self.channel_name(), system, history, query))
    }
}

/// OpenAI 兼容端点 Provider（cloud 通道：阿里云百炼兼容端点）
struct OpenAiCompatProvider;

impl LlmProvider for OpenAiCompatProvider {
    fn channel_name(&self) -> &'static str {
        "cloud"
    }

    fn chat_model(&self, web_search: bool) -> String {
        cloud_chat_model_for(web_search)
    }

    fn generate<'a>(
        &'a self,
        fn_name: &'a str,
        prompt: &'a str,
        format: Option<&'a str>,
        timeout: Duration,
        web_search: bool,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, LlmError>> + Send + 'a>> {
        // 尊重调用方传入的 timeout（聊天类 120s、抽取类 45s），避免长调用
        //（generate_profile_document / compress_context）被默认值截断
        Box::pin(call_cloud(prompt, format, timeout, fn_name, web_search))
    }

    fn chat<'a>(
        &'a self,
        system: &'a str,
        history: &'a ChatHistory,
        query: &'a str,
        web_search: bool,
        timeout: Duration,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, LlmError>> + Send + 'a>> {
        Box::pin(cloud_chat_messages(system, history, query, web_search, timeout))
    }

    fn chat_stream<'a>(
        &'a self,
        system: &'a str,
        history: &'a ChatHistory,
        query: &'a str,
        web_search: bool,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<ChatEventStream, String>> + Send + 'a>> {
        Box::pin(cloud_chat_stream(system, history, query, web_search))
    }
}

/// 通道 → Provider（唯一分发入口）：legacy/rig 为 OllamaProvider 的两种
/// 模式，cloud 为 OpenAiCompatProvider
fn provider_for_channel(channel: Channel) -> Box<dyn LlmProvider> {
    match channel {
        Channel::Legacy => Box::new(OllamaProvider { mode: OllamaMode::Legacy }),
        Channel::Rig => Box::new(OllamaProvider { mode: OllamaMode::Rig }),
        Channel::Cloud => Box::new(OpenAiCompatProvider),
    }
}

/// 函数 → Provider：按函数级通道决策（llm_channel）选择实现体
fn provider_for(fn_name: &str) -> Box<dyn LlmProvider> {
    provider_for_channel(llm_channel(fn_name))
}

#[cfg(test)]
mod provider_dispatch_tests {
    use super::*;

    /// 通道 → Provider 映射与通道标识（唯一分发入口回归）
    #[test]
    fn provider_for_channel_maps_channels_to_providers() {
        assert_eq!(provider_for_channel(Channel::Legacy).channel_name(), "legacy");
        assert_eq!(provider_for_channel(Channel::Rig).channel_name(), "rig");
        assert_eq!(provider_for_channel(Channel::Cloud).channel_name(), "cloud");
    }

    /// 聊天模型路由收敛到 trait：仅 cloud 按 web_search 分流，
    /// Ollama 两通道固定本地模型（与改造前 chat_model_route 语义一致）
    #[test]
    fn provider_chat_model_routing() {
        let cloud = provider_for_channel(Channel::Cloud);
        assert_eq!(cloud.chat_model(true), cloud_search_model());
        assert_eq!(cloud.chat_model(false), cloud_chat_model());
        for channel in [Channel::Legacy, Channel::Rig] {
            let ollama = provider_for_channel(channel);
            assert_eq!(ollama.chat_model(true), ollama_model());
            assert_eq!(ollama.chat_model(false), ollama_model());
        }
    }

    /// OllamaProvider 模式标识：Legacy/Rig 两种模式通道名不同，
    /// 对应 RG_LLM_BACKEND 的 legacy / rig 两个取值
    #[test]
    fn ollama_provider_mode_channel_names() {
        let legacy = OllamaProvider { mode: OllamaMode::Legacy };
        let rig = OllamaProvider { mode: OllamaMode::Rig };
        assert_eq!(legacy.channel_name(), "legacy");
        assert_eq!(rig.channel_name(), "rig");
        // 两种模式均为本地模型路由
        assert_eq!(legacy.chat_model(false), rig.chat_model(false));
    }
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
        // env、文件、DB 三层均缺失 → 报错
        let err = cloud_api_key_from(None, &missing, None).unwrap_err();
        assert!(err.contains("未配置云端 API Key"));
        // env 优先且 trim
        assert_eq!(cloud_api_key_from(Some("  sk-abc  ".to_string()), &missing, None).unwrap(), "sk-abc");
        // env 为空白 → 回退文件（trim）
        let file = std::env::temp_dir().join("rg-test-key-file");
        std::fs::write(&file, " sk-file \n").unwrap();
        assert_eq!(cloud_api_key_from(Some("   ".to_string()), &file, None).unwrap(), "sk-file");
        assert_eq!(cloud_api_key_from(None, &file, None).unwrap(), "sk-file");
        // 文件内容为空白 → 回退 DB；DB 也空白 → 报错
        std::fs::write(&file, "  \n").unwrap();
        assert_eq!(cloud_api_key_from(None, &file, Some(" sk-db ".to_string())).unwrap(), "sk-db");
        assert!(cloud_api_key_from(None, &file, Some("   ".to_string())).is_err());
        // Windows 记事本保存的 Key 文件携带 UTF-8 BOM（U+FEFF）→ 剥离后正常解析
        std::fs::write(&file, "\u{FEFF}sk-bom\n").unwrap();
        assert_eq!(cloud_api_key_from(None, &file, None).unwrap(), "sk-bom");
        // BOM + 空白包裹同样剥离
        std::fs::write(&file, "\u{FEFF}  sk-bom2  \n").unwrap();
        assert_eq!(cloud_api_key_from(None, &file, None).unwrap(), "sk-bom2");
        let _ = std::fs::remove_file(&file);
    }

    /// 三层优先级：env > 文件 > DB settings（既有两层行为不变，
    /// DB 仅作兜底来源）
    #[test]
    fn cloud_api_key_priority_env_over_file_over_db() {
        let file = std::env::temp_dir().join("rg-test-key-file-priority");
        std::fs::write(&file, "sk-file\n").unwrap();
        let db = Some("sk-sp-dbkey".to_string());

        // 三层同时存在 → env 胜出
        let (key, source) = resolve_cloud_api_key(Some("sk-env".to_string()), &file, db.clone()).unwrap();
        assert_eq!(key, "sk-env");
        assert_eq!(source, CloudApiKeySource::Env);
        assert_eq!(source.as_str(), "env");

        // env 缺失 → 文件胜出（即使 DB 有值）
        let (key, source) = resolve_cloud_api_key(None, &file, db.clone()).unwrap();
        assert_eq!(key, "sk-file");
        assert_eq!(source, CloudApiKeySource::File);
        assert_eq!(source.as_str(), "file");

        // env 与文件均缺失 → DB 胜出（trim）
        let missing = std::env::temp_dir().join("rg-test-nonexistent-priority");
        let _ = std::fs::remove_file(&missing);
        let (key, source) = resolve_cloud_api_key(Some("  ".to_string()), &missing, db.clone()).unwrap();
        assert_eq!(key, "sk-sp-dbkey");
        assert_eq!(source, CloudApiKeySource::Db);
        assert_eq!(source.as_str(), "db");

        // 三层全空 → 报错（错误文案提示三个来源，不泄露任何 Key 内容）
        assert!(resolve_cloud_api_key(None, &missing, None).is_err());
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
        assert_eq!(cloud_model_for("compress_context"), "qwen3.6-flash");
    }
}

#[cfg(test)]
mod model_routing_tests {
    use super::*;

    /// 回归防护（d9fe0de）：联网搜索请求必须路由到搜索模型而非聊天模型。
    /// 背景：百炼平台侧对 qwen3.7-plus 静默忽略 enable_search，若误回退为
    /// 单一聊天模型，联网搜索会静默失效。用相对断言（对比 getter）兼容 env 覆盖。
    #[test]
    fn cloud_chat_model_for_routes_to_search_model() {
        assert_eq!(cloud_chat_model_for(true), cloud_search_model());
        assert_eq!(cloud_chat_model_for(false), cloud_chat_model());
    }

    /// 默认值基线（仅在对应 env 未设置时断言，避免并发测试改 env 竞态）：
    /// Token Plan 网关上搜索/聊天统一 qwen3.7-plus（网关支持 enable_search）；
    /// 路由机制本身由相对断言测试保障，两者同值时路由退化为恒等不影响正确性
    #[test]
    fn search_model_default_baseline() {
        if std::env::var("RG_CLOUD_SEARCH_MODEL").is_ok() {
            return;
        }
        assert_eq!(cloud_search_model(), "qwen3.7-plus");
    }

    /// 通道路由语义：仅 cloud 通道按 web_search 分流，其余通道固定本地 Ollama
    #[test]
    fn chat_model_route_channel_semantics() {
        assert_eq!(chat_model_route(Channel::Cloud, true), cloud_search_model());
        assert_eq!(chat_model_route(Channel::Cloud, false), cloud_chat_model());
        assert_eq!(chat_model_route(Channel::Legacy, true), ollama_model());
        assert_eq!(chat_model_route(Channel::Legacy, false), ollama_model());
        assert_eq!(chat_model_route(Channel::Rig, true), ollama_model());
        assert_eq!(chat_model_route(Channel::Rig, false), ollama_model());
    }
}

#[cfg(test)]
mod scenario_model_tests {
    use super::*;

    /// 解析优先级纯函数：env > DB > 默认（不依赖真实 env，无并发竞态）
    #[test]
    fn resolve_scenario_model_priority_order() {
        // 全空 → 默认
        assert_eq!(resolve_scenario_model(None, &[None], "def"), "def");
        // 仅 DB 值 → 取 DB
        assert_eq!(resolve_scenario_model(Some("db-model"), &[None], "def"), "db-model");
        // env 值覆盖 DB
        let env = [Some("env-model".to_string())];
        assert_eq!(resolve_scenario_model(Some("db-model"), &env, "def"), "env-model");
        // env 空白/空白串不算覆盖，回退 DB
        let blank = [Some("  ".to_string()), None];
        assert_eq!(resolve_scenario_model(Some("db-model"), &blank, "def"), "db-model");
        // 多 env 键取第一个非空（且 trim）
        let multi = [Some(" ".to_string()), Some(" second ".to_string())];
        assert_eq!(resolve_scenario_model(None, &multi, "def"), "second");
        // DB 空白同样回退默认
        assert_eq!(resolve_scenario_model(Some("   "), &[None], "def"), "def");
    }

    /// 场景键 ↔ 枚举双向映射齐备，且与 db::model_config 元数据一致
    #[test]
    fn scenario_keys_align_with_db_metadata() {
        let keys = ["local", "chat", "chat_search", "extract", "summarize"];
        for key in keys {
            let scenario = ModelScenario::from_key(key).expect("已知场景应可解析");
            assert_eq!(scenario.as_str(), key);
            assert_eq!(
                scenario.default_model(),
                crate::db::model_config::default_model_for(key),
                "默认模型应与 SCENARIO_METAS 同源"
            );
        }
        assert!(ModelScenario::from_key("unknown").is_none());
    }

    /// fn → 场景映射：聊天类/联网搜索 → chat；compress_context →
    /// summarize；其余抽取类 → extract
    #[test]
    fn scenario_for_fn_mapping() {
        assert_eq!(scenario_for_fn("general_chat", false), "chat");
        assert_eq!(scenario_for_fn("extract_any", true), "chat");
        assert_eq!(scenario_for_fn("compress_context", false), "summarize");
        assert_eq!(scenario_for_fn("extract_contact_fields", false), "extract");
        assert_eq!(scenario_for_fn("whatever_else", false), "extract");
    }

    /// 未配置（DB 无行且 env 未设置）时 getter 与改造前 env 路由默认
    /// 逐一致（仅在对应 env 未设置时断言，避免并发改 env 竞态）
    #[test]
    fn getter_default_baseline_without_env() {
        if std::env::var("RG_CLOUD_CHAT_MODEL").is_ok() {
            return;
        }
        assert_eq!(cloud_chat_model(), "qwen3.7-plus");
        if std::env::var("RG_CLOUD_EXTRACT_MODEL").is_ok() {
            return;
        }
        assert_eq!(cloud_extract_model(), "qwen3.6-flash");
        // summarize 与 extract 共用 env 覆盖键（向后兼容），同为 qwen3.6-flash
        assert_eq!(cloud_summarize_model(), "qwen3.6-flash");
        if std::env::var("RG_OLLAMA_MODEL").is_ok() {
            return;
        }
        assert_eq!(ollama_model(), "qwen2.5:7b");
    }

    /// summarize 与 extract 共用 RG_CLOUD_EXTRACT_MODEL 覆盖键（向后兼容决策）
    #[test]
    fn summarize_shares_extract_env_key() {
        assert_eq!(ModelScenario::Summarize.env_keys(), &["RG_CLOUD_EXTRACT_MODEL"]);
        assert_eq!(ModelScenario::Extract.env_keys(), &["RG_CLOUD_EXTRACT_MODEL"]);
    }

    /// rig 响应 usage 提取：全 0 → (None, None)，非 0 → Some
    #[test]
    fn rig_usage_extraction_zero_means_absent() {
        use rig_core::completion::CompletionResponse;
        let make = |input: u64, output: u64| {
            let mut usage = rig_core::completion::Usage::new();
            usage.input_tokens = input;
            usage.output_tokens = output;
            usage.total_tokens = input + output;
            CompletionResponse::<()> {
                choice: rig_core::OneOrMany::one(rig_core::completion::AssistantContent::Text(
                    rig_core::completion::message::Text::new(""),
                )),
                usage,
                raw_response: (),
                message_id: None,
            }
        };
        assert_eq!(usage_from_rig_response(&make(0, 0)), (None, None));
        assert_eq!(usage_from_rig_response(&make(10, 5)), (Some(10), Some(5)));
        assert_eq!(usage_from_rig_response(&make(0, 5)), (None, Some(5)));
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

/// 通过 rig-core 调用 Ollama /api/chat，与 legacy 通道（ollama_generate_once）平行。
/// 超时与错误文案与 legacy 通道保持一致。
/// format 为 Some 时设置宽松 JSON Schema 约束（Ollama provider 会将
/// output_schema 映射为请求体的 format 字段），等价于 legacy 的 format:"json"。
/// 错误按 LlmError 分类（P1-6）：超时/连接类为 Transient，其余 Permanent。
/// 成功时 usage 落库（P1-7，仅元数据）。
async fn call_rig(fn_name: &str, prompt: &str, format: Option<&str>, timeout: Duration) -> Result<String, LlmError> {
    let client = rig_client().map_err(LlmError::Permanent)?;
    let model_name = ollama_model();
    let started = Instant::now();
    info!(
        target: "llm",
        "rig_request url={} model={} prompt_len={} format={:?}",
        ollama_url(),
        model_name,
        prompt.len(),
        format
    );

    let model = client.completion_model(&model_name);
    let request_builder = model
        .completion_request(prompt)
        // P1-5：输出上限（rig ollama provider 将 max_tokens 映射为
        // options.num_predict）
        .max_tokens(max_output_tokens());
    // P1-5：num_ctx 经 additional_params 合入 options（rig ollama provider
    // 将非顶层保留参数合并进 options 对象）
    let request_builder =
        request_builder.additional_params(serde_json::json!({"num_ctx": ollama_num_ctx()}));
    let request = if format.is_some() {
        let schema: schemars::Schema = serde_json::from_value(serde_json::json!({"type": "object"}))
            .map_err(|e| LlmError::Permanent(format!("Schema build error: {}", e)))?;
        request_builder.output_schema(schema).build()
    } else {
        request_builder.build()
    };

    match tokio::time::timeout(timeout, model.completion(request)).await {
        Ok(Ok(response)) => {
            let (prompt_tokens, completion_tokens) = usage_from_rig_response(&response);
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
                Err(LlmError::Permanent("Empty response from Ollama".to_string()))
            } else {
                // usage 落库（P1-7）：rig 未回传 token 时为 None；只落元数据
                record_usage(LlmUsageRecord {
                    scenario: scenario_for_fn(fn_name, false),
                    channel: "rig",
                    model: model_name,
                    fn_name: fn_name.to_string(),
                    prompt_tokens,
                    completion_tokens,
                    elapsed_ms: started.elapsed().as_millis() as u64,
                });
                Ok(text)
            }
        }
        Ok(Err(e)) => Err(classify_transport_error(&format!("Ollama request failed: {}", e))),
        Err(_) => Err(LlmError::Transient(format!(
            "Ollama 请求超时：模型 {} 在 {} 秒内未返回结果。你可以稍后重试，或提高环境变量 RG_OLLAMA_CHAT_TIMEOUT_SECS / RG_OLLAMA_TIMEOUT_SECS。",
            model_name,
            timeout.as_secs()
        ))),
    }
}

// ---------- cloud 通道（阿里云百炼，OpenAI 兼容端点） ----------

/// 缓存云端 CompletionsClient（必须用 CompletionsClient：默认 Client 走
/// Responses API，百炼兼容端点会 404）。构建失败/Key 缺失不缓存，下次调用可重试。
/// 静态句柄提升到模块级，便于 admin 更新/清除 DB 中的 Key 后失效缓存。
static CLOUD_CLIENT: Mutex<Option<openai::CompletionsClient>> = Mutex::new(None);

fn cloud_client() -> Result<openai::CompletionsClient, String> {
    let mut guard = CLOUD_CLIENT
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

/// 失效已缓存的云端客户端：admin 在 DB 中更新/清除 API Key 后调用，
/// 下次 cloud 调用按最新 Key 重建客户端。锁异常时静默降级（下次
/// 客户端重建自然重新读 Key，不阻断 admin 操作）。
pub fn invalidate_cloud_client() {
    if let Ok(mut guard) = CLOUD_CLIENT.lock() {
        *guard = None;
    }
}

/// 通过 rig openai CompletionsClient 调用百炼兼容端点（非流式）。
/// timeout 尊重调用方传入值（聊天类 120s / 抽取类 45s）；RG_CLOUD_TIMEOUT_SECS
/// 仅作 cloud 流式建连默认值，不覆盖此处。
/// 模型按 fn 类别选择：聊天类 → RG_CLOUD_CHAT_MODEL（默认 qwen3.7-plus），
/// 抽取/压缩类 → RG_CLOUD_EXTRACT_MODEL（默认 qwen3.6-flash）。
/// format=Some 时经 additional_params 注入 response_format=json_object
///（不用 output_schema 的 json_schema strict 模式，宽松 schema 可能被拒）。
/// web_search=true 时额外合入百炼 enable_search + search_strategy=turbo
///（思考开关保持现状：非流式聊天类仍显式 enable_thinking=false）。
/// 日志仅记元数据（model/fn/prompt_len/elapsed/web_search），不落内容。
/// 错误按 LlmError 分类（P1-6）：超时/传输层瞬时错误为 Transient。
async fn call_cloud(
    prompt: &str,
    format: Option<&str>,
    timeout: Duration,
    fn_name: &str,
    web_search: bool,
) -> Result<String, LlmError> {
    let client = cloud_client().map_err(LlmError::Permanent)?;
    // 聊天类函数开联网搜索时路由到搜索可用模型，其余按 fn 类别选模型
    let model_name = if web_search && is_cloud_chat_fn(fn_name) {
        cloud_search_model()
    } else {
        cloud_model_for(fn_name)
    };
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
    let request_builder = model
        .completion_request(prompt)
        // P1-5：输出上限（rig openai provider 映射为请求体 max_tokens）
        .max_tokens(max_output_tokens());
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
                Err(LlmError::Permanent("云端模型返回内容为空".to_string()))
            } else {
                // usage 落库（P1-7）：rig CompletionResponse 的 usage 由 provider
                // 从百炼响应映射（未回传时为 None）；只落元数据，不落内容
                let (prompt_tokens, completion_tokens) = usage_from_rig_response(&response);
                record_usage(LlmUsageRecord {
                    scenario: scenario_for_fn(fn_name, web_search),
                    channel: "cloud",
                    model: model_name,
                    fn_name: fn_name.to_string(),
                    prompt_tokens,
                    completion_tokens,
                    elapsed_ms: started.elapsed().as_millis() as u64,
                });
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
            Err(classify_transport_error(&format!(
                "云端请求失败（模型 {}）：{}",
                model_name, e
            )))
        }
        Err(_) => Err(LlmError::Transient(format!(
            "云端请求超时：模型 {} 在 {} 秒内未返回结果。你可以稍后重试，或提高环境变量 RG_CLOUD_TIMEOUT_SECS。",
            model_name,
            timeout.as_secs()
        ))),
    }
}

/// 多轮对话消息序列构建（messages 模式通用）：
/// system（角色+技能+文档，摘要已拼入）→ 历史轮次 → 本轮 user 问题。
fn chat_message_sequence(
    system: &str,
    history: &ChatHistory,
    query: &str,
) -> Vec<rig_core::completion::Message> {
    let mut messages = vec![rig_core::completion::Message::system(system.to_string())];
    for (role, content) in &history.turns {
        match role.as_str() {
            "user" => messages.push(rig_core::completion::Message::user(content.clone())),
            "assistant" => messages.push(rig_core::completion::Message::assistant(content.clone())),
            _ => {} // 历史组装层已过滤其他角色，此处双重防护
        }
    }
    messages.push(rig_core::completion::Message::user(query.to_string()));
    messages
}

/// messages 模式的 system prompt：角色+技能+文档 →（如有）[对话摘要]。
/// 注入顺序（计划 §2-4）：角色+技能+文档 → 摘要 → 最近历史轮次 → 本轮 query。
fn build_system_with_summary(
    skills: &str,
    web_search: bool,
    documents: &str,
    summary: Option<&str>,
) -> String {
    let mut system = general_chat_system_prompt(skills, web_search, documents);
    if let Some(s) = summary.map(str::trim).filter(|s| !s.is_empty()) {
        system.push_str(&format!("\n\n[对话摘要] {}", s));
    }
    system
}

/// general_chat_stream 的云端 messages 版本：消息序列 = system → 历史轮次 →
/// 本轮问题；百炼聊天模型开思考（enable_thinking=true），web_search 时合入
/// enable_search；事件映射与旧版一致。
async fn cloud_chat_stream(
    system: &str,
    history: &ChatHistory,
    query: &str,
    web_search: bool,
) -> Result<
    std::pin::Pin<Box<dyn futures::Stream<Item = Result<ChatStreamEvent, String>> + Send>>,
    String,
> {
    let client = cloud_client()?;
    // web_search 路由到搜索可用模型（qwen3.7-plus 平台侧已忽略 enable_search）
    let model_name = cloud_chat_model_for(web_search);
    let started = Instant::now();
    // 超时对齐聊天超时语义（默认 120s，RG_CLOUD_TIMEOUT_SECS 可覆盖）：
    // 同时覆盖建流阶段与流消费阶段每次 stream.next() 的兑底，
    // 避免云端公网链路中途 stall 时 SSE 无限挂起
    let timeout = cloud_timeout();
    info!(
        target: "llm",
        "cloud_stream_request url={} model={} system_len={} history_turns={} web_search={}",
        cloud_base_url(),
        model_name,
        system.len(),
        history.turns.len(),
        web_search
    );

    let model = client.completion_model(&model_name);
    // 联网搜索开启时在思考参数基础上合入百炼 enable_search 私有参数；
    // max_tokens 为 OpenAI 兼容标准参数，经 builder 注入（P1-5）
    let additional = if web_search {
        serde_json::json!({
            "enable_thinking": true,
            "enable_search": true,
            "search_options": {"search_strategy": "turbo"}
        })
    } else {
        serde_json::json!({"enable_thinking": true})
    };
    let mut messages = chat_message_sequence(system, history, query);
    let last = messages
        .pop()
        .ok_or_else(|| "消息序列为空".to_string())?;
    let request = model
        .completion_request(last)
        .messages(messages)
        .max_tokens(max_output_tokens())
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
        let model_name = model_name.clone();
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
                        // usage 落库（P1-7）：provider 回了 usage 才记录；elapsed 取
                        // 建流到 Final 的整流耗时；只落元数据，不落内容
                        if let Some((input_tokens, output_tokens)) = usage {
                            record_usage(LlmUsageRecord {
                                scenario: scenario_for_fn("general_chat", web_search),
                                channel: "cloud",
                                model: model_name.clone(),
                                fn_name: "general_chat".to_string(),
                                prompt_tokens: Some(input_tokens),
                                completion_tokens: Some(output_tokens),
                                elapsed_ms: started.elapsed().as_millis() as u64,
                            });
                        }
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
    // web_search 路由到搜索可用模型（与 cloud_chat_stream 同策略）
    let model_name = cloud_chat_model_for(web_search);
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
        // P1-5：输出上限（含思考 token 的总预算，百炼 max_tokens 语义）
        .max_tokens(max_output_tokens())
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
    /// 当前轮使用的模型名（供 usage 落库，P1-7）
    model: String,
    /// 当前轮流开始时刻（usage 落库的耗时口径，P1-7）
    round_started: Instant,
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
    history: &ChatHistory,
) -> Result<AgentEventStream, String> {
    // 多轮历史注入：摘要拼入 system 尾部，历史轮次置于 system 与本轮问题之间
    let mut system_prompt = system_prompt;
    if let Some(s) = history.summary.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        system_prompt.push_str(&format!("\n\n[对话摘要] {}", s));
    }
    let mut messages = vec![rig_core::completion::Message::system(system_prompt)];
    for (role, content) in &history.turns {
        match role.as_str() {
            "user" => messages.push(rig_core::completion::Message::user(content.clone())),
            "assistant" => messages.push(rig_core::completion::Message::assistant(content.clone())),
            _ => {}
        }
    }
    messages.push(rig_core::completion::Message::user(query));
    let round_model = cloud_chat_model_for(web_search);
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
        model: round_model,
        round_started: Instant::now(),
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
                    ctx.round_started = Instant::now();
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
                        // usage 落库（P1-7）：工具循环每轮流式各记一条（多轮
                        // 累计需自行求和）；只落元数据，不落内容
                        if let Some((input_tokens, output_tokens)) = ctx.last_usage {
                            record_usage(LlmUsageRecord {
                                scenario: ModelScenario::Chat.as_str(),
                                channel: "cloud",
                                model: ctx.model.clone(),
                                fn_name: "agent_tool_loop".to_string(),
                                prompt_tokens: Some(input_tokens),
                                completion_tokens: Some(output_tokens),
                                elapsed_ms: ctx.round_started.elapsed().as_millis() as u64,
                            });
                        }
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
    history: &ChatHistory,
) -> Result<String, String> {
    use futures::StreamExt;
    let stream = cloud_agent_stream(system_prompt, query, web_search, state, owner_id, max_turns, history).await?;
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
/// 仅含角色+技能+文档（不含用户问题）；messages 模式下作为 system 消息，
/// 单 prompt 模式下由 general_chat_prompt 追加“用户问题：”尾段。
fn general_chat_system_prompt(skills: &str, web_search: bool, documents: &str) -> String {
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
    prompt
}

/// 单 prompt 模式的完整提示词（与改造前逐字节一致）
fn general_chat_prompt(query: &str, skills: &str, web_search: bool, documents: &str) -> String {
    format!(
        "{}\n\n用户问题：{}",
        general_chat_system_prompt(skills, web_search, documents),
        query
    )
}

/// 当前 general_chat 实际走的通道（"rig" / "cloud" / "legacy"），供 SSE 端点发 routing 事件
///（通道标识由 provider trait 统一提供，P0-1）
pub fn general_chat_backend() -> &'static str {
    provider_for("general_chat").channel_name()
}

/// 当前对话模型名，供 SSE 端点发 llm_call 事件：cloud 通道且 web_search
/// 时返回搜索模型（实际路由），其余返回聊天模型
pub fn general_chat_model_for(web_search: bool) -> String {
    chat_model_route(llm_channel("general_chat"), web_search)
}

pub fn general_chat_model() -> String {
    general_chat_model_for(false)
}

pub async fn general_chat(
    query: &str,
    skills: &str,
    web_search: bool,
    documents: &str,
    history: &ChatHistory,
) -> Result<String, String> {
    let timeout = ollama_timeout("RG_OLLAMA_CHAT_TIMEOUT_SECS", DEFAULT_OLLAMA_CHAT_TIMEOUT_SECS);
    if history.is_empty() {
        // 无历史：行为与改造前逐字节一致（单 prompt + provider 三通道分发）
        let prompt = general_chat_prompt(query, skills, web_search, documents);
        return call_generate(&prompt, None, timeout, "general_chat", web_search).await;
    }
    // 有历史：messages 数组化（system → 摘要 → 历史轮次 → 本轮问题），
    // 经 provider trait 分发：cloud 百炼 / rig Ollama（经 rig）/ legacy Ollama /api/chat；
    // 非流式调用同样套 P1-6 指数退避重试（仅瞬时错误）
    let system = build_system_with_summary(skills, web_search, documents, history.summary.as_deref());
    let provider = provider_for("general_chat");
    retry_transient("general_chat", || {
        provider.chat(&system, history, query, web_search, timeout)
    })
    .await
}

/// messages 模式的非流式聊天（cloud 通道）：对齐 call_cloud 参数语义
///（聊天类 enable_thinking=false 避免 400；web_search 合入 enable_search），
/// 消息序列 = system → 历史轮次 → 本轮问题。
/// 错误按 LlmError 分类（P1-6）。
async fn cloud_chat_messages(
    system: &str,
    history: &ChatHistory,
    query: &str,
    web_search: bool,
    timeout: Duration,
) -> Result<String, LlmError> {
    let client = cloud_client().map_err(LlmError::Permanent)?;
    let model_name = if web_search { cloud_search_model() } else { cloud_chat_model() };
    let started = Instant::now();
    info!(
        target: "llm",
        "cloud_messages_request url={} model={} fn=general_chat system_len={} history_turns={} web_search={}",
        cloud_base_url(),
        model_name,
        system.len(),
        history.turns.len(),
        web_search
    );

    let model = client.completion_model(&model_name);
    let mut params = serde_json::Map::new();
    // 百炼思考模型非流式请求显式关闭思考避免 400（与 call_cloud 一致）
    params.insert("enable_thinking".to_string(), serde_json::json!(false));
    if web_search {
        params.insert("enable_search".to_string(), serde_json::json!(true));
        params.insert(
            "search_options".to_string(),
            serde_json::json!({"search_strategy": "turbo"}),
        );
    }
    let mut messages = chat_message_sequence(system, history, query);
    let last = messages.pop().expect("消息序列非空");
    let request = model
        .completion_request(last)
        .messages(messages)
        // P1-5：输出上限
        .max_tokens(max_output_tokens())
        .additional_params(serde_json::Value::Object(params))
        .build();

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
                "cloud_messages_response model={} fn=general_chat elapsed_ms={} text_chars={}",
                model_name,
                started.elapsed().as_millis(),
                text.chars().count()
            );
            if text.is_empty() {
                Err(LlmError::Permanent("云端模型返回内容为空".to_string()))
            } else {
                // usage 落库（P1-7，仅元数据）
                let (prompt_tokens, completion_tokens) = usage_from_rig_response(&response);
                record_usage(LlmUsageRecord {
                    scenario: scenario_for_fn("general_chat", web_search),
                    channel: "cloud",
                    model: model_name,
                    fn_name: "general_chat".to_string(),
                    prompt_tokens,
                    completion_tokens,
                    elapsed_ms: started.elapsed().as_millis() as u64,
                });
                Ok(text)
            }
        }
        Ok(Err(e)) => Err(classify_transport_error(&format!(
            "云端请求失败（模型 {}）：{}",
            model_name, e
        ))),
        Err(_) => Err(LlmError::Transient(format!(
            "云端请求超时：模型 {} 在 {} 秒内未返回结果。你可以稍后重试，或提高环境变量 RG_CLOUD_TIMEOUT_SECS。",
            model_name,
            timeout.as_secs()
        ))),
    }
}

/// messages 模式的非流式聊天（rig 通道，Ollama）：与 call_rig 平行，
/// 消息序列经 chat_history + prompt 重建，错误文案与 legacy 通道一致。
/// 错误按 LlmError 分类（P1-6）。
async fn rig_chat_messages(
    system: &str,
    history: &ChatHistory,
    query: &str,
    timeout: Duration,
) -> Result<String, LlmError> {
    let client = rig_client().map_err(LlmError::Permanent)?;
    let model_name = ollama_model();
    let started = Instant::now();
    info!(
        target: "llm",
        "rig_messages_request url={} model={} system_len={} history_turns={}",
        ollama_url(),
        model_name,
        system.len(),
        history.turns.len()
    );

    let model = client.completion_model(&model_name);
    let mut messages = chat_message_sequence(system, history, query);
    let last = messages.pop().expect("消息序列非空");
    let request = model
        .completion_request(last)
        .messages(messages)
        // P1-5：输出上限 + num_ctx（rig ollama provider：max_tokens →
        // options.num_predict；num_ctx 经 additional_params 合入 options）
        .max_tokens(max_output_tokens())
        .additional_params(serde_json::json!({"num_ctx": ollama_num_ctx()}))
        .build();

    match tokio::time::timeout(timeout, model.completion(request)).await {
        Ok(Ok(response)) => {
            let (prompt_tokens, completion_tokens) = usage_from_rig_response(&response);
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
                Err(LlmError::Permanent("Empty response from Ollama".to_string()))
            } else {
                // usage 落库（P1-7，仅元数据）
                record_usage(LlmUsageRecord {
                    scenario: ModelScenario::Chat.as_str(),
                    channel: "rig",
                    model: model_name,
                    fn_name: "general_chat".to_string(),
                    prompt_tokens,
                    completion_tokens,
                    elapsed_ms: started.elapsed().as_millis() as u64,
                });
                Ok(text)
            }
        }
        Ok(Err(e)) => Err(classify_transport_error(&format!("Ollama request failed: {}", e))),
        Err(_) => Err(LlmError::Transient(format!(
            "Ollama 请求超时：模型 {} 在 {} 秒内未返回结果。你可以稍后重试，或提高环境变量 RG_OLLAMA_CHAT_TIMEOUT_SECS / RG_OLLAMA_TIMEOUT_SECS。",
            model_name,
            timeout.as_secs()
        ))),
    }
}

/// Ollama /api/chat messages 格式（legacy 降级路径多轮支持）：
/// 单 prompt 的 /api/generate 无法表达消息序列，多轮历史时改走 /api/chat。
/// HTTP Client 复用 P2-11 全局单例，超时经 RequestBuilder::timeout 按请求指定。
#[derive(Serialize, Deserialize)]
struct OllamaChatMessage {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct OllamaChatRequest {
    model: String,
    stream: bool,
    messages: Vec<OllamaChatMessage>,
    /// 模型参数（P1-5：num_ctx / num_predict），始终携带
    options: serde_json::Value,
}

#[derive(Deserialize)]
struct OllamaChatResponse {
    message: Option<OllamaChatMessage>,
    /// 用量元数据（P1-7）：Ollama /api/chat 回传 token 计数，可能缺失 → None
    #[serde(default)]
    prompt_eval_count: Option<u64>,
    #[serde(default)]
    eval_count: Option<u64>,
}

async fn ollama_chat_messages(
    system: &str,
    history: &ChatHistory,
    query: &str,
    timeout: Duration,
) -> Result<String, LlmError> {
    let client = legacy_http_client();
    let started = Instant::now();

    let mut messages = vec![OllamaChatMessage {
        role: "system".to_string(),
        content: system.to_string(),
    }];
    for (role, content) in &history.turns {
        if role == "user" || role == "assistant" {
            messages.push(OllamaChatMessage {
                role: role.clone(),
                content: content.clone(),
            });
        }
    }
    messages.push(OllamaChatMessage {
        role: "user".to_string(),
        content: query.to_string(),
    });

    let req = OllamaChatRequest {
        model: ollama_model(),
        stream: false,
        messages,
        options: build_ollama_options(ollama_num_ctx(), max_output_tokens()),
    };
    let url = format!("{}/api/chat", ollama_url());
    info!(
        target: "llm",
        "ollama_chat_request url={} model={} messages={}",
        url,
        req.model,
        req.messages.len()
    );

    let resp = client
        .post(&url)
        .timeout(timeout)
        .json(&req)
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                LlmError::Transient(format!(
                    "Ollama 请求超时：模型 {} 在 {} 秒内未返回结果。你可以稍后重试，或提高环境变量 RG_OLLAMA_CHAT_TIMEOUT_SECS / RG_OLLAMA_TIMEOUT_SECS。",
                    req.model,
                    timeout.as_secs()
                ))
            } else if e.is_connect() || e.is_request() {
                LlmError::Transient(format!("Ollama request failed: {}", e))
            } else {
                LlmError::Permanent(format!("Ollama request failed: {}", e))
            }
        })?;

    if !resp.status().is_success() {
        let msg = format!("Ollama returned status {}", resp.status());
        return Err(if is_retryable_status(resp.status().as_u16()) {
            LlmError::Transient(msg)
        } else {
            LlmError::Permanent(msg)
        });
    }

    let data: OllamaChatResponse = resp.json().await.map_err(|e| LlmError::Permanent(format!("Parse error: {}", e)))?;
    let text = data
        .message
        .map(|m| m.content)
        .filter(|c| !c.is_empty())
        .ok_or_else(|| LlmError::Permanent("Empty response from Ollama".to_string()))?;
    // usage 落库（P1-7）：只落元数据，不落内容
    record_usage(LlmUsageRecord {
        scenario: ModelScenario::Chat.as_str(),
        channel: "legacy",
        model: req.model,
        fn_name: "general_chat".to_string(),
        prompt_tokens: data.prompt_eval_count.map(|v| v as usize),
        completion_tokens: data.eval_count.map(|v| v as usize),
        elapsed_ms: started.elapsed().as_millis() as u64,
    });
    Ok(text)
}

/// general_chat_stream 的流式输出事件：
/// Reasoning → thinking_delta；Text → text_delta；Done → done（usage 无则 None）
pub enum ChatStreamEvent {
    Reasoning(String),
    Text(String),
    Done(Option<(usize, usize)>),
}

/// general_chat 的流式版本：分发收敛到 provider trait（P0-1）——
/// cloud 走 OpenAiCompatProvider（百炼聊天模型开思考，web_search 时合入
/// enable_search），legacy/rig 走 OllamaProvider（经 rig-core 流式）。
/// SSE 事件契约不变（Reasoning → thinking_delta；Text → text_delta；
/// Done → done）；流式链路不重试。
pub async fn general_chat_stream(
    query: &str,
    skills: &str,
    web_search: bool,
    documents: &str,
    history: &ChatHistory,
) -> Result<ChatEventStream, String> {
    // messages 序列 = system（角色+技能+文档+摘要）→ 历史轮次 → 本轮问题；
    // 历史为空时 OllamaProvider 内部退化为单 prompt（与旧格式逐字节一致）
    let system = build_system_with_summary(skills, web_search, documents, history.summary.as_deref());
    provider_for("general_chat")
        .chat_stream(&system, history, query, web_search)
        .await
}

/// Ollama provider 的流式实现体（legacy/rig 通道共用，OllamaProvider::chat_stream）：
/// 通过 rig `model.stream()` 调用 Ollama /api/chat，将 StreamedAssistantContent::
/// Reasoning / ReasoningDelta 映射为 Reasoning 事件、Text 映射为 Text 事件、
/// Final 映射为 Done（usage 取 prompt_eval_count / eval_count，缺失时为 None）。
/// 复用 rig client 缓存、ollama_url()/ollama_model() 与聊天超时
///（RG_OLLAMA_CHAT_TIMEOUT_SECS），超时文案与 call_rig 一致。
/// 客户端断开时流自然 drop（rig stream 支持 cancel）。
/// channel 参数供 usage 落库区分 legacy / rig（P1-7）。
/// 注：改造前 legacy/rig 通道的流式即统一经 rig-core（legacy 通道未集成
/// Ollama 原生 API 流式），P0-1 仅做结构收敛，行为原样保留。
async fn ollama_chat_stream(
    channel: &'static str,
    system: &str,
    history: &ChatHistory,
    query: &str,
) -> Result<ChatEventStream, String> {
    let client = rig_client()?;
    let model_name = ollama_model();
    let started = Instant::now();
    let timeout = ollama_timeout("RG_OLLAMA_CHAT_TIMEOUT_SECS", DEFAULT_OLLAMA_CHAT_TIMEOUT_SECS);

    let model = client.completion_model(&model_name);
    // P1-5：输出上限（max_tokens → options.num_predict）+ num_ctx 上下文
    // 窗口（经 additional_params 合入 options），两个分支统一携带
    let request = if history.is_empty() {
        // 等价于改造前 general_chat_prompt(query, skills, web_search, documents)：
        // 历史为空时 summary 必为空（ChatHistory::is_empty 语义），system 即
        // 纯「角色+技能+文档」，追加「用户问题：」尾段后与旧格式逐字节一致
        let prompt = format!("{}\n\n用户问题：{}", system, query);
        info!(
            target: "llm",
            "rig_stream_request url={} model={} prompt_len={}",
            ollama_url(),
            model_name,
            prompt.len()
        );
        model
            .completion_request(prompt)
            .max_tokens(max_output_tokens())
            .additional_params(serde_json::json!({"num_ctx": ollama_num_ctx()}))
            .build()
    } else {
        info!(
            target: "llm",
            "rig_stream_request url={} model={} system_len={} history_turns={}",
            ollama_url(),
            model_name,
            system.len(),
            history.turns.len()
        );
        let mut messages = chat_message_sequence(system, history, query);
        let last = messages.pop().expect("消息序列非空");
        model
            .completion_request(last)
            .messages(messages)
            .max_tokens(max_output_tokens())
            .additional_params(serde_json::json!({"num_ctx": ollama_num_ctx()}))
            .build()
    };

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
    // move 捕获：映射闭包随返回的流存活，必须持有 model_name 所有权
    //（channel/started 均为 Copy）；usage 落库在 Final 分支（P1-7）
    let mapped = futures::stream::try_unfold(stream, move |mut stream| {
        let model_name = model_name.clone();
        async move {
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
                    // usage 落库（P1-7）：Ollama 回传了 token 计数才记录；
                    // 只落元数据，不落内容
                    if let Some((input_tokens, output_tokens)) = usage {
                        record_usage(LlmUsageRecord {
                            scenario: ModelScenario::Chat.as_str(),
                            channel,
                            model: model_name.clone(),
                            fn_name: "general_chat".to_string(),
                            prompt_tokens: Some(input_tokens),
                            completion_tokens: Some(output_tokens),
                            elapsed_ms: started.elapsed().as_millis() as u64,
                        });
                    }
                    return Ok(Some((ChatStreamEvent::Done(usage), stream)));
                }
                // ToolCall / ToolCallDelta / Unknown 等与通用聊天无关，忽略
                Some(Ok(_)) => continue,
                Some(Err(e)) => return Err(format!("Ollama 流式响应失败：{}", e)),
            }
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

    call_generate(
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

    call_generate(
        &prompt,
        None,
        ollama_timeout("RG_OLLAMA_CHAT_TIMEOUT_SECS", DEFAULT_OLLAMA_CHAT_TIMEOUT_SECS),
        "generate_profile_document",
        false,
    )
    .await
}

// ---------- JSON 结构化抽取（P1-6：纠错重试 + 显性报错） ----------

/// JSON 解析失败时的纠错重试 prompt（纯函数，可单测）：在原始提取 prompt
/// 尾部追加解析错误信息与纠错指令；原始 prompt 逐字节保留，首次抽取的
/// prompt 文本不受影响。
fn build_json_fix_prompt(original_prompt: &str, parse_error: &str) -> String {
    format!(
        "{}\n\n你上一次的输出不是合法的 JSON，解析错误：{}。请重新只输出一个合法的 JSON 对象，不要输出任何多余文字。",
        original_prompt, parse_error
    )
}

/// JSON 结构化抽取通用调用（P1-6）：
/// - 首次调用失败（超时/网络/HTTP 错误）由 call_generate 内部退避重试后仍失败 → 显性报错；
/// - 模型返回非法 JSON → 带解析错误信息拼入纠错 prompt 重试 1 次；
/// - 纠错后仍非法 → 显性报错（不再静默降级为空草稿/默认值）。
async fn call_json_extract(fn_name: &str, prompt: &str) -> Result<serde_json::Value, String> {
    let timeout = ollama_timeout("RG_OLLAMA_TIMEOUT_SECS", DEFAULT_OLLAMA_TIMEOUT_SECS);
    let raw = call_generate(prompt, Some("json"), timeout, fn_name, false)
        .await
        .map_err(|e| format!("模型调用失败：{}", e))?;
    match serde_json::from_str::<serde_json::Value>(&raw) {
        Ok(v) => Ok(v),
        Err(parse_err) => {
            // 日志只记元数据；serde_json 错误文案仅含行列位置，不含对话内容
            warn!(
                target: "llm",
                "json_extract_fix_retry fn={} reason=invalid_json",
                fn_name
            );
            let fix_prompt = build_json_fix_prompt(prompt, &parse_err.to_string());
            let raw = call_generate(&fix_prompt, Some("json"), timeout, fn_name, false)
                .await
                .map_err(|e| format!("模型调用失败：{}", e))?;
            serde_json::from_str::<serde_json::Value>(&raw)
                .map_err(|e| format!("模型返回的内容不是合法 JSON，抽取失败：{}", e))
        }
    }
}

#[cfg(test)]
mod json_fix_prompt_tests {
    use super::*;

    #[test]
    fn fix_prompt_preserves_original_and_appends_error() {
        let original = "请从以下文字中提取联系人信息，只输出JSON：\n文字：张三在上海";
        let fix = build_json_fix_prompt(original, "expected value at line 1 column 1");
        // 原始 prompt 逐字节保留在前
        assert!(fix.starts_with(original));
        // 解析错误信息拼入
        assert!(fix.contains("expected value at line 1 column 1"));
        // 含纠错指令关键词
        assert!(fix.contains("不是合法的 JSON"));
        assert!(fix.contains("只输出一个合法的 JSON 对象"));
    }

    #[test]
    fn fix_prompt_handles_empty_original() {
        let fix = build_json_fix_prompt("", "EOF while parsing a value at line 1 column 0");
        assert!(fix.starts_with("\n\n你上一次的输出不是合法的 JSON"));
        assert!(fix.contains("EOF while parsing"));
    }
}

/// 从自然语言中提取联系人字段（用于 create_person 意图）。
/// 失败时显性返回错误（P1-6），不再静默降级为空草稿。
pub async fn extract_person_fields(query: &str) -> Result<PersonDraft, String> {
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

    let v = call_json_extract("extract_person_fields", &prompt).await?;
    Ok(PersonDraft {
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
    })
}

/// 从自然语言中提取更新字段（用于 update_person 意图）。
/// 失败时显性返回错误（P1-6）。
pub async fn extract_update_fields(query: &str) -> Result<(String, Vec<FieldChange>), String> {
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

    let v = call_json_extract("extract_update_fields", &prompt).await?;
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
    Ok((name, changes))
}

/// 从自然语言中提取互动信息（用于 add_interaction 意图）。
/// 失败时显性返回错误（P1-6），不再静默降级为空草稿。
pub async fn extract_interaction_data(query: &str) -> Result<InteractionDraft, String> {
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

    let v = call_json_extract("extract_interaction_data", &prompt).await?;
    Ok(InteractionDraft {
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
    })
}

/// 从路径查询中提取目标人名。失败/未识别出人名时显性返回错误（P1-6）。
pub async fn extract_path_target(query: &str) -> Result<String, String> {
    let prompt = format!(
        r#"请从以下文字中提取目标人名（用户想查找与谁的关系路径），只输出JSON：
{{"target_name": "目标人名"}}

文字：{}"#,
        query
    );

    let v = call_json_extract("extract_path_target", &prompt).await?;
    v["target_name"]
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .ok_or_else(|| "未能从输入中识别出目标人名，请更明确地说明你想查找谁的关系路径".to_string())
}

/// 从自然语言中提取要删除的联系人名（用于 delete_person 意图）。
/// 失败/未识别出人名时显性返回错误（P1-6），调用方决定是否规则兜底。
pub async fn extract_delete_target(query: &str) -> Result<String, String> {
    let prompt = format!(
        r#"请从以下文字中提取要删除的联系人姓名，只输出JSON：
{{"target_name": "人名"}}

文字：{}"#,
        query
    );

    let v = call_json_extract("extract_delete_target", &prompt).await?;
    v["target_name"]
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .ok_or_else(|| "未能从输入中识别出要删除的联系人姓名".to_string())
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

    call_generate(
        &prompt,
        None,
        ollama_timeout("RG_OLLAMA_CHAT_TIMEOUT_SECS", DEFAULT_OLLAMA_CHAT_TIMEOUT_SECS),
        "compress_context",
        false,
    )
    .await
}
