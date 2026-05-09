use serde::{Deserialize, Serialize};

/// OpenCode session.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub id: String,
    pub title: Option<String>,
    pub parent_id: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

/// A message part sent to OpenCode.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum MessagePart {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image")]
    Image { url: String },
}

/// Model selection for a prompt.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelSelection {
    #[serde(rename = "providerID")]
    pub provider_id: String,
    #[serde(rename = "modelID")]
    pub model_id: String,
}

/// Request body for POST /session/:id/message
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptRequest {
    pub parts: Vec<MessagePart>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<ModelSelection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub no_reply: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<String>>,
}

/// A message returned from OpenCode.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub id: String,
    pub role: String,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

/// A part of an assistant message (tool call, text output, etc).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Part {
    #[serde(rename = "type")]
    pub part_type: String,
    /// Text content for "text" parts.
    #[serde(default)]
    pub text: Option<String>,
    /// Tool name if this is a tool-call or tool-result part.
    #[serde(default)]
    pub tool: Option<String>,
    /// Tool call ID for correlation.
    #[serde(rename = "toolCallId", default)]
    pub tool_call_id: Option<String>,
    /// The state of this part (e.g. "running", "completed", "error").
    #[serde(default)]
    pub state: Option<String>,
    /// Arbitrary content — structured data for tool calls/results.
    #[serde(default)]
    pub content: serde_json::Value,
    /// Tool input arguments.
    #[serde(default)]
    pub args: Option<serde_json::Value>,
    /// Part ID assigned by OpenCode.
    #[serde(default)]
    pub id: Option<String>,
    /// Session ID.
    #[serde(rename = "sessionID", default)]
    pub session_id: Option<String>,
    /// Message ID this part belongs to.
    #[serde(rename = "messageID", default)]
    pub message_id: Option<String>,
}

/// Response from GET /session/:id/message and POST /session/:id/message.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageResponse {
    pub info: Message,
    pub parts: Vec<Part>,
}

/// Health check response.
#[derive(Debug, Clone, Deserialize)]
pub struct HealthResponse {
    pub healthy: bool,
    pub version: String,
}

/// SSE event from /event or /global/event.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    #[serde(default)]
    pub properties: serde_json::Value,
}

/// Session status.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SessionStatus {
    Idle,
    Running,
    Error,
}

/// Create session request body.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSessionRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
}

/// Permission response body.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionResponse {
    pub response: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remember: Option<bool>,
}
