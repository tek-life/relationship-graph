//! 关系推断引擎（v1.3 设计）：基于既有资料生成"可能认识"的候选关系，
//! 全部以 pending 状态入库，必须由用户确认后才视为事实（关键决策 #8）。
//! 推断规则与置信度：
//! - 同一公司            → 0.8（colleague）
//! - 同一介绍人介绍认识  → 0.6（other）
//! - 同行业标签 + 同城   → 0.5（other）
//! - 同一学校            → 0.5（other）
//! - 同一项目            → 0.6（other）

use crate::db::relationship;
use rusqlite::{params, Connection};
use std::collections::HashMap;

/// 单次推断最多入库条数，防止大公司组合爆炸刷屏
const MAX_CREATED_PER_RUN: usize = 200;
/// 参与组合的分组人数上限（超过视为"大公司/大标签"，两两组合意义弱）
const MAX_GROUP_SIZE: usize = 15;

struct PersonLite {
    id: String,
    company: Option<String>,
    location: Option<String>,
    tags: Vec<String>,
    school: Option<String>,
    projects: Vec<String>,
}

pub fn run(conn: &Connection, owner_id: &str) -> Result<usize, rusqlite::Error> {
    let persons = load_persons(conn, owner_id)?;
    let mut created = 0usize;

    // 规则一：同公司
    let mut by_company: HashMap<String, Vec<&PersonLite>> = HashMap::new();
    for person in &persons {
        if let Some(company) = person.company.as_deref() {
            let key = company.trim();
            if !key.is_empty() {
                by_company.entry(key.to_string()).or_default().push(person);
            }
        }
    }
    for (company, members) in &by_company {
        if members.len() < 2 || members.len() > MAX_GROUP_SIZE {
            continue;
        }
        for i in 0..members.len() {
            for j in (i + 1)..members.len() {
                if created >= MAX_CREATED_PER_RUN {
                    return finish(created);
                }
                created += try_create(
                    conn,
                    owner_id,
                    &members[i].id,
                    &members[j].id,
                    "colleague",
                    0.8,
                    &format!("同一公司：{}", company),
                )?;
            }
        }
    }

    // 规则二：同一介绍人（既有 introduced 关系中，同一起点介绍认识的人互相可能认识）
    let mut by_introducer: HashMap<String, Vec<String>> = HashMap::new();
    {
        let mut stmt = conn.prepare(
            "SELECT from_person_id, to_person_id FROM relationships
             WHERE relationship_type = 'introduced' AND confirmation_status != 'rejected'
               AND from_person_id IN (SELECT id FROM persons WHERE owner_id = ?1)",
        )?;
        let rows = stmt.query_map(params![owner_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        for row in rows {
            let (from, to) = row?;
            by_introducer.entry(from).or_default().push(to);
        }
    }
    for introduced in by_introducer.values() {
        if introduced.len() < 2 || introduced.len() > MAX_GROUP_SIZE {
            continue;
        }
        for i in 0..introduced.len() {
            for j in (i + 1)..introduced.len() {
                if created >= MAX_CREATED_PER_RUN {
                    return finish(created);
                }
                created += try_create(
                    conn,
                    owner_id,
                    &introduced[i],
                    &introduced[j],
                    "other",
                    0.6,
                    "由同一位介绍人认识",
                )?;
            }
        }
    }

    // 规则三：同行业标签 + 同城
    let mut by_tag_city: HashMap<(String, String), Vec<&PersonLite>> = HashMap::new();
    for person in &persons {
        let Some(city) = person.location.as_deref() else { continue };
        let city = city.trim();
        if city.is_empty() {
            continue;
        }
        for tag in &person.tags {
            by_tag_city
                .entry((tag.clone(), city.to_string()))
                .or_default()
                .push(person);
        }
    }
    for ((tag, city), members) in &by_tag_city {
        if members.len() < 2 || members.len() > MAX_GROUP_SIZE {
            continue;
        }
        for i in 0..members.len() {
            for j in (i + 1)..members.len() {
                if created >= MAX_CREATED_PER_RUN {
                    return finish(created);
                }
                created += try_create(
                    conn,
                    owner_id,
                    &members[i].id,
                    &members[j].id,
                    "other",
                    0.5,
                    &format!("同行业（{}）且同在{}", tag, city),
                )?;
            }
        }
    }

    // 规则四：同学校
    let mut by_school: HashMap<String, Vec<&PersonLite>> = HashMap::new();
    for person in &persons {
        if let Some(school) = person.school.as_deref() {
            let key = school.trim();
            if !key.is_empty() {
                by_school.entry(key.to_string()).or_default().push(person);
            }
        }
    }
    for (school, members) in &by_school {
        if members.len() < 2 || members.len() > MAX_GROUP_SIZE {
            continue;
        }
        for i in 0..members.len() {
            for j in (i + 1)..members.len() {
                if created >= MAX_CREATED_PER_RUN {
                    return finish(created);
                }
                created += try_create(
                    conn,
                    owner_id,
                    &members[i].id,
                    &members[j].id,
                    "other",
                    0.5,
                    &format!("同学校：{}", school),
                )?;
            }
        }
    }

    // 规则五：同项目（projects 为多值，按每个项目分组）
    let mut by_project: HashMap<String, Vec<&PersonLite>> = HashMap::new();
    for person in &persons {
        for project in &person.projects {
            let key = project.trim();
            if !key.is_empty() {
                by_project.entry(key.to_string()).or_default().push(person);
            }
        }
    }
    for (project, members) in &by_project {
        if members.len() < 2 || members.len() > MAX_GROUP_SIZE {
            continue;
        }
        for i in 0..members.len() {
            for j in (i + 1)..members.len() {
                if created >= MAX_CREATED_PER_RUN {
                    return finish(created);
                }
                created += try_create(
                    conn,
                    owner_id,
                    &members[i].id,
                    &members[j].id,
                    "other",
                    0.6,
                    &format!("同项目：{}", project),
                )?;
            }
        }
    }

    finish(created)
}

fn finish(created: usize) -> Result<usize, rusqlite::Error> {
    log::info!(target: "infer", "infer_relationships_success created={}", created);
    Ok(created)
}

fn try_create(
    conn: &Connection,
    owner_id: &str,
    a: &str,
    b: &str,
    relationship_type: &str,
    confidence: f64,
    reason: &str,
) -> Result<usize, rusqlite::Error> {
    if relationship::exists_between(conn, a, b)? {
        return Ok(0);
    }
    relationship::create_inferred(conn, owner_id, a, b, relationship_type, confidence, reason)?;
    Ok(1)
}

fn load_persons(conn: &Connection, owner_id: &str) -> Result<Vec<PersonLite>, rusqlite::Error> {
    let mut stmt = conn.prepare("SELECT id, company, location, resource_tags, school, projects FROM persons WHERE owner_id = ?1")?;
    let rows = stmt.query_map(params![owner_id], |row| {
        let tags_json: String = row.get(3)?;
        let projects_json: Option<String> = row.get(5)?;
        Ok(PersonLite {
            id: row.get(0)?,
            company: row.get(1)?,
            location: row.get(2)?,
            tags: serde_json::from_str(&tags_json).unwrap_or_default(),
            school: row.get(4)?,
            projects: projects_json
                .as_deref()
                .map(|json| serde_json::from_str(json).unwrap_or_default())
                .unwrap_or_default(),
        })
    })?;
    rows.collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{person, relationship, schema};
    use crate::types::CreatePersonRequest;

    fn person_req(name: &str, company: Option<&str>, location: Option<&str>, tags: &[&str]) -> CreatePersonRequest {
        CreatePersonRequest {
            name: name.to_string(),
            aliases: vec![],
            avatar: None,
            phone: None,
            email: None,
            company: company.map(str::to_string),
            title: None,
            location: location.map(str::to_string),
            background: None,
            relationship_strength: None,
            resource_tags: tags.iter().map(|t| t.to_string()).collect(),
            sensitivity_level: "low".to_string(),
            status: None,
            next_step: None,
            notes: None,
            school: None,
            projects: vec![],
        }
    }

    fn person_req_edu(name: &str, school: Option<&str>, projects: &[&str]) -> CreatePersonRequest {
        let mut req = person_req(name, None, None, &[]);
        req.school = school.map(str::to_string);
        req.projects = projects.iter().map(|p| p.to_string()).collect();
        req
    }

    #[test]
    fn infers_same_company_and_skips_existing() {
        let conn = Connection::open_in_memory().unwrap();
        schema::migrate(&conn).unwrap();

        let a = person::create(&conn, "owner-a", person_req("甲", Some("万科"), Some("上海"), &["地产"])).unwrap();
        let b = person::create(&conn, "owner-a", person_req("乙", Some("万科"), Some("北京"), &[])).unwrap();
        let _c = person::create(&conn, "owner-a", person_req("丙", Some("龙湖"), None, &[])).unwrap();

        let created = run(&conn, "owner-a").unwrap();
        assert_eq!(created, 1, "只应推断出甲乙同公司一条");

        let pending = relationship::list_pending(&conn, "owner-a").unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].source, "inferred");
        assert!(pending[0].inference_reason.as_deref().unwrap().contains("万科"));

        // 再跑一次不应重复创建
        assert_eq!(run(&conn, "owner-a").unwrap(), 0);

        // 另一用户不应看到任何推断结果（隔离）
        assert_eq!(run(&conn, "owner-b").unwrap(), 0);

        // 否认后也不应再生成
        relationship::set_confirmation(&conn, "owner-a", &pending[0].id, "rejected").unwrap();
        assert_eq!(run(&conn, "owner-a").unwrap(), 0);
        let _ = (a, b);
    }

    #[test]
    fn infers_same_tag_and_city() {
        let conn = Connection::open_in_memory().unwrap();
        schema::migrate(&conn).unwrap();

        person::create(&conn, "owner-a", person_req("甲", None, Some("上海"), &["地产"])).unwrap();
        person::create(&conn, "owner-a", person_req("乙", None, Some("上海"), &["地产"])).unwrap();
        person::create(&conn, "owner-a", person_req("丙", None, Some("北京"), &["地产"])).unwrap();

        let created = run(&conn, "owner-a").unwrap();
        assert_eq!(created, 1);
        let pending = relationship::list_pending(&conn, "owner-a").unwrap();
        assert!(pending[0].inference_reason.as_deref().unwrap().contains("上海"));
    }

    #[test]
    fn infers_same_school() {
        let conn = Connection::open_in_memory().unwrap();
        schema::migrate(&conn).unwrap();

        person::create(&conn, "owner-a", person_req_edu("甲", Some("复旦大学"), &[])).unwrap();
        person::create(&conn, "owner-a", person_req_edu("乙", Some("复旦大学"), &[])).unwrap();
        person::create(&conn, "owner-a", person_req_edu("丙", Some("交通大学"), &[])).unwrap();

        let created = run(&conn, "owner-a").unwrap();
        assert_eq!(created, 1, "只应推断出甲乙同学校一条");

        let pending = relationship::list_pending(&conn, "owner-a").unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].confidence, Some(0.5));
        assert_eq!(pending[0].inference_reason.as_deref(), Some("同学校：复旦大学"));

        // 再跑一次不应重复创建
        assert_eq!(run(&conn, "owner-a").unwrap(), 0);
    }

    #[test]
    fn infers_same_project_with_multiple_projects() {
        let conn = Connection::open_in_memory().unwrap();
        schema::migrate(&conn).unwrap();

        person::create(&conn, "owner-a", person_req_edu("甲", None, &["旧改项目", "数据中台"])).unwrap();
        person::create(&conn, "owner-a", person_req_edu("乙", None, &["旧改项目"])).unwrap();
        person::create(&conn, "owner-a", person_req_edu("丙", None, &["数据中台"])).unwrap();
        person::create(&conn, "owner-a", person_req_edu("丁", None, &["独立项目"])).unwrap();

        // 甲-乙（旧改项目）、甲-丙（数据中台）两条
        let created = run(&conn, "owner-a").unwrap();
        assert_eq!(created, 2);

        let pending = relationship::list_pending(&conn, "owner-a").unwrap();
        assert_eq!(pending.len(), 2);
        for rel in &pending {
            assert_eq!(rel.confidence, Some(0.6));
            assert!(rel.inference_reason.as_deref().unwrap().starts_with("同项目："));
        }

        // 再跑一次不应重复创建
        assert_eq!(run(&conn, "owner-a").unwrap(), 0);
    }
}
