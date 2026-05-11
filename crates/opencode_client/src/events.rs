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
    SessionStatusChanged { session_id: String, status: String },

    /// An error occurred.
    Error { message: String },

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
///
/// Real SSE format from OpenCode server (v1.14+):
/// ```json
/// {"id":"evt_...","type":"message.part.delta","properties":{"sessionID":"...","messageID":"...","partID":"...","field":"text","delta":"Hi"}}
/// {"id":"evt_...","type":"message.part.updated","properties":{"sessionID":"...","part":{"type":"text","text":"Hi.","messageID":"...","id":"..."}}}
/// {"id":"evt_...","type":"message.updated","properties":{"sessionID":"...","info":{"id":"...","role":"assistant","finish":"stop",...}}}
/// {"id":"evt_...","type":"session.idle","properties":{"sessionID":"..."}}
/// ```
pub(crate) fn parse_sse_message(event_type: &str, data: &str) -> Result<Vec<OpenCodeEvent>> {
    let value: serde_json::Value =
        serde_json::from_str(data).context("failed to parse SSE data as JSON")?;

    let etype = value
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or(event_type);

    let props = value.get("properties").unwrap_or(&value);

    let session_id = || {
        props
            .get("sessionID")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    };

    let mut events = Vec::new();

    match etype {
        "server.connected" => {
            // Ignored — we already emit Connected from the SSE Open event.
            // Processing this duplicate would cause double StreamInit.
        }

        // ── Streaming text deltas ───────────────────────────────────
        // {"type":"message.part.delta","properties":{"sessionID","messageID","partID","field":"text","delta":"Hi"}}
        "message.part.delta" => {
            let field = props.get("field").and_then(|v| v.as_str()).unwrap_or("");
            if field == "text" {
                let delta = props
                    .get("delta")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let message_id = props
                    .get("messageID")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                if !delta.is_empty() {
                    events.push(OpenCodeEvent::TextDelta {
                        session_id: session_id(),
                        message_id,
                        text: delta,
                    });
                }
            }
        }

        // ── Part updates (tool calls, text completion, step-finish) ─
        // {"type":"message.part.updated","properties":{"sessionID","part":{"type":"tool-call"|"text"|"step-finish",...}}}
        "message.part.updated" => {
            if let Some(part) = props.get("part") {
                let part_type = part.get("type").and_then(|v| v.as_str()).unwrap_or("");
                let msg_id = part
                    .get("messageID")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                match part_type {
                    "tool-call" => {
                        let tool_name = part
                            .get("tool")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let tool_call_id = part
                            .get("toolCallId")
                            .or_else(|| part.get("id"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let state = part.get("state").and_then(|v| v.as_str()).unwrap_or("");
                        let args = part.get("args").cloned().unwrap_or(serde_json::Value::Null);

                        match state {
                            "completed" | "error" => {
                                let result = part
                                    .get("content")
                                    .cloned()
                                    .unwrap_or(serde_json::Value::Null);
                                events.push(OpenCodeEvent::ToolCallCompleted {
                                    session_id: session_id(),
                                    message_id: msg_id,
                                    tool_call_id,
                                    tool_name,
                                    result,
                                });
                            }
                            _ => {
                                events.push(OpenCodeEvent::ToolCallStarted {
                                    session_id: session_id(),
                                    message_id: msg_id,
                                    tool_call_id,
                                    tool_name,
                                    args,
                                });
                            }
                        }
                    }
                    // step-finish with reason "stop" means the message is done.
                    "step-finish" => {
                        // We'll let session.idle handle the final "finished" signal.
                    }
                    // text part update — full text (not delta). Ignore since we
                    // already stream via message.part.delta.
                    "text" => {}
                    _ => {
                        log::debug!("Unhandled message.part.updated type: {}", part_type);
                    }
                }
            }
        }

        // ── Message-level updates ───────────────────────────────────
        // {"type":"message.updated","properties":{"sessionID","info":{"id","role","finish":"stop",...}}}
        "message.updated" | "message.created" => {
            if let Some(info) = props.get("info") {
                let role = info.get("role").and_then(|v| v.as_str()).unwrap_or("");
                let finish = info.get("finish").and_then(|v| v.as_str());
                let msg_id = info
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                // Only emit MessageComplete when the assistant message has a
                // finish reason (i.e. the LLM is done).
                if role == "assistant" && finish.is_some() {
                    events.push(OpenCodeEvent::MessageComplete {
                        session_id: session_id(),
                        message_id: msg_id,
                        parts: vec![],
                    });
                }
            }
        }

        // ── Session status ──────────────────────────────────────────
        "session.status" => {
            if let Some(status) = props.get("status") {
                let status_type = status
                    .get("type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                events.push(OpenCodeEvent::SessionStatusChanged {
                    session_id: session_id(),
                    status: status_type,
                });
            }
        }

        "session.idle" => {
            events.push(OpenCodeEvent::SessionStatusChanged {
                session_id: session_id(),
                status: "idle".to_string(),
            });
        }

        "session.updated" | "session.created" | "session.diff" => {
            // Informational — no action needed.
        }

        "permission.requested" => {
            let permission_id = props
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let tool_name = props
                .get("tool")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let args = props
                .get("args")
                .cloned()
                .unwrap_or(serde_json::Value::Null);

            events.push(OpenCodeEvent::PermissionRequired {
                session_id: session_id(),
                permission_id,
                tool_name,
                args,
            });
        }

        _ => {
            log::debug!("Unhandled OpenCode event type: {}", etype);
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
