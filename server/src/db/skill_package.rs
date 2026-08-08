//! 技能包（多文件技能）数据层：纯函数（路径规范化 / 包解析 / 折叠注入段拼装）
//! 与 DB CRUD（skill_packages / skill_package_files / agent_skill_bindings）。
//!
//! 注入链路：`agent_config::build_skills_prompt` 按绑定遍历 is_active=1 的包，
//! 每包取 SKILL.md 入口经 parse/assemble 折叠为一段 `### 技能：<name>`，
//! 拼接后整体走 `apply_skill_budget` 共享预算。
//! 日志仅记元数据（路径、字符数、数量），不落文件内容。

use crate::db::agent_config::{strip_skill_frontmatter, validate_skill_markdown};
use crate::types::{SkillBinding, SkillPackage, SkillPackageFile};
use chrono::Utc;
use rand::RngCore;
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ============================================================
// 纯函数：路径规范化
// ============================================================

/// 规范化包内相对路径（纯函数，可单测）：
/// 拒绝空路径、绝对路径（`/` 开头）、含 `..` 分量与反斜杠的路径；
/// 忽略 `.` 分量并合并重复分隔符，返回 `/` 分隔的相对路径。
pub fn normalize_rel_path(path: &str) -> Result<String, String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err("文件路径不能为空".to_string());
    }
    if trimmed.starts_with('/') {
        return Err(format!("不允许绝对路径：{}", trimmed));
    }
    if trimmed.contains('\\') {
        return Err(format!("路径不允许包含反斜杠：{}", trimmed));
    }
    let mut components: Vec<&str> = Vec::new();
    for part in trimmed.split('/') {
        match part {
            "" | "." => continue,
            ".." => return Err(format!("路径不允许包含 .. 分量：{}", trimmed)),
            _ => components.push(part),
        }
    }
    if components.is_empty() {
        return Err(format!("无效的文件路径：{}", trimmed));
    }
    Ok(components.join("/"))
}

// ============================================================
// 纯函数：技能包解析
// ============================================================

/// 技能包清单（parse_skill_package 产物，可序列化为 manifest_json 落库）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageManifest {
    /// SKILL.md frontmatter 中的 name
    pub name: String,
    /// SKILL.md frontmatter 中的 description
    pub description: String,
    /// SKILL.md 正文（已剥离 frontmatter，已 trim）
    pub body: String,
    /// 入口文件在包内的目录前缀（嵌套包如 "pkg/"，根级入口为空串）；
    /// create/import 落库时用它归一化文件路径，使 rel_path 以 SKILL.md 为根
    #[serde(default)]
    pub entry_prefix: String,
    /// 入口文件在包内的相对路径（已按包根去前缀，恒为 SKILL.md）
    pub entry_path: String,
    /// 附属文档清单（不含入口文件），按 rel_path 升序
    pub sub_docs: Vec<SubDocInfo>,
}

/// 附属文档索引项：文本文件带首行摘要并参与注入，
/// 非文本文件（脚本等）仅记录路径、标记不注入。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubDocInfo {
    pub rel_path: String,
    pub summary: String,
    pub injectable: bool,
}

/// 解析 frontmatter 顶层键（语义与 agent_config::validate_skill_markdown 一致：
/// 手写切分、首个出现的键生效、容忍 BOM 与首部空白）。
/// 返回 (name, description)；缺失时返回空串，由调用方决定报错策略。
fn parse_frontmatter_kv(markdown: &str) -> (String, String) {
    let trimmed = markdown
        .trim_start()
        .trim_start_matches('\u{FEFF}')
        .trim_start();
    let mut lines = trimmed.lines();
    if !matches!(lines.next(), Some(line) if line.trim() == "---") {
        return (String::new(), String::new());
    }
    let mut name = String::new();
    let mut description = String::new();
    for line in lines {
        if line.trim() == "---" {
            break;
        }
        if line.trim_start().starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once(':') {
            match key.trim() {
                "name" if name.is_empty() => name = value.trim().to_string(),
                "description" if description.is_empty() => {
                    description = value.trim().to_string()
                }
                _ => {}
            }
        }
    }
    (name, description)
}

/// 附属文档摘要：取首行非空内容，截断到 80 字符。
fn first_line_summary(content: &str) -> String {
    let line = content
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("");
    line.chars().take(80).collect()
}

/// 按扩展名判定是否文本文件（参与注入索引）；脚本等非文本文件仅记录路径。
fn is_text_file(rel_path: &str) -> bool {
    let ext = rel_path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    matches!(ext.as_str(), "md" | "txt")
}

/// 定位并解析技能包（纯函数，可单测）。
///
/// - 文件名大小写不敏感地定位根 `SKILL.md`；根目录无 SKILL.md 时兼容
///   嵌套一层目录的仓库风格：取最浅层的那个，其所在目录视为包根，
///   其余文件路径相应去前缀；
/// - 入口 frontmatter 经 `validate_skill_markdown` 校验（必须含 name/description）；
/// - 收集附属文档清单：.md/.txt 文件记录路径 + 首行摘要（≤80 字符）；
///   非文本文件记录路径但标记不注入。
/// - 所有路径先过 `normalize_rel_path`；重复路径报错。
pub fn parse_skill_package(files: &[(String, String)]) -> Result<PackageManifest, String> {
    if files.is_empty() {
        return Err("技能包不能为空（至少需要 SKILL.md）".to_string());
    }

    // 规范化全部路径并查重
    let mut normalized: Vec<(String, String)> = Vec::with_capacity(files.len());
    for (rel_path, content) in files {
        let path = normalize_rel_path(rel_path)?;
        if normalized.iter().any(|(p, _)| *p == path) {
            return Err(format!("技能包内存在重复文件路径：{}", path));
        }
        normalized.push((path, content.clone()));
    }

    // 定位 SKILL.md：文件名大小写不敏感，取最浅层（目录深度最小）的候选；
    // 同深度多个候选时报错，避免歧义
    let mut candidates: Vec<(usize, usize)> = normalized
        .iter()
        .enumerate()
        .filter(|(_, (path, _))| {
            path.rsplit('/')
                .next()
                .map(|f| f.eq_ignore_ascii_case("skill.md"))
                .unwrap_or(false)
        })
        .map(|(idx, (path, _))| (path.matches('/').count(), idx))
        .collect();
    if candidates.is_empty() {
        return Err("技能包缺少入口文件 SKILL.md".to_string());
    }
    candidates.sort();
    let min_depth = candidates[0].0;
    if candidates.iter().filter(|(d, _)| *d == min_depth).count() > 1 {
        return Err("技能包存在多个同级 SKILL.md 入口，无法确定包根".to_string());
    }
    let entry_idx = candidates[0].1;

    // 包根 = 入口所在目录；其余文件路径去前缀
    let entry_path = &normalized[entry_idx].0;
    let prefix = match entry_path.rfind('/') {
        Some(pos) => &entry_path[..pos + 1],
        None => "",
    };
    let entry_content = normalized[entry_idx].1.clone();

    // 入口 frontmatter 校验 + 解析
    validate_skill_markdown(&entry_content).map_err(|e| format!("SKILL.md 校验失败：{}", e))?;
    let (name, description) = parse_frontmatter_kv(&entry_content);
    let body = strip_skill_frontmatter(&entry_content).trim().to_string();

    // 收集附属文档清单（不含入口），按 rel_path 升序保证确定性
    let mut sub_docs: Vec<SubDocInfo> = normalized
        .iter()
        .enumerate()
        .filter(|(idx, _)| *idx != entry_idx)
        .map(|(_, (path, content))| {
            let rel_path = path
                .strip_prefix(prefix)
                .unwrap_or(path)
                .to_string();
            let injectable = is_text_file(&rel_path);
            let summary = if injectable {
                first_line_summary(content)
            } else {
                String::new()
            };
            SubDocInfo { rel_path, summary, injectable }
        })
        .collect();
    sub_docs.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));

    Ok(PackageManifest {
        name,
        description,
        body,
        entry_prefix: prefix.to_string(),
        entry_path: entry_path
            .strip_prefix(prefix)
            .unwrap_or(entry_path)
            .to_string(),
        sub_docs,
    })
}

// ============================================================
// 纯函数：折叠注入段拼装与单包预算
// ============================================================

/// 单包注入段字符预算：默认 2000，env `RG_SKILL_PACKAGE_BUDGET_CHARS`
/// 可覆盖（非法值回退默认）。
pub fn skill_package_budget_chars() -> usize {
    const DEFAULT_SKILL_PACKAGE_BUDGET_CHARS: usize = 2000;
    std::env::var("RG_SKILL_PACKAGE_BUDGET_CHARS")
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .filter(|n| *n > 0)
        .unwrap_or(DEFAULT_SKILL_PACKAGE_BUDGET_CHARS)
}

/// 拼装单个技能包的折叠注入段（纯函数，可单测）。
///
/// 格式：`### 技能：<name>\n<SKILL.md 正文>\n\n#### 附属文档\n- <rel_path>: <摘要>\n...\n\n`；
/// 无附属文档时省略 `#### 附属文档` 小节。正文同样受单包预算约束：超预算时
/// 按字符截断正文并附截断说明，再拼附属文档索引；附属文档索引超预算时
/// 截断索引尾部并追加一行截断说明（仿 apply_profile_budget 风格）。
pub fn assemble_package_section(manifest: &PackageManifest, budget: usize) -> String {
    let prefix = format!("### 技能：{}\n", manifest.name);
    const BODY_TRUNCATED_NOTE: &str = "（注：SKILL.md 正文超出单包预算，已截断）\n";

    // 正文预算约束：`前缀 + 正文 + \n\n` 超预算时按字符截断正文并附说明
    let body_truncated =
        prefix.chars().count() + manifest.body.chars().count() + 2 > budget;
    let mut body = manifest.body.as_str();
    let truncated_body: String;
    if body_truncated {
        let allowed = budget
            .saturating_sub(prefix.chars().count() + BODY_TRUNCATED_NOTE.chars().count() + 2);
        truncated_body = manifest.body.chars().take(allowed).collect();
        body = &truncated_body;
    }

    let mut section = format!("{}{}\n", prefix, body);
    if body_truncated {
        // 正文截断后预算已用尽：不再拼附属文档索引，保证整段不超单包预算
        section.push_str(BODY_TRUNCATED_NOTE);
        section.push('\n');
        return section;
    }
    section.push('\n');

    let docs: Vec<&SubDocInfo> = manifest
        .sub_docs
        .iter()
        .filter(|d| d.injectable)
        .collect();
    if docs.is_empty() {
        return section;
    }

    section.push_str("#### 附属文档\n");
    let mut truncated = false;
    for doc in docs {
        let line = format!("- {}: {}\n", doc.rel_path, doc.summary);
        if section.chars().count() + line.chars().count() > budget {
            truncated = true;
            break;
        }
        section.push_str(&line);
    }
    if truncated {
        section.push_str("（注：附属文档索引超出单包预算，已截断）\n");
    }
    section.push('\n');
    section
}

// ============================================================
// DB：skill_packages / skill_package_files
// ============================================================

const PACKAGE_SELECT: &str =
    "SELECT id, slug, display_name, description, source_kind, total_chars, is_active, created_at, updated_at FROM skill_packages";

fn map_package(row: &Row) -> Result<SkillPackage, rusqlite::Error> {
    let is_active_int: i32 = row.get(6)?;
    let total_chars: i64 = row.get(5)?;
    Ok(SkillPackage {
        id: row.get(0)?,
        slug: row.get(1)?,
        display_name: row.get(2)?,
        description: row.get(3)?,
        source_kind: row.get(4)?,
        total_chars: total_chars.max(0) as usize,
        is_active: is_active_int != 0,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
        files: None,
    })
}

fn map_package_file(row: &Row) -> Result<SkillPackageFile, rusqlite::Error> {
    let size_chars: i64 = row.get(4)?;
    Ok(SkillPackageFile {
        id: row.get(0)?,
        package_id: row.get(1)?,
        rel_path: row.get(2)?,
        content: row.get(3)?,
        size_chars: size_chars.max(0) as usize,
    })
}

pub fn list_skill_packages(conn: &Connection) -> Result<Vec<SkillPackage>, rusqlite::Error> {
    let sql = PACKAGE_SELECT.to_owned() + " ORDER BY created_at ASC";
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], map_package)?;
    rows.collect()
}

pub fn get_skill_package(
    conn: &Connection,
    id: &str,
) -> Result<Option<SkillPackage>, rusqlite::Error> {
    let sql = PACKAGE_SELECT.to_owned() + " WHERE id = ?1";
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query(params![id])?;
    let Some(row) = rows.next()? else {
        return Ok(None);
    };
    let mut package = map_package(row)?;
    package.files = Some(list_package_files(conn, id)?);
    Ok(Some(package))
}

pub fn list_package_files(
    conn: &Connection,
    package_id: &str,
) -> Result<Vec<SkillPackageFile>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, package_id, rel_path, content, size_chars FROM skill_package_files
         WHERE package_id = ?1 ORDER BY rel_path ASC",
    )?;
    let rows = stmt.query_map(params![package_id], map_package_file)?;
    rows.collect()
}

/// 由展示名生成 slug 基：ASCII 字母数字转小写，其余字符折叠为 `-`；
/// 纯非 ASCII（如中文名）退化为 `skill`。
fn slug_base(display_name: &str) -> String {
    let mut base = String::new();
    let mut last_dash = false;
    for ch in display_name.chars() {
        if ch.is_ascii_alphanumeric() {
            base.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash && !base.is_empty() {
            base.push('-');
            last_dash = true;
        }
    }
    while base.ends_with('-') {
        base.pop();
    }
    if base.is_empty() {
        base.push_str("skill");
    }
    base
}

/// 生成唯一 slug：`<slug_base>-<4位随机hex>`；与库内已有 slug 冲突时
/// 重新生成，最多重试 5 次，仍冲突则报错（携带中文原因）。
fn generate_slug(conn: &Connection, display_name: &str) -> Result<String, rusqlite::Error> {
    for _ in 0..5 {
        let mut suffix = [0u8; 2];
        rand::thread_rng().fill_bytes(&mut suffix);
        let slug = format!("{}-{}", slug_base(display_name), hex::encode(suffix));
        let exists: bool = conn.query_row(
            "SELECT EXISTS (SELECT 1 FROM skill_packages WHERE slug = ?1)",
            params![slug],
            |row| row.get(0),
        )?;
        if !exists {
            return Ok(slug);
        }
    }
    Err(rusqlite::Error::ToSqlConversionFailure(
        format!("生成唯一 slug 失败（展示名：{}，连续 5 次冲突）", display_name).into(),
    ))
}

/// 判断 slug 是否为旧技能同步出的 legacy 包（slug 形如 `legacy-<skill_id>`）。
/// legacy 包的生命周期由数字人技能面板管理，不允许经技能包端点删除。
pub fn is_legacy_package_slug(slug: &str) -> bool {
    slug.starts_with("legacy-")
}

fn insert_package_files(
    conn: &Connection,
    package_id: &str,
    files: &[(String, String)],
) -> Result<(), rusqlite::Error> {
    let now = Utc::now().to_rfc3339();
    for (rel_path, content) in files {
        let file_id = Uuid::new_v4().to_string();
        let size_chars = content.chars().count() as i64;
        conn.execute(
            "INSERT INTO skill_package_files (id, package_id, rel_path, content, size_chars, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![file_id, package_id, rel_path, content, size_chars, now],
        )?;
    }
    Ok(())
}

/// 创建技能包：解析清单（定位 SKILL.md、校验 frontmatter，失败错误携带中文原因）
/// 后在短事务内插入包与全部文件；文件路径按清单归一化（去除入口所在目录前缀，
/// 使嵌套入口包的 rel_path 以 SKILL.md 为根，保证注入 JOIN 命中）；
/// slug 由展示名生成并加短随机后缀保证 UNIQUE（冲突重试）；
/// total_chars 统计全部文件字符数。日志仅记元数据。
pub fn create_skill_package(
    conn: &Connection,
    display_name: &str,
    description: Option<String>,
    source_kind: &str,
    files: &[(String, String)],
) -> Result<SkillPackage, rusqlite::Error> {
    let manifest = parse_skill_package(files)
        .map_err(|reason| rusqlite::Error::ToSqlConversionFailure(reason.into()))?;
    let description = description.or_else(|| {
        if manifest.description.is_empty() {
            None
        } else {
            Some(manifest.description.clone())
        }
    });
    // 归一化文件路径：去掉入口目录前缀（路径已由 parse 校验，此处不会失败）
    let normalized_files: Vec<(String, String)> = files
        .iter()
        .map(|(rel_path, content)| {
            let path = normalize_rel_path(rel_path).unwrap_or_else(|_| rel_path.clone());
            let stripped = path
                .strip_prefix(manifest.entry_prefix.as_str())
                .unwrap_or(path.as_str());
            (stripped.to_string(), content.clone())
        })
        .collect();
    let total_chars: usize = normalized_files.iter().map(|(_, c)| c.chars().count()).sum();
    let manifest_json = serde_json::to_string(&manifest).unwrap_or_else(|_| "{}".to_string());

    let id = Uuid::new_v4().to_string();
    let slug = generate_slug(conn, display_name)?;
    let now = Utc::now().to_rfc3339();
    let total_chars_i64 = total_chars.min(i64::MAX as usize) as i64;

    // 共享连接为 &Connection（锁内短事务），用 unchecked_transaction
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "INSERT INTO skill_packages (id, slug, display_name, description, source_kind, manifest_json, total_chars, is_active, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1, ?8, ?8)",
        params![id, slug, display_name, description, source_kind, manifest_json, total_chars_i64, now],
    )?;
    insert_package_files(&tx, &id, &normalized_files)?;
    tx.commit()?;

    log::info!(
        target: "db",
        "create_skill_package id={} slug={} source_kind={} files={} total_chars={}",
        id,
        slug,
        source_kind,
        normalized_files.len(),
        total_chars
    );
    get_skill_package(conn, &id)?.ok_or(rusqlite::Error::QueryReturnedNoRows)
}

pub fn delete_skill_package(conn: &Connection, id: &str) -> Result<(), rusqlite::Error> {
    // 显式删除文件（测试用内存库不强制外键，不依赖级联）
    conn.execute("DELETE FROM skill_package_files WHERE package_id = ?1", params![id])?;
    conn.execute("DELETE FROM skill_packages WHERE id = ?1", params![id])?;
    log::info!(target: "db", "delete_skill_package id={}", id);
    Ok(())
}

/// 旧技能写入路径的 legacy 包幂等同步：按 slug（`legacy-<skill_id>`）
/// upsert 单文件 inline 包（is_active 跟随技能启停），并确保绑定存在
/// （sort_order 缺省 0）。skill_markdown 为空时由调用方改走 delete_legacy_package。
///
/// 绑定严格跟随技能的 agent_id（legacy 包与技能 1:1）：写绑定前先删除
/// 该包在其他数字人名下的绑定，避免 PUT 移动技能后旧数字人双份注入；
/// 包已存在但无任何绑定时视为管理员已主动解绑，编辑保存不得复活绑定。
pub fn upsert_legacy_package(
    conn: &Connection,
    slug: &str,
    agent_id: &str,
    skill_name: &str,
    skill_markdown: &str,
    is_active: bool,
) -> Result<(), rusqlite::Error> {
    let now = Utc::now().to_rfc3339();
    let total_chars = skill_markdown.chars().count() as i64;
    let is_active_int: i32 = is_active.into();

    // 区分「无此行」与真实查询错误：.optional() 仅把 QueryReturnedNoRows 转 None
    let existing: Option<String> = conn
        .query_row(
            "SELECT id FROM skill_packages WHERE slug = ?1",
            params![slug],
            |row| row.get(0),
        )
        .optional()?;
    let existed = existing.is_some();

    let package_id = match existing {
        Some(package_id) => {
            conn.execute(
                "UPDATE skill_packages SET display_name=?1, description=NULL, source_kind='inline', total_chars=?2, is_active=?3, updated_at=?4 WHERE id=?5",
                params![skill_name, total_chars, is_active_int, now, package_id],
            )?;
            conn.execute(
                "DELETE FROM skill_package_files WHERE package_id = ?1",
                params![package_id],
            )?;
            package_id
        }
        None => {
            let package_id = Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO skill_packages (id, slug, display_name, description, source_kind, manifest_json, total_chars, is_active, created_at, updated_at)
                 VALUES (?1, ?2, ?3, NULL, 'inline', NULL, ?4, ?5, ?6, ?6)",
                params![package_id, slug, skill_name, total_chars, is_active_int, now],
            )?;
            package_id
        }
    };

    let file_id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO skill_package_files (id, package_id, rel_path, content, size_chars, created_at)
         VALUES (?1, ?2, 'SKILL.md', ?3, ?4, ?5)",
        params![file_id, package_id, skill_markdown, total_chars, now],
    )?;

    // 包已存在但当前无任何绑定：管理员已主动解绑，编辑保存不复活绑定；
    // 新建包或仍有绑定时，先清理其他数字人的绑定再保证当前绑定存在
    let binding_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM agent_skill_bindings WHERE package_id = ?1",
        params![package_id],
        |row| row.get(0),
    )?;
    if !existed || binding_count > 0 {
        conn.execute(
            "DELETE FROM agent_skill_bindings WHERE package_id = ?1 AND agent_id <> ?2",
            params![package_id, agent_id],
        )?;
        conn.execute(
            "INSERT OR IGNORE INTO agent_skill_bindings (agent_id, package_id, sort_order, created_at)
             VALUES (?1, ?2, 0, ?3)",
            params![agent_id, package_id, now],
        )?;
    } else {
        log::info!(
            target: "db",
            "upsert_legacy_package_skip_rebind slug={} package_id={} reason=admin_unbound",
            slug,
            package_id
        );
    }
    log::info!(
        target: "db",
        "upsert_legacy_package slug={} package_id={} agent_id={} chars={}",
        slug,
        package_id,
        agent_id,
        total_chars
    );
    Ok(())
}

/// 删除 legacy 包（不存在则忽略）；绑定经显式删除兜底（不依赖外键级联）。
pub fn delete_legacy_package(conn: &Connection, slug: &str) -> Result<(), rusqlite::Error> {
    let Some(package_id): Option<String> = conn
        .query_row(
            "SELECT id FROM skill_packages WHERE slug = ?1",
            params![slug],
            |row| row.get(0),
        )
        .optional()?
    else {
        return Ok(());
    };
    conn.execute("DELETE FROM agent_skill_bindings WHERE package_id = ?1", params![package_id])?;
    conn.execute("DELETE FROM skill_package_files WHERE package_id = ?1", params![package_id])?;
    conn.execute("DELETE FROM skill_packages WHERE id = ?1", params![package_id])?;
    log::info!(target: "db", "delete_legacy_package slug={} package_id={}", slug, package_id);
    Ok(())
}

/// 清理已无任何绑定的孤儿 legacy 包（source_kind='inline' 且 slug 形如
/// `legacy-%`）：数字人删除后其 legacy 包若未被其他数字人绑定则同步清除，
/// 避免死包残留。返回删除的包数量；文件随包一并清理（不依赖外键级联）。
pub fn cleanup_orphan_legacy_packages(conn: &Connection) -> Result<usize, rusqlite::Error> {
    const ORPHAN_LEGACY_FILTER: &str =
        "source_kind = 'inline' AND slug LIKE 'legacy-%' AND id NOT IN (SELECT package_id FROM agent_skill_bindings)";
    conn.execute(
        &format!("DELETE FROM skill_package_files WHERE package_id IN (SELECT id FROM skill_packages WHERE {})", ORPHAN_LEGACY_FILTER),
        [],
    )?;
    let deleted = conn.execute(
        &format!("DELETE FROM skill_packages WHERE {}", ORPHAN_LEGACY_FILTER),
        [],
    )?;
    if deleted > 0 {
        log::info!(target: "db", "cleanup_orphan_legacy_packages count={}", deleted);
    }
    Ok(deleted)
}

// ============================================================
// DB：agent_skill_bindings
// ============================================================

pub fn list_bindings(conn: &Connection, agent_id: &str) -> Result<Vec<SkillBinding>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT b.agent_id, b.package_id, b.sort_order, p.display_name
         FROM agent_skill_bindings b
         JOIN skill_packages p ON p.id = b.package_id
         WHERE b.agent_id = ?1
         ORDER BY b.sort_order ASC, b.created_at ASC",
    )?;
    let rows = stmt.query_map(params![agent_id], |row| {
        Ok(SkillBinding {
            agent_id: row.get(0)?,
            package_id: row.get(1)?,
            sort_order: row.get(2)?,
            package_display_name: row.get(3)?,
        })
    })?;
    rows.collect()
}

/// 全量替换指定数字人的技能包绑定（事务内先删后插）；
/// package_id 按输入顺序去重，重复项以最后一次出现为准。
pub fn replace_bindings(
    conn: &Connection,
    agent_id: &str,
    bindings: Vec<(String, i32)>,
) -> Result<(), rusqlite::Error> {
    let mut seen = std::collections::HashSet::new();
    let deduped: Vec<(String, i32)> = bindings
        .into_iter()
        .rev()
        .filter(|(package_id, _)| seen.insert(package_id.clone()))
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();

    let now = Utc::now().to_rfc3339();
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "DELETE FROM agent_skill_bindings WHERE agent_id = ?1",
        params![agent_id],
    )?;
    for (package_id, sort_order) in &deduped {
        tx.execute(
            "INSERT INTO agent_skill_bindings (agent_id, package_id, sort_order, created_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![agent_id, package_id, sort_order, now],
        )?;
    }
    tx.commit()?;
    log::info!(
        target: "db",
        "replace_bindings agent_id={} count={}",
        agent_id,
        deduped.len()
    );
    Ok(())
}
