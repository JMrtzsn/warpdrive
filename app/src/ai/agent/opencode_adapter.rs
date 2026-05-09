//! OpenCode adapter — drop-in replacement for the Warp server agent backend.
//!
//! Provides `generate_opencode_output` with the same return type as
//! `generate_multi_agent_output`, allowing the existing `ResponseStream` /
//! `BlocklistAIController` pipeline to work unchanged.

use std::sync::Arc;

use opencode_client::{
    map_tool_call, CreateSessionRequest, MappedAction, MessagePart,
    ModelSelection, OpenCodeClient, PromptRequest, SidecarManager,
};
use tokio::sync::Mutex;
use warp_multi_agent_api as api;

use super::api::{ConvertToAPITypeError, RequestParams, ResponseStream as ApiResponseStream};

/// Persistent state for the OpenCode integration.
///
/// Supports two modes:
/// - **External**: connect to an already-running OpenCode instance (e.g. user's
///   main instance on port 4097). No sidecar process is managed.
/// - **Sidecar**: spawn and manage an OpenCode sidecar process.
pub struct OpenCodeAdapter {
    pub sidecar: Option<SidecarManager>,
    client: OpenCodeClient,
    session_id: Option<String>,
    /// The run_id / task_id from the first CreateTask, reused for all
    /// subsequent messages in the same conversation.
    run_id: Option<String>,
}

impl OpenCodeAdapter {
    /// Connect to an already-running OpenCode instance at the given base URL.
    pub fn connect(base_url: impl Into<String>) -> Self {
        let url = base_url.into();
        let client = OpenCodeClient::new(&url);
        Self {
            sidecar: None,
            client,
            session_id: None,
            run_id: None,
        }
    }

    /// Create an adapter that manages its own sidecar process.
    pub fn with_sidecar(sidecar: SidecarManager) -> Self {
        let client = sidecar.client();
        Self {
            sidecar: Some(sidecar),
            client,
            session_id: None,
            run_id: None,
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
        self.run_id = None;
    }
}

/// Drop-in replacement for `generate_multi_agent_output`.
///
/// Uses the synchronous prompt endpoint to avoid SSE event replay issues.
/// The response is converted to a stream of Warp events after completion.
pub async fn generate_opencode_output(
    adapter: Arc<Mutex<OpenCodeAdapter>>,
    params: RequestParams,
    _cancellation_rx: futures::channel::oneshot::Receiver<()>,
) -> Result<ApiResponseStream, ConvertToAPITypeError> {
    log::info!("generate_opencode_output: starting");
    let mut guard = adapter.lock().await;

    // If we have a sidecar, ensure it's running.
    if let Some(sidecar) = &guard.sidecar {
        if !sidecar.is_running().await {
            log::info!("generate_opencode_output: starting sidecar");
            guard
                .sidecar
                .as_ref()
                .unwrap()
                .start()
                .await
                .map_err(|e| ConvertToAPITypeError::Other(e.into()))?;
        }
    }

    let session_id = guard
        .ensure_session()
        .await
        .map_err(|e| {
            log::error!("generate_opencode_output: ensure_session failed: {e:#}");
            ConvertToAPITypeError::Other(e.into())
        })?;
    log::info!("generate_opencode_output: session_id={session_id}");

    let user_text = extract_user_text(&params);
    log::info!("generate_opencode_output: user_text={user_text:?}");
    let model_id = resolve_model_id(&params.model.to_string());
    log::info!("generate_opencode_output: resolved model={model_id:?}");

    let prompt_req = PromptRequest {
        parts: vec![MessagePart::Text { text: user_text }],
        model: model_id.map(|id| ModelSelection {
            provider_id: infer_provider(&id),
            model_id: id,
        }),
        agent: None,
        no_reply: None,
        system: None,
        tools: None,
    };

    let client = guard.client.clone();

    // Determine if this is the first message (need CreateTask) or a follow-up.
    let is_first_message = guard.run_id.is_none();
    let run_id = guard
        .run_id
        .get_or_insert_with(|| uuid::Uuid::new_v4().to_string())
        .clone();
    let request_id = uuid::Uuid::new_v4().to_string();

    drop(guard);

    // Use synchronous prompt — blocks until LLM completes.
    log::info!("generate_opencode_output: sending synchronous prompt");
    let response = client
        .prompt(&session_id, &prompt_req)
        .await
        .map_err(|e| {
            log::error!("generate_opencode_output: prompt failed: {e:#}");
            ConvertToAPITypeError::Other(e.into())
        })?;
    log::info!(
        "generate_opencode_output: got response with {} parts",
        response.parts.len()
    );

    // Build Warp events from the response.
    let events = build_warp_events(&response, is_first_message, &run_id, &request_id);

    log::info!(
        "generate_opencode_output: emitting {} events",
        events.len()
    );

    let stream = futures::stream::iter(events.into_iter().map(Ok));
    Ok(Box::pin(stream))
}

/// Pure function: converts an OpenCode `MessageResponse` into Warp proto events.
///
/// - `is_first_message`: if true, emits a `CreateTask` event.
/// - `run_id`: the stable task/run ID for the conversation.
/// - `request_id`: unique per-request ID.
fn build_warp_events(
    response: &opencode_client::MessageResponse,
    is_first_message: bool,
    run_id: &str,
    request_id: &str,
) -> Vec<api::ResponseEvent> {
    let mut events: Vec<api::ResponseEvent> = Vec::new();

    // 1. StreamInit
    events.push(api::ResponseEvent {
        r#type: Some(api::response_event::Type::Init(
            api::response_event::StreamInit {
                request_id: request_id.to_string(),
                conversation_id: String::new(),
                run_id: run_id.to_string(),
            },
        )),
    });

    // 2. CreateTask — only on the first message in a conversation.
    if is_first_message {
        events.push(api::ResponseEvent {
            r#type: Some(api::response_event::Type::ClientActions(
                api::response_event::ClientActions {
                    actions: vec![api::ClientAction {
                        action: Some(api::client_action::Action::CreateTask(
                            api::client_action::CreateTask {
                                task: Some(api::Task {
                                    id: run_id.to_string(),
                                    description: String::new(),
                                    dependencies: None,
                                    messages: vec![],
                                    summary: String::new(),
                                    server_data: String::new(),
                                }),
                            },
                        )),
                    }],
                },
            )),
        });
    }

    // 3. Convert response parts to messages.
    for part in &response.parts {
        log::info!(
            "build_warp_events: part type={:?} content={:?} tool={:?}",
            part.part_type,
            part.content,
            part.tool
        );
        let part_type = part.part_type.as_str();
        match part_type {
            "text" => {
                let text = part.text.clone().unwrap_or_default();
                if !text.is_empty() {
                    events.push(wrap_in_client_actions_with_task(
                        api::Message {
                            id: part
                                .tool_call_id
                                .clone()
                                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
                            task_id: run_id.to_string(),
                            request_id: request_id.to_string(),
                            timestamp: None,
                            server_message_data: String::new(),
                            citations: vec![],
                            message: Some(api::message::Message::AgentOutput(
                                api::message::AgentOutput { text },
                            )),
                        },
                        run_id,
                    ));
                }
            }
            "tool-call" => {
                let tool_name = part.tool.clone().unwrap_or_default();
                let args = part.args.clone().unwrap_or(serde_json::Value::Null);
                let tool_call_id = part
                    .tool_call_id
                    .clone()
                    .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
                let tool = map_to_proto_tool(&tool_name, &args);
                events.push(wrap_in_client_actions_with_task(
                    api::Message {
                        id: uuid::Uuid::new_v4().to_string(),
                        task_id: run_id.to_string(),
                        request_id: request_id.to_string(),
                        timestamp: None,
                        server_message_data: String::new(),
                        citations: vec![],
                        message: Some(api::message::Message::ToolCall(
                            api::message::ToolCall {
                                tool_call_id: tool_call_id.clone(),
                                tool: Some(tool),
                            },
                        )),
                    },
                    run_id,
                ));
                // Also emit the tool result.
                let result = part.content.clone();
                events.push(wrap_in_client_actions_with_task(
                    api::Message {
                        id: uuid::Uuid::new_v4().to_string(),
                        task_id: run_id.to_string(),
                        request_id: request_id.to_string(),
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
                    },
                    run_id,
                ));
            }
            _ => {
                log::debug!("build_warp_events: skipping part type={part_type}");
            }
        }
    }

    // 4. StreamFinished
    events.push(api::ResponseEvent {
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
    });

    events
}

fn wrap_in_client_actions_with_task(message: api::Message, task_id: &str) -> api::ResponseEvent {
    api::ResponseEvent {
        r#type: Some(api::response_event::Type::ClientActions(
            api::response_event::ClientActions {
                actions: vec![api::ClientAction {
                    action: Some(api::client_action::Action::AddMessagesToTask(
                        api::client_action::AddMessagesToTask {
                            task_id: task_id.to_string(),
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
    } else if model_id.contains("copilot") {
        "copilot".to_string()
    } else {
        // Default — let OpenCode pick based on its configured provider.
        String::new()
    }
}

/// Map Warp's internal model IDs to real model IDs that OpenCode understands.
/// Returns `None` for auto-* IDs, letting OpenCode use its configured default.
fn resolve_model_id(model_id: &str) -> Option<String> {
    match model_id {
        "auto-efficient" | "auto_efficient" | "auto-best" | "auto_best" | "auto" => None,
        other => Some(other.to_string()),
    }
}

#[cfg(test)]
#[path = "opencode_adapter_tests.rs"]
mod tests;
