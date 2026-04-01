//! Discourse REST API client.
//!
//! Constructs HTTP requests and parses responses for the Discourse API.
//! Auth is via `Api-Key` and `Api-Username` headers.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A Discourse topic as returned by the API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Topic {
    pub id: u64,
    pub title: String,
    pub slug: String,
    pub category_id: Option<u64>,
    #[serde(default)]
    pub posts_count: u64,
    #[serde(default)]
    pub views: u64,
}

/// A Discourse post (the first post in a topic, or a reply).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Post {
    pub id: u64,
    pub topic_id: u64,
    #[serde(default)]
    pub topic_title: Option<String>,
    pub username: String,
    #[serde(default)]
    pub raw: Option<String>,
    pub cooked: String,
    pub post_number: u64,
    pub created_at: String,
}

/// Response from fetching a topic — includes both topic metadata and posts.
#[derive(Debug, Serialize)]
pub struct TopicResponse {
    pub topic: Topic,
    pub posts: Vec<Post>,
}

/// Response from listing latest posts.
#[derive(Debug, Serialize)]
pub struct LatestPostsResponse {
    pub posts: Vec<Post>,
}

/// Response from creating a post or reply.
#[derive(Debug, Serialize)]
pub struct CreatePostResponse {
    pub post: Post,
}

/// Shared HTTP client state for making Discourse API calls.
pub struct Client {
    pub base_url: String,
    pub api_key: String,
    pub api_username: String,
    pub debug: bool,
}

impl Client {
    fn debug_request(&self, method: &str, url: &str, body: Option<&str>) {
        if self.debug {
            eprintln!(">>> {method} {url}");
            eprintln!(">>> Api-Key: <redacted>");
            eprintln!(">>> Api-Username: {}", self.api_username);
            if let Some(b) = body {
                eprintln!(">>> Content-Type: application/json");
                eprintln!(">>> ");
                eprintln!(">>> {b}");
            }
            eprintln!();
        }
    }

    fn get(&self, path: &str) -> Result<Value, String> {
        let url = format!("{}{path}", self.base_url);
        self.debug_request("GET", &url, None);

        let mut response = ureq::get(&url)
            .header("Api-Key", &self.api_key)
            .header("Api-Username", &self.api_username)
            .header("Accept", "application/json")
            .call()
            .map_err(|e| format!("HTTP request failed: {e}"))?;

        if self.debug {
            eprintln!("<<< {}", response.status());
            for (name, value) in response.headers() {
                eprintln!("<<<   {}: {}", name, value.to_str().unwrap_or("<binary>"));
            }
        }

        let text = response
            .body_mut()
            .read_to_string()
            .map_err(|e| format!("Failed to read response: {e}"))?;

        if self.debug {
            eprintln!("<<<");
            eprintln!("<<< {text}");
            eprintln!();
        }
        serde_json::from_str(&text).map_err(|e| format!("Failed to parse response JSON: {e}"))
    }

    fn post(&self, path: &str, body: &Value) -> Result<Value, String> {
        let url = format!("{}{path}", self.base_url);
        let body_str =
            serde_json::to_string(body).map_err(|e| format!("Serialization error: {e}"))?;

        self.debug_request("POST", &url, Some(&body_str));

        let mut response = ureq::post(&url)
            .header("Api-Key", &self.api_key)
            .header("Api-Username", &self.api_username)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .send(&body_str)
            .map_err(|e| format!("HTTP request failed: {e}"))?;

        if self.debug {
            eprintln!("<<< {}", response.status());
            for (name, value) in response.headers() {
                eprintln!("<<<   {}: {}", name, value.to_str().unwrap_or("<binary>"));
            }
        }

        let text = response
            .body_mut()
            .read_to_string()
            .map_err(|e| format!("Failed to read response: {e}"))?;

        if self.debug {
            eprintln!("<<<");
            eprintln!("<<< {text}");
            eprintln!();
        }
        serde_json::from_str(&text).map_err(|e| format!("Failed to parse response JSON: {e}"))
    }

    fn delete(&self, path: &str) -> Result<(), String> {
        let url = format!("{}{path}", self.base_url);
        self.debug_request("DELETE", &url, None);

        let mut response = ureq::delete(&url)
            .header("Api-Key", &self.api_key)
            .header("Api-Username", &self.api_username)
            .call()
            .map_err(|e| format!("HTTP request failed: {e}"))?;

        if self.debug {
            eprintln!("<<< {}", response.status());
            for (name, value) in response.headers() {
                eprintln!("<<<   {}: {}", name, value.to_str().unwrap_or("<binary>"));
            }
            let text = response
                .body_mut()
                .read_to_string()
                .unwrap_or_default();
            eprintln!("<<<");
            eprintln!("<<< {text}");
            eprintln!();
        }
        Ok(())
    }

    /// List the latest posts across all topics.
    pub fn list_latest_posts(&self) -> Result<LatestPostsResponse, String> {
        let body = self.get("/posts.json")?;
        parse_latest_posts_response(&body)
    }

    /// Fetch a topic by ID, including its posts.
    pub fn get_topic(&self, topic_id: u64) -> Result<TopicResponse, String> {
        let body = self.get(&format!("/t/{topic_id}.json"))?;
        parse_topic_response(&body)
    }

    /// Create a new topic with a first post.
    pub fn create_topic(
        &self,
        title: &str,
        category: &str,
        raw: Option<&str>,
    ) -> Result<CreatePostResponse, String> {
        // Try category as numeric ID first, fall back to name lookup.
        let category_id = match category.parse::<u64>() {
            Ok(id) => id,
            Err(_) => self.lookup_category_id(category)?,
        };

        let body = serde_json::json!({
            "title": title,
            "raw": raw.unwrap_or(""),
            "category": category_id,
        });

        let response = self.post("/posts.json", &body)?;
        parse_create_post_response(&response)
    }

    /// Delete a topic by ID.
    pub fn delete_topic(&self, topic_id: u64) -> Result<(), String> {
        self.delete(&format!("/t/{topic_id}.json"))
    }

    /// Create a reply to an existing topic.
    pub fn create_reply(&self, topic_id: u64, raw: &str) -> Result<CreatePostResponse, String> {
        let body = serde_json::json!({
            "topic_id": topic_id,
            "raw": raw,
        });

        let response = self.post("/posts.json", &body)?;
        parse_create_post_response(&response)
    }

    /// Delete a post (comment) by ID.
    pub fn delete_post(&self, post_id: u64) -> Result<(), String> {
        self.delete(&format!("/posts/{post_id}.json"))
    }

    /// Look up a category ID by name.
    fn lookup_category_id(&self, name: &str) -> Result<u64, String> {
        let body = self.get("/categories.json")?;
        parse_category_id(&body, name)
    }
}

/// Parse the latest posts response from `GET /posts.json`.
pub fn parse_latest_posts_response(body: &Value) -> Result<LatestPostsResponse, String> {
    let posts_array = body
        .pointer("/latest_posts")
        .and_then(|v| v.as_array())
        .ok_or("Unexpected response: missing latest_posts")?;

    let posts: Vec<Post> = posts_array
        .iter()
        .filter_map(|p| serde_json::from_value(p.clone()).ok())
        .collect();

    Ok(LatestPostsResponse { posts })
}

/// Parse a topic response from the Discourse API.
pub fn parse_topic_response(body: &Value) -> Result<TopicResponse, String> {
    let topic: Topic = serde_json::from_value(
        serde_json::json!({
            "id": body.get("id").ok_or("Missing topic id")?,
            "title": body.get("title").ok_or("Missing topic title")?,
            "slug": body.get("slug").ok_or("Missing topic slug")?,
            "category_id": body.get("category_id"),
            "posts_count": body.get("posts_count").and_then(|v| v.as_u64()).unwrap_or(0),
            "views": body.get("views").and_then(|v| v.as_u64()).unwrap_or(0),
        }),
    )
    .map_err(|e| format!("Failed to parse topic: {e}"))?;

    let posts = body
        .pointer("/post_stream/posts")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|p| serde_json::from_value(p.clone()).ok())
                .collect()
        })
        .unwrap_or_default();

    Ok(TopicResponse { topic, posts })
}

/// Parse a create-post response.
pub fn parse_create_post_response(body: &Value) -> Result<CreatePostResponse, String> {
    // Check for errors array in response.
    if let Some(errors) = body.get("errors").and_then(|v| v.as_array()) {
        let msgs: Vec<&str> = errors.iter().filter_map(|e| e.as_str()).collect();
        return Err(format!("Discourse error: {}", msgs.join(", ")));
    }

    let post: Post =
        serde_json::from_value(body.clone()).map_err(|e| format!("Failed to parse post: {e}"))?;

    Ok(CreatePostResponse { post })
}

/// Parse a category ID from the categories list response.
pub fn parse_category_id(body: &Value, name: &str) -> Result<u64, String> {
    let categories = body
        .pointer("/category_list/categories")
        .and_then(|v| v.as_array())
        .ok_or("Unexpected response: missing categories list")?;

    let lower_name = name.to_lowercase();
    for cat in categories {
        if let Some(cat_name) = cat.get("name").and_then(|n| n.as_str()) {
            if cat_name.to_lowercase() == lower_name {
                if let Some(id) = cat.get("id").and_then(|id| id.as_u64()) {
                    return Ok(id);
                }
            }
        }
    }

    Err(format!("Category '{name}' not found"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_latest_posts_response_extracts_posts() {
        let body = serde_json::json!({
            "latest_posts": [
                {
                    "id": 301,
                    "topic_id": 50,
                    "topic_title": "Welcome",
                    "username": "james",
                    "cooked": "<p>Hello world</p>",
                    "post_number": 1,
                    "created_at": "2026-04-01T00:00:00Z"
                },
                {
                    "id": 302,
                    "topic_id": 51,
                    "topic_title": "Second topic",
                    "username": "alice",
                    "cooked": "<p>Another post</p>",
                    "post_number": 1,
                    "created_at": "2026-04-01T01:00:00Z"
                }
            ]
        });
        let result = parse_latest_posts_response(&body).unwrap();
        assert_eq!(result.posts.len(), 2);
        assert_eq!(result.posts[0].id, 301);
        assert_eq!(result.posts[0].topic_title.as_deref(), Some("Welcome"));
        assert_eq!(result.posts[1].username, "alice");
    }

    #[test]
    fn parse_latest_posts_response_empty() {
        let body = serde_json::json!({ "latest_posts": [] });
        let result = parse_latest_posts_response(&body).unwrap();
        assert!(result.posts.is_empty());
    }

    #[test]
    fn parse_latest_posts_response_missing_key() {
        let body = serde_json::json!({});
        assert!(parse_latest_posts_response(&body).is_err());
    }

    #[test]
    fn parse_topic_response_extracts_fields() {
        let body = serde_json::json!({
            "id": 42,
            "title": "My Topic",
            "slug": "my-topic",
            "category_id": 5,
            "posts_count": 3,
            "views": 100,
            "post_stream": {
                "posts": [
                    {
                        "id": 101,
                        "topic_id": 42,
                        "username": "james",
                        "cooked": "<p>Hello</p>",
                        "post_number": 1,
                        "created_at": "2026-01-01T00:00:00Z"
                    }
                ]
            }
        });
        let result = parse_topic_response(&body).unwrap();
        assert_eq!(result.topic.id, 42);
        assert_eq!(result.topic.title, "My Topic");
        assert_eq!(result.topic.slug, "my-topic");
        assert_eq!(result.topic.category_id, Some(5));
        assert_eq!(result.topic.posts_count, 3);
        assert_eq!(result.posts.len(), 1);
        assert_eq!(result.posts[0].username, "james");
    }

    #[test]
    fn parse_topic_response_missing_posts() {
        let body = serde_json::json!({
            "id": 42,
            "title": "My Topic",
            "slug": "my-topic",
            "category_id": null,
            "posts_count": 0,
            "views": 0
        });
        let result = parse_topic_response(&body).unwrap();
        assert_eq!(result.topic.id, 42);
        assert!(result.posts.is_empty());
    }

    #[test]
    fn parse_topic_response_missing_title_is_error() {
        let body = serde_json::json!({ "id": 42, "slug": "s" });
        assert!(parse_topic_response(&body).is_err());
    }

    #[test]
    fn parse_create_post_response_success() {
        let body = serde_json::json!({
            "id": 201,
            "topic_id": 42,
            "username": "james",
            "cooked": "<p>New post</p>",
            "post_number": 1,
            "created_at": "2026-01-01T00:00:00Z"
        });
        let result = parse_create_post_response(&body).unwrap();
        assert_eq!(result.post.id, 201);
        assert_eq!(result.post.topic_id, 42);
    }

    #[test]
    fn parse_create_post_response_with_errors() {
        let body = serde_json::json!({
            "errors": ["Title is too short", "Body is required"]
        });
        let err = parse_create_post_response(&body).unwrap_err();
        assert!(err.contains("Title is too short"));
        assert!(err.contains("Body is required"));
    }

    #[test]
    fn parse_category_id_finds_match() {
        let body = serde_json::json!({
            "category_list": {
                "categories": [
                    { "id": 1, "name": "General" },
                    { "id": 5, "name": "Support" },
                    { "id": 10, "name": "Announcements" }
                ]
            }
        });
        assert_eq!(parse_category_id(&body, "Support").unwrap(), 5);
    }

    #[test]
    fn parse_category_id_case_insensitive() {
        let body = serde_json::json!({
            "category_list": {
                "categories": [
                    { "id": 1, "name": "General" }
                ]
            }
        });
        assert_eq!(parse_category_id(&body, "general").unwrap(), 1);
    }

    #[test]
    fn parse_category_id_not_found() {
        let body = serde_json::json!({
            "category_list": {
                "categories": [
                    { "id": 1, "name": "General" }
                ]
            }
        });
        let err = parse_category_id(&body, "missing").unwrap_err();
        assert!(err.contains("missing"));
        assert!(err.contains("not found"));
    }

    #[test]
    fn parse_category_id_missing_categories() {
        let body = serde_json::json!({});
        assert!(parse_category_id(&body, "General").is_err());
    }
}
