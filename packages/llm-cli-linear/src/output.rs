//! Output formatting for JSON (default) and human-readable modes.
//!
//! Success responses wrap data in `{"success": true, "data": ...}`.
//! Error responses use `{"success": false, "error": {"code": "...", "message": "...", "suggestion": "..."}}`.
//! With `--human`, errors go to stderr as plain text, data to stdout as formatted text.

use crate::api::{Issue, IssueListResult};
use serde::Serialize;

/// A structured error with code, message, and suggestion for recovery.
#[derive(Debug, Serialize)]
pub struct ErrorDetail {
    pub code: &'static str,
    pub message: String,
    pub suggestion: String,
}

/// Represents a CLI-level error that can be rendered as JSON or human text.
#[derive(Debug)]
pub struct CliError {
    pub detail: ErrorDetail,
    pub human: bool,
}

impl CliError {
    pub fn exit_code(&self) -> i32 {
        1
    }

    /// Render this error to the appropriate output stream.
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

/// Format a success response as JSON.
pub fn format_success<T: Serialize>(data: &T) -> String {
    let wrapper = serde_json::json!({
        "success": true,
        "data": data,
    });
    serde_json::to_string_pretty(&wrapper).expect("serialization should not fail")
}

/// Format an error response as JSON.
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

/// Format a single issue as markdown.
pub fn format_issue_human(issue: &Issue) -> String {
    let state = issue
        .state
        .as_ref()
        .map(|s| s.name.as_str())
        .unwrap_or("Unknown");
    let priority = issue
        .priority
        .map(|p| format!("P{}", p as u8))
        .unwrap_or_else(|| "None".to_string());

    let mut out = format!("## {} — {}\n\n", issue.identifier, issue.title);
    out.push_str(&format!("- **State:** {state}\n"));
    out.push_str(&format!("- **Priority:** {priority}\n"));
    out.push_str(&format!("- **URL:** {}\n", issue.url));

    if let Some(ref desc) = issue.description {
        out.push_str(&format!("\n{desc}\n"));
    }
    out
}

/// Format a list of issues as markdown.
pub fn format_issue_list_human(result: &IssueListResult) -> String {
    let mut out = String::new();
    if result.issues.is_empty() {
        out.push_str("No issues found.\n");
    } else {
        for (i, issue) in result.issues.iter().enumerate() {
            if i > 0 {
                out.push_str("\n---\n\n");
            }
            out.push_str(&format_issue_human(issue));
        }
    }
    if let Some(ref msg) = result.message {
        out.push_str(&format!("\n> {msg}\n"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_success_wraps_data() {
        let data = serde_json::json!({"id": "PROJ-123", "title": "Fix bug"});
        let output = format_success(&data);
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed["success"], true);
        assert_eq!(parsed["data"]["id"], "PROJ-123");
        assert_eq!(parsed["data"]["title"], "Fix bug");
    }

    #[test]
    fn format_success_with_struct() {
        #[derive(Serialize)]
        struct Issue {
            id: String,
            title: String,
        }
        let issue = Issue {
            id: "PROJ-1".to_string(),
            title: "Test issue".to_string(),
        };
        let output = format_success(&issue);
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed["success"], true);
        assert_eq!(parsed["data"]["id"], "PROJ-1");
    }

    #[test]
    fn format_error_includes_all_fields() {
        let detail = ErrorDetail {
            code: "CONFIG_NOT_FOUND",
            message: "Config file not found".to_string(),
            suggestion: "Create a config file".to_string(),
        };
        let output = format_error(&detail);
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed["success"], false);
        assert_eq!(parsed["error"]["code"], "CONFIG_NOT_FOUND");
        assert_eq!(parsed["error"]["message"], "Config file not found");
        assert_eq!(parsed["error"]["suggestion"], "Create a config file");
    }

    #[test]
    fn format_issue_human_includes_all_fields() {
        let issue = Issue {
            id: "uuid-1".to_string(),
            identifier: "PROJ-1".to_string(),
            title: "Fix the thing".to_string(),
            state: Some(crate::api::IssueState {
                name: "In Progress".to_string(),
            }),
            priority: Some(2.0),
            description: Some("A detailed description".to_string()),
            url: "https://linear.app/proj/issue/PROJ-1".to_string(),
        };
        let output = format_issue_human(&issue);
        assert!(output.contains("PROJ-1"));
        assert!(output.contains("Fix the thing"));
        assert!(output.contains("In Progress"));
        assert!(output.contains("P2"));
        assert!(output.contains("A detailed description"));
        assert!(output.contains("https://linear.app/proj/issue/PROJ-1"));
    }

    #[test]
    fn format_issue_human_handles_missing_fields() {
        let issue = Issue {
            id: "uuid-1".to_string(),
            identifier: "PROJ-2".to_string(),
            title: "No desc".to_string(),
            state: None,
            priority: None,
            description: None,
            url: "https://linear.app/proj/issue/PROJ-2".to_string(),
        };
        let output = format_issue_human(&issue);
        assert!(output.contains("Unknown"));
        assert!(output.contains("None"));
        // No description field should be absent, not "(no description)"
        assert!(!output.contains("description"));
    }

    #[test]
    fn format_issue_list_human_empty() {
        let result = IssueListResult {
            issues: vec![],
            total_count: None,
            message: None,
        };
        let output = format_issue_list_human(&result);
        assert!(output.contains("No issues found"));
    }

    #[test]
    fn format_issue_list_human_with_truncation_message() {
        let result = IssueListResult {
            issues: vec![Issue {
                id: "uuid-1".to_string(),
                identifier: "PROJ-1".to_string(),
                title: "Issue 1".to_string(),
                state: None,
                priority: None,
                description: None,
                url: "https://linear.app/proj/issue/PROJ-1".to_string(),
            }],
            total_count: None,
            message: Some("Results truncated to 25.".to_string()),
        };
        let output = format_issue_list_human(&result);
        assert!(output.contains("PROJ-1"));
        assert!(output.contains("truncated"));
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
