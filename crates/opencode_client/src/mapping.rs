/// Mapping between OpenCode tool names and Warp's internal tool/action types.
///
/// OpenCode tools: bash, edit, write, read, grep, glob, webfetch, websearch,
///                 todowrite, question, skill, lsp, apply_patch
///
/// Warp actions:   RequestCommandOutput, RequestFileEdits, ReadFiles, Grep,
///                 FileGlobV2, AskUserQuestion, ReadSkill, CallMCPTool, etc.

use serde_json::Value;

#[cfg(test)]
#[path = "mapping_tests.rs"]
mod tests;

/// Maps an OpenCode tool name + args to a canonical action descriptor
/// that the adapter layer uses to build Warp's protobuf types.
#[derive(Debug, Clone)]
pub enum MappedAction {
    /// Shell command execution.
    ShellCommand {
        command: String,
        timeout: Option<u64>,
    },
    /// File read.
    ReadFiles {
        paths: Vec<FileReadTarget>,
    },
    /// File edit (search/replace).
    EditFile {
        file_path: String,
        old_string: String,
        new_string: String,
    },
    /// File write (create/overwrite).
    WriteFile {
        file_path: String,
        content: String,
    },
    /// Apply a patch.
    ApplyPatch {
        patch_text: String,
    },
    /// Grep search.
    Grep {
        pattern: String,
        path: Option<String>,
        include: Option<String>,
    },
    /// Glob file search.
    Glob {
        pattern: String,
        path: Option<String>,
    },
    /// Ask the user a question.
    AskQuestion {
        questions: Vec<QuestionItem>,
    },
    /// Web fetch.
    WebFetch {
        url: String,
    },
    /// Unknown/unsupported tool — pass through as generic.
    Unknown {
        tool_name: String,
        args: Value,
    },
}

#[derive(Debug, Clone)]
pub struct FileReadTarget {
    pub path: String,
    pub offset: Option<usize>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct QuestionItem {
    pub question: String,
    pub options: Vec<String>,
}

/// Parse OpenCode tool call args into a MappedAction.
pub fn map_tool_call(tool_name: &str, args: &Value) -> MappedAction {
    match tool_name {
        "bash" => {
            let command = args
                .get("command")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let timeout = args.get("timeout").and_then(|v| v.as_u64());
            MappedAction::ShellCommand { command, timeout }
        }
        "read" => {
            let path = args
                .get("filePath")
                .or_else(|| args.get("file_path"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let offset = args.get("offset").and_then(|v| v.as_u64()).map(|v| v as usize);
            let limit = args.get("limit").and_then(|v| v.as_u64()).map(|v| v as usize);
            MappedAction::ReadFiles {
                paths: vec![FileReadTarget { path, offset, limit }],
            }
        }
        "edit" => {
            let file_path = args
                .get("filePath")
                .or_else(|| args.get("file_path"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let old_string = args
                .get("oldString")
                .or_else(|| args.get("old_string"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let new_string = args
                .get("newString")
                .or_else(|| args.get("new_string"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            MappedAction::EditFile {
                file_path,
                old_string,
                new_string,
            }
        }
        "write" => {
            let file_path = args
                .get("filePath")
                .or_else(|| args.get("file_path"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let content = args
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            MappedAction::WriteFile { file_path, content }
        }
        "apply_patch" => {
            let patch_text = args
                .get("patchText")
                .or_else(|| args.get("patch_text"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            MappedAction::ApplyPatch { patch_text }
        }
        "grep" => {
            let pattern = args
                .get("pattern")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let path = args.get("path").and_then(|v| v.as_str()).map(String::from);
            let include = args.get("include").and_then(|v| v.as_str()).map(String::from);
            MappedAction::Grep {
                pattern,
                path,
                include,
            }
        }
        "glob" => {
            let pattern = args
                .get("pattern")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let path = args.get("path").and_then(|v| v.as_str()).map(String::from);
            MappedAction::Glob { pattern, path }
        }
        "question" => {
            let questions = args
                .get("questions")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|q| {
                            let question = q.get("question")?.as_str()?.to_string();
                            let options = q
                                .get("options")
                                .and_then(|o| o.as_array())
                                .map(|opts| {
                                    opts.iter()
                                        .filter_map(|o| {
                                            o.get("label").and_then(|l| l.as_str()).map(String::from)
                                        })
                                        .collect()
                                })
                                .unwrap_or_default();
                            Some(QuestionItem { question, options })
                        })
                        .collect()
                })
                .unwrap_or_default();
            MappedAction::AskQuestion { questions }
        }
        "webfetch" => {
            let url = args
                .get("url")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            MappedAction::WebFetch { url }
        }
        _ => MappedAction::Unknown {
            tool_name: tool_name.to_string(),
            args: args.clone(),
        },
    }
}
