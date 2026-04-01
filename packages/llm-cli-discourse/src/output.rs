//! Output formatting for JSON (default) and human-readable modes.
//!
//! Success responses wrap data in `{"success": true, "data": ...}`.
//! Error responses use `{"success": false, "error": {"code": "...", "message": "...", "suggestion": "..."}}`.
//! With `--human`, errors go to stderr as plain text, data to stdout as formatted text.

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

/// Format a topic response as human-readable text.
pub fn format_topic_human(response: &TopicResponse) -> String {
    let t = &response.topic;
    let mut out = format!(
        "Topic #{} — {}\nSlug: {}  Category: {}  Posts: {}  Views: {}\n",
        t.id,
        t.title,
        t.slug,
        t.category_id
            .map(|id| id.to_string())
            .unwrap_or_else(|| "none".to_string()),
        t.posts_count,
        t.views,
    );

    for post in &response.posts {
        out.push_str(&format!(
            "\n  #{} by {} ({})\n  {}\n",
            post.post_number, post.username, post.created_at, post.cooked
        ));
    }
    out
}

/// Format a created post as human-readable text.
pub fn format_post_human(response: &CreatePostResponse) -> String {
    let p = &response.post;
    format!(
        "Post #{} in topic #{} by {} ({})\n{}",
        p.id, p.topic_id, p.username, p.created_at, p.cooked
    )
}

/// Format the latest posts list as human-readable text.
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
            out.push('\n');
        }
    }
    out
}

fn format_post_summary(post: &Post) -> String {
    let title = post
        .topic_title
        .as_deref()
        .unwrap_or("(untitled)");
    format!(
        "#{} — {}\nBy {} in topic #{} ({})\n{}",
        post.id, title, post.username, post.topic_id, post.created_at, post.cooked
    )
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
            }],
        };
        let output = format_topic_human(&response);
        assert!(output.contains("My Topic"));
        assert!(output.contains("my-topic"));
        assert!(output.contains("james"));
        assert!(output.contains("<p>Hello</p>"));
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
            },
        };
        let output = format_post_human(&response);
        assert!(output.contains("201"));
        assert!(output.contains("42"));
        assert!(output.contains("james"));
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
