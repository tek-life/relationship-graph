use crate::db::{get_conn, person, relationship, AppState};
use crate::security::sensitivity;
use crate::types::{GraphData, GraphEdge, GraphNode};
use std::time::Instant;
use tauri::State;

#[tauri::command]
pub fn get_graph_data(state: State<AppState>) -> Result<GraphData, String> {
    let started = Instant::now();
    log::info!(target: "graph_cmd", "get_graph_data_start");
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = get_conn(&guard)?;

    let persons = person::list_all(conn).map_err(|e| e.to_string())?;
    let relationships = relationship::list_all(conn).map_err(|e| e.to_string())?;

    let high_sensitive_count = persons.iter().filter(|p| p.sensitivity_level == "high").count();
    let medium_sensitive_count = persons.iter().filter(|p| p.sensitivity_level == "medium").count();

    let nodes = persons
        .into_iter()
        .map(|p| GraphNode {
            id: p.id,
            label: sensitivity::display_name(&p.name, &p.aliases, &p.sensitivity_level, false),
            sensitivity_level: p.sensitivity_level,
            status: p.status,
        })
        .collect();

    let edges = relationships
        .into_iter()
        .map(|r| GraphEdge {
            id: r.id,
            source: r.from_person_id,
            target: r.to_person_id,
            label: r.relationship_type,
            strength: r.strength,
        })
        .collect();

    log::info!(
        target: "graph_cmd",
        "get_graph_data_success nodes={} edges={} high_sensitive={} medium_sensitive={} elapsed_ms={}",
        nodes.len(),
        edges.len(),
        high_sensitive_count,
        medium_sensitive_count,
        started.elapsed().as_millis()
    );

    Ok(GraphData { nodes, edges })
}
