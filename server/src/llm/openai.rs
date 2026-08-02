//! OpenAiProvider — 调用 OpenAI 兼容 API 进行 LLM 提取。

use async_trait::async_trait;
use log::{info, warn};
use serde::Deserialize;
use std::time::Duration;

use super::LlmProvider;
use crate::types::{FieldChange, InteractionDraft, PersonDraft};

pub struct OpenAiProvider {
    api_key: String,
    base_url: String,
    model: String,
    timeout: Duration,
}

impl OpenAiProvider {
    /// 尝试从环境变量构建，如果 API key 未配置则返回 None
    pub fn try_new() -> Option<Self> {
        let api_key = std::env::var("RG_OPENAI_API_KEY").ok()?;
        if api_key.is_empty() {
            return None;
        }
        let timeout_secs: u64 = std::env::var("RG_OPENAI_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(15);
        Some(Self {
            api_key,
            base_url: std::env::var("RG_OPENAI_BASE_URL")
                .unwrap_or_else(|_| "https://api.openai.com/v1".to_string()),
            model: std::env::var("RG_OPENAI_MODEL")
                .unwrap_or_else(|_| "gpt-4o-mini".to_string()),
            timeout: Duration::from_secs(timeout_secs),
        })
    }

    async fn call_openai(&self, prompt: &str) -> Result<String, String> {
        let client = reqwest::Client::builder()
            .timeout(self.timeout)
            .build()
            .map_err(|e| format!("HTTP client error: {}", e))?;

        let url = format!("{}/chat/completions", self.base_url);
        info!(target: "llm", "openai_request url={} model={} prompt_len={}", url, self.model, prompt.len());

        let body = serde_json::json!({
            "model": self.model,
            "messages": [
                {
                    "role": "system",
                    "content": "你是一个信息提取助手，请严格按要求输出JSON，不要输出任何多余内容。"
                },
                {
                    "role": "user",
                    "content": prompt
                }
            ],
            "temperature": 0.1,
            "response_format": { "type": "json_object" }
        });

        let resp = client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("OpenAI request failed: {}", e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("OpenAI returned status {} body={}", status, &text[..text.len().min(200)]));
        }

        let data: OpenAiResponse = resp.json().await.map_err(|e| format!("Parse error: {}", e))?;
        data.choices
            .into_iter()
            .next()
            .and_then(|c| c.message.content)
            .ok_or_else(|| "Empty response from OpenAI".to_string())
    }
}

#[derive(Deserialize)]
struct OpenAiResponse {
    choices: Vec<OpenAiChoice>,
}

#[derive(Deserialize)]
struct OpenAiChoice {
    message: OpenAiMessage,
}

#[derive(Deserialize)]
struct OpenAiMessage {
    content: Option<String>,
}

#[async_trait]
impl LlmProvider for OpenAiProvider {
    async fn extract_person_fields(&self, query: &str) -> PersonDraft {
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

        match self.call_openai(&prompt).await {
            Ok(json_str) => match serde_json::from_str::<serde_json::Value>(&json_str) {
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
                    confidence: 80,
                },
                Err(_) => empty_person_draft(),
            },
            Err(e) => {
                warn!(target: "llm", "openai extract_person_fields failed: {}", e);
                empty_person_draft()
            }
        }
    }

    async fn extract_update_fields(&self, query: &str) -> (String, Vec<FieldChange>) {
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

        match self.call_openai(&prompt).await {
            Ok(json_str) => match serde_json::from_str::<serde_json::Value>(&json_str) {
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
            },
            Err(e) => {
                warn!(target: "llm", "openai extract_update_fields failed: {}", e);
                (String::new(), vec![])
            }
        }
    }

    async fn extract_interaction_data(&self, query: &str) -> InteractionDraft {
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

        match self.call_openai(&prompt).await {
            Ok(json_str) => match serde_json::from_str::<serde_json::Value>(&json_str) {
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
                    confidence: 75,
                },
                Err(_) => empty_interaction_draft(),
            },
            Err(e) => {
                warn!(target: "llm", "openai extract_interaction_data failed: {}", e);
                empty_interaction_draft()
            }
        }
    }

    async fn extract_path_target(&self, query: &str) -> String {
        let prompt = format!(
            r#"请从以下文字中提取目标人名（用户想查找与谁的关系路径），只输出JSON：
{{"target_name": "目标人名"}}

文字：{}"#,
            query
        );

        match self.call_openai(&prompt).await {
            Ok(json_str) => serde_json::from_str::<serde_json::Value>(&json_str)
                .ok()
                .and_then(|v| v["target_name"].as_str().map(String::from))
                .unwrap_or_default(),
            Err(e) => {
                warn!(target: "llm", "openai extract_path_target failed: {}", e);
                String::new()
            }
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
