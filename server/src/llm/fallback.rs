//! RuleFallback — 纯正则/规则提取，不依赖任何外部服务。
//! 作为终极降级保证功能不完全中断。

use async_trait::async_trait;
use log::info;

use super::LlmProvider;
use crate::types::{FieldChange, InteractionDraft, PersonDraft};

pub struct RuleFallback;

impl RuleFallback {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl LlmProvider for RuleFallback {
    async fn extract_person_fields(&self, query: &str) -> PersonDraft {
        info!(target: "llm", "rule_fallback extract_person_fields query_len={}", query.len());

        // 尝试提取第一个看起来像人名的词（2-4个中文字符的词）
        let name = extract_first_name(query).unwrap_or_default();

        // 尝试提取公司（包含"公司"、"科技"、"集团"等关键词的词组）
        let company = extract_company(query);

        // 尝试提取职位
        let title = extract_title(query);

        PersonDraft {
            name,
            company,
            location: None,
            title,
            resource_tags: vec![],
            background: None,
            school: None,
            confidence: 30,
        }
    }

    async fn extract_update_fields(&self, query: &str) -> (String, Vec<FieldChange>) {
        info!(target: "llm", "rule_fallback extract_update_fields query_len={}", query.len());
        // 尝试提取人名作为 target
        let name = extract_first_name(query).unwrap_or_default();
        (name, vec![])
    }

    async fn extract_interaction_data(&self, query: &str) -> InteractionDraft {
        info!(target: "llm", "rule_fallback extract_interaction_data query_len={}", query.len());

        // 尝试提取人名
        let person_mention = extract_first_name(query).unwrap_or_default();

        InteractionDraft {
            person_mention,
            resolved_person: None,
            candidates: vec![],
            topic: None,
            summary: None,
            action_items: vec![],
            confidence: 20,
        }
    }

    async fn extract_path_target(&self, query: &str) -> String {
        info!(target: "llm", "rule_fallback extract_path_target query_len={}", query.len());
        // 返回最后一个看起来像人名的词
        extract_last_name(query).unwrap_or_default()
    }
}

/// 提取第一个看起来像中文人名的词（2-4个连续中文字符）
fn extract_first_name(text: &str) -> Option<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        // 跳过非中文字符和常见非人名前缀
        if !is_cjk(chars[i]) || is_common_prefix(chars[i]) {
            i += 1;
            continue;
        }

        // 收集连续中文字符
        let start = i;
        while i < chars.len() && is_cjk(chars[i]) {
            i += 1;
        }
        let word: String = chars[start..i].iter().collect();

        // 2-4个字符的可能是人名
        if word.chars().count() >= 2 && word.chars().count() <= 4 && !is_common_word(&word) {
            return Some(word);
        }
    }
    None
}

/// 提取最后一个看起来像中文人名的词
fn extract_last_name(text: &str) -> Option<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut last_name: Option<String> = None;
    let mut i = 0;
    while i < chars.len() {
        if !is_cjk(chars[i]) || is_common_prefix(chars[i]) {
            i += 1;
            continue;
        }
        let start = i;
        while i < chars.len() && is_cjk(chars[i]) {
            i += 1;
        }
        let word: String = chars[start..i].iter().collect();
        if word.chars().count() >= 2 && word.chars().count() <= 4 && !is_common_word(&word) {
            last_name = Some(word);
        }
    }
    last_name
}

/// 提取公司名
fn extract_company(text: &str) -> Option<String> {
    let markers = ["公司", "科技", "集团", "有限", "股份", "企业"];
    for marker in &markers {
        if let Some(pos) = text.find(marker) {
            // 向前搜索公司名开始位置
            let before = &text[..pos + marker.len()];
            let chars: Vec<char> = before.chars().collect();
            let end = chars.len();
            let start = end.saturating_sub(10);
            let company: String = chars[start..end].iter().collect();
            if company.chars().count() >= 3 {
                return Some(company);
            }
        }
    }
    None
}

/// 提取职位
fn extract_title(text: &str) -> Option<String> {
    let titles = [
        "总监", "经理", "主管", "总裁", "副总", "董事", "总经理",
        "CEO", "CTO", "CFO", "COO", "VP", "老师", "教授", "工程师",
        "设计师", "产品经理", "架构师", "顾问", "合伙人",
    ];
    for title in &titles {
        if text.contains(title) {
            return Some(title.to_string());
        }
    }
    None
}

fn is_cjk(c: char) -> bool {
    ('\u{4e00}'..='\u{9fff}').contains(&c)
}

fn is_common_prefix(c: char) -> bool {
    "的了在和与是有为这那我你他她它们".contains(c)
}

fn is_common_word(word: &str) -> bool {
    let common = [
        "认识", "朋友", "同事", "联系", "关系", "公司", "科技", "集团",
        "今天", "昨天", "明天", "这个", "那个", "什么", "怎么", "通过",
        "新认识", "刚认识", "一个", "互动", "聊了", "谈了", "讨论",
        "地产", "融资", "投标", "设计", "上海", "北京", "深圳", "广州",
        "杭州", "新加", "添加", "录入", "新增", "新建", "联系人",
    ];
    common.contains(&word)
}
