//! Output formatting for JSON (default) and human-readable modes.

use std::io::Write;

use crate::api::{HistoryResult, Reaction, SearchResult, SendResult, SummaryResult};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ErrorDetail {
    pub code: &'static str,
    pub message: String,
    pub suggestion: String,
}

#[derive(Debug)]
pub struct CliError {
    pub detail: ErrorDetail,
    pub human: bool,
}

impl CliError {
    pub fn exit_code(&self) -> i32 {
        match self.detail.code {
            code if code.starts_with("CONFIG_") => 2,
            "OP_NOT_FOUND" | "OP_FAILED" => 3,
            "API_ERROR" => 4,
            "INVALID_DEBUG_MODE" => 5,
            _ => 1,
        }
    }

    pub fn render(&self) {
        self.render_to(&mut std::io::stdout(), &mut std::io::stderr());
    }

    /// Render this error to the given writers.
    ///
    /// In JSON mode (the default), the structured error goes to `stdout_w` so
    /// that agents capturing stdout receive the error envelope. In `--human`
    /// mode, plain-text diagnostics go to `stderr_w`.
    pub fn render_to(&self, stdout_w: &mut dyn Write, stderr_w: &mut dyn Write) {
        if self.human {
            let _ = writeln!(stderr_w, "Error: {}", self.detail.message);
            let _ = writeln!(stderr_w, "Suggestion: {}", self.detail.suggestion);
        } else {
            let json = format_error(&self.detail);
            let _ = writeln!(stdout_w, "{json}");
        }
    }
}

/// Pagination metadata for list responses.
#[derive(Debug, Serialize)]
pub struct Pagination {
    pub has_more: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

/// Format a success response with optional pagination metadata as JSON.
pub fn format_success_with_pagination<T: Serialize>(
    data: &T,
    pagination: Option<&Pagination>,
) -> String {
    let mut wrapper = serde_json::json!({
        "success": true,
        "data": data,
    });
    if let Some(p) = pagination {
        wrapper["pagination"] = serde_json::to_value(p).unwrap();
    }
    serde_json::to_string_pretty(&wrapper).expect("serialization should not fail")
}

pub fn format_success<T: Serialize>(data: &T) -> String {
    let wrapper = serde_json::json!({
        "success": true,
        "data": data,
    });
    serde_json::to_string_pretty(&wrapper).expect("serialization should not fail")
}

pub fn format_error(detail: &ErrorDetail) -> String {
    let wrapper = serde_json::json!({
        "success": false,
        "error": {
            "code": detail.code,
            "message": detail.message,
            "suggestion": detail.suggestion,
        },
    });
    serde_json::to_string_pretty(&wrapper).expect("serialization should not fail")
}

pub fn format_send_human(result: &SendResult) -> String {
    format!(
        "## Message sent\n\n- **Channel:** {}\n- **Timestamp:** {}\n\n{}\n",
        result.channel, result.ts, result.message.text
    )
}

/// Format an annotation string for reply count and reactions.
///
/// Returns e.g. ` [3 replies, 7 reactions]` or empty string if both are None/zero.
fn format_message_annotation(reply_count: Option<u64>, reactions: Option<&[Reaction]>) -> String {
    let replies = reply_count.filter(|&c| c > 0);
    let total_reactions: u64 = reactions
        .map(|r| r.iter().map(|rx| rx.count).sum())
        .unwrap_or(0);
    let reaction_part = if total_reactions > 0 {
        Some(total_reactions)
    } else {
        None
    };

    match (replies, reaction_part) {
        (Some(r), Some(rx)) => format!(" [{r} replies, {rx} reactions]"),
        (Some(r), None) => format!(" [{r} replies]"),
        (None, Some(rx)) => format!(" [{rx} reactions]"),
        (None, None) => String::new(),
    }
}

pub fn format_history_human(result: &HistoryResult) -> String {
    let mut out = String::new();
    if result.messages.is_empty() {
        out.push_str("No messages found.\n");
    } else {
        for (i, msg) in result.messages.iter().enumerate() {
            if i > 0 {
                out.push('\n');
            }
            let user = msg.user.as_deref().unwrap_or("unknown");
            let annotation = format_message_annotation(msg.reply_count, msg.reactions.as_deref());
            out.push_str(&format!(
                "### {} `{}`{annotation}\n\n{}\n",
                user, msg.ts, msg.text
            ));
        }
    }
    if let Some(ref msg) = result.message {
        out.push_str(&format!("\n> {msg}\n"));
    }
    out
}

pub fn format_search_human(result: &SearchResult) -> String {
    let mut out = format!("**{} result(s) found.**\n\n", result.total);
    for (i, msg) in result.messages.iter().enumerate() {
        if i > 0 {
            out.push_str("\n---\n\n");
        }
        let user = msg.user.as_deref().unwrap_or("unknown");
        let channel_name = msg
            .channel
            .as_ref()
            .map(|c| c.name.as_str())
            .unwrap_or("DM");
        let annotation = format_message_annotation(msg.reply_count, msg.reactions.as_deref());
        out.push_str(&format!(
            "### {} in #{} `{}`{annotation}\n\n",
            user, channel_name, msg.ts
        ));
        out.push_str(&format!("{}\n", msg.text));
        if let Some(ref link) = msg.permalink {
            out.push_str(&format!("\n[View in Slack]({link})\n"));
        }
    }
    out
}

pub fn format_summary_human(result: &SummaryResult) -> String {
    format!("## Channel Summary\n\n{}\n", result.summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::Message;

    #[test]
    fn format_success_wraps_data() {
        let data = serde_json::json!({"channel": "C1", "ts": "1.0"});
        let output = format_success(&data);
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed["success"], true);
        assert_eq!(parsed["data"]["channel"], "C1");
    }

    #[test]
    fn format_error_includes_all_fields() {
        let detail = ErrorDetail {
            code: "NOT_FOUND",
            message: "Channel not found".to_string(),
            suggestion: "Check the channel name".to_string(),
        };
        let output = format_error(&detail);
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed["success"], false);
        assert_eq!(parsed["error"]["code"], "NOT_FOUND");
    }

    #[test]
    fn format_send_human_includes_fields() {
        let result = SendResult {
            channel: "C12345".to_string(),
            ts: "1.0".to_string(),
            message: Message {
                ts: "1.0".to_string(),
                user: Some("U1".to_string()),
                text: "hello".to_string(),
                thread_ts: None,
                channel: None,
                reply_count: None,
                reactions: None,
                edited: None,
            },
        };
        let output = format_send_human(&result);
        assert!(output.contains("C12345"));
        assert!(output.contains("hello"));
    }

    #[test]
    fn format_history_human_empty() {
        let result = HistoryResult {
            messages: vec![],
            has_more: false,
            message: None,
            next_cursor: None,
        };
        let output = format_history_human(&result);
        assert!(output.contains("No messages found"));
    }

    #[test]
    fn format_history_human_with_messages() {
        let result = HistoryResult {
            messages: vec![
                Message {
                    ts: "1.0".to_string(),
                    user: Some("alice".to_string()),
                    text: "hey".to_string(),
                    thread_ts: None,
                    channel: None,
                    reply_count: None,
                    reactions: None,
                    edited: None,
                },
                Message {
                    ts: "2.0".to_string(),
                    user: Some("bob".to_string()),
                    text: "hi".to_string(),
                    thread_ts: None,
                    channel: None,
                    reply_count: None,
                    reactions: None,
                    edited: None,
                },
            ],
            has_more: false,
            message: None,
            next_cursor: None,
        };
        let output = format_history_human(&result);
        assert!(output.contains("### alice"));
        assert!(output.contains("hey"));
        assert!(output.contains("### bob"));
        assert!(output.contains("hi"));
    }

    #[test]
    fn format_summary_human_returns_text() {
        let result = SummaryResult {
            summary: "The team discussed plans.".to_string(),
        };
        assert!(format_summary_human(&result).contains("The team discussed plans."));
        assert!(format_summary_human(&result).contains("## Channel Summary"));
    }

    #[test]
    fn render_json_error_writes_to_stdout_writer() {
        let err = CliError {
            detail: ErrorDetail {
                code: "TEST_ERROR",
                message: "something broke".to_string(),
                suggestion: "try again".to_string(),
            },
            human: false,
        };
        let mut stdout_buf = Vec::new();
        let mut stderr_buf = Vec::new();
        err.render_to(&mut stdout_buf, &mut stderr_buf);
        let stdout_str = String::from_utf8(stdout_buf).unwrap();
        let stderr_str = String::from_utf8(stderr_buf).unwrap();
        assert!(stderr_str.is_empty(), "JSON errors should not go to stderr");
        let parsed: serde_json::Value = serde_json::from_str(&stdout_str).unwrap();
        assert_eq!(parsed["success"], false);
        assert_eq!(parsed["error"]["code"], "TEST_ERROR");
    }

    #[test]
    fn render_human_error_writes_to_stderr_writer() {
        let err = CliError {
            detail: ErrorDetail {
                code: "TEST_ERROR",
                message: "something broke".to_string(),
                suggestion: "try again".to_string(),
            },
            human: true,
        };
        let mut stdout_buf = Vec::new();
        let mut stderr_buf = Vec::new();
        err.render_to(&mut stdout_buf, &mut stderr_buf);
        let stdout_str = String::from_utf8(stdout_buf).unwrap();
        let stderr_str = String::from_utf8(stderr_buf).unwrap();
        assert!(
            stdout_str.is_empty(),
            "Human errors should not go to stdout"
        );
        assert!(stderr_str.contains("something broke"));
        assert!(stderr_str.contains("try again"));
    }

    #[test]
    fn format_message_annotation_both_present() {
        let reactions = vec![
            crate::api::Reaction {
                name: "thumbsup".to_string(),
                count: 5,
            },
            crate::api::Reaction {
                name: "heart".to_string(),
                count: 2,
            },
        ];
        let result = format_message_annotation(Some(3), Some(&reactions));
        assert!(result.contains("3 replies"), "Expected reply count");
        assert!(
            result.contains("7 reactions"),
            "Expected total reaction count"
        );
    }

    #[test]
    fn format_message_annotation_replies_only() {
        let result = format_message_annotation(Some(5), None);
        assert!(result.contains("5 replies"), "Expected reply count");
        assert!(
            !result.contains("reactions"),
            "Should not mention reactions"
        );
    }

    #[test]
    fn format_message_annotation_reactions_only() {
        let reactions = vec![crate::api::Reaction {
            name: "thumbsup".to_string(),
            count: 3,
        }];
        let result = format_message_annotation(None, Some(&reactions));
        assert!(!result.contains("replies"), "Should not mention replies");
        assert!(result.contains("3 reactions"), "Expected reaction count");
    }

    #[test]
    fn format_message_annotation_both_none() {
        let result = format_message_annotation(None, None);
        assert!(result.is_empty(), "Expected empty string when both None");
    }

    #[test]
    fn format_message_annotation_zero_reply_count() {
        let result = format_message_annotation(Some(0), None);
        assert!(result.is_empty(), "Expected empty string for zero replies");
    }

    #[test]
    fn format_message_annotation_empty_reactions() {
        let result = format_message_annotation(None, Some(&[]));
        assert!(
            result.is_empty(),
            "Expected empty string for empty reactions"
        );
    }

    #[test]
    fn format_history_human_with_annotations() {
        let reactions = vec![crate::api::Reaction {
            name: "thumbsup".to_string(),
            count: 2,
        }];
        let result = HistoryResult {
            messages: vec![Message {
                ts: "1.0".to_string(),
                user: Some("alice".to_string()),
                text: "hey".to_string(),
                thread_ts: None,
                channel: None,
                reply_count: Some(3),
                reactions: Some(reactions),
                edited: None,
            }],
            has_more: false,
            message: None,
            next_cursor: None,
        };
        let output = format_history_human(&result);
        assert!(output.contains("3 replies"), "Expected reply annotation");
        assert!(
            output.contains("2 reactions"),
            "Expected reaction annotation"
        );
    }

    #[test]
    fn format_search_human_with_annotations() {
        let reactions = vec![crate::api::Reaction {
            name: "wave".to_string(),
            count: 4,
        }];
        let result = SearchResult {
            messages: vec![crate::api::SearchMessage {
                ts: "1.0".to_string(),
                user: Some("bob".to_string()),
                text: "mention".to_string(),
                channel: Some(crate::api::SearchChannel {
                    id: "C1".to_string(),
                    name: "general".to_string(),
                }),
                permalink: None,
                reply_count: Some(2),
                reactions: Some(reactions),
            }],
            total: 1,
        };
        let output = format_search_human(&result);
        assert!(output.contains("2 replies"), "Expected reply annotation");
        assert!(
            output.contains("4 reactions"),
            "Expected reaction annotation"
        );
    }

    #[test]
    fn format_success_with_pagination_includes_pagination_object() {
        let data = serde_json::json!({"messages": []});
        let pagination = Pagination {
            has_more: true,
            next_cursor: Some("next_abc".to_string()),
        };
        let output = format_success_with_pagination(&data, Some(&pagination));
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed["success"], true);
        assert_eq!(parsed["pagination"]["has_more"], true);
        assert_eq!(parsed["pagination"]["next_cursor"], "next_abc");
    }

    #[test]
    fn format_success_with_pagination_omits_pagination_when_none() {
        let data = serde_json::json!({"messages": []});
        let output = format_success_with_pagination(&data, None);
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed["success"], true);
        assert!(parsed.get("pagination").is_none());
    }

    #[test]
    fn format_success_with_pagination_omits_cursor_when_none() {
        let data = serde_json::json!({"messages": []});
        let pagination = Pagination {
            has_more: false,
            next_cursor: None,
        };
        let output = format_success_with_pagination(&data, Some(&pagination));
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed["pagination"]["has_more"], false);
        assert!(parsed["pagination"].get("next_cursor").is_none());
    }

    #[test]
    fn cli_error_exit_code_unknown_is_one() {
        let err = CliError {
            detail: ErrorDetail {
                code: "TEST",
                message: "test".to_string(),
                suggestion: "test".to_string(),
            },
            human: false,
        };
        assert_eq!(err.exit_code(), 1);
    }

    #[test]
    fn exit_code_config_not_found() {
        let err = CliError {
            detail: ErrorDetail {
                code: "CONFIG_NOT_FOUND",
                message: "Config file not found".into(),
                suggestion: "Create config".into(),
            },
            human: false,
        };
        assert_eq!(err.exit_code(), 2);
    }

    #[test]
    fn exit_code_config_parse_error() {
        let err = CliError {
            detail: ErrorDetail {
                code: "CONFIG_PARSE_ERROR",
                message: "Bad TOML".into(),
                suggestion: "Fix syntax".into(),
            },
            human: false,
        };
        assert_eq!(err.exit_code(), 2);
    }

    #[test]
    fn exit_code_op_not_found() {
        let err = CliError {
            detail: ErrorDetail {
                code: "OP_NOT_FOUND",
                message: "1Password CLI not found".into(),
                suggestion: "Install op".into(),
            },
            human: false,
        };
        assert_eq!(err.exit_code(), 3);
    }

    #[test]
    fn exit_code_op_failed() {
        let err = CliError {
            detail: ErrorDetail {
                code: "OP_FAILED",
                message: "Credential retrieval failed".into(),
                suggestion: "Check item ID".into(),
            },
            human: false,
        };
        assert_eq!(err.exit_code(), 3);
    }

    #[test]
    fn exit_code_api_error() {
        let err = CliError {
            detail: ErrorDetail {
                code: "API_ERROR",
                message: "HTTP 500".into(),
                suggestion: "Retry".into(),
            },
            human: false,
        };
        assert_eq!(err.exit_code(), 4);
    }

    #[test]
    fn exit_code_invalid_debug_mode() {
        let err = CliError {
            detail: ErrorDetail {
                code: "INVALID_DEBUG_MODE",
                message: "Bad debug mode".into(),
                suggestion: "Use compact, pretty, curl, or dangerous_no_redact".into(),
            },
            human: false,
        };
        assert_eq!(err.exit_code(), 5);
    }
}
