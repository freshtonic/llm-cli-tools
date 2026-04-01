//! Linear GraphQL API client.
//!
//! Constructs GraphQL queries/mutations and parses responses.
//! The HTTP transport is kept thin (single function) so that
//! query construction and response parsing are independently testable.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// An issue as returned by the Linear API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Issue {
    pub id: String,
    pub identifier: String,
    pub title: String,
    pub state: Option<IssueState>,
    pub priority: Option<f64>,
    pub description: Option<String>,
    pub url: String,
}

/// The state of an issue (e.g., "In Progress", "Done").
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueState {
    pub name: String,
}

/// Result of listing issues, including truncation info.
#[derive(Debug, Serialize)]
pub struct IssueListResult {
    pub issues: Vec<Issue>,
    pub total_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// A GraphQL request body.
#[derive(Debug, Serialize)]
pub struct GraphqlRequest {
    pub query: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variables: Option<Value>,
}

/// Build the GraphQL query for listing issues assigned to the viewer.
pub fn build_list_query(limit: u32) -> GraphqlRequest {
    let query = format!(
        r#"query {{
  viewer {{
    assignedIssues(first: {limit}) {{
      nodes {{
        id
        identifier
        title
        state {{ name }}
        priority
        description
        url
      }}
      pageInfo {{
        hasNextPage
      }}
    }}
  }}
}}"#
    );
    GraphqlRequest {
        query,
        variables: None,
    }
}

/// Parse the response from a list issues query.
pub fn parse_list_response(body: &Value, limit: u32) -> Result<IssueListResult, String> {
    let nodes = body
        .pointer("/data/viewer/assignedIssues/nodes")
        .and_then(|v| v.as_array())
        .ok_or("Unexpected response: missing assignedIssues.nodes")?;

    let issues: Vec<Issue> = nodes
        .iter()
        .map(|n| serde_json::from_value(n.clone()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("Failed to parse issue: {e}"))?;

    let has_next_page = body
        .pointer("/data/viewer/assignedIssues/pageInfo/hasNextPage")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let message = if has_next_page {
        Some(format!(
            "Results truncated to {limit}. Use --limit to fetch more, or narrow your query."
        ))
    } else {
        None
    };

    Ok(IssueListResult {
        issues,
        total_count: None,
        message,
    })
}

/// Build the GraphQL query for fetching a single issue by identifier.
pub fn build_get_query(identifier: &str) -> GraphqlRequest {
    let query = r#"query($id: String!) {
  issue(id: $id) {
    id
    identifier
    title
    state { name }
    priority
    description
    url
  }
}"#
    .to_string();
    let variables = serde_json::json!({ "id": identifier });
    GraphqlRequest {
        query,
        variables: Some(variables),
    }
}

/// Parse the response from a get issue query.
pub fn parse_get_response(body: &Value) -> Result<Issue, String> {
    let issue_value = body
        .pointer("/data/issue")
        .ok_or("Unexpected response: missing issue data")?;

    if issue_value.is_null() {
        return Err("Issue not found".to_string());
    }

    serde_json::from_value(issue_value.clone()).map_err(|e| format!("Failed to parse issue: {e}"))
}

/// Build the GraphQL mutation for creating an issue.
pub fn build_create_mutation(
    title: &str,
    team_id: &str,
    description: Option<&str>,
    priority: Option<u8>,
) -> GraphqlRequest {
    let query = r#"mutation($input: IssueCreateInput!) {
  issueCreate(input: $input) {
    success
    issue {
      id
      identifier
      title
      state { name }
      priority
      description
      url
    }
  }
}"#
    .to_string();

    let mut input = serde_json::json!({
        "title": title,
        "teamId": team_id,
    });

    if let Some(desc) = description {
        input["description"] = Value::String(desc.to_string());
    }
    if let Some(p) = priority {
        input["priority"] = Value::Number(p.into());
    }

    let variables = serde_json::json!({ "input": input });
    GraphqlRequest {
        query,
        variables: Some(variables),
    }
}

/// Parse the response from a create issue mutation.
pub fn parse_create_response(body: &Value) -> Result<Issue, String> {
    let success = body
        .pointer("/data/issueCreate/success")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if !success {
        return Err("Issue creation failed".to_string());
    }

    let issue_value = body
        .pointer("/data/issueCreate/issue")
        .ok_or("Unexpected response: missing created issue data")?;

    serde_json::from_value(issue_value.clone()).map_err(|e| format!("Failed to parse issue: {e}"))
}

/// Build the GraphQL mutation for closing an issue (setting state to "Done").
///
/// This requires knowing the "Done" state ID for the issue's team. The approach
/// is to first look up the issue to find its team, then find the "Done" state,
/// then update. For simplicity, we use a two-step approach: the caller resolves
/// the done state ID and passes it here.
pub fn build_close_mutation(issue_id: &str, done_state_id: &str) -> GraphqlRequest {
    let query = r#"mutation($id: String!, $input: IssueUpdateInput!) {
  issueUpdate(id: $id, input: $input) {
    success
    issue {
      id
      identifier
      title
      state { name }
      priority
      description
      url
    }
  }
}"#
    .to_string();

    let variables = serde_json::json!({
        "id": issue_id,
        "input": { "stateId": done_state_id },
    });

    GraphqlRequest {
        query,
        variables: Some(variables),
    }
}

/// Parse the response from a close (update) issue mutation.
pub fn parse_close_response(body: &Value) -> Result<Issue, String> {
    let success = body
        .pointer("/data/issueUpdate/success")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if !success {
        return Err("Issue update failed".to_string());
    }

    let issue_value = body
        .pointer("/data/issueUpdate/issue")
        .ok_or("Unexpected response: missing updated issue data")?;

    serde_json::from_value(issue_value.clone()).map_err(|e| format!("Failed to parse issue: {e}"))
}

/// Build the query to find the "Done" workflow state for a given issue.
/// First fetches the issue to get its team, then queries the team's workflow states.
pub fn build_issue_team_query(identifier: &str) -> GraphqlRequest {
    let query = r#"query($id: String!) {
  issue(id: $id) {
    id
    team {
      states {
        nodes {
          id
          name
          type
        }
      }
    }
  }
}"#
    .to_string();
    let variables = serde_json::json!({ "id": identifier });
    GraphqlRequest {
        query,
        variables: Some(variables),
    }
}

/// Parse the team states response to find the "Done" state ID.
pub fn parse_done_state_id(body: &Value) -> Result<(String, String), String> {
    let issue_id = body
        .pointer("/data/issue/id")
        .and_then(|v| v.as_str())
        .ok_or("Issue not found")?
        .to_string();

    let states = body
        .pointer("/data/issue/team/states/nodes")
        .and_then(|v| v.as_array())
        .ok_or("Unexpected response: missing team states")?;

    // Look for a state with type "completed" (Linear's "Done" states have this type).
    let done_state_id = states
        .iter()
        .find(|s| s.get("type").and_then(|t| t.as_str()) == Some("completed"))
        .and_then(|s| s.get("id"))
        .and_then(|id| id.as_str())
        .ok_or("No 'Done' state found for this issue's team")?
        .to_string();

    Ok((issue_id, done_state_id))
}

/// Format a body string for debug output, optionally pretty-printing.
fn format_debug_body(body: &str, pretty: bool) -> String {
    if !pretty {
        return body.to_string();
    }
    if let Ok(mut parsed) = serde_json::from_str::<Value>(body) {
        // Extract GraphQL query to print separately, unescaped.
        let query = parsed
            .get("query")
            .and_then(|q| q.as_str())
            .map(|q| q.to_string());

        if query.is_some() {
            parsed
                .as_object_mut()
                .unwrap()
                .remove("query");
        }

        let mut out = String::new();

        if let Some(q) = &query {
            out.push_str("--- GraphQL Query ---\n");
            out.push_str(q.trim());
            out.push_str("\n---------------------\n");
        }

        // Print remaining fields (variables, etc.) if any exist.
        if let Some(obj) = parsed.as_object() {
            if !obj.is_empty() {
                out.push_str(&serde_json::to_string_pretty(&parsed)
                    .unwrap_or_else(|_| body.to_string()));
            }
        }

        if out.is_empty() { body.to_string() } else { out }
    } else {
        body.to_string()
    }
}

/// Send a GraphQL request to the Linear API.
pub fn execute(
    api_url: &str,
    api_key: &str,
    request: &GraphqlRequest,
    debug: Option<crate::cli::DebugMode>,
) -> Result<Value, String> {
    let url = format!("{api_url}/graphql");
    let body = serde_json::to_string(request).map_err(|e| format!("Serialization error: {e}"))?;
    let pretty = debug == Some(crate::cli::DebugMode::Pretty);

    if debug.is_some() {
        eprintln!(">>> POST {url}");
        eprintln!(">>> Authorization: Bearer <redacted>");
        eprintln!(">>> Content-Type: application/json");
        eprintln!(">>> ");
        eprintln!(">>> {}", format_debug_body(&body, pretty));
        eprintln!();
    }

    let mut response = match ureq::post(&url)
        .header("Authorization", &format!("Bearer {api_key}"))
        .header("Content-Type", "application/json")
        .send(&body)
    {
        Ok(resp) => resp,
        Err(e) => {
            if debug.is_some() {
                eprintln!("<<< ERROR: {e}");
                eprintln!();
            }
            return Err(format!("HTTP request failed: {e}"));
        }
    };

    if debug.is_some() {
        eprintln!("<<< {}", response.status());
        for (name, value) in response.headers() {
            eprintln!("<<<   {}: {}", name, value.to_str().unwrap_or("<binary>"));
        }
    }

    let response_text = response
        .body_mut()
        .read_to_string()
        .map_err(|e| format!("Failed to read response: {e}"))?;

    if debug.is_some() {
        eprintln!("<<<");
        eprintln!("<<< {}", format_debug_body(&response_text, pretty));
        eprintln!();
    }

    let response_body: Value = serde_json::from_str(&response_text)
        .map_err(|e| format!("Failed to parse response JSON: {e}"))?;

    // Check for GraphQL-level errors.
    if let Some(errors) = response_body.get("errors").and_then(Value::as_array)
        && let Some(first) = errors.first()
    {
        let msg = first
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("Unknown GraphQL error");
        return Err(format!("GraphQL error: {msg}"));
    }

    Ok(response_body)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- Query/mutation construction tests ----

    #[test]
    fn build_list_query_includes_limit() {
        let req = build_list_query(10);
        assert!(req.query.contains("first: 10"));
        assert!(req.variables.is_none());
    }

    #[test]
    fn build_get_query_sets_variable() {
        let req = build_get_query("PROJ-123");
        assert!(req.query.contains("issue(id: $id)"));
        let vars = req.variables.unwrap();
        assert_eq!(vars["id"], "PROJ-123");
    }

    #[test]
    fn build_create_mutation_required_fields() {
        let req = build_create_mutation("Fix bug", "team-uuid", None, None);
        assert!(req.query.contains("issueCreate"));
        let vars = req.variables.unwrap();
        assert_eq!(vars["input"]["title"], "Fix bug");
        assert_eq!(vars["input"]["teamId"], "team-uuid");
        assert!(vars["input"].get("description").is_none());
        assert!(vars["input"].get("priority").is_none());
    }

    #[test]
    fn build_create_mutation_all_fields() {
        let req = build_create_mutation("Fix bug", "team-uuid", Some("Details here"), Some(2));
        let vars = req.variables.unwrap();
        assert_eq!(vars["input"]["description"], "Details here");
        assert_eq!(vars["input"]["priority"], 2);
    }

    #[test]
    fn build_close_mutation_sets_state() {
        let req = build_close_mutation("issue-uuid", "done-state-uuid");
        assert!(req.query.contains("issueUpdate"));
        let vars = req.variables.unwrap();
        assert_eq!(vars["id"], "issue-uuid");
        assert_eq!(vars["input"]["stateId"], "done-state-uuid");
    }

    // ---- Response parsing tests ----

    #[test]
    fn parse_list_response_extracts_issues() {
        let body = serde_json::json!({
            "data": {
                "viewer": {
                    "assignedIssues": {
                        "nodes": [
                            {
                                "id": "uuid-1",
                                "identifier": "PROJ-1",
                                "title": "First issue",
                                "state": { "name": "In Progress" },
                                "priority": 2.0,
                                "description": "Desc",
                                "url": "https://linear.app/proj/issue/PROJ-1"
                            }
                        ],
                        "pageInfo": { "hasNextPage": false }
                    }
                }
            }
        });
        let result = parse_list_response(&body, 25).unwrap();
        assert_eq!(result.issues.len(), 1);
        assert_eq!(result.issues[0].identifier, "PROJ-1");
        assert!(result.message.is_none());
    }

    #[test]
    fn parse_list_response_truncation_message() {
        let body = serde_json::json!({
            "data": {
                "viewer": {
                    "assignedIssues": {
                        "nodes": [],
                        "pageInfo": { "hasNextPage": true }
                    }
                }
            }
        });
        let result = parse_list_response(&body, 25).unwrap();
        assert!(result.message.is_some());
        assert!(result.message.unwrap().contains("truncated"));
    }

    #[test]
    fn parse_get_response_extracts_issue() {
        let body = serde_json::json!({
            "data": {
                "issue": {
                    "id": "uuid-1",
                    "identifier": "PROJ-1",
                    "title": "My issue",
                    "state": { "name": "Todo" },
                    "priority": 3.0,
                    "description": null,
                    "url": "https://linear.app/proj/issue/PROJ-1"
                }
            }
        });
        let issue = parse_get_response(&body).unwrap();
        assert_eq!(issue.identifier, "PROJ-1");
        assert_eq!(issue.title, "My issue");
    }

    #[test]
    fn parse_get_response_null_issue() {
        let body = serde_json::json!({
            "data": { "issue": null }
        });
        let err = parse_get_response(&body).unwrap_err();
        assert!(err.contains("not found"));
    }

    #[test]
    fn parse_create_response_success() {
        let body = serde_json::json!({
            "data": {
                "issueCreate": {
                    "success": true,
                    "issue": {
                        "id": "uuid-new",
                        "identifier": "PROJ-99",
                        "title": "New issue",
                        "state": { "name": "Backlog" },
                        "priority": 1.0,
                        "description": null,
                        "url": "https://linear.app/proj/issue/PROJ-99"
                    }
                }
            }
        });
        let issue = parse_create_response(&body).unwrap();
        assert_eq!(issue.identifier, "PROJ-99");
    }

    #[test]
    fn parse_create_response_failure() {
        let body = serde_json::json!({
            "data": {
                "issueCreate": {
                    "success": false,
                    "issue": null
                }
            }
        });
        let err = parse_create_response(&body).unwrap_err();
        assert!(err.contains("failed"));
    }

    #[test]
    fn parse_close_response_success() {
        let body = serde_json::json!({
            "data": {
                "issueUpdate": {
                    "success": true,
                    "issue": {
                        "id": "uuid-1",
                        "identifier": "PROJ-1",
                        "title": "Closed issue",
                        "state": { "name": "Done" },
                        "priority": 2.0,
                        "description": null,
                        "url": "https://linear.app/proj/issue/PROJ-1"
                    }
                }
            }
        });
        let issue = parse_close_response(&body).unwrap();
        assert_eq!(issue.state.unwrap().name, "Done");
    }

    #[test]
    fn parse_close_response_failure() {
        let body = serde_json::json!({
            "data": {
                "issueUpdate": {
                    "success": false,
                    "issue": null
                }
            }
        });
        let err = parse_close_response(&body).unwrap_err();
        assert!(err.contains("failed"));
    }

    #[test]
    fn parse_done_state_id_finds_completed_state() {
        let body = serde_json::json!({
            "data": {
                "issue": {
                    "id": "issue-uuid",
                    "team": {
                        "states": {
                            "nodes": [
                                { "id": "state-1", "name": "Backlog", "type": "backlog" },
                                { "id": "state-2", "name": "In Progress", "type": "started" },
                                { "id": "state-3", "name": "Done", "type": "completed" },
                                { "id": "state-4", "name": "Cancelled", "type": "canceled" }
                            ]
                        }
                    }
                }
            }
        });
        let (issue_id, done_state_id) = parse_done_state_id(&body).unwrap();
        assert_eq!(issue_id, "issue-uuid");
        assert_eq!(done_state_id, "state-3");
    }

    #[test]
    fn parse_done_state_id_no_completed_state() {
        let body = serde_json::json!({
            "data": {
                "issue": {
                    "id": "issue-uuid",
                    "team": {
                        "states": {
                            "nodes": [
                                { "id": "state-1", "name": "Backlog", "type": "backlog" }
                            ]
                        }
                    }
                }
            }
        });
        let err = parse_done_state_id(&body).unwrap_err();
        assert!(err.contains("Done"));
    }

    #[test]
    fn parse_done_state_id_issue_not_found() {
        let body = serde_json::json!({
            "data": { "issue": null }
        });
        let err = parse_done_state_id(&body).unwrap_err();
        assert!(err.contains("not found"));
    }

    #[test]
    fn build_issue_team_query_sets_variable() {
        let req = build_issue_team_query("PROJ-1");
        assert!(req.query.contains("states"));
        let vars = req.variables.unwrap();
        assert_eq!(vars["id"], "PROJ-1");
    }
}
