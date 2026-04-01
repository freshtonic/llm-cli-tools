//! Slack Web API client.
//!
//! Uses the Slack Web API (https://api.slack.com/methods).
//! Auth is via `Authorization: Bearer <token>` header.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A Slack message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub ts: String,
    #[serde(default)]
    pub user: Option<String>,
    pub text: String,
    #[serde(default)]
    pub thread_ts: Option<String>,
    #[serde(default)]
    pub channel: Option<String>,
}

/// Result from sending a message.
#[derive(Debug, Serialize)]
pub struct SendResult {
    pub channel: String,
    pub ts: String,
    pub message: Message,
}

/// Result from reading channel history.
#[derive(Debug, Serialize)]
pub struct HistoryResult {
    pub messages: Vec<Message>,
    pub has_more: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Result from searching messages.
#[derive(Debug, Serialize)]
pub struct SearchResult {
    pub messages: Vec<SearchMessage>,
    pub total: u64,
}

/// A message from search results (has different structure than channel messages).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchMessage {
    pub ts: String,
    #[serde(default)]
    pub user: Option<String>,
    pub text: String,
    #[serde(default)]
    pub channel: Option<SearchChannel>,
    #[serde(default)]
    pub permalink: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchChannel {
    pub id: String,
    pub name: String,
}

/// Result from requesting a channel summary.
#[derive(Debug, Serialize)]
pub struct SummaryResult {
    pub summary: String,
}

/// Slack API client.
pub struct Client {
    pub token: String,
    pub debug: Option<crate::cli::DebugMode>,
}

impl Client {
    fn debug_error(&self, e: &ureq::Error) -> String {
        if self.is_debug() {
            eprintln!("<<< ERROR: {e}");
            eprintln!();
        }
        format!("HTTP request failed: {e}")
    }

    fn is_debug(&self) -> bool {
        self.debug.is_some()
    }

    fn format_body(&self, body: &str) -> String {
        if self.debug == Some(crate::cli::DebugMode::Pretty) {
            serde_json::from_str::<Value>(body)
                .ok()
                .and_then(|v| serde_json::to_string_pretty(&v).ok())
                .unwrap_or_else(|| body.to_string())
        } else {
            body.to_string()
        }
    }

    fn post(&self, method: &str, body: &Value) -> Result<Value, String> {
        let url = format!("https://slack.com/api/{method}");
        let body_str =
            serde_json::to_string(body).map_err(|e| format!("Serialization error: {e}"))?;

        if self.is_debug() {
            eprintln!(">>> POST {url}");
            eprintln!(">>> Authorization: Bearer <redacted>");
            eprintln!(">>> Content-Type: application/json; charset=utf-8");
            eprintln!(">>> ");
            eprintln!(">>> {}", self.format_body(&body_str));
            eprintln!();
        }

        let mut response = ureq::post(&url)
            .header("Authorization", &format!("Bearer {}", self.token))
            .header("Content-Type", "application/json; charset=utf-8")
            .send(&body_str)
            .map_err(|e| self.debug_error(&e))?;

        if self.is_debug() {
            eprintln!("<<< {}", response.status());
            for (name, value) in response.headers() {
                eprintln!("<<<   {}: {}", name, value.to_str().unwrap_or("<binary>"));
            }
        }

        let text = response
            .body_mut()
            .read_to_string()
            .map_err(|e| format!("Failed to read response: {e}"))?;

        if self.is_debug() {
            eprintln!("<<<");
            eprintln!("<<< {}", self.format_body(&text));
            eprintln!();
        }

        let parsed: Value =
            serde_json::from_str(&text).map_err(|e| format!("Failed to parse JSON: {e}"))?;

        check_slack_error(&parsed)?;
        Ok(parsed)
    }

    fn get(&self, method: &str, params: &[(&str, &str)]) -> Result<Value, String> {
        let mut url = format!("https://slack.com/api/{method}?");
        for (i, (k, v)) in params.iter().enumerate() {
            if i > 0 {
                url.push('&');
            }
            url.push_str(&format!(
                "{}={}",
                k,
                urlencoded::encode(v)
            ));
        }

        if self.is_debug() {
            eprintln!(">>> GET {url}");
            eprintln!(">>> Authorization: Bearer <redacted>");
            eprintln!();
        }

        let mut response = ureq::get(&url)
            .header("Authorization", &format!("Bearer {}", self.token))
            .call()
            .map_err(|e| self.debug_error(&e))?;

        if self.is_debug() {
            eprintln!("<<< {}", response.status());
            for (name, value) in response.headers() {
                eprintln!("<<<   {}: {}", name, value.to_str().unwrap_or("<binary>"));
            }
        }

        let text = response
            .body_mut()
            .read_to_string()
            .map_err(|e| format!("Failed to read response: {e}"))?;

        if self.is_debug() {
            eprintln!("<<<");
            eprintln!("<<< {}", self.format_body(&text));
            eprintln!();
        }

        let parsed: Value =
            serde_json::from_str(&text).map_err(|e| format!("Failed to parse JSON: {e}"))?;

        check_slack_error(&parsed)?;
        Ok(parsed)
    }

    /// Send a message to a channel, optionally as a thread reply.
    pub fn send_message(
        &self,
        channel: &str,
        text: &str,
        thread_ts: Option<&str>,
    ) -> Result<SendResult, String> {
        let mut body = serde_json::json!({
            "channel": channel,
            "text": text,
        });
        if let Some(ts) = thread_ts {
            body["thread_ts"] = Value::String(ts.to_string());
        }

        let response = self.post("chat.postMessage", &body)?;
        parse_send_response(&response)
    }

    /// Read recent messages from a channel.
    pub fn read_history(&self, channel: &str, limit: u32) -> Result<HistoryResult, String> {
        let limit_str = limit.to_string();
        let response = self.get(
            "conversations.history",
            &[("channel", channel), ("limit", &limit_str)],
        )?;
        parse_history_response(&response, limit)
    }

    /// Send a direct message to a user. Opens a DM channel first.
    pub fn send_dm(&self, user: &str, text: &str) -> Result<SendResult, String> {
        // Open (or get existing) DM channel.
        let open_response =
            self.post("conversations.open", &serde_json::json!({ "users": user }))?;

        let dm_channel = open_response
            .pointer("/channel/id")
            .and_then(|v| v.as_str())
            .ok_or("Failed to open DM channel: missing channel ID")?;

        self.send_message(dm_channel, text, None)
    }

    /// Search for messages mentioning the authenticated user.
    pub fn search_mentions(&self, limit: u32) -> Result<SearchResult, String> {
        // Get the authenticated user's ID first.
        let auth_response = self.get("auth.test", &[])?;
        let user_id = auth_response
            .get("user_id")
            .and_then(|v| v.as_str())
            .ok_or("Failed to get authenticated user ID")?;

        let query = format!("<@{user_id}>");
        let count_str = limit.to_string();
        let response = self.get(
            "search.messages",
            &[("query", &query), ("count", &count_str), ("sort", "timestamp")],
        )?;
        parse_search_response(&response)
    }

    /// Request a Slack AI-generated channel summary.
    pub fn get_summary(
        &self,
        channel: &str,
        oldest_ts: &str,
        latest_ts: &str,
    ) -> Result<SummaryResult, String> {
        let body = serde_json::json!({
            "channel": channel,
            "oldest_ts": oldest_ts,
            "latest_ts": latest_ts,
        });
        let response = self.post("conversations.requestSummarize", &body)?;
        parse_summary_response(&response)
    }
}

/// Check the Slack API `ok` field and extract error details.
fn check_slack_error(response: &Value) -> Result<(), String> {
    let ok = response
        .get("ok")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if !ok {
        let error = response
            .get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown_error");
        return Err(format!("Slack API error: {error}"));
    }
    Ok(())
}

/// Parse a chat.postMessage response.
pub fn parse_send_response(body: &Value) -> Result<SendResult, String> {
    let channel = body
        .get("channel")
        .and_then(|v| v.as_str())
        .ok_or("Missing channel in response")?
        .to_string();

    let ts = body
        .get("ts")
        .and_then(|v| v.as_str())
        .ok_or("Missing ts in response")?
        .to_string();

    let msg = body
        .get("message")
        .ok_or("Missing message in response")?;

    let message: Message =
        serde_json::from_value(msg.clone()).map_err(|e| format!("Failed to parse message: {e}"))?;

    Ok(SendResult {
        channel,
        ts,
        message,
    })
}

/// Parse a conversations.history response.
pub fn parse_history_response(body: &Value, limit: u32) -> Result<HistoryResult, String> {
    let messages_arr = body
        .get("messages")
        .and_then(|v| v.as_array())
        .ok_or("Missing messages in response")?;

    let messages: Vec<Message> = messages_arr
        .iter()
        .filter_map(|m| serde_json::from_value(m.clone()).ok())
        .collect();

    let has_more = body
        .get("has_more")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let message = if has_more {
        Some(format!(
            "Results truncated to {limit}. Use --limit to fetch more."
        ))
    } else {
        None
    };

    Ok(HistoryResult {
        messages,
        has_more,
        message,
    })
}

/// Parse a search.messages response.
pub fn parse_search_response(body: &Value) -> Result<SearchResult, String> {
    let total = body
        .pointer("/messages/total")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    let matches = body
        .pointer("/messages/matches")
        .and_then(|v| v.as_array())
        .ok_or("Missing messages.matches in response")?;

    let messages: Vec<SearchMessage> = matches
        .iter()
        .filter_map(|m| serde_json::from_value(m.clone()).ok())
        .collect();

    Ok(SearchResult { messages, total })
}

/// Parse a conversations.requestSummarize response.
pub fn parse_summary_response(body: &Value) -> Result<SummaryResult, String> {
    let summary = body
        .get("summary")
        .and_then(|v| v.as_str())
        .ok_or("Missing summary in response. This feature requires a Slack plan with AI.")?
        .to_string();

    Ok(SummaryResult { summary })
}

/// Minimal URL encoding for query parameters.
mod urlencoded {
    pub fn encode(s: &str) -> String {
        let mut result = String::with_capacity(s.len());
        for c in s.chars() {
            match c {
                'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => result.push(c),
                ' ' => result.push('+'),
                _ => {
                    for byte in c.to_string().as_bytes() {
                        result.push_str(&format!("%{byte:02X}"));
                    }
                }
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_slack_error_ok() {
        let body = serde_json::json!({ "ok": true });
        assert!(check_slack_error(&body).is_ok());
    }

    #[test]
    fn check_slack_error_not_ok() {
        let body = serde_json::json!({ "ok": false, "error": "channel_not_found" });
        let err = check_slack_error(&body).unwrap_err();
        assert!(err.contains("channel_not_found"));
    }

    #[test]
    fn check_slack_error_missing_ok() {
        let body = serde_json::json!({});
        assert!(check_slack_error(&body).is_err());
    }

    #[test]
    fn parse_send_response_success() {
        let body = serde_json::json!({
            "ok": true,
            "channel": "C12345",
            "ts": "1234567890.123456",
            "message": {
                "ts": "1234567890.123456",
                "user": "U12345",
                "text": "hello"
            }
        });
        let result = parse_send_response(&body).unwrap();
        assert_eq!(result.channel, "C12345");
        assert_eq!(result.ts, "1234567890.123456");
        assert_eq!(result.message.text, "hello");
    }

    #[test]
    fn parse_history_response_extracts_messages() {
        let body = serde_json::json!({
            "ok": true,
            "messages": [
                { "ts": "1.0", "user": "U1", "text": "hello" },
                { "ts": "2.0", "user": "U2", "text": "world" }
            ],
            "has_more": false
        });
        let result = parse_history_response(&body, 25).unwrap();
        assert_eq!(result.messages.len(), 2);
        assert_eq!(result.messages[0].text, "hello");
        assert!(!result.has_more);
        assert!(result.message.is_none());
    }

    #[test]
    fn parse_history_response_truncation_message() {
        let body = serde_json::json!({
            "ok": true,
            "messages": [],
            "has_more": true
        });
        let result = parse_history_response(&body, 25).unwrap();
        assert!(result.has_more);
        assert!(result.message.unwrap().contains("truncated"));
    }

    #[test]
    fn parse_search_response_extracts_matches() {
        let body = serde_json::json!({
            "ok": true,
            "messages": {
                "total": 2,
                "matches": [
                    {
                        "ts": "1.0",
                        "user": "U1",
                        "text": "hey <@U99>",
                        "channel": { "id": "C1", "name": "general" },
                        "permalink": "https://slack.com/archives/C1/p1"
                    }
                ]
            }
        });
        let result = parse_search_response(&body).unwrap();
        assert_eq!(result.total, 2);
        assert_eq!(result.messages.len(), 1);
        assert_eq!(result.messages[0].text, "hey <@U99>");
        assert_eq!(result.messages[0].channel.as_ref().unwrap().name, "general");
    }

    #[test]
    fn parse_search_response_empty() {
        let body = serde_json::json!({
            "ok": true,
            "messages": { "total": 0, "matches": [] }
        });
        let result = parse_search_response(&body).unwrap();
        assert_eq!(result.total, 0);
        assert!(result.messages.is_empty());
    }

    #[test]
    fn parse_summary_response_success() {
        let body = serde_json::json!({
            "ok": true,
            "summary": "The team discussed deployment plans."
        });
        let result = parse_summary_response(&body).unwrap();
        assert_eq!(result.summary, "The team discussed deployment plans.");
    }

    #[test]
    fn parse_summary_response_missing_summary() {
        let body = serde_json::json!({ "ok": true });
        let err = parse_summary_response(&body).unwrap_err();
        assert!(err.contains("summary"));
    }

    #[test]
    fn urlencoded_basic() {
        assert_eq!(urlencoded::encode("hello world"), "hello+world");
        assert_eq!(urlencoded::encode("<@U123>"), "%3C%40U123%3E");
        assert_eq!(urlencoded::encode("plain"), "plain");
    }
}
