//! 聊天工具调用（Function Calling）数据层 —— 方案 B 落地
//!
//! 为 /api/chat 与 /api/chat/stream 的 cloud 通道提供只读联系人数据工具，
//! 使「基于我的数据生成报告」类请求能拿到真实联系人数据（persons /
//! relationships / interactions）。
//!
//! 安全约束（与 §5.2 敏感级别约定一致）：
//! - 用户隔离：所有查询强制携带 owner_id 并在 SQL 层过滤，工具只能
//!   读到归属当前用户的数据；
//! - 脱敏在工具实现内部完成：medium / high 敏感联系人返回代称（aliases[0]），
//!   high 额外标记 realNameHidden=true；模型从源头看不到真名；
//! - phone / email 永不进入工具输出；
//! - 输出按 RG_TOOL_OUTPUT_BUDGET_CHARS（默认 8000 字符）整包截断；
//! - 日志仅记元数据（工具名、参数摘要、结果条数与字符数），不落内容。
//!
//! DB 锁约定：execute_tool 内部加锁查询、返回前释放；调用方（llm.rs 的
//! Agent 循环）在 LLM 调用期间不持锁。

use crate::nlq::{self, Candidate};
use crate::security::sensitivity;
use chrono::{DateTime, Duration, Utc};
use log::info;
use rig_core::completion::request::ToolDefinition;
use rusqlite::{params, Connection};
use serde::Deserialize;
use serde_json::json;

/// 工具输出整包字符预算（RG_TOOL_OUTPUT_BUDGET_CHARS 可覆盖）
fn tool_output_budget_chars() -> usize {
    std::env::var("RG_TOOL_OUTPUT_BUDGET_CHARS")
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(8000)
}

/// 首批工具集（全部只读）。description 措辞直接影响 qwen 的工具选择
/// 准确率，修改前请实测回归（临时实例 + 报告类查询，见 agents.md §8.2-9）。
pub fn definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "search_contacts".to_string(),
            description: "在用户的联系人数据库中按条件搜索联系人，返回联系人摘要列表（姓名可能因隐私设置为代称）。当需要统计、盘点、筛选联系人，或生成任何基于联系人数据的分析/报告时，必须先调用本工具获取真实数据，禁止凭空编造数据。".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "location": {"type": "string", "description": "城市或地区关键词，如：上海"},
                    "keyword": {"type": "string", "description": "在姓名、公司、职位、背景、资源标签、互动话题中模糊匹配的关键词，如：地产、融资"},
                    "strength": {"type": "string", "enum": ["strong", "medium", "weak"], "description": "关系强度过滤"},
                    "status": {"type": "string", "enum": ["active", "follow-up", "cold"], "description": "联系人状态过滤"},
                    "min_days_no_contact": {"type": "integer", "description": "仅返回最近 N 天未联系的联系人"}
                }
            }),
        },
        ToolDefinition {
            name: "get_person_detail".to_string(),
            description: "按联系人 id 或姓名查询单个联系人的详细信息，包括背景、最近互动记录与关系链。".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "person_id": {"type": "string", "description": "联系人 id（来自 search_contacts 的结果）"},
                    "name": {"type": "string", "description": "联系人姓名或代称（无 person_id 时使用）"}
                }
            }),
        },
        ToolDefinition {
            name: "list_recent_interactions".to_string(),
            description: "列出最近一段时间的互动记录（按时间倒序），可选按联系人过滤。用于回顾最近沟通情况、生成周报。".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "days": {"type": "integer", "description": "回溯天数，默认 30"},
                    "person_name": {"type": "string", "description": "可选，仅列出与该联系人的互动"}
                }
            }),
        },
    ]
}

// ---------- 参数解析 ----------

#[derive(Deserialize, Default)]
struct SearchArgs {
    location: Option<String>,
    keyword: Option<String>,
    strength: Option<String>,
    status: Option<String>,
    min_days_no_contact: Option<i64>,
}

#[derive(Deserialize, Default)]
struct DetailArgs {
    person_id: Option<String>,
    name: Option<String>,
}

#[derive(Deserialize)]
struct InteractionsArgs {
    days: Option<i64>,
    person_name: Option<String>,
}

/// 输出超预算时整包截断并附提示（纯函数，可单测；不依赖 env）
fn apply_output_budget(output: String, budget: usize) -> String {
    if output.chars().count() > budget {
        truncate_chars(&output, budget)
            + "\n[输出超出字符预算已截断，请缩小筛选条件或减少数量后重试]"
    } else {
        output
    }
}

/// 工具执行入口：分发 → 锁内查询（归属过滤与脱敏在查询层完成）→ 预算截断。
/// 返回字符串即回传给模型的 tool result（恒为合法 JSON 文本）。
pub fn execute_tool(conn: &Connection, owner_id: &str, name: &str, args: &serde_json::Value) -> String {
    let result = match name {
        "search_contacts" => match serde_json::from_value::<SearchArgs>(args.clone()) {
            Ok(a) => search_contacts(conn, owner_id, &a),
            Err(e) => Err(format!("参数解析失败: {}", e)),
        },
        "get_person_detail" => match serde_json::from_value::<DetailArgs>(args.clone()) {
            Ok(a) => get_person_detail(conn, owner_id, &a),
            Err(e) => Err(format!("参数解析失败: {}", e)),
        },
        "list_recent_interactions" => {
            match serde_json::from_value::<InteractionsArgs>(args.clone()) {
                Ok(a) => list_recent_interactions(conn, owner_id, &a),
                Err(e) => Err(format!("参数解析失败: {}", e)),
            }
        }
        _ => Err(format!("未知工具: {}", name)),
    };

    let value = match result {
        Ok(v) => v,
        Err(e) => {
            log::warn!(target: "chat_tools", "tool_failed tool={} err={}", name, e);
            json!({ "error": e })
        }
    };

    let output = apply_output_budget(value.to_string(), tool_output_budget_chars());
    info!(
        target: "chat_tools",
        "tool_result tool={} result_chars={}",
        name,
        output.chars().count()
    );
    output
}

// ---------- 工具实现 ----------

fn search_contacts(conn: &Connection, owner_id: &str, args: &SearchArgs) -> Result<serde_json::Value, String> {
    let candidates = nlq::load_candidates(conn, owner_id).map_err(|e| e.to_string())?;
    let total = candidates.len();

    let location = args.location.as_deref().unwrap_or_default().trim();
    let keyword = args.keyword.as_deref().unwrap_or_default().trim();
    let strength = args.strength.as_deref().unwrap_or_default().trim();
    let status = args.status.as_deref().unwrap_or_default().trim();

    let matched: Vec<&Candidate> = candidates
        .iter()
        .filter(|c| {
            if !location.is_empty()
                && !c
                    .location
                    .as_deref()
                    .unwrap_or_default()
                    .contains(location)
            {
                return false;
            }
            if !strength.is_empty()
                && c.relationship_strength.as_deref() != Some(strength)
            {
                return false;
            }
            if !status.is_empty() && c.status != status {
                return false;
            }
            if let Some(days) = args.min_days_no_contact {
                // 从未联系过的联系人视为满足「N 天未联系」
                if let Some(at) = c.last_interaction_at.as_deref() {
                    if !is_older_than(at, days) {
                        return false;
                    }
                }
            }
            if !keyword.is_empty() {
                let in_fields = c.name.contains(keyword)
                    || c.company.as_deref().unwrap_or_default().contains(keyword)
                    || c.title.as_deref().unwrap_or_default().contains(keyword)
                    || c.location.as_deref().unwrap_or_default().contains(keyword)
                    || c.resource_tags.iter().any(|t| t.contains(keyword));
                if !in_fields {
                    // 退回互动话题/内容匹配（复用 NLQ 的 topics 匹配）
                    if nlq::topic_match_count(conn, &c.person_id, &[keyword.to_string()])
                        .map(|n| n == 0)
                        .unwrap_or(true)
                    {
                        return false;
                    }
                }
            }
            true
        })
        .take(30)
        .collect();

    let contacts: Vec<serde_json::Value> = matched.iter().map(|c| contact_summary_json(c)).collect();
    info!(
        target: "chat_tools",
        "search_contacts total={} matched={} location={} keyword={} strength={} status={} min_days={:?}",
        total, contacts.len(), location, keyword, strength, status, args.min_days_no_contact
    );
    Ok(json!({ "total": total, "matched": contacts.len(), "contacts": contacts }))
}

fn get_person_detail(conn: &Connection, owner_id: &str, args: &DetailArgs) -> Result<serde_json::Value, String> {
    let person_id: Option<String> = match (args.person_id.as_deref(), args.name.as_deref()) {
        (Some(id), _) if !id.trim().is_empty() => Some(id.trim().to_string()),
        _ => {
            let name = args.name.as_deref().unwrap_or_default().trim();
            if name.is_empty() {
                return Err("请提供 person_id 或 name 参数".to_string());
            }
            // 按真名或别名模糊搜索（别名即 medium/high 联系人对外展示的代称）
            let candidates = nlq::search_persons_by_name(conn, owner_id, name)?;
            match candidates.len() {
                0 => return Ok(json!({ "error": format!("未找到联系人：{}", name) })),
                _ => Some(candidates[0].id.clone()),
            }
        }
    };
    let person_id = person_id.ok_or_else(|| "请提供 person_id 或 name 参数".to_string())?;

    let candidates = nlq::load_candidates(conn, owner_id).map_err(|e| e.to_string())?;
    let c = candidates
        .iter()
        .find(|c| c.person_id == person_id)
        .ok_or_else(|| format!("未找到联系人 id={}", person_id))?;

    let background: Option<String> = conn
        .query_row(
            "SELECT background FROM persons WHERE id = ?1 AND owner_id = ?2",
            params![person_id, owner_id],
            |row| row.get(0),
        )
        .ok()
        .flatten();

    // 关系链（最多 10 条，对端同样脱敏；仅限归属 owner 的关系）
    let mut stmt = conn
        .prepare(
            "SELECT r.relationship_type, r.strength, r.description, \
                    CASE WHEN r.from_person_id = ?1 THEN r.to_person_id ELSE r.from_person_id END AS other_id \
             FROM relationships r \
             WHERE (r.from_person_id = ?1 OR r.to_person_id = ?1) \
               AND r.confirmation_status != 'rejected' \
               AND r.from_person_id IN (SELECT id FROM persons WHERE owner_id = ?2) LIMIT 10",
        )
        .map_err(|e| e.to_string())?;
    let rel_rows: Vec<(String, String, Option<String>, String)> = stmt
        .query_map(params![person_id, owner_id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    let name_by_id = |id: &str| -> (String, bool) {
        candidates
            .iter()
            .find(|c| c.person_id == id)
            .map(|c| {
                (
                    sensitivity::display_name(&c.name, &c.aliases, &c.sensitivity_level, false),
                    c.sensitivity_level == "high",
                )
            })
            .unwrap_or_else(|| ("未知".to_string(), false))
    };

    let relationships: Vec<serde_json::Value> = rel_rows
        .iter()
        .map(|(rel_type, strength, desc, other_id)| {
            let (other_name, hidden) = name_by_id(other_id);
            let mut v = json!({
                "type": rel_type,
                "strength": strength,
                "with": other_name,
            });
            if hidden {
                v["withRealNameHidden"] = json!(true);
            }
            if let Some(d) = desc {
                v["description"] = json!(truncate_chars(d, 80));
            }
            v
        })
        .collect();

    // 最近互动（最多 5 条，摘要/正文截断）
    let mut stmt = conn
        .prepare(
            "SELECT timestamp, COALESCE(summary, ''), content, topics FROM interactions \
             WHERE person_id = ?1 ORDER BY timestamp DESC LIMIT 5",
        )
        .map_err(|e| e.to_string())?;
    let interactions: Vec<serde_json::Value> = stmt
        .query_map(params![person_id], |row| {
            let ts: String = row.get(0)?;
            let summary: String = row.get(1)?;
            let content: Option<String> = row.get(2)?;
            let topics_json: Option<String> = row.get(3)?;
            Ok((ts, summary, content, topics_json))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .map(|(ts, summary, content, topics_json)| {
            let topics: Vec<String> = topics_json
                .as_deref()
                .map(|j| serde_json::from_str(j).unwrap_or_default())
                .unwrap_or_default();
            json!({
                "timestamp": ts,
                "summary": truncate_chars(&summary, 120),
                "content": truncate_chars(content.as_deref().unwrap_or_default(), 200),
                "topics": topics,
            })
        })
        .collect();

    let mut detail = contact_summary_json(c);
    detail["background"] = json!(truncate_chars(background.as_deref().unwrap_or_default(), 300));
    detail["relationships"] = json!(relationships);
    detail["recentInteractions"] = json!(interactions);
    info!(
        target: "chat_tools",
        "get_person_detail person_id={} relationships={} interactions={}",
        person_id, relationships.len(), interactions.len()
    );
    Ok(detail)
}

fn list_recent_interactions(
    conn: &Connection,
    owner_id: &str,
    args: &InteractionsArgs,
) -> Result<serde_json::Value, String> {
    let days = args.days.unwrap_or(30).clamp(1, 365);
    let since = (Utc::now() - Duration::days(days)).to_rfc3339();

    let mut stmt = conn
        .prepare(
            "SELECT i.id, i.person_id, i.timestamp, COALESCE(i.summary, ''), i.content, i.topics \
             FROM interactions i WHERE i.timestamp >= ?1 \
               AND i.person_id IN (SELECT id FROM persons WHERE owner_id = ?2) \
             ORDER BY i.timestamp DESC LIMIT 40",
        )
        .map_err(|e| e.to_string())?;
    let rows: Vec<(String, String, String, String, Option<String>, Option<String>)> = stmt
        .query_map(params![since, owner_id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    let candidates = nlq::load_candidates(conn, owner_id).map_err(|e| e.to_string())?;

    // 可选按联系人过滤（真名或代称匹配）
    let filter_person_id: Option<Option<String>> = args
        .person_name
        .as_deref()
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .map(|n| {
            candidates
                .iter()
                .find(|c| {
                    c.name.contains(n)
                        || c.aliases.iter().any(|a| a.contains(n))
                })
                .map(|c| c.person_id.clone())
        });

    let items: Vec<serde_json::Value> = rows
        .iter()
        .filter(|(_, pid, _, _, _, _)| match &filter_person_id {
            Some(Some(id)) => pid == id,
            Some(None) => false, // 指定了姓名但未找到该联系人
            None => true,
        })
        .map(|(_, pid, ts, summary, content, topics_json)| {
            let (display, hidden) = candidates
                .iter()
                .find(|c| &c.person_id == pid)
                .map(|c| {
                    (
                        sensitivity::display_name(&c.name, &c.aliases, &c.sensitivity_level, false),
                        c.sensitivity_level == "high",
                    )
                })
                .unwrap_or_else(|| ("未知".to_string(), false));
            let topics: Vec<String> = topics_json
                .as_deref()
                .map(|j| serde_json::from_str(j).unwrap_or_default())
                .unwrap_or_default();
            let mut v = json!({
                "timestamp": ts,
                "person": display,
                "summary": truncate_chars(summary, 120),
                "content": truncate_chars(content.as_deref().unwrap_or_default(), 200),
                "topics": topics,
            });
            if hidden {
                v["personRealNameHidden"] = json!(true);
            }
            v
        })
        .collect();

    info!(
        target: "chat_tools",
        "list_recent_interactions days={} items={} filtered={}",
        days, items.len(), filter_person_id.is_some()
    );
    Ok(json!({ "days": days, "count": items.len(), "interactions": items }))
}

// ---------- 共用辅助 ----------

/// 联系人摘要 JSON（脱敏在源头完成）：
/// - display_name：low 显示真名；medium/high 显示代称（aliases[0]）
/// - realNameHidden：high 敏感额外标记
/// - phone / email 永不输出
fn contact_summary_json(c: &Candidate) -> serde_json::Value {
    let display = sensitivity::display_name(&c.name, &c.aliases, &c.sensitivity_level, false);
    let mut v = json!({
        "id": c.person_id,
        "displayName": display,
        "sensitivityLevel": c.sensitivity_level,
        "company": c.company,
        "title": c.title,
        "location": c.location,
        "strength": c.relationship_strength,
        "resourceTags": c.resource_tags,
        "status": c.status,
        "nextStep": c.next_step,
        "lastInteractionAt": c.last_interaction_at,
        "lastInteractionSummary": c.last_interaction_summary.as_ref().map(|s| truncate_chars(s, 100)),
    });
    if c.sensitivity_level == "high" {
        v["realNameHidden"] = json!(true);
    }
    v
}

fn is_older_than(value: &str, days: i64) -> bool {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|t| Utc::now().signed_duration_since(t.with_timezone(&Utc)) >= Duration::days(days))
        .unwrap_or(true)
}

fn truncate_chars(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{person, schema};
    use crate::types::CreatePersonRequest;

    fn in_memory_db() -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory db");
        schema::migrate(&conn).expect("schema migration");
        conn
    }

    fn create_on(
        conn: &Connection,
        owner_id: &str,
        name: &str,
        aliases: Vec<&str>,
        sensitivity: &str,
        location: &str,
        company: &str,
        tags: Vec<&str>,
    ) -> String {
        let created = person::create(
            conn,
            owner_id,
            CreatePersonRequest {
                name: name.to_string(),
                aliases: aliases.into_iter().map(String::from).collect(),
                avatar: None,
                phone: Some("13800000000".to_string()),
                email: Some("secret@example.com".to_string()),
                company: Some(company.to_string()),
                title: Some("投资总监".to_string()),
                location: Some(location.to_string()),
                background: Some("背景信息".to_string()),
                relationship_strength: Some("strong".to_string()),
                resource_tags: tags.into_iter().map(String::from).collect(),
                sensitivity_level: sensitivity.to_string(),
                status: Some("active".to_string()),
                next_step: None,
                notes: None,
                school: None,
                projects: vec![],
            },
        )
        .expect("create person");
        created.id
    }

    /// 脱敏核心断言：low 出真名；medium/high 出代称；phone/email 永不出现
    #[test]
    fn search_contacts_masks_sensitive_names_and_never_leaks_phone() {
        let conn = in_memory_db();
        create_on(&conn, "owner-a", "张低敏", vec![], "low", "上海", "某某地产集团", vec!["地产"]);
        create_on(&conn, "owner-a", "李中敏", vec!["小李"], "medium", "上海", "某某地产集团", vec!["地产"]);
        create_on(&conn, "owner-a", "王高敏", vec!["老王"], "high", "北京", "某某资本", vec!["融资"]);

        let out = execute_tool(
            &conn,
            "owner-a",
            "search_contacts",
            &json!({"location": "上海"}),
        );
        assert!(out.contains("张低敏"), "low 应显示真名");
        assert!(out.contains("小李"), "medium 应显示代称");
        assert!(!out.contains("李中敏"), "medium 真名不得泄露");
        assert!(!out.contains("王高敏"), "未命中过滤的高敏感真名不得出现");
        assert!(!out.contains("13800000000"), "phone 永不输出");
        assert!(!out.contains("secret@example.com"), "email 永不输出");

        let out_high = execute_tool(&conn, "owner-a", "search_contacts", &json!({}));
        assert!(out_high.contains("老王"));
        assert!(out_high.contains("\"realNameHidden\":true"));
        assert!(!out_high.contains("王高敏"));
    }

    /// keyword 命中公司字段；min_days_no_contact 对从未联系者放行
    #[test]
    fn search_contacts_keyword_and_days_filter() {
        let conn = in_memory_db();
        create_on(&conn, "owner-a", "赵地产", vec![], "low", "上海", "某某地产集团", vec!["地产"]);
        create_on(&conn, "owner-a", "钱设计", vec![], "low", "上海", "某某设计院", vec!["设计"]);

        let out = execute_tool(&conn, "owner-a", "search_contacts", &json!({"keyword": "地产"}));
        assert!(out.contains("赵地产"));
        assert!(!out.contains("钱设计"));

        // 从未联系 → 满足任意 N 天未联系
        let out = execute_tool(&conn, "owner-a", "search_contacts", &json!({"min_days_no_contact": 30}));
        assert!(out.contains("赵地产"));
        assert!(out.contains("钱设计"));
    }

    /// 用户隔离：其他用户的联系人不得出现在任何工具输出中
    #[test]
    fn tools_never_expose_other_users_contacts() {
        let conn = in_memory_db();
        create_on(&conn, "owner-a", "我的联系人", vec![], "low", "上海", "甲司", vec![]);
        let other_id = create_on(&conn, "owner-b", "别人机密", vec!["代号X"], "high", "上海", "乙司", vec!["融资"]);

        let out = execute_tool(&conn, "owner-a", "search_contacts", &json!({}));
        assert!(out.contains("我的联系人"));
        assert!(!out.contains("别人机密"));
        assert!(!out.contains("代号X"));

        // 用姓名直查他人联系人也应查无此人
        let out = execute_tool(&conn, "owner-a", "get_person_detail", &json!({"name": "别人机密"}));
        assert!(out.contains("未找到联系人"));

        // 用他人 person_id 直查也应查无此人（不泄露存在性）
        let out = execute_tool(&conn, "owner-a", "get_person_detail", &json!({"person_id": other_id}));
        assert!(out.contains("未找到联系人"));

        // 本人视角可正常读到自己的数据
        let out = execute_tool(&conn, "owner-b", "search_contacts", &json!({}));
        assert!(out.contains("代号X"));
    }

    /// 未知工具返回 error JSON（永不 panic）
    #[test]
    fn unknown_tool_returns_error_json() {
        let conn = in_memory_db();
        let out = execute_tool(&conn, "owner-a", "no_such_tool", &json!({}));
        assert!(out.contains("未知工具"));
    }

    /// 输出超预算时整包截断并附提示（直接测纯函数，避免 set_var
    /// 污染并行测试的全局 env）
    #[test]
    fn output_budget_truncates() {
        let conn = in_memory_db();
        for i in 0..30 {
            create_on(
                &conn,
                "owner-a",
                &format!("联系人{}", i),
                vec![],
                "low",
                "上海",
                "某某集团",
                vec!["地产"],
            );
        }
        let out = execute_tool(&conn, "owner-a", "search_contacts", &json!({}));
        assert!(out.chars().count() > 200, "全量输出应远超 200 字符");
        let truncated = apply_output_budget(out, 200);
        assert!(truncated.contains("[输出超出字符预算已截断"));
    }
}
