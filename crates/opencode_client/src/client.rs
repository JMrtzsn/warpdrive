use anyhow::{Context, Result};
use reqwest::Client;

use crate::types::*;

/// HTTP client for the OpenCode server API.
#[derive(Debug, Clone)]
pub struct OpenCodeClient {
    base_url: String,
    http: Client,
}

impl OpenCodeClient {
    /// Create a new client pointing at the given OpenCode server.
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            http: Client::new(),
        }
    }

    /// Create with explicit reqwest Client (for custom TLS, timeouts, etc).
    pub fn with_client(base_url: impl Into<String>, http: Client) -> Self {
        Self {
            base_url: base_url.into(),
            http,
        }
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    // ── Health ──────────────────────────────────────────────────────────

    pub async fn health(&self) -> Result<HealthResponse> {
        let resp = self
            .http
            .get(format!("{}/global/health", self.base_url))
            .send()
            .await
            .context("health check request failed")?
            .error_for_status()
            .context("health check returned error status")?;
        resp.json().await.context("failed to parse health response")
    }

    // ── Sessions ────────────────────────────────────────────────────────

    pub async fn create_session(&self, req: &CreateSessionRequest) -> Result<Session> {
        let resp = self
            .http
            .post(format!("{}/session", self.base_url))
            .json(req)
            .send()
            .await
            .context("create session request failed")?
            .error_for_status()
            .context("create session returned error status")?;
        resp.json().await.context("failed to parse session response")
    }

    pub async fn get_session(&self, session_id: &str) -> Result<Session> {
        let resp = self
            .http
            .get(format!("{}/session/{}", self.base_url, session_id))
            .send()
            .await
            .context("get session request failed")?
            .error_for_status()?;
        resp.json().await.context("failed to parse session")
    }

    pub async fn list_sessions(&self) -> Result<Vec<Session>> {
        let resp = self
            .http
            .get(format!("{}/session", self.base_url))
            .send()
            .await
            .context("list sessions request failed")?
            .error_for_status()?;
        resp.json().await.context("failed to parse sessions list")
    }

    pub async fn delete_session(&self, session_id: &str) -> Result<bool> {
        let resp = self
            .http
            .delete(format!("{}/session/{}", self.base_url, session_id))
            .send()
            .await
            .context("delete session request failed")?
            .error_for_status()?;
        resp.json().await.context("failed to parse delete response")
    }

    pub async fn abort_session(&self, session_id: &str) -> Result<bool> {
        let resp = self
            .http
            .post(format!("{}/session/{}/abort", self.base_url, session_id))
            .send()
            .await
            .context("abort session request failed")?
            .error_for_status()?;
        resp.json().await.context("failed to parse abort response")
    }

    // ── Messages ────────────────────────────────────────────────────────

    /// Send a prompt and return the raw response body as a string (for debugging).
    pub async fn raw_prompt(
        &self,
        session_id: &str,
        req: &PromptRequest,
    ) -> Result<String> {
        let resp = self
            .http
            .post(format!("{}/session/{}/message", self.base_url, session_id))
            .json(req)
            .send()
            .await
            .context("prompt request failed")?
            .error_for_status()
            .context("prompt returned error status")?;
        resp.text().await.context("failed to read prompt response body")
    }

    /// Send a prompt and wait for the full response (synchronous mode).
    pub async fn prompt(
        &self,
        session_id: &str,
        req: &PromptRequest,
    ) -> Result<MessageResponse> {
        let resp = self
            .http
            .post(format!("{}/session/{}/message", self.base_url, session_id))
            .json(req)
            .send()
            .await
            .context("prompt request failed")?
            .error_for_status()
            .context("prompt returned error status")?;
        resp.json().await.context("failed to parse prompt response")
    }

    /// Send a prompt asynchronously (returns immediately, results arrive via events).
    pub async fn prompt_async(&self, session_id: &str, req: &PromptRequest) -> Result<()> {
        self.http
            .post(format!(
                "{}/session/{}/prompt_async",
                self.base_url, session_id
            ))
            .json(req)
            .send()
            .await
            .context("async prompt request failed")?
            .error_for_status()
            .context("async prompt returned error status")?;
        Ok(())
    }

    /// Get all messages in a session.
    pub async fn list_messages(&self, session_id: &str) -> Result<Vec<MessageResponse>> {
        let resp = self
            .http
            .get(format!("{}/session/{}/message", self.base_url, session_id))
            .send()
            .await
            .context("list messages request failed")?
            .error_for_status()?;
        resp.json()
            .await
            .context("failed to parse messages response")
    }

    /// Get a specific message.
    pub async fn get_message(
        &self,
        session_id: &str,
        message_id: &str,
    ) -> Result<MessageResponse> {
        let resp = self
            .http
            .get(format!(
                "{}/session/{}/message/{}",
                self.base_url, session_id, message_id
            ))
            .send()
            .await
            .context("get message request failed")?
            .error_for_status()?;
        resp.json().await.context("failed to parse message response")
    }

    // ── Permissions ─────────────────────────────────────────────────────

    /// Respond to a permission request from the agent.
    pub async fn respond_to_permission(
        &self,
        session_id: &str,
        permission_id: &str,
        response: &PermissionResponse,
    ) -> Result<bool> {
        let resp = self
            .http
            .post(format!(
                "{}/session/{}/permissions/{}",
                self.base_url, session_id, permission_id
            ))
            .json(response)
            .send()
            .await
            .context("permission response request failed")?
            .error_for_status()?;
        resp.json()
            .await
            .context("failed to parse permission response")
    }

    // ── Config ──────────────────────────────────────────────────────────

    pub async fn get_config(&self) -> Result<serde_json::Value> {
        let resp = self
            .http
            .get(format!("{}/config", self.base_url))
            .send()
            .await
            .context("get config request failed")?
            .error_for_status()?;
        resp.json().await.context("failed to parse config")
    }

    pub async fn list_agents(&self) -> Result<serde_json::Value> {
        let resp = self
            .http
            .get(format!("{}/agent", self.base_url))
            .send()
            .await
            .context("list agents request failed")?
            .error_for_status()?;
        resp.json().await.context("failed to parse agents list")
    }

    /// Get the SSE event stream URL (caller uses reqwest-eventsource directly).
    pub fn event_stream_url(&self) -> String {
        format!("{}/event", self.base_url)
    }

    pub fn global_event_stream_url(&self) -> String {
        format!("{}/global/event", self.base_url)
    }
}

#[cfg(test)]
#[path = "client_tests.rs"]
mod tests;
