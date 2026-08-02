use crate::security::sensitivity;
use crate::types::{
    FieldChange, InteractionDraft, NlqResponse, PathData, PathEdge, PathNode, Person, UpdateDraft,
};
use chrono::{DateTime, Duration, Utc};
use log::{debug, info};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NlqRequest {
    pub query: String,
    pub reveal_sensitive: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NlqResult {
    pub person_id: String,
    pub display_name: String,
    pub real_name_hidden: bool,
    pub sensitivity_level: String,
    pub company: Option<String>,
    pub title: Option<String>,
    pub relationship_strength: Option<String>,
    pub last_interaction_summary: Option<String>,
    pub status: String,
    pub next_step: Option<String>,
    pub suggestion: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct QueryIntent {
    pub intent: String,
    pub filters: QueryFilters,
    pub sort: Vec<SortSpec>,
    pub limit: usize,
    pub confidence: u8,
    pub needs_confirmation: bool,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct QueryFilters {
    pub locations: Vec<String>,
    pub resource_tags: Vec<String>,
    pub topics: Vec<String>,
    pub statuses: Vec<String>,
    pub relationship_strengths: Vec<String>,
    pub last_interaction_older_than_days: Option<i64>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SortSpec {
    pub field: String,
    pub order: String,
}

#[derive(Debug)]
struct Candidate {
    person_id: String,
    name: String,
    aliases: Vec<String>,
    sensitivity_level: String,
    company: Option<String>,
    title: Option<String>,
    location: Option<String>,
    relationship_strength: Option<String>,
    resource_tags: Vec<String>,
    status: String,
    next_step: Option<String>,
    last_interaction_at: Option<String>,
    last_interaction_summary: Option<String>,
}

#[derive(Debug)]
struct ScoredCandidate {
    candidate: Candidate,
    score: i64,
}

pub fn natural_language_query(conn: &Connection, owner_id: &str, req: NlqRequest) -> Result<Vec<NlqResult>, String> {
    let query_len = req.query.chars().count();
    let reveal_sensitive = req.reveal_sensitive.unwrap_or(false);
    let intent = validate_query_intent(parse_query_intent(&req.query));

    info!(
        target: "nlq",
        "nlq_request query_len={} reveal_sensitive={} intent={} confidence={} needs_confirmation={} filters={}",
        query_len,
        reveal_sensitive,
        intent.intent,
        intent.confidence,
        intent.needs_confirmation,
        safe_filter_summary(&intent.filters)
    );

    let candidates = load_candidates(conn, owner_id).map_err(|e| e.to_string())?;
    let candidate_count = candidates.len();
    let mut scored = Vec::new();

    for candidate in candidates {
        if candidate_matches(conn, owner_id, &candidate, &intent.filters).map_err(|e| e.to_string())? {
            let score = score_candidate(conn, owner_id, &candidate, &intent.filters).map_err(|e| e.to_string())?;
            scored.push(ScoredCandidate { candidate, score });
        }
    }

    scored.sort_by(|a, b| b.score.cmp(&a.score));
    let results = scored
        .into_iter()
        .take(intent.limit)
        .map(|scored| to_result(scored, reveal_sensitive))
        .collect::<Vec<_>>();

    info!(
        target: "nlq",
        "nlq_result query_len={} candidates={} results={} limit={}",
        query_len,
        candidate_count,
        results.len(),
        intent.limit
    );

    Ok(results)
}

fn parse_query_intent(query: &str) -> QueryIntent {
    let mut filters = QueryFilters::default();

    filters.locations = collect_matches(query, &["上海", "北京", "深圳", "广州", "杭州", "苏州", "南京"]);
    filters.resource_tags = collect_matches(
        query,
        &["地产", "政府资源", "融资", "设计", "设计圈", "汽车", "投标", "园区", "招商"],
    );
    filters.topics = collect_matches(query, &["融资", "投标", "懂车帝", "园区", "项目合作", "地产", "设计", "招商"]);

    if contains_any(query, &["待跟进", "没跟进", "未跟进", "还没跟进", "该联系"]){
        filters.statuses.push("follow-up".to_string());
    }
    if query.contains("活跃") {
        filters.statuses.push("active".to_string());
    }
    if query.contains("冷却") {
        filters.statuses.push("cold".to_string());
    }

    if contains_any(query, &["关系比较近", "关系近", "比较熟", "熟", "靠谱"]){
        filters.relationship_strengths.push("strong".to_string());
        filters.relationship_strengths.push("medium".to_string());
    } else if query.contains("关系强") {
        filters.relationship_strengths.push("strong".to_string());
    } else if query.contains("关系中") {
        filters.relationship_strengths.push("medium".to_string());
    } else if query.contains("关系弱") {
        filters.relationship_strengths.push("weak".to_string());
    }

    if contains_any(query, &["最近3个月没联系", "最近 3 个月没联系", "三个月没联系", "3个月没联系"]){
        filters.last_interaction_older_than_days = Some(90);
    } else if contains_any(query, &["最近1个月没联系", "最近 1 个月没联系", "一个月没联系", "1个月没联系"]){
        filters.last_interaction_older_than_days = Some(30);
    }

    let filter_count = filters.locations.len()
        + filters.resource_tags.len()
        + filters.topics.len()
        + filters.statuses.len()
        + filters.relationship_strengths.len()
        + usize::from(filters.last_interaction_older_than_days.is_some());

    QueryIntent {
        intent: "search_people".to_string(),
        filters,
        sort: vec![
            SortSpec { field: "match_score".to_string(), order: "desc".to_string() },
            SortSpec { field: "relationship_strength".to_string(), order: "desc".to_string() },
            SortSpec { field: "last_interaction_at".to_string(), order: "desc".to_string() },
        ],
        limit: 20,
        confidence: if filter_count >= 2 { 85 } else { 55 },
        needs_confirmation: filter_count == 0 || query.contains("最近没联系"),
    }
}

fn validate_query_intent(mut intent: QueryIntent) -> QueryIntent {
    intent.intent = "search_people".to_string();
    intent.limit = intent.limit.clamp(1, 50);
    intent.filters.locations = dedupe(intent.filters.locations);
    intent.filters.resource_tags = dedupe(intent.filters.resource_tags);
    intent.filters.topics = dedupe(intent.filters.topics);
    intent.filters.statuses = allow_only(intent.filters.statuses, &["follow-up", "active", "cold"]);
    intent.filters.relationship_strengths = allow_only(intent.filters.relationship_strengths, &["strong", "medium", "weak"]);
    intent
}

fn load_candidates(conn: &Connection, owner_id: &str) -> Result<Vec<Candidate>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT
            p.id,
            p.name,
            p.aliases,
            p.sensitivity_level,
            p.company,
            p.title,
            p.location,
            p.relationship_strength,
            p.resource_tags,
            p.status,
            p.next_step,
            (
                SELECT i.timestamp FROM interactions i
                WHERE i.person_id = p.id AND i.owner_id = ?1
                ORDER BY i.timestamp DESC
                LIMIT 1
            ) AS last_interaction_at,
            (
                SELECT i.summary FROM interactions i
                WHERE i.person_id = p.id AND i.owner_id = ?1
                ORDER BY i.timestamp DESC
                LIMIT 1
            ) AS last_interaction_summary
         FROM persons p
         WHERE p.owner_id = ?1
         ORDER BY p.updated_at DESC
         LIMIT 500",
    )?;

    let rows = stmt.query_map(params![owner_id], |row| {
        let aliases_json: String = row.get(2)?;
        let tags_json: String = row.get(8)?;
        Ok(Candidate {
            person_id: row.get(0)?,
            name: row.get(1)?,
            aliases: serde_json::from_str(&aliases_json).unwrap_or_default(),
            sensitivity_level: row.get(3)?,
            company: row.get(4)?,
            title: row.get(5)?,
            location: row.get(6)?,
            relationship_strength: row.get(7)?,
            resource_tags: serde_json::from_str(&tags_json).unwrap_or_default(),
            status: row.get(9)?,
            next_step: row.get(10)?,
            last_interaction_at: row.get(11)?,
            last_interaction_summary: row.get(12)?,
        })
    })?;

    rows.collect()
}

fn candidate_matches(conn: &Connection, owner_id: &str, candidate: &Candidate, filters: &QueryFilters) -> Result<bool, rusqlite::Error> {
    if !filters.locations.is_empty()
        && !matches_any(candidate.location.as_deref().unwrap_or_default(), &filters.locations)
    {
        return Ok(false);
    }

    if !filters.resource_tags.is_empty()
        && !filters.resource_tags.iter().any(|filter| candidate.resource_tags.iter().any(|tag| tag.contains(filter)))
    {
        return Ok(false);
    }

    if !filters.statuses.is_empty() && !filters.statuses.iter().any(|status| status == &candidate.status) {
        return Ok(false);
    }

    if !filters.relationship_strengths.is_empty()
        && !filters.relationship_strengths.iter().any(|strength| candidate.relationship_strength.as_deref() == Some(strength.as_str()))
    {
        return Ok(false);
    }

    if let Some(days) = filters.last_interaction_older_than_days {
        if !is_older_than(candidate.last_interaction_at.as_deref(), days) {
            return Ok(false);
        }
    }

    if !filters.topics.is_empty() && !person_has_any_topic(conn, owner_id, &candidate.person_id, &filters.topics)? {
        return Ok(false);
    }

    Ok(true)
}

fn score_candidate(conn: &Connection, owner_id: &str, candidate: &Candidate, filters: &QueryFilters) -> Result<i64, rusqlite::Error> {
    let mut score = 0;

    score += match candidate.relationship_strength.as_deref() {
        Some("strong") => 40,
        Some("medium") => 25,
        Some("weak") => 10,
        _ => 0,
    };

    if candidate.status == "follow-up" {
        score += 18;
    }

    score += filters
        .resource_tags
        .iter()
        .filter(|filter| candidate.resource_tags.iter().any(|tag| tag.contains(*filter)))
        .count() as i64
        * 12;

    score += topic_match_count(conn, owner_id, &candidate.person_id, &filters.topics)? as i64 * 10;

    if is_recent(candidate.last_interaction_at.as_deref(), 30) {
        score += 8;
    }

    if candidate.sensitivity_level == "high" {
        score -= 5;
    }

    debug!(
        target: "nlq",
        "nlq_score person_id={} score={} sensitivity={} status={} strength={:?}",
        candidate.person_id,
        score,
        candidate.sensitivity_level,
        candidate.status,
        candidate.relationship_strength
    );

    Ok(score)
}

fn to_result(scored: ScoredCandidate, reveal_sensitive: bool) -> NlqResult {
    let candidate = scored.candidate;
    let real_name_hidden = sensitivity::requires_reveal(&candidate.sensitivity_level) && !reveal_sensitive;
    let display_name = sensitivity::display_name(
        &candidate.name,
        &candidate.aliases,
        &candidate.sensitivity_level,
        reveal_sensitive,
    );

    NlqResult {
        person_id: candidate.person_id,
        display_name,
        real_name_hidden,
        sensitivity_level: candidate.sensitivity_level,
        company: candidate.company,
        title: candidate.title,
        relationship_strength: candidate.relationship_strength,
        status: candidate.status.clone(),
        next_step: candidate.next_step.clone(),
        last_interaction_summary: candidate.last_interaction_summary,
        suggestion: build_suggestion(&candidate.status, candidate.next_step.as_deref()),
    }
}

fn person_has_any_topic(conn: &Connection, owner_id: &str, person_id: &str, topics: &[String]) -> Result<bool, rusqlite::Error> {
    Ok(topic_match_count(conn, owner_id, person_id, topics)? > 0)
}

fn topic_match_count(conn: &Connection, owner_id: &str, person_id: &str, topics: &[String]) -> Result<usize, rusqlite::Error> {
    if topics.is_empty() {
        return Ok(0);
    }

    let mut count = 0;
    for topic in topics {
        let pattern = format!("%{}%", topic);
        let exists: i64 = conn.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM interactions
                WHERE owner_id = ?1 AND person_id = ?2
                  AND (topics LIKE ?3 OR content LIKE ?3 OR COALESCE(summary, '') LIKE ?3)
            )",
            params![owner_id, person_id, pattern],
            |row| row.get(0),
        )?;
        if exists == 1 {
            count += 1;
        }
    }
    Ok(count)
}

fn build_suggestion(status: &str, next_step: Option<&str>) -> String {
    if let Some(next_step) = next_step {
        if !next_step.trim().is_empty() {
            return next_step.to_string();
        }
    }

    match status {
        "follow-up" => "建议尽快补一次跟进，并记录新的沟通结果。".to_string(),
        "cold" => "建议先用轻量话题恢复联系，再判断是否推进具体事项。".to_string(),
        _ => "建议结合上次互动摘要，选择一个明确的小事项继续推进。".to_string(),
    }
}

fn safe_filter_summary(filters: &QueryFilters) -> String {
    format!(
        "locations={} tags={} topics={} statuses={} strengths={} older_days={:?}",
        filters.locations.len(),
        filters.resource_tags.len(),
        filters.topics.len(),
        filters.statuses.join("|"),
        filters.relationship_strengths.join("|"),
        filters.last_interaction_older_than_days
    )
}

fn collect_matches(query: &str, dictionary: &[&str]) -> Vec<String> {
    dictionary
        .iter()
        .filter(|word| query.contains(**word))
        .map(|word| word.to_string())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect()
}

fn contains_any(query: &str, words: &[&str]) -> bool {
    words.iter().any(|word| query.contains(word))
}

fn matches_any(value: &str, filters: &[String]) -> bool {
    filters.iter().any(|filter| value.contains(filter))
}

fn dedupe(values: Vec<String>) -> Vec<String> {
    values.into_iter().collect::<HashSet<_>>().into_iter().collect()
}

fn allow_only(values: Vec<String>, allowed: &[&str]) -> Vec<String> {
    values
        .into_iter()
        .filter(|value| allowed.contains(&value.as_str()))
        .collect::<HashSet<_>>()
        .into_iter()
        .collect()
}

fn is_recent(value: Option<&str>, days: i64) -> bool {
    value
        .and_then(|text| DateTime::parse_from_rfc3339(text).ok())
        .map(|time| Utc::now().signed_duration_since(time.with_timezone(&Utc)) <= Duration::days(days))
        .unwrap_or(false)
}

fn is_older_than(value: Option<&str>, days: i64) -> bool {
    value
        .and_then(|text| DateTime::parse_from_rfc3339(text).ok())
        .map(|time| Utc::now().signed_duration_since(time.with_timezone(&Utc)) >= Duration::days(days))
        .unwrap_or(true)
}

// === 多意图分类（规则式，不使用 LLM） ===

pub fn classify_intent(query: &str) -> &'static str {
    // 优先级：find_path > add_interaction > update_person > create_person > search_people
    if contains_any(query, &["怎么认识", "通过谁", "什么关系", "联系到", "认识路径", "关系链"]) {
        return "find_path";
    }
    if contains_any(query, &["聊了", "谈了", "讨论了", "沟通了", "见了面", "吃饭", "开会", "打了电话", "发了消息"]) {
        return "add_interaction";
    }
    if contains_any(query, &["去了", "换了", "改为", "变成了", "加入了", "离开了", "新公司", "新职位", "升为", "调到"]) {
        return "update_person";
    }
    if contains_any(query, &["刚认识", "新认识", "认识了", "遇到了", "介绍了一个", "新朋友", "新同事", "新加", "添加", "录入", "加一个", "新增", "加个", "新联系人", "新建联系人"]) {
        return "create_person";
    }
    "search_people"
}

// === 多意图处理器（供 api handler 调用） ===

/// 按人名模糊搜索联系人（参数化 SQL，LIMIT 10）
pub fn search_persons_by_name(conn: &Connection, owner_id: &str, name: &str) -> Result<Vec<Person>, String> {
    let pattern = format!("%{}%", name);
    let mut stmt = conn
        .prepare(
            "SELECT id, name, aliases, avatar, phone, email, company, title, location, background, \
             relationship_strength, resource_tags, sensitivity_level, status, next_step, notes, \
             school, projects, created_at, updated_at \
             FROM persons WHERE owner_id = ?1 AND (name LIKE ?2 OR aliases LIKE ?2) LIMIT 10",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map(params![owner_id, pattern], |row| {
            let aliases_json: String = row.get(2)?;
            let tags_json: String = row.get(11)?;
            let projects_json: Option<String> = row.get(17)?;
            Ok(Person {
                id: row.get(0)?,
                name: row.get(1)?,
                aliases: serde_json::from_str(&aliases_json).unwrap_or_default(),
                avatar: row.get(3)?,
                phone: row.get(4)?,
                email: row.get(5)?,
                company: row.get(6)?,
                title: row.get(7)?,
                location: row.get(8)?,
                background: row.get(9)?,
                relationship_strength: row.get(10)?,
                resource_tags: serde_json::from_str(&tags_json).unwrap_or_default(),
                sensitivity_level: row.get(12)?,
                status: row.get(13)?,
                next_step: row.get(14)?,
                notes: row.get(15)?,
                school: row.get(16)?,
                projects: projects_json
                    .as_deref()
                    .map(|json| serde_json::from_str(json).unwrap_or_default())
                    .unwrap_or_default(),
                created_at: row.get(18)?,
                updated_at: row.get(19)?,
            })
        })
        .map_err(|e| e.to_string())?;

    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

/// update_person 意图：人名消歧 + 组装草稿
pub fn handle_update_person_sync(
    conn: &Connection,
    owner_id: &str,
    target_name: &str,
    changes: Vec<FieldChange>,
) -> Result<NlqResponse, String> {
    let candidates = search_persons_by_name(conn, owner_id, target_name)?;
    let (target_person, error_hint, confidence) = if candidates.len() == 1 {
        (Some(candidates[0].clone()), None, 80u8)
    } else if candidates.is_empty() {
        (None, Some("未找到匹配的联系人".to_string()), 20)
    } else {
        (None, None, 40)
    };

    Ok(NlqResponse::UpdatePersonDraft {
        draft: UpdateDraft {
            target_person,
            candidates,
            changes,
            confidence,
            error_hint,
        },
    })
}

/// add_interaction 意图：人名消歧 + 组装草稿
pub fn handle_add_interaction_sync(
    conn: &Connection,
    owner_id: &str,
    mut draft: InteractionDraft,
) -> Result<NlqResponse, String> {
    if !draft.person_mention.is_empty() {
        let candidates = search_persons_by_name(conn, owner_id, &draft.person_mention)?;
        if candidates.len() == 1 {
            draft.resolved_person = Some(candidates[0].clone());
        }
        draft.candidates = candidates;
    }
    Ok(NlqResponse::AddInteractionDraft { draft })
}

/// find_path 意图：查找与目标人的最短关系路径
pub fn handle_find_path_sync(conn: &Connection, owner_id: &str, target_name: &str) -> Result<NlqResponse, String> {
    let candidates = search_persons_by_name(conn, owner_id, target_name)?;
    if candidates.is_empty() {
        return Ok(NlqResponse::FindPath {
            path: PathData {
                nodes: vec![],
                edges: vec![],
                hops: 0,
                includes_pending: false,
                summary: format!("未找到名为\"{}\"的联系人", target_name),
            },
        });
    }
    let target = &candidates[0];
    let path = find_shortest_path(conn, owner_id, &target.id)?;
    Ok(NlqResponse::FindPath { path })
}

/// BFS 查找从图中最远可达节点到 target 的最短路径
pub fn find_shortest_path(conn: &Connection, owner_id: &str, target_id: &str) -> Result<PathData, String> {
    // 加载所有 relationships（参数化查询无需用户输入）
    let mut stmt = conn
        .prepare(
            "SELECT id, from_person_id, to_person_id, relationship_type, strength, confirmation_status \
             FROM relationships WHERE owner_id = ?1 AND confirmation_status != 'rejected'",
        )
        .map_err(|e| e.to_string())?;

    let edges: Vec<(String, String, String, String, Option<String>, String)> = stmt
        .query_map(params![owner_id], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
            ))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    // 构建邻接表
    let mut adjacency: HashMap<String, Vec<(String, usize)>> = HashMap::new();
    for (idx, (_id, from, to, _rel, _str, _status)) in edges.iter().enumerate() {
        adjacency
            .entry(from.clone())
            .or_default()
            .push((to.clone(), idx));
        adjacency
            .entry(to.clone())
            .or_default()
            .push((from.clone(), idx));
    }

    // 从 target 做 BFS，找到所有可达节点及回溯父节点
    let mut visited: HashMap<String, (Option<String>, Option<usize>)> = HashMap::new();
    visited.insert(target_id.to_string(), (None, None));
    let mut queue = VecDeque::new();
    queue.push_back((target_id.to_string(), 0usize));

    let mut best_start = target_id.to_string();
    let mut best_depth = 0usize;

    while let Some((current, depth)) = queue.pop_front() {
        if depth >= 6 {
            continue;
        }
        if depth > best_depth {
            best_depth = depth;
            best_start = current.clone();
        }
        if let Some(neighbors) = adjacency.get(&current) {
            for (neighbor, edge_idx) in neighbors {
                if !visited.contains_key(neighbor) {
                    visited.insert(
                        neighbor.clone(),
                        (Some(current.clone()), Some(*edge_idx)),
                    );
                    queue.push_back((neighbor.clone(), depth + 1));
                }
            }
        }
    }

    // 如果 target 无任何连接，返回空路径
    if best_depth == 0 {
        return Ok(PathData {
            nodes: vec![],
            edges: vec![],
            hops: 0,
            includes_pending: false,
            summary: "该联系人暂无已知关系链".to_string(),
        });
    }

    // 从 best_start 沿 visited 回溯到 target
    let mut path_node_ids: Vec<String> = vec![];
    let mut path_edges: Vec<PathEdge> = vec![];
    let mut cursor = best_start.clone();
    loop {
        path_node_ids.push(cursor.clone());
        if cursor == target_id {
            break;
        }
        match visited.get(&cursor) {
            Some((Some(prev), Some(edge_idx))) => {
                let (_id, from, to, rel_type, strength, status) = &edges[*edge_idx];
                path_edges.push(PathEdge {
                    from_id: from.clone(),
                    to_id: to.clone(),
                    relationship_type: rel_type.clone(),
                    strength: strength.clone(),
                    confirmation_status: status.clone(),
                });
                cursor = prev.clone();
            }
            _ => break,
        }
    }

    // 加载节点名称
    let mut result_nodes: Vec<PathNode> = Vec::new();
    for nid in &path_node_ids {
        let name: String = conn
            .query_row("SELECT name FROM persons WHERE id = ?1", params![nid], |row| {
                row.get(0)
            })
            .unwrap_or_else(|_| "未知".to_string());
        let company: Option<String> = conn
            .query_row(
                "SELECT company FROM persons WHERE id = ?1",
                params![nid],
                |row| row.get(0),
            )
            .ok();
        result_nodes.push(PathNode {
            id: nid.to_string(),
            name,
            company,
        });
    }

    let includes_pending = path_edges
        .iter()
        .any(|e| e.confirmation_status == "pending");
    let summary = if result_nodes.len() >= 2 {
        result_nodes
            .iter()
            .map(|n| n.name.clone())
            .collect::<Vec<_>>()
            .join(" → ")
    } else {
        "未找到关系路径".to_string()
    };

    Ok(PathData {
        hops: path_node_ids.len().saturating_sub(1),
        nodes: result_nodes,
        edges: path_edges,
        includes_pending,
        summary,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_location_tag_and_close_relationship() {
        let intent = validate_query_intent(parse_query_intent("谁在上海做地产，和我关系比较近？"));

        assert_eq!(intent.intent, "search_people");
        assert!(intent.filters.locations.contains(&"上海".to_string()));
        assert!(intent.filters.resource_tags.contains(&"地产".to_string()));
        assert!(intent.filters.relationship_strengths.contains(&"strong".to_string()));
        assert!(intent.filters.relationship_strengths.contains(&"medium".to_string()));
        assert!(!intent.needs_confirmation);
    }

    #[test]
    fn parses_follow_up_financing_topic() {
        let intent = validate_query_intent(parse_query_intent("上次聊过融资的人里，还没跟进的有谁？"));

        assert!(intent.filters.topics.contains(&"融资".to_string()));
        assert!(intent.filters.statuses.contains(&"follow-up".to_string()));
    }

    #[test]
    fn parses_project_help_query_without_sql() {
        let intent = validate_query_intent(parse_query_intent("这个懂车帝的投标，谁能帮上忙？"));

        assert!(intent.filters.topics.contains(&"懂车帝".to_string()));
        assert!(intent.filters.topics.contains(&"投标".to_string()));
        assert_eq!(intent.sort[0].field, "match_score");
    }

    #[test]
    fn parses_stale_follow_up_window() {
        let intent = validate_query_intent(parse_query_intent("最近3个月没联系但标记了待跟进的人有哪些？"));

        assert_eq!(intent.filters.last_interaction_older_than_days, Some(90));
        assert!(intent.filters.statuses.contains(&"follow-up".to_string()));
    }

    #[test]
    fn test_classify_intent_search() {
        assert_eq!(classify_intent("谁在上海做地产"), "search_people");
        assert_eq!(classify_intent("关系比较近的人"), "search_people");
    }

    #[test]
    fn test_classify_intent_create() {
        assert_eq!(classify_intent("刚认识张明，做地产的"), "create_person");
        assert_eq!(classify_intent("新认识了一个做融资的朋友"), "create_person");
        assert_eq!(classify_intent("新加了一个联系人他的名字叫李悠然，他在上海交通大学当老师"), "create_person");
        assert_eq!(classify_intent("添加一个联系人叫王强"), "create_person");
        assert_eq!(classify_intent("录入一个新同事赵海"), "create_person");
        assert_eq!(classify_intent("加一个联系人叫刘芳"), "create_person");
        assert_eq!(classify_intent("新增联系人张三"), "create_person");
    }

    #[test]
    fn test_classify_intent_update() {
        assert_eq!(classify_intent("张明去了新公司XX科技"), "update_person");
        assert_eq!(classify_intent("李华的职位变成了总监"), "update_person");
    }

    #[test]
    fn test_classify_intent_interaction() {
        assert_eq!(classify_intent("刚和李明聊了园区项目"), "add_interaction");
        assert_eq!(classify_intent("昨天和王总吃饭谈投资"), "add_interaction");
    }

    #[test]
    fn test_classify_intent_path() {
        assert_eq!(classify_intent("我怎么认识张三的"), "find_path");
        assert_eq!(classify_intent("通过谁可以联系到李四"), "find_path");
    }
}
