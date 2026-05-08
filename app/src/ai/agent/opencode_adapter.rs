//! OpenCode adapter — drop-in replacement for the Warp server agent backend.
//!
//! Provides `generate_opencode_output` with the same return type as
//! `generate_multi_agent_output`, allowing the existing `ResponseStream` /
//! `BlocklistAIController` pipeline to work unchanged.

use std::sync::Arc;

use futures::stream::StreamExt;
use opencode_client::{
    map_tool_call, subscribe_events, CreateSessionRequest, MappedAction, MessagePart,
    ModelSelection, OpenCodeClient, OpenCodeEvent, PromptRequest, SidecarManager,
};
use tokio::sync::Mutex;
use warp_multi_agent_api as api;

use super::api::{ConvertToAPITypeError, RequestParams, ResponseStream as ApiResponseStream};

/// Persistent state for the OpenCode integration.
pub struct OpenCodeAdapter {
    pub sidecar: SidecarManager,
    client: OpenCodeClient,
    session_id: Option<String>,
}

impl OpenCodeAdapter {
    pub fn new(sidecar: SidecarManager) -> Self {
        let client = sidecar.client();
        Self {
            sidecar,
            client,
            session_id: None,
        }
    }

    pub async fn ensure_session(&mut self) -> anyhow::Result<String> {
        if let Some(id) = &self.session_id {
            return Ok(id.clone());
        }
        let session = self
            .client
            .create_session(&CreateSessionRequest {
                title: Some("Warp Agent Session".to_string()),
                parent_id: None,
            })
            .await?;
        self.session_id = Some(session.id.clone());
        Ok(session.id)
    }

    pub fn reset_session(&mut self) {
        self.session_id = None;
    }
}

/// Drop-in replacement for `generate_multi_agent_output`.
pub async fn generate_opencode_output(
    adapter: Arc<Mutex<OpenCodeAdapter>>,
    params: RequestParams,
    cancellation_rx: futures::channel::oneshot::Receiver<()>,
) -> Result<ApiResponseStream, ConvertToAPITypeError> {
    let mut guard = adapter.lock().await;

    if !guard.sidecar.is_running().await {
        guard
            .sidecar
            .start()
            .await
            .map_err(|e| ConvertToAPITypeError::Other(e.into()))?;
    }

    let session_id = guard
        .ensure_session()
        .await
        .map_err(|e| ConvertToAPITypeError::Other(e.into()))?;

    let user_text = extract_user_text(&params);
    let model_id = params.model.to_string();

    let prompt_req = PromptRequest {
        parts: vec![MessagePart::Text { text: user_text }],
        model: Some(ModelSelection {
            provider_id: infer_provider(&model_id),
            model_id: model_id.clone(),
        }),
        agent: None,
        no_reply: None,
        system: None,
        tools: None,
    };

    let client = guard.client.clone();
    client
        .prompt_async(&session_id, &prompt_req)
        .await
        .map_err(|e| ConvertToAPITypeError::Other(e.into()))?;

    let event_url = client.event_stream_url();
    let session_id_clone = session_id.clone();
    drop(guard);

    let event_stream = subscribe_events(&event_url);
    let mapped_stream = event_stream
        .filter_map(move |event| {
            let sid = session_id_clone.clone();
            async move { translate_event(event, &sid) }
        })
        .map(Ok)
        .take_until(cancellation_rx);

    Ok(Box::pin(mapped_stream))
}

// ─── Event translation ──────────────────────────────────────────────────────

fn translate_event(event: OpenCodeEvent, _session_id: &str) -> Option<api::ResponseEvent> {
    match event {
        OpenCodeEvent::Connected => Some(api::ResponseEvent {
            r#type: Some(api::response_event::Type::Init(
                api::response_event::StreamInit {
                    request_id: uuid::Uuid::new_v4().to_string(),
                    conversation_id: String::new(),
                    run_id: uuid::Uuid::new_v4().to_string(),
                },
            )),
        }),

        OpenCodeEvent::TextDelta {
            message_id, text, ..
        } => {
            let msg = api::Message {
                id: message_id,
                task_id: String::new(),
                request_id: String::new(),
                timestamp: None,
                server_message_data: String::new(),
                citations: vec![],
                message: Some(api::message::Message::AgentOutput(
                    api::message::AgentOutput { text },
                )),
            };
            Some(wrap_in_client_actions(msg))
        }

        OpenCodeEvent::ToolCallStarted {
            message_id,
            tool_call_id,
            tool_name,
            args,
            ..
        } => {
            let tool = map_to_proto_tool(&tool_name, &args);
            let msg = api::Message {
                id: message_id,
                task_id: String::new(),
                request_id: String::new(),
                timestamp: None,
                server_message_data: String::new(),
                citations: vec![],
                message: Some(api::message::Message::ToolCall(api::message::ToolCall {
                    tool_call_id,
                    tool: Some(tool),
                })),
            };
            Some(wrap_in_client_actions(msg))
        }

        OpenCodeEvent::ToolCallCompleted {
            message_id,
            tool_call_id,
            result,
            ..
        } => {
            // ToolCallResult uses a `result` oneof. We use ServerResult as a generic
            // container for the serialized result from OpenCode.
            let msg = api::Message {
                id: message_id,
                task_id: String::new(),
                request_id: String::new(),
                timestamp: None,
                server_message_data: String::new(),
                citations: vec![],
                message: Some(api::message::Message::ToolCallResult(
                    api::message::ToolCallResult {
                        tool_call_id,
                        context: None,
                        result: Some(api::message::tool_call_result::Result::Server(
                            api::message::tool_call_result::ServerResult {
                                serialized_result: serde_json::to_string(&result)
                                    .unwrap_or_default(),
                            },
                        )),
                    },
                )),
            };
            Some(wrap_in_client_actions(msg))
        }

        OpenCodeEvent::MessageComplete { .. } | OpenCodeEvent::Closed => {
            Some(api::ResponseEvent {
                r#type: Some(api::response_event::Type::Finished(
                    api::response_event::StreamFinished {
                        token_usage: vec![],
                        should_refresh_model_config: false,
                        request_cost: None,
                        conversation_usage_metadata: None,
                        reason: Some(api::response_event::stream_finished::Reason::Done(
                            api::response_event::stream_finished::Done {},
                        )),
                    },
                )),
            })
        }

        OpenCodeEvent::Error { message } => Some(api::ResponseEvent {
            r#type: Some(api::response_event::Type::Finished(
                api::response_event::StreamFinished {
                    token_usage: vec![],
                    should_refresh_model_config: false,
                    request_cost: None,
                    conversation_usage_metadata: None,
                    reason: Some(
                        api::response_event::stream_finished::Reason::InternalError(
                            api::response_event::stream_finished::InternalError { message },
                        ),
                    ),
                },
            )),
        }),

        _ => None,
    }
}

fn wrap_in_client_actions(message: api::Message) -> api::ResponseEvent {
    api::ResponseEvent {
        r#type: Some(api::response_event::Type::ClientActions(
            api::response_event::ClientActions {
                actions: vec![api::ClientAction {
                    action: Some(api::client_action::Action::AddMessagesToTask(
                        api::client_action::AddMessagesToTask {
                            task_id: String::new(),
                            messages: vec![message],
                        },
                    )),
                }],
            },
        )),
    }
}

// ─── Tool mapping ───────────────────────────────────────────────────────────

fn map_to_proto_tool(tool_name: &str, args: &serde_json::Value) -> api::message::tool_call::Tool {
    let mapped = map_tool_call(tool_name, args);

    match mapped {
        MappedAction::ShellCommand { command, .. } => {
            api::message::tool_call::Tool::RunShellCommand(
                api::message::tool_call::RunShellCommand {
                    command,
                    is_read_only: false,
                    uses_pager: false,
                    citations: vec![],
                    is_risky: false,
                    risk_category: 0,
                    wait_until_complete_value: None,
                },
            )
        }
        MappedAction::ReadFiles { paths } => {
            api::message::tool_call::Tool::ReadFiles(api::message::tool_call::ReadFiles {
                files: paths
                    .into_iter()
                    .map(|p| api::message::tool_call::read_files::File {
                        name: p.path,
                        line_ranges: vec![],
                    })
                    .collect(),
            })
        }
        MappedAction::EditFile {
            file_path,
            old_string,
            new_string,
        } => api::message::tool_call::Tool::ApplyFileDiffs(
            api::message::tool_call::ApplyFileDiffs {
                summary: String::new(),
                diffs: vec![api::message::tool_call::apply_file_diffs::FileDiff {
                    file_path,
                    search: old_string,
                    replace: new_string,
                }],
                new_files: vec![],
                deleted_files: vec![],
                v4a_updates: vec![],
            },
        ),
        MappedAction::WriteFile { file_path, content } => {
            api::message::tool_call::Tool::ApplyFileDiffs(
                api::message::tool_call::ApplyFileDiffs {
                    summary: String::new(),
                    diffs: vec![],
                    new_files: vec![api::message::tool_call::apply_file_diffs::NewFile {
                        file_path,
                        content,
                    }],
                    deleted_files: vec![],
                    v4a_updates: vec![],
                },
            )
        }
        MappedAction::ApplyPatch { patch_text } => {
            // Treat patch as a new file diff with the patch text as content.
            api::message::tool_call::Tool::ApplyFileDiffs(
                api::message::tool_call::ApplyFileDiffs {
                    summary: String::new(),
                    diffs: vec![api::message::tool_call::apply_file_diffs::FileDiff {
                        file_path: String::new(),
                        search: String::new(),
                        replace: patch_text,
                    }],
                    new_files: vec![],
                    deleted_files: vec![],
                    v4a_updates: vec![],
                },
            )
        }
        MappedAction::Grep { pattern, path, .. } => {
            api::message::tool_call::Tool::Grep(api::message::tool_call::Grep {
                queries: vec![pattern],
                path: path.unwrap_or_default(),
            })
        }
        MappedAction::Glob { pattern, path } => {
            api::message::tool_call::Tool::FileGlobV2(api::message::tool_call::FileGlobV2 {
                patterns: vec![pattern],
                search_dir: path.unwrap_or_default(),
                max_matches: 0,
                max_depth: 0,
                min_depth: 0,
            })
        }
        MappedAction::AskQuestion { .. } => {
            // AskUserQuestion is at the top level, not in tool_call module.
            // Use Server as a pass-through for question tool calls.
            api::message::tool_call::Tool::Server(api::message::tool_call::Server {
                payload: serde_json::to_string(args).unwrap_or_default(),
            })
        }
        MappedAction::WebFetch { url } => {
            api::message::tool_call::Tool::Server(api::message::tool_call::Server {
                payload: serde_json::json!({ "tool": "webfetch", "url": url }).to_string(),
            })
        }
        MappedAction::Unknown { args, .. } => {
            api::message::tool_call::Tool::Server(api::message::tool_call::Server {
                payload: serde_json::to_string(&args).unwrap_or_default(),
            })
        }
    }
}

// ─── Helpers ────────────────────────────────────────────────────────────────

fn extract_user_text(params: &RequestParams) -> String {
    use super::AIAgentInput;

    params
        .input
        .iter()
        .filter_map(|input| match input {
            AIAgentInput::UserQuery { query, .. } => Some(query.clone()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn infer_provider(model_id: &str) -> String {
    if model_id.contains("claude") || model_id.contains("anthropic") {
        "anthropic".to_string()
    } else if model_id.contains("gpt") || model_id.contains("o1") || model_id.contains("o3") {
        "openai".to_string()
    } else if model_id.contains("gemini") {
        "google".to_string()
    } else {
        "anthropic".to_string()
    }
}
