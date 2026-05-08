#[cfg(test)]
mod tests {
    use crate::mapping::*;

    #[test]
    fn test_map_bash_tool() {
        let args = serde_json::json!({
            "command": "ls -la",
            "timeout": 5000
        });
        let mapped = map_tool_call("bash", &args);
        match mapped {
            MappedAction::ShellCommand { command, timeout } => {
                assert_eq!(command, "ls -la");
                assert_eq!(timeout, Some(5000));
            }
            _ => panic!("expected ShellCommand"),
        }
    }

    #[test]
    fn test_map_read_tool() {
        let args = serde_json::json!({
            "filePath": "/src/main.rs",
            "offset": 10,
            "limit": 50
        });
        let mapped = map_tool_call("read", &args);
        match mapped {
            MappedAction::ReadFiles { paths } => {
                assert_eq!(paths.len(), 1);
                assert_eq!(paths[0].path, "/src/main.rs");
                assert_eq!(paths[0].offset, Some(10));
                assert_eq!(paths[0].limit, Some(50));
            }
            _ => panic!("expected ReadFiles"),
        }
    }

    #[test]
    fn test_map_edit_tool() {
        let args = serde_json::json!({
            "filePath": "src/lib.rs",
            "oldString": "fn old()",
            "newString": "fn new()"
        });
        let mapped = map_tool_call("edit", &args);
        match mapped {
            MappedAction::EditFile {
                file_path,
                old_string,
                new_string,
            } => {
                assert_eq!(file_path, "src/lib.rs");
                assert_eq!(old_string, "fn old()");
                assert_eq!(new_string, "fn new()");
            }
            _ => panic!("expected EditFile"),
        }
    }

    #[test]
    fn test_map_write_tool() {
        let args = serde_json::json!({
            "filePath": "new_file.rs",
            "content": "fn main() {}"
        });
        let mapped = map_tool_call("write", &args);
        match mapped {
            MappedAction::WriteFile { file_path, content } => {
                assert_eq!(file_path, "new_file.rs");
                assert_eq!(content, "fn main() {}");
            }
            _ => panic!("expected WriteFile"),
        }
    }

    #[test]
    fn test_map_grep_tool() {
        let args = serde_json::json!({
            "pattern": "fn main",
            "path": "/src",
            "include": "*.rs"
        });
        let mapped = map_tool_call("grep", &args);
        match mapped {
            MappedAction::Grep {
                pattern,
                path,
                include,
            } => {
                assert_eq!(pattern, "fn main");
                assert_eq!(path, Some("/src".to_string()));
                assert_eq!(include, Some("*.rs".to_string()));
            }
            _ => panic!("expected Grep"),
        }
    }

    #[test]
    fn test_map_glob_tool() {
        let args = serde_json::json!({
            "pattern": "**/*.rs",
            "path": "/src"
        });
        let mapped = map_tool_call("glob", &args);
        match mapped {
            MappedAction::Glob { pattern, path } => {
                assert_eq!(pattern, "**/*.rs");
                assert_eq!(path, Some("/src".to_string()));
            }
            _ => panic!("expected Glob"),
        }
    }

    #[test]
    fn test_map_question_tool() {
        let args = serde_json::json!({
            "questions": [
                {
                    "question": "Which framework?",
                    "options": [
                        {"label": "React"},
                        {"label": "Vue"}
                    ]
                }
            ]
        });
        let mapped = map_tool_call("question", &args);
        match mapped {
            MappedAction::AskQuestion { questions } => {
                assert_eq!(questions.len(), 1);
                assert_eq!(questions[0].question, "Which framework?");
                assert_eq!(questions[0].options, vec!["React", "Vue"]);
            }
            _ => panic!("expected AskQuestion"),
        }
    }

    #[test]
    fn test_map_webfetch_tool() {
        let args = serde_json::json!({ "url": "https://example.com" });
        let mapped = map_tool_call("webfetch", &args);
        match mapped {
            MappedAction::WebFetch { url } => {
                assert_eq!(url, "https://example.com");
            }
            _ => panic!("expected WebFetch"),
        }
    }

    #[test]
    fn test_map_apply_patch_tool() {
        let args = serde_json::json!({
            "patchText": "*** Update File: src/lib.rs\n@@ -1,3 +1,3 @@\n-old\n+new"
        });
        let mapped = map_tool_call("apply_patch", &args);
        match mapped {
            MappedAction::ApplyPatch { patch_text } => {
                assert!(patch_text.contains("Update File"));
            }
            _ => panic!("expected ApplyPatch"),
        }
    }

    #[test]
    fn test_map_unknown_tool() {
        let args = serde_json::json!({"foo": "bar"});
        let mapped = map_tool_call("some_custom_tool", &args);
        match mapped {
            MappedAction::Unknown { tool_name, args } => {
                assert_eq!(tool_name, "some_custom_tool");
                assert_eq!(args["foo"], "bar");
            }
            _ => panic!("expected Unknown"),
        }
    }

    #[test]
    fn test_map_edit_snake_case_keys() {
        let args = serde_json::json!({
            "file_path": "src/lib.rs",
            "old_string": "fn old()",
            "new_string": "fn new()"
        });
        let mapped = map_tool_call("edit", &args);
        match mapped {
            MappedAction::EditFile {
                file_path,
                old_string,
                new_string,
            } => {
                assert_eq!(file_path, "src/lib.rs");
                assert_eq!(old_string, "fn old()");
                assert_eq!(new_string, "fn new()");
            }
            _ => panic!("expected EditFile"),
        }
    }
}
