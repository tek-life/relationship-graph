//! LLM Provider 抽象层：定义统一 trait，支持多 provider 降级链。

pub mod chain;
pub mod fallback;
pub mod ollama;
pub mod openai;

use async_trait::async_trait;
use crate::types::{PersonDraft, FieldChange, InteractionDraft};

/// LLM Provider trait — 所有 provider 必须实现此接口
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// 从自然语言中提取联系人字段（用于 create_person 意图）
    async fn extract_person_fields(&self, query: &str) -> PersonDraft;

    /// 从自然语言中提取更新字段（用于 update_person 意图）
    async fn extract_update_fields(&self, query: &str) -> (String, Vec<FieldChange>);

    /// 从自然语言中提取互动信息（用于 add_interaction 意图）
    async fn extract_interaction_data(&self, query: &str) -> InteractionDraft;

    /// 从路径查询中提取目标人名
    async fn extract_path_target(&self, query: &str) -> String;
}

pub use chain::build_llm_chain;
