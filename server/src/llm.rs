use serde::{Deserialize, Serialize};
use std::time::Duration;
use log::{info, warn};

use crate::types::{PersonDraft, FieldChange, InteractionDraft};

const OLLAMA_TIMEOUT: Duration = Duration::from_secs(10);

fn ollama_url() -> String {
    std::env::var("RG_OLLAMA_URL").unwrap_or_else(|_| "http://localhost:11434".to_string())
}

fn ollama_model() -> String {
    std::env::var("RG_OLLAMA_MODEL").unwrap_or_else(|_| "qwen2.5:7b".to_string())
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

async fn call_ollama(prompt: &str, format: Option<&str>) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(OLLAMA_TIMEOUT)
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
        .map_err(|e| format!("Ollama request failed: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("Ollama returned status {}", resp.status()));
    }

    let data: OllamaResponse = resp.json().await.map_err(|e| format!("Parse error: {}", e))?;
    data.response.ok_or_else(|| "Empty response from Ollama".to_string())
}

pub async fn general_chat(query: &str) -> Result<String, String> {
    let prompt = format!(
        "你是关系图谱应用中的通用助理。请直接回答用户问题，默认使用简体中文。\
当用户问题涉及实时互联网信息时，明确说明你无法联网，并给出可行替代方案。\
\n\n用户问题：{}",
        query
    );

    call_ollama(&prompt, None).await
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

    match call_ollama(&prompt, Some("json")).await {
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

    match call_ollama(&prompt, Some("json")).await {
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

    match call_ollama(&prompt, Some("json")).await {
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

    match call_ollama(&prompt, Some("json")).await {
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
