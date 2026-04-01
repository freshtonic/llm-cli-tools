//! Output formatting for JSON (default) and human-readable modes.

use crate::api::{HistoryResult, SearchResult, SendResult, SummaryResult};
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
        1
    }

    pub fn render(&self) {
        if self.human {
            eprintln!("Error: {}", self.detail.message);
            eprintln!("Suggestion: {}", self.detail.suggestion);
        } else {
            let json = format_error(&self.detail);
            eprintln!("{json}");
        }
    }
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
        "Sent to {} (ts: {})\n{}",
        result.channel, result.ts, result.message.text
    )
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
            out.push_str(&format!("[{}] {}: {}\n", msg.ts, user, msg.text));
        }
    }
    if let Some(ref msg) = result.message {
        out.push_str(&format!("\n{msg}\n"));
    }
    out
}

pub fn format_search_human(result: &SearchResult) -> String {
    let mut out = format!("{} result(s) found.\n\n", result.total);
    for (i, msg) in result.messages.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        let user = msg.user.as_deref().unwrap_or("unknown");
        let channel_name = msg
            .channel
            .as_ref()
            .map(|c| c.name.as_str())
            .unwrap_or("DM");
        out.push_str(&format!(
            "[{}] {} in #{}: {}\n",
            msg.ts, user, channel_name, msg.text
        ));
        if let Some(ref link) = msg.permalink {
            out.push_str(&format!("  {link}\n"));
        }
    }
    out
}

pub fn format_summary_human(result: &SummaryResult) -> String {
    result.summary.clone()
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
                },
                Message {
                    ts: "2.0".to_string(),
                    user: Some("bob".to_string()),
                    text: "hi".to_string(),
                    thread_ts: None,
                    channel: None,
                },
            ],
            has_more: false,
            message: None,
        };
        let output = format_history_human(&result);
        assert!(output.contains("alice: hey"));
        assert!(output.contains("bob: hi"));
    }

    #[test]
    fn format_summary_human_returns_text() {
        let result = SummaryResult {
            summary: "The team discussed plans.".to_string(),
        };
        assert_eq!(format_summary_human(&result), "The team discussed plans.");
    }

    #[test]
    fn cli_error_exit_code_is_one() {
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
}
