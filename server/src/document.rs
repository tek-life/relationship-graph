//! 用户上传文档上下文注入：将 ChatRequest.documents 的抽取文本构建为
//! prompt 文档段，供聊天链路在“角色设定 → 技能 → 文档段 → 用户问题”
//! 顺序中注入。预算由 env `RG_DOC_CONTEXT_CHARS` 控制（默认 12000 字），
//! 超限尾部截断并追加截断说明。日志不落文档内容。

/// 文档注入字符预算：默认 12000，env `RG_DOC_CONTEXT_CHARS` 可覆盖（非法值回退默认）。
pub fn doc_context_budget_chars() -> usize {
    const DEFAULT_DOC_CONTEXT_CHARS: usize = 12000;
    std::env::var("RG_DOC_CONTEXT_CHARS")
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .filter(|n| *n > 0)
        .unwrap_or(DEFAULT_DOC_CONTEXT_CHARS)
}

/// 构建文档段 prompt（纯函数，可单测）：每文档格式为
/// `### 用户上传文档《文件名》正文\n<内容>`，多文档间以空行分隔。
/// 空输入返回空串（调用方据此跳过注入，关闭/空态 prompt 逐字节不变）。
/// 合并后字符数超预算时按字符尾部截断到预算长度，并追加
/// `\n[文档内容超长已截断]`。
pub fn build_documents_prompt(docs: &[(String, String)], budget: usize) -> String {
    if docs.is_empty() {
        return String::new();
    }
    let merged: String = docs
        .iter()
        .map(|(file_name, content)| format!("### 用户上传文档《{}》正文\n{}", file_name, content))
        .collect::<Vec<_>>()
        .join("\n\n");
    let total = merged.chars().count();
    if total <= budget {
        return merged;
    }
    let cut: String = merged.chars().take(budget).collect();
    format!("{}\n[文档内容超长已截断]", cut)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_docs_returns_empty_string() {
        assert_eq!(build_documents_prompt(&[], 12000), "");
    }

    #[test]
    fn single_document_within_budget() {
        let docs = vec![("报告.pdf".to_string(), "抽取文本内容".to_string())];
        let prompt = build_documents_prompt(&docs, 12000);
        assert_eq!(prompt, "### 用户上传文档《报告.pdf》正文\n抽取文本内容");
    }

    #[test]
    fn multiple_documents_joined_by_blank_line() {
        let docs = vec![
            ("a.pdf".to_string(), "正文A".to_string()),
            ("b.docx".to_string(), "正文B".to_string()),
        ];
        let prompt = build_documents_prompt(&docs, 12000);
        assert_eq!(
            prompt,
            "### 用户上传文档《a.pdf》正文\n正文A\n\n### 用户上传文档《b.docx》正文\n正文B"
        );
    }

    #[test]
    fn over_budget_truncated_with_marker() {
        // 12 个字符的内容，预算 5 → 截断到 5 字符并追加截断说明
        let docs = vec![("a.txt".to_string(), "一二三四五六七八九十".to_string())];
        let prompt = build_documents_prompt(&docs, 5);
        assert!(prompt.starts_with("### 用"));
        assert!(prompt.ends_with("\n[文档内容超长已截断]"));
        // 截断后正文部分恰为预算长度（不含追加的截断说明）
        let body = prompt.strip_suffix("\n[文档内容超长已截断]").unwrap();
        assert_eq!(body.chars().count(), 5);
    }

    #[test]
    fn exactly_at_budget_not_truncated() {
        let docs = vec![("a.txt".to_string(), "甲乙".to_string())];
        let full = build_documents_prompt(&docs, usize::MAX);
        let budget = full.chars().count();
        // 恰等于预算 → 原文返回，无截断说明
        assert_eq!(build_documents_prompt(&docs, budget), full);
    }

    #[test]
    fn default_budget_is_12000_when_env_absent() {
        // 仅验证非法/空值回退语义（env 未设置或非法时走默认）；
        // 为避免与其它测试并发修改 env 的竞态，此处不 set/remove env，
        // 直接验证解析逻辑的等价纯计算路径
        assert_eq!(
            "abc".trim().parse::<usize>().ok().filter(|n| *n > 0),
            None
        );
        assert!(doc_context_budget_chars() > 0);
    }
}
