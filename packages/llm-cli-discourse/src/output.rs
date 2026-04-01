//! Output formatting for JSON (default) and human-readable modes.
//!
//! Success responses wrap data in `{"success": true, "data": ...}`.
//! Error responses use `{"success": false, "error": {"code": "...", "message": "...", "suggestion": "..."}}`.
//! With `--human`, errors go to stderr as plain text, data to stdout as formatted text.

use std::io::Write;

use crate::api::{CreatePostResponse, LatestPostsResponse, Post, TopicResponse};
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

/// Get the best available content from a post — prefer raw (markdown) over cooked (HTML).
fn post_content(post: &Post) -> String {
    if let Some(ref raw) = post.raw
        && !raw.is_empty()
    {
        return raw.clone();
    }
    strip_html(&post.cooked)
}

/// Strip HTML tags for a basic plain-text/markdown fallback.
fn strip_html(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out.trim().to_string()
}

/// Format a topic response as markdown.
pub fn format_topic_human(response: &TopicResponse) -> String {
    let t = &response.topic;
    let category = t
        .category_id
        .map(|id| id.to_string())
        .unwrap_or_else(|| "none".to_string());

    let mut out = format!("## {}\n\n", t.title);
    out.push_str(&format!("- **Topic ID:** {}\n", t.id));
    out.push_str(&format!("- **Category:** {category}\n"));
    out.push_str(&format!("- **Posts:** {}\n", t.posts_count));
    out.push_str(&format!("- **Views:** {}\n", t.views));
    if t.like_count > 0 {
        out.push_str(&format!("- **Likes:** {}\n", t.like_count));
    }
    if t.reply_count > 0 {
        out.push_str(&format!("- **Replies:** {}\n", t.reply_count));
    }
    if let Some(ref tags) = t.tags
        && !tags.is_empty()
    {
        out.push_str(&format!("- **Tags:** {}\n", tags.join(", ")));
    }
    if let Some(ref last_posted_at) = t.last_posted_at {
        out.push_str(&format!("- **Last posted:** {last_posted_at}\n"));
    }

    for post in &response.posts {
        out.push_str(&format!(
            "\n### #{} by {} ({})\n\n{}\n",
            post.post_number,
            post.username,
            post.created_at,
            post_content(post),
        ));
    }
    out
}

/// Format a created post as markdown.
pub fn format_post_human(response: &CreatePostResponse) -> String {
    let p = &response.post;
    let mut out = format!(
        "## Post #{} in topic #{}\n\n- **By:** {} ({})\n",
        p.id, p.topic_id, p.username, p.created_at,
    );
    if p.like_count > 0 {
        out.push_str(&format!("- **Likes:** {}\n", p.like_count));
    }
    if p.reply_count > 0 {
        out.push_str(&format!("- **Replies:** {}\n", p.reply_count));
    }
    out.push_str(&format!("\n{}\n", post_content(p)));
    out
}

/// Format the latest posts list as markdown.
pub fn format_latest_posts_human(response: &LatestPostsResponse) -> String {
    let mut out = String::new();
    if response.posts.is_empty() {
        out.push_str("No posts found.\n");
    } else {
        for (i, post) in response.posts.iter().enumerate() {
            if i > 0 {
                out.push_str("\n---\n\n");
            }
            out.push_str(&format_post_summary(post));
        }
    }
    out
}

fn format_post_summary(post: &Post) -> String {
    let title = post.topic_title.as_deref().unwrap_or("(untitled)");
    let mut out = format!(
        "## #{} — {}\n\n- **By:** {} in topic #{}\n- **Date:** {}\n",
        post.id, title, post.username, post.topic_id, post.created_at,
    );
    if post.like_count > 0 {
        out.push_str(&format!("- **Likes:** {}\n", post.like_count));
    }
    if post.reply_count > 0 {
        out.push_str(&format!("- **Replies:** {}\n", post.reply_count));
    }
    out.push_str(&format!("\n{}\n", post_content(post)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{Post, Topic};

    #[test]
    fn format_latest_posts_human_with_posts() {
        let response = LatestPostsResponse {
            posts: vec![
                Post {
                    id: 301,
                    topic_id: 50,
                    topic_title: Some("Welcome".to_string()),
                    username: "james".to_string(),
                    raw: None,
                    cooked: "<p>Hello</p>".to_string(),
                    post_number: 1,
                    created_at: "2026-04-01".to_string(),
                    like_count: 0,
                    reply_count: 0,
                    score: None,
                },
                Post {
                    id: 302,
                    topic_id: 51,
                    topic_title: None,
                    username: "alice".to_string(),
                    raw: None,
                    cooked: "<p>World</p>".to_string(),
                    post_number: 1,
                    created_at: "2026-04-01".to_string(),
                    like_count: 0,
                    reply_count: 0,
                    score: None,
                },
            ],
        };
        let output = format_latest_posts_human(&response);
        assert!(output.contains("Welcome"));
        assert!(output.contains("james"));
        assert!(output.contains("(untitled)"));
        assert!(output.contains("alice"));
        assert!(output.contains("---"));
    }

    #[test]
    fn format_latest_posts_human_empty() {
        let response = LatestPostsResponse { posts: vec![] };
        let output = format_latest_posts_human(&response);
        assert!(output.contains("No posts found"));
    }

    #[test]
    fn format_success_wraps_data() {
        let data = serde_json::json!({"id": 42, "title": "Test"});
        let output = format_success(&data);
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed["success"], true);
        assert_eq!(parsed["data"]["id"], 42);
    }

    #[test]
    fn format_error_includes_all_fields() {
        let detail = ErrorDetail {
            code: "NOT_FOUND",
            message: "Topic not found".to_string(),
            suggestion: "Check the topic ID".to_string(),
        };
        let output = format_error(&detail);
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed["success"], false);
        assert_eq!(parsed["error"]["code"], "NOT_FOUND");
        assert_eq!(parsed["error"]["message"], "Topic not found");
    }

    #[test]
    fn format_topic_human_includes_fields() {
        let response = TopicResponse {
            topic: Topic {
                id: 42,
                title: "My Topic".to_string(),
                slug: "my-topic".to_string(),
                category_id: Some(5),
                posts_count: 2,
                views: 100,
                like_count: 0,
                reply_count: 0,
                last_posted_at: None,
                tags: None,
            },
            posts: vec![Post {
                id: 101,
                topic_id: 42,
                topic_title: None,
                username: "james".to_string(),
                raw: None,
                cooked: "<p>Hello</p>".to_string(),
                post_number: 1,
                created_at: "2026-01-01".to_string(),
                like_count: 0,
                reply_count: 0,
                score: None,
            }],
        };
        let output = format_topic_human(&response);
        assert!(output.contains("## My Topic"));
        assert!(output.contains("james"));
        // HTML should be stripped since raw is None
        assert!(output.contains("Hello"));
        assert!(!output.contains("<p>"));
    }

    #[test]
    fn format_post_human_includes_fields() {
        let response = CreatePostResponse {
            post: Post {
                id: 201,
                topic_id: 42,
                topic_title: None,
                username: "james".to_string(),
                raw: None,
                cooked: "<p>New post</p>".to_string(),
                post_number: 1,
                created_at: "2026-01-01".to_string(),
                like_count: 0,
                reply_count: 0,
                score: None,
            },
        };
        let output = format_post_human(&response);
        assert!(output.contains("201"));
        assert!(output.contains("42"));
        assert!(output.contains("james"));
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

    // ---- New field formatting tests ----

    #[test]
    fn format_topic_human_shows_like_and_reply_counts() {
        let response = TopicResponse {
            topic: Topic {
                id: 42,
                title: "My Topic".to_string(),
                slug: "my-topic".to_string(),
                category_id: Some(5),
                posts_count: 2,
                views: 100,
                like_count: 15,
                reply_count: 7,
                last_posted_at: Some("2026-04-01T12:00:00Z".to_string()),
                tags: Some(vec!["rust".to_string(), "cli".to_string()]),
            },
            posts: vec![],
        };
        let output = format_topic_human(&response);
        assert!(output.contains("15"), "Expected like_count in output");
        assert!(output.contains("Likes"), "Expected Likes label");
        assert!(output.contains("Replies"), "Expected Replies label");
        assert!(output.contains("7"), "Expected reply_count in output");
        assert!(output.contains("rust"), "Expected tag in output");
        assert!(output.contains("cli"), "Expected tag in output");
        assert!(output.contains("2026-04-01"), "Expected last_posted_at");
    }

    #[test]
    fn format_topic_human_omits_zero_counts_and_missing_fields() {
        let response = TopicResponse {
            topic: Topic {
                id: 42,
                title: "My Topic".to_string(),
                slug: "my-topic".to_string(),
                category_id: Some(5),
                posts_count: 2,
                views: 100,
                like_count: 0,
                reply_count: 0,
                last_posted_at: None,
                tags: None,
            },
            posts: vec![],
        };
        let output = format_topic_human(&response);
        assert!(!output.contains("Likes"), "Should not show Likes when zero");
        assert!(!output.contains("Replies"), "Should not show Replies when zero");
        assert!(!output.contains("Tags"), "Should not show Tags when missing");
        assert!(
            !output.contains("Last posted"),
            "Should not show Last posted when missing"
        );
    }

    #[test]
    fn format_post_human_shows_counts_when_nonzero() {
        let response = CreatePostResponse {
            post: Post {
                id: 201,
                topic_id: 42,
                topic_title: None,
                username: "james".to_string(),
                raw: None,
                cooked: "<p>New post</p>".to_string(),
                post_number: 1,
                created_at: "2026-01-01".to_string(),
                like_count: 3,
                reply_count: 1,
                score: None,
            },
        };
        let output = format_post_human(&response);
        assert!(output.contains("3"), "Expected like_count");
        assert!(output.contains("1"), "Expected reply_count");
    }

    #[test]
    fn format_post_human_omits_zero_counts() {
        let response = CreatePostResponse {
            post: Post {
                id: 201,
                topic_id: 42,
                topic_title: None,
                username: "james".to_string(),
                raw: None,
                cooked: "<p>New post</p>".to_string(),
                post_number: 1,
                created_at: "2026-01-01".to_string(),
                like_count: 0,
                reply_count: 0,
                score: None,
            },
        };
        let output = format_post_human(&response);
        assert!(!output.contains("Likes"), "Should not show Likes when zero");
        assert!(
            !output.contains("Replies"),
            "Should not show Replies when zero"
        );
    }

    #[test]
    fn format_latest_posts_human_shows_counts_when_nonzero() {
        let response = LatestPostsResponse {
            posts: vec![Post {
                id: 301,
                topic_id: 50,
                topic_title: Some("Welcome".to_string()),
                username: "james".to_string(),
                raw: None,
                cooked: "<p>Hello</p>".to_string(),
                post_number: 1,
                created_at: "2026-04-01".to_string(),
                like_count: 10,
                reply_count: 4,
                score: None,
            }],
        };
        let output = format_latest_posts_human(&response);
        assert!(output.contains("10"), "Expected like_count in summary");
        assert!(output.contains("4"), "Expected reply_count in summary");
    }
}
