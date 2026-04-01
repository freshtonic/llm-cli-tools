//! Output formatting for JSON (default) and human-readable modes.
//!
//! Success responses wrap data in `{"success": true, "data": ...}`.
//! Error responses use `{"success": false, "error": {"code": "...", "message": "...", "suggestion": "..."}}`.
//! With `--human`, errors go to stderr as plain text, data to stdout as formatted text.

use std::io::Write;

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
    if let Some(ref assignee) = issue.assignee {
        out.push_str(&format!("- **Assignee:** {}\n", assignee.name));
    }
    if let Some(ref team) = issue.team {
        out.push_str(&format!("- **Team:** {}\n", team.key));
    }
    out.push_str(&format!("- **Priority:** {priority}\n"));
    if let Some(ref labels) = issue.labels
        && !labels.nodes.is_empty()
    {
        let label_names: Vec<&str> = labels.nodes.iter().map(|l| l.name.as_str()).collect();
        out.push_str(&format!("- **Labels:** {}\n", label_names.join(", ")));
    }
    out.push_str(&format!("- **URL:** {}\n", issue.url));
    if let Some(ref created_at) = issue.created_at {
        out.push_str(&format!("- **Created:** {created_at}\n"));
    }

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
            assignee: None,
            team: None,
            labels: None,
            created_at: None,
            updated_at: None,
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
            assignee: None,
            team: None,
            labels: None,
            created_at: None,
            updated_at: None,
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
                assignee: None,
                team: None,
                labels: None,
                created_at: None,
                updated_at: None,
            }],
            total_count: None,
            message: Some("Results truncated to 25.".to_string()),
        };
        let output = format_issue_list_human(&result);
        assert!(output.contains("PROJ-1"));
        assert!(output.contains("truncated"));
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
        assert!(stdout_str.is_empty(), "Human errors should not go to stdout");
        assert!(stderr_str.contains("something broke"));
        assert!(stderr_str.contains("try again"));
    }

    #[test]
    fn format_issue_human_shows_assignee_when_present() {
        let issue = Issue {
            id: "uuid-1".to_string(),
            identifier: "PROJ-1".to_string(),
            title: "Test".to_string(),
            state: Some(crate::api::IssueState {
                name: "In Progress".to_string(),
            }),
            priority: None,
            description: None,
            url: "https://example.com".to_string(),
            assignee: Some(crate::api::Assignee {
                name: "Alice".to_string(),
                email: None,
            }),
            team: None,
            labels: None,
            created_at: None,
            updated_at: None,
        };
        let output = format_issue_human(&issue);
        assert!(output.contains("Alice"), "Expected assignee name in output");
    }

    #[test]
    fn format_issue_human_shows_team_when_present() {
        let issue = Issue {
            id: "uuid-1".to_string(),
            identifier: "PROJ-1".to_string(),
            title: "Test".to_string(),
            state: None,
            priority: None,
            description: None,
            url: "https://example.com".to_string(),
            assignee: None,
            team: Some(crate::api::IssueTeam {
                key: "ENG".to_string(),
                name: "Engineering".to_string(),
            }),
            labels: None,
            created_at: None,
            updated_at: None,
        };
        let output = format_issue_human(&issue);
        assert!(output.contains("ENG"), "Expected team key in output");
    }

    #[test]
    fn format_issue_human_shows_labels_when_present() {
        let issue = Issue {
            id: "uuid-1".to_string(),
            identifier: "PROJ-1".to_string(),
            title: "Test".to_string(),
            state: None,
            priority: None,
            description: None,
            url: "https://example.com".to_string(),
            assignee: None,
            team: None,
            labels: Some(crate::api::LabelsConnection {
                nodes: vec![
                    crate::api::IssueLabel { name: "bug".to_string() },
                    crate::api::IssueLabel { name: "urgent".to_string() },
                ],
            }),
            created_at: None,
            updated_at: None,
        };
        let output = format_issue_human(&issue);
        assert!(output.contains("bug"), "Expected label in output");
        assert!(output.contains("urgent"), "Expected label in output");
    }

    #[test]
    fn format_issue_human_shows_created_at_when_present() {
        let issue = Issue {
            id: "uuid-1".to_string(),
            identifier: "PROJ-1".to_string(),
            title: "Test".to_string(),
            state: None,
            priority: None,
            description: None,
            url: "https://example.com".to_string(),
            assignee: None,
            team: None,
            labels: None,
            created_at: Some("2026-01-01T00:00:00Z".to_string()),
            updated_at: None,
        };
        let output = format_issue_human(&issue);
        assert!(output.contains("2026-01-01"), "Expected created_at in output");
    }

    #[test]
    fn format_issue_human_omits_empty_labels() {
        let issue = Issue {
            id: "uuid-1".to_string(),
            identifier: "PROJ-1".to_string(),
            title: "Test".to_string(),
            state: None,
            priority: None,
            description: None,
            url: "https://example.com".to_string(),
            assignee: None,
            team: None,
            labels: Some(crate::api::LabelsConnection { nodes: vec![] }),
            created_at: None,
            updated_at: None,
        };
        let output = format_issue_human(&issue);
        assert!(!output.contains("Labels"), "Should not show Labels line when empty");
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
