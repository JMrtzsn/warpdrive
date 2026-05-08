#[cfg(test)]
mod tests {
    use crate::events::parse_sse_message;
    use crate::OpenCodeEvent;

    #[test]
    fn test_parse_session_updated_event() {
        let data = r#"{"type": "session.updated", "properties": {"id": "sess_123", "status": "running"}}"#;
        let events = parse_sse_message("message", data).unwrap();
        assert_eq!(events.len(), 1);
        match &events[0] {
            OpenCodeEvent::SessionStatusChanged { session_id, status } => {
                assert_eq!(session_id, "sess_123");
                assert_eq!(status, "running");
            }
            _ => panic!("expected SessionStatusChanged"),
        }
    }

    #[test]
    fn test_parse_message_with_text_part() {
        let data = r#"{
            "type": "message.updated",
            "properties": {
                "sessionID": "sess_1",
                "id": "msg_1",
                "parts": [
                    {"type": "text", "content": "Hello world"}
                ]
            }
        }"#;
        let events = parse_sse_message("message", data).unwrap();
        assert_eq!(events.len(), 1);
        match &events[0] {
            OpenCodeEvent::TextDelta {
                session_id,
                message_id,
                text,
            } => {
                assert_eq!(session_id, "sess_1");
                assert_eq!(message_id, "msg_1");
                assert_eq!(text, "Hello world");
            }
            _ => panic!("expected TextDelta, got {:?}", events[0]),
        }
    }

    #[test]
    fn test_parse_tool_call_started() {
        let data = r#"{
            "type": "message.updated",
            "properties": {
                "sessionID": "sess_1",
                "id": "msg_1",
                "parts": [
                    {
                        "type": "tool-call",
                        "tool": "bash",
                        "toolCallId": "tc_1",
                        "state": "running",
                        "args": {"command": "ls"}
                    }
                ]
            }
        }"#;
        let events = parse_sse_message("message", data).unwrap();
        assert_eq!(events.len(), 1);
        match &events[0] {
            OpenCodeEvent::ToolCallStarted {
                tool_name,
                tool_call_id,
                args,
                ..
            } => {
                assert_eq!(tool_name, "bash");
                assert_eq!(tool_call_id, "tc_1");
                assert_eq!(args["command"], "ls");
            }
            _ => panic!("expected ToolCallStarted"),
        }
    }

    #[test]
    fn test_parse_tool_call_completed() {
        let data = r#"{
            "type": "message.updated",
            "properties": {
                "sessionID": "sess_1",
                "id": "msg_1",
                "parts": [
                    {
                        "type": "tool-call",
                        "tool": "bash",
                        "toolCallId": "tc_1",
                        "state": "completed",
                        "content": "file1.rs\nfile2.rs"
                    }
                ]
            }
        }"#;
        let events = parse_sse_message("message", data).unwrap();
        assert_eq!(events.len(), 1);
        match &events[0] {
            OpenCodeEvent::ToolCallCompleted {
                tool_name,
                tool_call_id,
                result,
                ..
            } => {
                assert_eq!(tool_name, "bash");
                assert_eq!(tool_call_id, "tc_1");
                assert_eq!(result, "file1.rs\nfile2.rs");
            }
            _ => panic!("expected ToolCallCompleted"),
        }
    }

    #[test]
    fn test_parse_permission_requested() {
        let data = r#"{
            "type": "permission.requested",
            "properties": {
                "sessionID": "sess_1",
                "id": "perm_1",
                "tool": "bash",
                "args": {"command": "rm -rf /"}
            }
        }"#;
        let events = parse_sse_message("message", data).unwrap();
        assert_eq!(events.len(), 1);
        match &events[0] {
            OpenCodeEvent::PermissionRequired {
                session_id,
                permission_id,
                tool_name,
                args,
            } => {
                assert_eq!(session_id, "sess_1");
                assert_eq!(permission_id, "perm_1");
                assert_eq!(tool_name, "bash");
                assert_eq!(args["command"], "rm -rf /");
            }
            _ => panic!("expected PermissionRequired"),
        }
    }

    #[test]
    fn test_parse_server_connected() {
        let data = r#"{"type": "server.connected", "properties": {}}"#;
        let events = parse_sse_message("message", data).unwrap();
        assert_eq!(events.len(), 1);
        match &events[0] {
            OpenCodeEvent::Connected => {}
            _ => panic!("expected Connected"),
        }
    }

    #[test]
    fn test_parse_unknown_event_type() {
        let data = r#"{"type": "some.unknown.event", "properties": {"foo": "bar"}}"#;
        let events = parse_sse_message("message", data).unwrap();
        assert!(events.is_empty());
    }

    #[test]
    fn test_parse_invalid_json() {
        let result = parse_sse_message("message", "not json");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_multiple_parts_in_message() {
        let data = r#"{
            "type": "message.updated",
            "properties": {
                "sessionID": "sess_1",
                "id": "msg_1",
                "parts": [
                    {"type": "text", "content": "Here's the result:"},
                    {
                        "type": "tool-call",
                        "tool": "read",
                        "toolCallId": "tc_2",
                        "state": "completed",
                        "content": "file contents here"
                    }
                ]
            }
        }"#;
        let events = parse_sse_message("message", data).unwrap();
        assert_eq!(events.len(), 2);
        assert!(matches!(&events[0], OpenCodeEvent::TextDelta { .. }));
        assert!(matches!(&events[1], OpenCodeEvent::ToolCallCompleted { .. }));
    }
}
