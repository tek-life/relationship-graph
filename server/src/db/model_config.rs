//! 模型配置与 LLM 用量（P1-7）。
//!
//! # 存储选型决策（settings 复用 vs 独立建表）
//! 本模块选择**独立建表**而非复用 settings（key-value）：
//! - model_configs 是「场景 → 模型」的结构化映射，场景为天然主键，
//!   需要枚举校验与 admin 列表展示，key-value 表达会让场景元数据
//!   （默认值/env 覆盖键/说明）散落代码各处；
//! - llm_usages 是只追加的遥测数据（provider/model/耗时/token/场景/
//!   时间戳），需要按时间倒序检索，塞进单行 settings 会无限膨胀；
//!   settings 定位是「少量全局开关/密钥」，两者语义不同。
//!
//! # 解析优先级（向后兼容决策，见 llm.rs resolve_scenario_model）
//! env 覆盖层（RG_* 实时最高优先级）> 本表配置 > 硬编码默认值。
//! env 保持覆盖层而非仅作初始种子：存量部署依赖 RG_CLOUD_*_MODEL
//! 等环境变量的行为逐字节不变，且运维可随时通过 env 紧急回滚。
//!
//! # 隐私红线
//! llm_usages 只落调用元数据（场景/通道/模型/函数名/token 数/耗时/
//! 时间戳），绝不落 prompt、回复等对话内容（与全链路日志脱敏原则一致）。

use rusqlite::Connection;

/// 模型场景元数据：场景键（model_configs.scenario / llm_usages.scenario
/// 共用）+ 硬编码默认模型 + 场景说明（admin UI 展示）。
///
/// 默认值与改造前 llm.rs env 路由默认逐一致（未配置时行为不变）：
/// - local：Ollama 本地通道（legacy/rig）单一模型
/// - chat：cloud 聊天主力模型（开思考）
/// - chat_search：cloud 联网搜索模型（web_search 路由逃生门）
/// - extract：cloud 结构化抽取（无思考、json_object）
/// - summarize：上下文压缩摘要（改造前与 extract 共用模型，故默认同值）
pub const SCENARIO_METAS: &[(&str, &str, &str)] = &[
    ("local", "qwen2.5:7b", "本地 Ollama 通道（legacy/rig）：聊天、抽取等全部本地调用共用"),
    ("chat", "qwen3.7-plus", "cloud 聊天主力模型（开思考）"),
    ("chat_search", "qwen3.7-plus", "cloud 联网搜索模型（启用联网搜索的聊天请求路由到此模型）"),
    ("extract", "qwen3.6-flash", "cloud 结构化抽取（联系人字段/互动/意图等 JSON 抽取）"),
    ("summarize", "qwen3.6-flash", "上下文压缩摘要（会话超 50 条时的历史压缩）"),
];

/// 按场景键查元数据；未知场景返回 None
pub fn scenario_meta(scenario: &str) -> Option<&'static (&'static str, &'static str, &'static str)> {
    SCENARIO_METAS.iter().find(|(key, _, _)| *key == scenario)
}

/// 场景合法性校验（写入前的白名单检查）
pub fn is_known_scenario(scenario: &str) -> bool {
    scenario_meta(scenario).is_some()
}

/// 场景的硬编码默认模型（未知场景返回空串）
pub fn default_model_for(scenario: &str) -> &'static str {
    scenario_meta(scenario).map(|(_, default, _)| *default).unwrap_or("")
}

// ---------- model_configs 读写 ----------

/// 读取场景在配置表中的模型值；未配置（行不存在）返回 None
pub fn get_model(conn: &Connection, scenario: &str) -> Result<Option<String>, rusqlite::Error> {
    let mut stmt = conn.prepare("SELECT model FROM model_configs WHERE scenario = ?1")?;
    let mut rows = stmt.query([scenario])?;
    match rows.next()? {
        Some(row) => Ok(Some(row.get::<_, String>(0)?)),
        None => Ok(None),
    }
}

/// 写入场景 → 模型配置（upsert）；未知场景拒绝（InvalidQuery 语义，
/// 与数据层越权写入拒绝模式一致），空白模型名拒绝
pub fn set_model(conn: &Connection, scenario: &str, model: &str) -> Result<(), rusqlite::Error> {
    if !is_known_scenario(scenario) {
        return Err(rusqlite::Error::InvalidQuery);
    }
    let model = model.trim();
    if model.is_empty() {
        return Err(rusqlite::Error::InvalidQuery);
    }
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO model_configs (scenario, model, updated_at) VALUES (?1, ?2, ?3)
         ON CONFLICT(scenario) DO UPDATE SET model = excluded.model, updated_at = excluded.updated_at",
        rusqlite::params![scenario, model, now],
    )?;
    Ok(())
}

/// 删除场景配置行（恢复为「未配置」，解析时回退 env/默认值）；幂等
pub fn delete_model(conn: &Connection, scenario: &str) -> Result<(), rusqlite::Error> {
    conn.execute("DELETE FROM model_configs WHERE scenario = ?1", [scenario])?;
    Ok(())
}

/// 列出全部已配置行（scenario, model, updated_at），按 SCENARIO_METAS
/// 定义顺序排序（未配置的场景不出现在结果中）
pub fn list_configs(conn: &Connection) -> Result<Vec<(String, String, String)>, rusqlite::Error> {
    let mut stmt =
        conn.prepare("SELECT scenario, model, updated_at FROM model_configs")?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;
    let mut all: Vec<(String, String, String)> = rows.collect::<Result<_, _>>()?;
    all.sort_by_key(|(scenario, _, _)| {
        SCENARIO_METAS
            .iter()
            .position(|(key, _, _)| key == scenario)
            .unwrap_or(usize::MAX)
    });
    Ok(all)
}

/// 迁移种子：为全部场景写入硬编码默认值（INSERT OR IGNORE，幂等）。
/// 已有行（admin 已改过的配置）零覆盖；种子值即 env 未设置时的
/// 回退默认，与改造前行为一致。
pub fn seed_default_models(conn: &Connection) -> Result<(), rusqlite::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    for (scenario, default, _desc) in SCENARIO_METAS {
        conn.execute(
            "INSERT OR IGNORE INTO model_configs (scenario, model, updated_at) VALUES (?1, ?2, ?3)",
            rusqlite::params![scenario, default, now],
        )?;
    }
    Ok(())
}

// ---------- llm_usages 读写（只追加遥测，仅元数据） ----------

/// 用量落库行（与 llm.rs 的 LlmUsageRecord 一一对应）
pub struct UsageInsert<'a> {
    pub scenario: &'a str,
    pub channel: &'a str,
    pub model: &'a str,
    pub fn_name: &'a str,
    pub prompt_tokens: Option<i64>,
    pub completion_tokens: Option<i64>,
    pub elapsed_ms: i64,
}

/// 写入一条用量元数据（id/created_at 自动生成）。
/// 失败由调用方降级记日志，不阻断 LLM 主链路。
pub fn insert_usage(conn: &Connection, record: &UsageInsert) -> Result<(), rusqlite::Error> {
    conn.execute(
        "INSERT INTO llm_usages
            (id, scenario, channel, model, fn_name, prompt_tokens, completion_tokens, elapsed_ms, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        rusqlite::params![
            uuid::Uuid::new_v4().to_string(),
            record.scenario,
            record.channel,
            record.model,
            record.fn_name,
            record.prompt_tokens,
            record.completion_tokens,
            record.elapsed_ms,
            chrono::Utc::now().to_rfc3339(),
        ],
    )?;
    Ok(())
}

/// 用量查询行（admin 展示，camelCase 序列化由 API 层负责）
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageRow {
    pub id: String,
    pub scenario: String,
    pub channel: String,
    pub model: String,
    pub fn_name: String,
    pub prompt_tokens: Option<i64>,
    pub completion_tokens: Option<i64>,
    pub elapsed_ms: i64,
    pub created_at: String,
}

/// 按时间倒序取最近 limit 条用量记录
pub fn recent_usages(conn: &Connection, limit: i64) -> Result<Vec<UsageRow>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, scenario, channel, model, fn_name, prompt_tokens, completion_tokens, elapsed_ms, created_at
         FROM llm_usages ORDER BY created_at DESC, id DESC LIMIT ?1",
    )?;
    let rows = stmt.query_map([limit], |row| {
        Ok(UsageRow {
            id: row.get(0)?,
            scenario: row.get(1)?,
            channel: row.get(2)?,
            model: row.get(3)?,
            fn_name: row.get(4)?,
            prompt_tokens: row.get(5)?,
            completion_tokens: row.get(6)?,
            elapsed_ms: row.get(7)?,
            created_at: row.get(8)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 独立建最小库（不依赖 schema::migrate，避免耦合其它表）
    fn test_db() -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory db");
        conn.execute_batch(
            "CREATE TABLE model_configs (
                scenario TEXT PRIMARY KEY,
                model TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE llm_usages (
                id TEXT PRIMARY KEY,
                scenario TEXT NOT NULL,
                channel TEXT NOT NULL,
                model TEXT NOT NULL,
                fn_name TEXT NOT NULL DEFAULT '',
                prompt_tokens INTEGER,
                completion_tokens INTEGER,
                elapsed_ms INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL
            );",
        )
        .expect("create tables");
        conn
    }

    #[test]
    fn scenario_metadata_is_consistent() {
        // 五个场景键齐备且互不重复
        assert_eq!(SCENARIO_METAS.len(), 5);
        let keys: Vec<&str> = SCENARIO_METAS.iter().map(|(k, _, _)| *k).collect();
        assert_eq!(keys, vec!["local", "chat", "chat_search", "extract", "summarize"]);
        assert!(is_known_scenario("chat"));
        assert!(!is_known_scenario("unknown_scenario"));
        assert_eq!(default_model_for("chat"), "qwen3.7-plus");
        assert_eq!(default_model_for("extract"), "qwen3.6-flash");
        assert_eq!(default_model_for("summarize"), "qwen3.6-flash");
        assert_eq!(default_model_for("local"), "qwen2.5:7b");
        assert_eq!(default_model_for("nope"), "");
    }

    #[test]
    fn seed_default_models_is_idempotent_and_ordered_list() {
        let conn = test_db();
        seed_default_models(&conn).expect("seed");
        seed_default_models(&conn).expect("seed again");
        let configs = list_configs(&conn).expect("list");
        assert_eq!(configs.len(), 5);
        // 按 SCENARIO_METAS 定义顺序
        assert_eq!(configs[0].0, "local");
        assert_eq!(configs[0].1, "qwen2.5:7b");
        assert_eq!(configs[1].0, "chat");
        assert_eq!(configs[4].0, "summarize");
    }

    #[test]
    fn set_get_delete_model_roundtrip() {
        let conn = test_db();
        assert_eq!(get_model(&conn, "chat").unwrap(), None);
        set_model(&conn, "chat", "  qwen-max  ").expect("set");
        // 写入前 trim
        assert_eq!(get_model(&conn, "chat").unwrap().as_deref(), Some("qwen-max"));
        // upsert 覆盖
        set_model(&conn, "chat", "qwen3.7-plus").expect("set again");
        assert_eq!(get_model(&conn, "chat").unwrap().as_deref(), Some("qwen3.7-plus"));
        // 删除后回到未配置
        delete_model(&conn, "chat").expect("delete");
        assert_eq!(get_model(&conn, "chat").unwrap(), None);
        // 幂等删除
        delete_model(&conn, "chat").expect("delete again");
    }

    #[test]
    fn set_model_rejects_unknown_scenario_and_blank_model() {
        let conn = test_db();
        assert!(matches!(
            set_model(&conn, "hacker_scenario", "x").unwrap_err(),
            rusqlite::Error::InvalidQuery
        ));
        assert!(matches!(
            set_model(&conn, "chat", "   ").unwrap_err(),
            rusqlite::Error::InvalidQuery
        ));
    }

    #[test]
    fn seed_does_not_overwrite_existing_config() {
        let conn = test_db();
        set_model(&conn, "extract", "my-custom-model").expect("set");
        seed_default_models(&conn).expect("seed");
        // 已有行零覆盖，其余场景照常播种
        assert_eq!(get_model(&conn, "extract").unwrap().as_deref(), Some("my-custom-model"));
        assert_eq!(get_model(&conn, "chat").unwrap().as_deref(), Some("qwen3.7-plus"));
    }

    #[test]
    fn insert_and_list_usages_metadata_only() {
        let conn = test_db();
        insert_usage(
            &conn,
            &UsageInsert {
                scenario: "chat",
                channel: "cloud",
                model: "qwen3.7-plus",
                fn_name: "general_chat",
                prompt_tokens: Some(1200),
                completion_tokens: Some(340),
                elapsed_ms: 4321,
            },
        )
        .expect("insert with tokens");
        insert_usage(
            &conn,
            &UsageInsert {
                scenario: "extract",
                channel: "legacy",
                model: "qwen2.5:7b",
                fn_name: "extract_person_fields",
                prompt_tokens: None,
                completion_tokens: None,
                elapsed_ms: 900,
            },
        )
        .expect("insert without tokens");

        let rows = recent_usages(&conn, 10).expect("recent");
        assert_eq!(rows.len(), 2);
        // 倒序：后写入的在前
        assert_eq!(rows[0].scenario, "extract");
        assert_eq!(rows[0].prompt_tokens, None);
        assert_eq!(rows[1].scenario, "chat");
        assert_eq!(rows[1].prompt_tokens, Some(1200));
        assert_eq!(rows[1].completion_tokens, Some(340));
        assert_eq!(rows[1].elapsed_ms, 4321);
        // id 唯一
        assert_ne!(rows[0].id, rows[1].id);
        // limit 生效
        assert_eq!(recent_usages(&conn, 1).unwrap().len(), 1);
    }
}
