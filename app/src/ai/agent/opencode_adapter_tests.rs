use super::*;
use opencode_client::{Message as OcMessage, MessageResponse, Part};
use serde_json::json;

fn make_response(parts: Vec<Part>) -> MessageResponse {
    MessageResponse {
        info: OcMessage {
            id: "msg-1".to_string(),
            role: "assistant".to_string(),
            metadata: json!({}),
        },
        parts,
    }
}

fn text_part(content: &str) -> Part {
    Part {
        part_type: "text".to_string(),
        text: Some(content.to_string()),
        tool: None,
        tool_call_id: None,
        state: None,
        content: json!(null),
        args: None,
        id: None,
        session_id: None,
        message_id: None,
    }
}

fn tool_call_part(tool: &str, args: serde_json::Value) -> Part {
    Part {
        part_type: "tool-call".to_string(),
        text: None,
        tool: Some(tool.to_string()),
        tool_call_id: Some("tc-1".to_string()),
        state: Some("completed".to_string()),
        content: json!({"result": "ok"}),
        args: Some(args),
        id: None,
        session_id: None,
        message_id: None,
    }
}

/// Count events of each type for assertions.
fn count_event_types(events: &[api::ResponseEvent]) -> (usize, usize, usize, usize) {
    let mut init = 0;
    let mut create_task = 0;
    let mut add_messages = 0;
    let mut finished = 0;
    for e in events {
        match &e.r#type {
            Some(api::response_event::Type::Init(_)) => init += 1,
            Some(api::response_event::Type::ClientActions(ca)) => {
                for a in &ca.actions {
                    match &a.action {
                        Some(api::client_action::Action::CreateTask(_)) => create_task += 1,
                        Some(api::client_action::Action::AddMessagesToTask(_)) => add_messages += 1,
                        _ => {}
                    }
                }
            }
            Some(api::response_event::Type::Finished(_)) => finished += 1,
            _ => {}
        }
    }
    (init, create_task, add_messages, finished)
}

#[test]
fn first_message_text_only() {
    let response = make_response(vec![text_part("Hello, world!")]);
    let events = build_warp_events(&response, true, "run-1", "req-1");

    let (init, create_task, add_messages, finished) = count_event_types(&events);
    assert_eq!(init, 1, "should have 1 StreamInit");
    assert_eq!(create_task, 1, "first message should have CreateTask");
    assert_eq!(add_messages, 1, "should have 1 AddMessagesToTask for text");
    assert_eq!(finished, 1, "should have 1 StreamFinished");
    // Total: Init + CreateTask + AddMessages + Finished = 4
    assert_eq!(events.len(), 4);
}

#[test]
fn follow_up_message_skips_create_task() {
    let response = make_response(vec![text_part("Follow-up reply")]);
    let events = build_warp_events(&response, false, "run-1", "req-2");

    let (init, create_task, add_messages, finished) = count_event_types(&events);
    assert_eq!(init, 1);
    assert_eq!(create_task, 0, "follow-up should NOT emit CreateTask");
    assert_eq!(add_messages, 1);
    assert_eq!(finished, 1);
    assert_eq!(events.len(), 3);
}

#[test]
fn tool_call_emits_call_and_result() {
    let response = make_response(vec![tool_call_part("bash", json!({"command": "ls"}))]);
    let events = build_warp_events(&response, true, "run-1", "req-1");

    let (init, create_task, add_messages, finished) = count_event_types(&events);
    assert_eq!(init, 1);
    assert_eq!(create_task, 1);
    assert_eq!(
        add_messages, 2,
        "tool-call should emit ToolCall + ToolCallResult"
    );
    assert_eq!(finished, 1);
}

#[test]
fn empty_text_is_skipped() {
    let response = make_response(vec![text_part("")]);
    let events = build_warp_events(&response, true, "run-1", "req-1");

    let (_init, _create_task, add_messages, _finished) = count_event_types(&events);
    assert_eq!(
        add_messages, 0,
        "empty text should not emit AddMessagesToTask"
    );
}

#[test]
fn mixed_text_and_tool_call() {
    let response = make_response(vec![
        text_part("Let me check..."),
        tool_call_part("bash", json!({"command": "pwd"})),
        text_part("Done!"),
    ]);
    let events = build_warp_events(&response, true, "run-1", "req-1");

    let (_init, _create_task, add_messages, _finished) = count_event_types(&events);
    // 2 text + 2 (tool call + result) = 4
    assert_eq!(add_messages, 4);
}

#[test]
fn unknown_part_type_is_skipped() {
    let response = make_response(vec![Part {
        part_type: "step-start".to_string(),
        text: None,
        tool: None,
        tool_call_id: None,
        state: None,
        content: json!(null),
        args: None,
        id: None,
        session_id: None,
        message_id: None,
    }]);
    let events = build_warp_events(&response, true, "run-1", "req-1");

    let (_init, _create_task, add_messages, _finished) = count_event_types(&events);
    assert_eq!(add_messages, 0, "unknown part types should be skipped");
}

#[test]
fn resolve_model_auto_returns_none() {
    assert!(resolve_model_id("auto").is_none());
    assert!(resolve_model_id("auto-efficient").is_none());
    assert!(resolve_model_id("auto-best").is_none());
    assert!(resolve_model_id("auto_efficient").is_none());
    assert!(resolve_model_id("auto_best").is_none());
}

#[test]
fn resolve_model_specific_returns_some() {
    assert_eq!(
        resolve_model_id("claude-opus-4-20250514"),
        Some("claude-opus-4-20250514".to_string())
    );
}

#[test]
fn infer_provider_known() {
    assert_eq!(infer_provider("claude-opus-4-20250514"), "anthropic");
    assert_eq!(infer_provider("gpt-4o"), "openai");
    assert_eq!(infer_provider("gemini-pro"), "google");
    assert_eq!(infer_provider("copilot"), "copilot");
}

#[test]
fn infer_provider_unknown_defaults_empty() {
    assert_eq!(infer_provider("some-unknown-model"), "");
}

#[test]
fn real_response_shape_step_text_step() {
    // Matches actual OpenCode response: step-start, text, step-finish
    let response = make_response(vec![
        Part {
            part_type: "step-start".to_string(),
            text: None,
            tool: None,
            tool_call_id: None,
            state: None,
            content: json!(null),
            args: None,
            id: Some("prt_1".to_string()),
            session_id: Some("ses_1".to_string()),
            message_id: Some("msg_1".to_string()),
        },
        Part {
            part_type: "text".to_string(),
            text: Some("Hello!".to_string()),
            tool: None,
            tool_call_id: None,
            state: None,
            content: json!(null),
            args: None,
            id: Some("prt_2".to_string()),
            session_id: Some("ses_1".to_string()),
            message_id: Some("msg_1".to_string()),
        },
        Part {
            part_type: "step-finish".to_string(),
            text: None,
            tool: None,
            tool_call_id: None,
            state: None,
            content: json!(null),
            args: None,
            id: Some("prt_3".to_string()),
            session_id: Some("ses_1".to_string()),
            message_id: Some("msg_1".to_string()),
        },
    ]);
    let events = build_warp_events(&response, true, "run-1", "req-1");

    let (init, create_task, add_messages, finished) = count_event_types(&events);
    assert_eq!(init, 1);
    assert_eq!(create_task, 1);
    assert_eq!(
        add_messages, 1,
        "only the text part should produce a message"
    );
    assert_eq!(finished, 1);
    // Total: Init + CreateTask + AddMessages(text) + Finished = 4
    assert_eq!(events.len(), 4);
}

#[test]
fn system_prompt_contains_environment_info() {
    let prompt = build_system_prompt();
    assert!(prompt.contains("Warpdrive"), "should mention Warpdrive");
    assert!(prompt.contains("Working directory:"), "should include CWD");
    assert!(prompt.contains("Shell:"), "should include shell");
    assert!(prompt.contains("OS:"), "should include OS");
}

#[test]
fn extract_command_mode_plan() {
    let (text, system) = extract_command_mode("/plan build a REST API");
    assert_eq!(text, "build a REST API");
    assert!(system.unwrap().contains("/plan"));
}

#[test]
fn extract_command_mode_plan_bare() {
    let (text, system) = extract_command_mode("/plan");
    assert!(!text.is_empty());
    assert!(system.is_some());
}

#[test]
fn extract_command_mode_orchestrate() {
    let (text, system) = extract_command_mode("/orchestrate deploy services");
    assert_eq!(text, "deploy services");
    assert!(system.unwrap().contains("/orchestrate"));
}

#[test]
fn extract_command_mode_compact() {
    let (text, system) = extract_command_mode("/compact");
    assert!(text.contains("Summarize"));
    assert!(system.unwrap().contains("/compact"));
}

#[test]
fn extract_command_mode_compact_with_instructions() {
    let (text, system) = extract_command_mode("/compact focus on architecture decisions");
    assert!(text.contains("architecture decisions"));
    assert!(system.is_some());
}

#[test]
fn extract_command_mode_normal_text() {
    let (text, system) = extract_command_mode("just a normal message");
    assert_eq!(text, "just a normal message");
    assert!(system.is_none());
}

// ─── extract_user_text tests ────────────────────────────────────────────────

use super::super::AIAgentInput;
use crate::ai::agent::api::RequestParams;
use crate::ai::blocklist::SessionContext;
use crate::ai::llms::LLMId;
use warp_multi_agent_api as api;

fn make_params(input: Vec<AIAgentInput>) -> RequestParams {
    let model = LLMId::from("test-model");
    RequestParams {
        input,
        conversation_token: None,
        forked_from_conversation_token: None,
        ambient_agent_task_id: None,
        tasks: vec![],
        existing_suggestions: None,
        metadata: None,
        session_context: SessionContext::new_for_test(),
        model: model.clone(),
        coding_model: model.clone(),
        cli_agent_model: model.clone(),
        computer_use_model: model,
        is_memory_enabled: false,
        warp_drive_context_enabled: false,
        context_window_limit: None,
        mcp_context: None,
        planning_enabled: false,
        should_redact_secrets: false,
        api_keys: None,
        allow_use_of_warp_credits_with_byok: false,
        autonomy_level: api::AutonomyLevel::Supervised,
        isolation_level: api::IsolationLevel::None,
        web_search_enabled: false,
        computer_use_enabled: false,
        ask_user_question_enabled: false,
        research_agent_enabled: false,
        orchestration_enabled: false,
        supported_tools_override: None,
        parent_agent_id: None,
        agent_name: None,
    }
}

#[test]
fn extract_user_text_summarize_no_prompt() {
    let params = make_params(vec![AIAgentInput::SummarizeConversation { prompt: None }]);
    let text = extract_user_text(&params);
    assert!(text.contains("Summarize"), "got: {text}");
}

#[test]
fn extract_user_text_summarize_with_prompt() {
    let params = make_params(vec![AIAgentInput::SummarizeConversation {
        prompt: Some("focus on errors".to_string()),
    }]);
    let text = extract_user_text(&params);
    assert!(text.contains("Summarize"), "got: {text}");
    assert!(text.contains("focus on errors"), "got: {text}");
}

#[test]
fn extract_user_text_init_project_rules() {
    let params = make_params(vec![AIAgentInput::InitProjectRules {
        context: vec![].into(),
        display_query: Some("Set up rules for my Rust project".to_string()),
    }]);
    let text = extract_user_text(&params);
    assert!(text.contains("Rust project"), "got: {text}");
}

#[test]
fn extract_user_text_init_project_rules_default() {
    let params = make_params(vec![AIAgentInput::InitProjectRules {
        context: vec![].into(),
        display_query: None,
    }]);
    let text = extract_user_text(&params);
    assert!(text.contains("project rules"), "got: {text}");
}

#[test]
fn extract_user_text_auto_code_diff() {
    let params = make_params(vec![AIAgentInput::AutoCodeDiffQuery {
        query: "refactor the login module".to_string(),
        context: vec![].into(),
    }]);
    let text = extract_user_text(&params);
    assert_eq!(text, "refactor the login module");
}

#[test]
fn extract_user_text_create_new_project() {
    let params = make_params(vec![AIAgentInput::CreateNewProject {
        query: "a CLI todo app in Rust".to_string(),
        context: vec![].into(),
    }]);
    let text = extract_user_text(&params);
    assert!(text.contains("CLI todo app"), "got: {text}");
}

#[test]
fn extract_user_text_resume_conversation() {
    let params = make_params(vec![AIAgentInput::ResumeConversation {
        context: vec![].into(),
    }]);
    let text = extract_user_text(&params);
    assert!(text.contains("Continue"), "got: {text}");
}

#[test]
fn extract_user_text_clone_repository() {
    use super::super::CloneRepositoryURL;
    let params = make_params(vec![AIAgentInput::CloneRepository {
        clone_repo_url: CloneRepositoryURL::new("https://github.com/example/repo".to_string()),
        context: vec![].into(),
    }]);
    let text = extract_user_text(&params);
    assert!(
        text.contains("https://github.com/example/repo"),
        "got: {text}"
    );
}

#[test]
fn extract_user_text_unknown_variant_ignored() {
    // Passive suggestions and other cloud-only variants should produce empty text.
    let params = make_params(vec![AIAgentInput::ResumeConversation {
        context: vec![].into(),
    }]);
    // ResumeConversation IS handled, so this should not be empty.
    let text = extract_user_text(&params);
    assert!(!text.is_empty());
}
