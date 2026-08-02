//! FallbackChain — 按优先级依次尝试 provider，失败自动降级到下一个。

use async_trait::async_trait;
use log::{info, warn};
use std::sync::Arc;

use super::fallback::RuleFallback;
use super::ollama::OllamaProvider;
use super::openai::OpenAiProvider;
use super::LlmProvider;
use crate::types::{FieldChange, InteractionDraft, PersonDraft};

pub struct FallbackChain {
    providers: Vec<(String, Arc<dyn LlmProvider>)>,
}

impl FallbackChain {
    pub fn new(providers: Vec<(String, Arc<dyn LlmProvider>)>) -> Self {
        Self { providers }
    }
}

#[async_trait]
impl LlmProvider for FallbackChain {
    async fn extract_person_fields(&self, query: &str) -> PersonDraft {
        for (name, provider) in &self.providers {
            let result = tokio::time::timeout(
                std::time::Duration::from_secs(30),
                provider.extract_person_fields(query),
            )
            .await;

            match result {
                Ok(draft) => {
                    if draft.confidence > 0 || !draft.name.is_empty() {
                        info!(target: "llm", "chain extract_person_fields success provider={}", name);
                        return draft;
                    }
                    warn!(target: "llm", "chain extract_person_fields empty result provider={}, trying next", name);
                }
                Err(_) => {
                    warn!(target: "llm", "chain extract_person_fields timeout provider={}, trying next", name);
                }
            }
        }
        // 不应到达这里（fallback 总会返回）
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

    async fn extract_update_fields(&self, query: &str) -> (String, Vec<FieldChange>) {
        for (name, provider) in &self.providers {
            let result = tokio::time::timeout(
                std::time::Duration::from_secs(30),
                provider.extract_update_fields(query),
            )
            .await;

            match result {
                Ok((target_name, changes)) => {
                    if !target_name.is_empty() || !changes.is_empty() {
                        info!(target: "llm", "chain extract_update_fields success provider={}", name);
                        return (target_name, changes);
                    }
                    warn!(target: "llm", "chain extract_update_fields empty result provider={}, trying next", name);
                }
                Err(_) => {
                    warn!(target: "llm", "chain extract_update_fields timeout provider={}, trying next", name);
                }
            }
        }
        (String::new(), vec![])
    }

    async fn extract_interaction_data(&self, query: &str) -> InteractionDraft {
        for (name, provider) in &self.providers {
            let result = tokio::time::timeout(
                std::time::Duration::from_secs(30),
                provider.extract_interaction_data(query),
            )
            .await;

            match result {
                Ok(draft) => {
                    if draft.confidence > 0 || !draft.person_mention.is_empty() {
                        info!(target: "llm", "chain extract_interaction_data success provider={}", name);
                        return draft;
                    }
                    warn!(target: "llm", "chain extract_interaction_data empty result provider={}, trying next", name);
                }
                Err(_) => {
                    warn!(target: "llm", "chain extract_interaction_data timeout provider={}, trying next", name);
                }
            }
        }
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

    async fn extract_path_target(&self, query: &str) -> String {
        for (name, provider) in &self.providers {
            let result = tokio::time::timeout(
                std::time::Duration::from_secs(30),
                provider.extract_path_target(query),
            )
            .await;

            match result {
                Ok(target) => {
                    if !target.is_empty() {
                        info!(target: "llm", "chain extract_path_target success provider={}", name);
                        return target;
                    }
                    warn!(target: "llm", "chain extract_path_target empty result provider={}, trying next", name);
                }
                Err(_) => {
                    warn!(target: "llm", "chain extract_path_target timeout provider={}, trying next", name);
                }
            }
        }
        String::new()
    }
}

/// 根据环境变量 RG_LLM_PROVIDER 构建降级链
/// 格式: "ollama,openai,fallback"（逗号分隔优先级列表）
/// 默认: "ollama,fallback"
pub fn build_llm_chain() -> Arc<dyn LlmProvider> {
    let provider_list = std::env::var("RG_LLM_PROVIDER")
        .unwrap_or_else(|_| "ollama,fallback".to_string());

    let mut providers: Vec<(String, Arc<dyn LlmProvider>)> = Vec::new();

    for name in provider_list.split(',').map(|s| s.trim()) {
        match name {
            "ollama" => {
                providers.push(("ollama".to_string(), Arc::new(OllamaProvider::new())));
                info!(target: "llm", "chain_add provider=ollama");
            }
            "openai" => {
                if let Some(p) = OpenAiProvider::try_new() {
                    providers.push(("openai".to_string(), Arc::new(p)));
                    info!(target: "llm", "chain_add provider=openai");
                } else {
                    warn!(target: "llm", "chain_skip provider=openai reason=missing_api_key");
                }
            }
            "fallback" => {
                providers.push(("fallback".to_string(), Arc::new(RuleFallback::new())));
                info!(target: "llm", "chain_add provider=fallback");
            }
            other => {
                warn!(target: "llm", "chain_skip provider={} reason=unknown", other);
            }
        }
    }

    // 确保至少有 fallback
    if providers.is_empty() {
        providers.push(("fallback".to_string(), Arc::new(RuleFallback::new())));
    }

    info!(
        target: "llm",
        "chain_built providers=[{}]",
        providers.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>().join(", ")
    );

    Arc::new(FallbackChain::new(providers))
}
