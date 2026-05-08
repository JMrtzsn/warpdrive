use std::pin::Pin;

use anyhow::{Context, Result};
use futures::stream::Stream;
use reqwest_eventsource::{Event as SseEvent, EventSource};

use crate::types::Part;

#[cfg(test)]
#[path = "events_tests.rs"]
mod tests;

/// High-level events emitted by the OpenCode event stream, mapped to concepts
/// that Warp's UI understands.
#[derive(Debug, Clone)]
pub enum OpenCodeEvent {
    /// Stream connected successfully. Contains session and message IDs if available.
    Connected,

    /// The assistant is producing text output (streaming token).
    TextDelta {
        session_id: String,
        message_id: String,
        text: String,
    },

    /// A tool call has started.
    ToolCallStarted {
        session_id: String,
        message_id: String,
        tool_call_id: String,
        tool_name: String,
        args: serde_json::Value,
    },

    /// A tool call completed with a result.
    ToolCallCompleted {
        session_id: String,
        message_id: String,
        tool_call_id: String,
        tool_name: String,
        result: serde_json::Value,
    },

    /// The assistant's response is complete.
    MessageComplete {
        session_id: String,
        message_id: String,
        parts: Vec<Part>,
    },

    /// A permission request — the agent needs user approval.
    PermissionRequired {
        session_id: String,
        permission_id: String,
        tool_name: String,
        args: serde_json::Value,
    },

    /// Session status changed (idle, running, error).
    SessionStatusChanged {
        session_id: String,
        status: String,
    },

    /// An error occurred.
    Error {
        message: String,
    },

    /// Stream ended.
    Closed,
}

/// A stream of parsed OpenCode events.
pub type OpenCodeEventStream = Pin<Box<dyn Stream<Item = OpenCodeEvent> + Send>>;

/// Subscribe to the OpenCode server event stream and parse into typed events.
pub fn subscribe_events(event_url: &str) -> OpenCodeEventStream {
    let url = event_url.to_string();
    let stream = async_stream::stream! {
        let mut es = EventSource::get(&url);

        loop {
            match es.next().await {
                Some(Ok(SseEvent::Open)) => {
                    yield OpenCodeEvent::Connected;
                }
                Some(Ok(SseEvent::Message(msg))) => {
                    match parse_sse_message(&msg.event, &msg.data) {
                        Ok(events) => {
                            for event in events {
                                yield event;
                            }
                        }
                        Err(e) => {
                            log::warn!("Failed to parse SSE event: {}", e);
                        }
                    }
                }
                Some(Err(e)) => {
                    yield OpenCodeEvent::Error {
                        message: format!("SSE error: {}", e),
                    };
                    break;
                }
                None => {
                    yield OpenCodeEvent::Closed;
                    break;
                }
            }
        }
    };

    Box::pin(stream)
}

/// Parse a raw SSE message into OpenCode events.
pub(crate) fn parse_sse_message(event_type: &str, data: &str) -> Result<Vec<OpenCodeEvent>> {
    let value: serde_json::Value =
        serde_json::from_str(data).context("failed to parse SSE data as JSON")?;

    let mut events = Vec::new();

    match event_type {
        // OpenCode emits events with type field in the JSON payload
        _ => {
            let etype = value
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or(event_type);

            let properties = value.get("properties").cloned().unwrap_or(value.clone());

            match etype {
                "session.updated" | "session.created" => {
                    if let Some(id) = properties.get("id").and_then(|v| v.as_str()) {
                        events.push(OpenCodeEvent::SessionStatusChanged {
                            session_id: id.to_string(),
                            status: properties
                                .get("status")
                                .and_then(|v| v.as_str())
                                .unwrap_or("idle")
                                .to_string(),
                        });
                    }
                }
                "message.updated" | "message.created" => {
                    let session_id = properties
                        .get("sessionID")
                        .or_else(|| properties.get("session_id"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let message_id = properties
                        .get("id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();

                    // Check for parts to detect tool calls and text
                    if let Some(parts) = properties.get("parts").and_then(|v| v.as_array()) {
                        for part in parts {
                            let part_type = part
                                .get("type")
                                .and_then(|v| v.as_str())
                                .unwrap_or("");

                            match part_type {
                                "text" => {
                                    if let Some(text) =
                                        part.get("content").and_then(|v| v.as_str())
                                    {
                                        events.push(OpenCodeEvent::TextDelta {
                                            session_id: session_id.clone(),
                                            message_id: message_id.clone(),
                                            text: text.to_string(),
                                        });
                                    }
                                }
                                "tool-call" => {
                                    let tool_name = part
                                        .get("tool")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("")
                                        .to_string();
                                    let tool_call_id = part
                                        .get("toolCallId")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("")
                                        .to_string();
                                    let state = part
                                        .get("state")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("");
                                    let args = part
                                        .get("args")
                                        .cloned()
                                        .unwrap_or(serde_json::Value::Null);

                                    match state {
                                        "completed" | "error" => {
                                            let result = part
                                                .get("content")
                                                .cloned()
                                                .unwrap_or(serde_json::Value::Null);
                                            events.push(OpenCodeEvent::ToolCallCompleted {
                                                session_id: session_id.clone(),
                                                message_id: message_id.clone(),
                                                tool_call_id,
                                                tool_name,
                                                result,
                                            });
                                        }
                                        _ => {
                                            events.push(OpenCodeEvent::ToolCallStarted {
                                                session_id: session_id.clone(),
                                                message_id: message_id.clone(),
                                                tool_call_id,
                                                tool_name,
                                                args,
                                            });
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
                "permission.requested" => {
                    let session_id = properties
                        .get("sessionID")
                        .or_else(|| properties.get("session_id"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let permission_id = properties
                        .get("id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let tool_name = properties
                        .get("tool")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let args = properties
                        .get("args")
                        .cloned()
                        .unwrap_or(serde_json::Value::Null);

                    events.push(OpenCodeEvent::PermissionRequired {
                        session_id,
                        permission_id,
                        tool_name,
                        args,
                    });
                }
                "server.connected" => {
                    events.push(OpenCodeEvent::Connected);
                }
                _ => {
                    log::debug!("Unhandled OpenCode event type: {}", etype);
                }
            }
        }
    }

    Ok(events)
}

/// Trait extension on EventSource for easier usage.
trait EventSourceExt {
    async fn next(&mut self) -> Option<Result<SseEvent, reqwest_eventsource::Error>>;
}

impl EventSourceExt for EventSource {
    async fn next(&mut self) -> Option<Result<SseEvent, reqwest_eventsource::Error>> {
        use futures::StreamExt;
        StreamExt::next(self).await
    }
}
