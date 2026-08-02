//! NLQ 关键词配置加载与缓存模块。
//! 支持从外部 JSON 文件加载关键词配置，当文件不存在时使用内置默认值。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// 单个意图的关键词配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentConfig {
    pub keywords: Vec<String>,
    pub weight: f64,
    pub confidence_threshold: u8,
}

/// 顶层关键词配置结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NlqKeywords {
    pub intents: HashMap<String, IntentConfig>,
    pub version: String,
}

impl NlqKeywords {
    /// 返回内置的默认关键词配置（向后兼容）
    pub fn default_builtin() -> Self {
        let mut intents = HashMap::new();

        intents.insert(
            "create_person".to_string(),
            IntentConfig {
                keywords: vec![
                    "新加", "添加", "录入", "新增", "加个", "新联系人", "新建",
                    "刚认识", "新认识", "遇到了", "介绍了", "新朋友", "新同事",
                    "加一个人", "来了个新人", "新建联系人", "加一个",
                ]
                .into_iter()
                .map(String::from)
                .collect(),
                weight: 1.0,
                confidence_threshold: 50,
            },
        );

        intents.insert(
            "update_person".to_string(),
            IntentConfig {
                keywords: vec![
                    "换了", "去了", "改为", "变成了", "加入了", "离开了", "新公司",
                    "新职位", "升为", "调到", "跳槽", "晋升", "转岗", "搬到了", "改名了",
                ]
                .into_iter()
                .map(String::from)
                .collect(),
                weight: 1.0,
                confidence_threshold: 50,
            },
        );

        intents.insert(
            "add_interaction".to_string(),
            IntentConfig {
                keywords: vec![
                    "聊了", "谈了", "讨论了", "沟通了", "见了面", "吃饭", "开会",
                    "打了电话", "发了消息", "碰过面", "一起开会", "视频了", "约了",
                    "面谈", "同步了",
                ]
                .into_iter()
                .map(String::from)
                .collect(),
                weight: 1.0,
                confidence_threshold: 50,
            },
        );

        intents.insert(
            "find_path".to_string(),
            IntentConfig {
                keywords: vec![
                    "怎么认识", "通过谁", "什么关系", "联系到", "认识路径", "关系链",
                    "介绍人", "中间人", "谁引荐的",
                ]
                .into_iter()
                .map(String::from)
                .collect(),
                weight: 1.2,
                confidence_threshold: 40,
            },
        );

        intents.insert(
            "search_people".to_string(),
            IntentConfig {
                keywords: vec![
                    "找", "搜索", "查找", "谁是", "哪些人", "列出",
                ]
                .into_iter()
                .map(String::from)
                .collect(),
                weight: 0.8,
                confidence_threshold: 30,
            },
        );

        Self {
            intents,
            version: "1.0-builtin".to_string(),
        }
    }
}

/// 从指定路径加载关键词配置。
/// 如果文件不存在或解析失败，返回内置默认值。
pub fn load_keywords(path: &str) -> NlqKeywords {
    match std::fs::read_to_string(path) {
        Ok(content) => match serde_json::from_str::<NlqKeywords>(&content) {
            Ok(keywords) => {
                log::info!(
                    target: "nlq_config",
                    "load_keywords_success path={} version={} intent_count={}",
                    path,
                    keywords.version,
                    keywords.intents.len()
                );
                keywords
            }
            Err(e) => {
                log::warn!(
                    target: "nlq_config",
                    "load_keywords_parse_error path={} error={}, using builtin defaults",
                    path,
                    e
                );
                NlqKeywords::default_builtin()
            }
        },
        Err(e) => {
            log::warn!(
                target: "nlq_config",
                "load_keywords_file_not_found path={} error={}, using builtin defaults",
                path,
                e
            );
            NlqKeywords::default_builtin()
        }
    }
}

/// 获取关键词配置文件路径（从环境变量或使用默认值）
pub fn keywords_path() -> String {
    std::env::var("RG_NLQ_KEYWORDS_PATH")
        .unwrap_or_else(|_| "config/nlq_keywords.json".to_string())
}

/// 加载关键词配置并包装为 Arc（供 AppState 使用）
pub fn load_keywords_arc() -> Arc<NlqKeywords> {
    let path = keywords_path();
    Arc::new(load_keywords(&path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_builtin_has_all_intents() {
        let kw = NlqKeywords::default_builtin();
        assert!(kw.intents.contains_key("create_person"));
        assert!(kw.intents.contains_key("update_person"));
        assert!(kw.intents.contains_key("add_interaction"));
        assert!(kw.intents.contains_key("find_path"));
        assert!(kw.intents.contains_key("search_people"));
        assert_eq!(kw.intents.len(), 5);
    }

    #[test]
    fn test_load_keywords_missing_file_returns_default() {
        let kw = load_keywords("/nonexistent/path/keywords.json");
        assert_eq!(kw.version, "1.0-builtin");
        assert_eq!(kw.intents.len(), 5);
    }

    #[test]
    fn test_find_path_weight_is_higher() {
        let kw = NlqKeywords::default_builtin();
        let fp = kw.intents.get("find_path").unwrap();
        assert!(fp.weight > 1.0);
    }
}
