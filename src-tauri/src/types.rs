use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Person {
    pub id: String,
    pub name: String,
    pub aliases: Vec<String>,
    pub avatar: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub company: Option<String>,
    pub title: Option<String>,
    pub location: Option<String>,
    pub background: Option<String>,
    pub relationship_strength: Option<String>,
    pub resource_tags: Vec<String>,
    pub sensitivity_level: String,
    pub status: String,
    pub next_step: Option<String>,
    pub notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatePersonRequest {
    pub name: String,
    pub aliases: Vec<String>,
    pub avatar: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub company: Option<String>,
    pub title: Option<String>,
    pub location: Option<String>,
    pub background: Option<String>,
    pub relationship_strength: Option<String>,
    pub resource_tags: Vec<String>,
    pub sensitivity_level: String,
    pub status: Option<String>,
    pub next_step: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Relationship {
    pub id: String,
    pub from_person_id: String,
    pub to_person_id: String,
    pub relationship_type: String,
    pub strength: Option<String>,
    pub description: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateRelationshipRequest {
    pub from_person_id: String,
    pub to_person_id: String,
    pub relationship_type: String,
    pub strength: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Interaction {
    pub id: String,
    pub person_id: String,
    pub timestamp: String,
    pub content: String,
    pub summary: Option<String>,
    pub topics: Vec<String>,
    pub action_items: Vec<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateInteractionRequest {
    pub person_id: String,
    pub timestamp: String,
    pub content: String,
    pub summary: Option<String>,
    pub topics: Vec<String>,
    pub action_items: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntityMention {
    pub id: String,
    pub interaction_id: String,
    pub person_id: Option<String>,
    pub mention_text: String,
    pub confidence: f64,
    pub resolved: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateEntityMentionRequest {
    pub interaction_id: String,
    pub person_id: Option<String>,
    pub mention_text: String,
    pub confidence: f64,
    pub resolved: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphNode {
    pub id: String,
    pub label: String,
    pub sensitivity_level: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphEdge {
    pub id: String,
    pub source: String,
    pub target: String,
    pub label: String,
    pub strength: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphData {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}
