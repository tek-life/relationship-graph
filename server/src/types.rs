use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatRequest {
    pub query: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatResponse {
    pub reply: String,
}

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
    pub school: Option<String>,
    pub projects: Vec<String>,
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
    pub school: Option<String>,
    #[serde(default)]
    pub projects: Vec<String>,
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
    pub source: String,
    pub confidence: Option<f64>,
    pub confirmation_status: String,
    pub inference_reason: Option<String>,
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
    pub edge_source: String,
    pub confirmation_status: String,
    pub confidence: Option<f64>,
    pub inference_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphData {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

// === NLQ 多意图响应 ===

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "intentType", rename_all = "camelCase")]
pub enum NlqResponse {
    SearchPeople { results: Vec<crate::nlq::NlqResult> },
    CreatePersonDraft { draft: PersonDraft },
    UpdatePersonDraft { draft: UpdateDraft },
    AddInteractionDraft { draft: InteractionDraft },
    FindPath { path: PathData },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NlqMultiRequest {
    pub query: String,
    pub reveal_sensitive: Option<bool>,
    pub route_mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonDraft {
    pub name: String,
    pub company: Option<String>,
    pub location: Option<String>,
    pub title: Option<String>,
    pub resource_tags: Vec<String>,
    pub background: Option<String>,
    pub school: Option<String>,
    pub confidence: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateDraft {
    pub target_person: Option<Person>,
    pub candidates: Vec<Person>,
    pub changes: Vec<FieldChange>,
    pub confidence: u8,
    pub error_hint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldChange {
    pub field: String,
    pub old_value: Option<String>,
    pub new_value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InteractionDraft {
    pub person_mention: String,
    pub resolved_person: Option<Person>,
    pub candidates: Vec<Person>,
    pub topic: Option<String>,
    pub summary: Option<String>,
    pub action_items: Vec<String>,
    pub confidence: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PathData {
    pub nodes: Vec<PathNode>,
    pub edges: Vec<PathEdge>,
    pub hops: usize,
    pub includes_pending: bool,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PathNode {
    pub id: String,
    pub name: String,
    pub company: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PathEdge {
    pub from_id: String,
    pub to_id: String,
    pub relationship_type: String,
    pub strength: Option<String>,
    pub confirmation_status: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NlqConfirmRequest {
    pub intent_type: String,
    pub data: serde_json::Value,
}

// === 用户与邀请 ===

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub id: String,
    pub username: String,
    pub password_hash: String,
    pub display_name: Option<String>,
    pub role: String,
    pub profile_doc: Option<String>,
    pub profile_completed: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateUserRequest {
    pub username: String,
    pub password_hash: String,
    pub display_name: Option<String>,
    pub role: Option<String>,
    pub profile_doc: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InviteToken {
    pub id: String,
    pub token: String,
    pub created_by: String,
    pub used_by: Option<String>,
    pub expires_at: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateInviteTokenRequest {
    pub token: String,
    pub created_by: String,
    pub expires_at: String,
}

// === 会话与消息 ===

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub id: String,
    pub user_id: String,
    pub title: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSessionRequest {
    pub user_id: String,
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessage {
    pub id: String,
    pub session_id: String,
    pub role: String,
    pub content: String,
    pub metadata_json: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateChatMessageRequest {
    pub session_id: String,
    pub role: String,
    pub content: String,
    pub metadata_json: Option<String>,
}

// === 数字人配置 ===

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DigitalAgent {
    pub id: String,
    pub display_name: String,
    pub mention: String,
    pub aliases: Vec<String>,
    pub route_mode: String,
    pub avatar_url: Option<String>,
    pub description: Option<String>,
    pub skill_description: Option<String>,
    pub is_active: bool,
    pub sort_order: i32,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateDigitalAgentRequest {
    pub display_name: String,
    pub mention: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    pub route_mode: Option<String>,
    pub avatar_url: Option<String>,
    pub description: Option<String>,
    pub skill_description: Option<String>,
    pub is_active: Option<bool>,
    pub sort_order: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSkill {
    pub id: String,
    pub agent_id: String,
    pub skill_name: String,
    pub skill_config_json: String,
    pub trigger_scenario: Option<String>,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateAgentSkillRequest {
    pub agent_id: String,
    pub skill_name: String,
    pub skill_config_json: String,
    pub trigger_scenario: Option<String>,
    pub is_active: Option<bool>,
}

// === Profile QA 指令配置 ===

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QaInstructionModule {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub system_prompt: String,
    pub guidance_text: Option<String>,
    pub sort_order: i32,
    pub trigger_scenario: String,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateQaInstructionModuleRequest {
    pub name: String,
    pub description: Option<String>,
    pub system_prompt: String,
    pub guidance_text: Option<String>,
    pub sort_order: Option<i32>,
    pub trigger_scenario: Option<String>,
    pub is_active: Option<bool>,
}
