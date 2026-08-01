#[cfg(test)]
mod tests {
    use crate::db::{person, schema};
    use crate::types::CreatePersonRequest;
    use rusqlite::Connection;

    fn in_memory_db() -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory db");
        schema::migrate(&conn).expect("schema migration");
        conn
    }

    #[test]
    fn creates_and_lists_person() {
        let conn = in_memory_db();
        let created = person::create(
            &conn,
            CreatePersonRequest {
                name: "张三".to_string(),
                aliases: vec!["老张".to_string()],
                avatar: None,
                phone: Some("13800000000".to_string()),
                email: None,
                company: Some("某公司".to_string()),
                title: Some("工程师".to_string()),
                location: Some("上海".to_string()),
                background: Some("通过朋友介绍认识".to_string()),
                relationship_strength: Some("strong".to_string()),
                resource_tags: vec!["地产".to_string(), "融资".to_string()],
                sensitivity_level: "low".to_string(),
                status: Some("active".to_string()),
                next_step: Some("约饭聊项目".to_string()),
                notes: None,
            },
        )
        .expect("create person");

        let persons = person::list_all(&conn).expect("list persons");
        assert_eq!(persons.len(), 1);
        assert_eq!(persons[0].id, created.id);
        assert_eq!(persons[0].aliases, vec!["老张"]);
        assert_eq!(persons[0].resource_tags, vec!["地产", "融资"]);
    }
}
