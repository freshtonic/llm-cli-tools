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
    #[serde(default)]
    pub assignee: Option<Assignee>,
    #[serde(default)]
    pub team: Option<IssueTeam>,
    #[serde(default)]
    pub labels: Option<LabelsConnection>,
    #[serde(default, alias = "createdAt")]
    pub created_at: Option<String>,
    #[serde(default, alias = "updatedAt")]
    pub updated_at: Option<String>,
}

/// The state of an issue (e.g., "In Progress", "Done").
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueState {
    pub name: String,
}

/// The assignee of an issue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Assignee {
    pub name: String,
    #[serde(default)]
    pub email: Option<String>,
}

/// The team an issue belongs to.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueTeam {
    pub key: String,
    pub name: String,
}

/// A label on an issue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueLabel {
    pub name: String,
}

/// A connection of labels on an issue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LabelsConnection {
    pub nodes: Vec<IssueLabel>,
}

/// Result of listing issues, including truncation info.
#[derive(Debug, Serialize)]
pub struct IssueListResult {
    pub issues: Vec<Issue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip)]
    pub has_more: bool,
    #[serde(skip)]
    pub next_cursor: Option<String>,
}

/// A GraphQL request body.
#[derive(Debug, Serialize)]
pub struct GraphqlRequest {
    pub query: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variables: Option<Value>,
}

/// Filters for the issues list query.
#[derive(Debug, Default)]
pub struct IssueFilters {
    pub assignee_id: Option<String>,
    pub team_key: Option<String>,
    pub state_name: Option<String>,
    pub priority: Option<u8>,
    pub label_name: Option<String>,
}

/// Build the GraphQL query for listing issues with optional filters and cursor.
pub fn build_list_query(limit: u32, filters: &IssueFilters, after: Option<&str>) -> GraphqlRequest {
    let query = r#"query($first: Int!, $filter: IssueFilter, $after: String) {
  issues(first: $first, filter: $filter, after: $after) {
    nodes {
      id
      identifier
      title
      state { name }
      priority
      description
      url
      assignee { name email }
      team { key name }
      labels { nodes { name } }
      createdAt
      updatedAt
    }
    pageInfo {
      hasNextPage
      endCursor
    }
  }
}"#
    .to_string();

    let mut filter = serde_json::Map::new();
    if let Some(ref id) = filters.assignee_id {
        filter.insert(
            "assignee".to_string(),
            serde_json::json!({ "id": { "eq": id } }),
        );
    }
    if let Some(ref key) = filters.team_key {
        filter.insert(
            "team".to_string(),
            serde_json::json!({ "key": { "eq": key } }),
        );
    }
    if let Some(ref name) = filters.state_name {
        filter.insert(
            "state".to_string(),
            serde_json::json!({ "name": { "eq": name } }),
        );
    }
    if let Some(priority) = filters.priority {
        filter.insert(
            "priority".to_string(),
            serde_json::json!({ "eq": priority }),
        );
    }
    if let Some(ref label_name) = filters.label_name {
        filter.insert(
            "labels".to_string(),
            serde_json::json!({ "some": { "name": { "eq": label_name } } }),
        );
    }

    let variables = serde_json::json!({
        "first": limit,
        "filter": if filter.is_empty() { Value::Null } else { Value::Object(filter) },
        "after": after,
    });

    GraphqlRequest {
        query,
        variables: Some(variables),
    }
}

/// Build a query to get the authenticated user's ID (for --mine filter).
pub fn build_viewer_id_query() -> GraphqlRequest {
    GraphqlRequest {
        query: "query { viewer { id } }".to_string(),
        variables: None,
    }
}

/// Parse the viewer ID from the response.
pub fn parse_viewer_id(body: &Value) -> Result<String, String> {
    body.pointer("/data/viewer/id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "Failed to get viewer ID".to_string())
}

/// Parse the response from a list issues query.
pub fn parse_list_response(body: &Value, limit: u32) -> Result<IssueListResult, String> {
    let nodes = body
        .pointer("/data/issues/nodes")
        .and_then(|v| v.as_array())
        .ok_or("Unexpected response: missing issues.nodes")?;

    let issues: Vec<Issue> = nodes
        .iter()
        .map(|n| serde_json::from_value(n.clone()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("Failed to parse issue: {e}"))?;

    let has_next_page = body
        .pointer("/data/issues/pageInfo/hasNextPage")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let end_cursor = body
        .pointer("/data/issues/pageInfo/endCursor")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let message = if has_next_page {
        Some(format!(
            "Results truncated to {limit}. Use --limit to fetch more, or narrow your query."
        ))
    } else {
        None
    };

    Ok(IssueListResult {
        issues,
        message,
        has_more: has_next_page,
        next_cursor: if has_next_page { end_cursor } else { None },
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
    assignee { name email }
    team { key name }
    labels { nodes { name } }
    createdAt
    updatedAt
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
      assignee { name email }
      team { key name }
      labels { nodes { name } }
      createdAt
      updatedAt
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
      assignee { name email }
      team { key name }
      labels { nodes { name } }
      createdAt
      updatedAt
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

/// Whether an HTTP status code is retryable (429 or 5xx).
fn is_retryable_status(status: u16) -> bool {
    status == 429 || status >= 500
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
            parsed.as_object_mut().unwrap().remove("query");
        }

        let mut out = String::new();

        if let Some(q) = &query {
            out.push_str("--- GraphQL Query ---\n");
            out.push_str(q.trim());
            out.push_str("\n---------------------\n");
        }

        // Print remaining fields (variables, etc.) if any exist.
        if let Some(obj) = parsed.as_object()
            && !obj.is_empty()
        {
            out.push_str(
                &serde_json::to_string_pretty(&parsed).unwrap_or_else(|_| body.to_string()),
            );
        }

        if out.is_empty() {
            body.to_string()
        } else {
            out
        }
    } else {
        body.to_string()
    }
}

/// Send a GraphQL request to the Linear API.
pub fn execute(
    api_url: &str,
    api_key: &str,
    request: &GraphqlRequest,
    debug: Option<&crate::cli::DebugConfig>,
) -> Result<Value, String> {
    let url = format!("{api_url}/graphql");
    let body = serde_json::to_string(request).map_err(|e| format!("Serialization error: {e}"))?;
    let pretty = debug.is_some_and(|d| d.pretty);
    let curl = debug.is_some_and(|d| d.curl);
    let dangerous_no_redact = debug.is_some_and(|d| d.dangerous_no_redact);

    if debug.is_some() {
        let auth_display = if dangerous_no_redact {
            api_key.to_string()
        } else {
            "<redacted>".to_string()
        };
        eprintln!(">>> POST {url}");
        eprintln!(">>> Authorization: {auth_display}");
        eprintln!(">>> Content-Type: application/json");
        eprintln!(">>> ");
        eprintln!(">>> {}", format_debug_body(&body, pretty));
        if curl {
            let curl_auth = if dangerous_no_redact {
                api_key.to_string()
            } else {
                "<redacted>".to_string()
            };
            eprintln!(">>> ");
            eprintln!(">>> curl -X POST '{url}' \\");
            eprintln!(">>>   -H 'Authorization: {curl_auth}' \\");
            eprintln!(">>>   -H 'Content-Type: application/json' \\");
            eprintln!(">>>   -d '{body}'");
        }
        eprintln!();
    }

    let agent = ureq::Agent::new_with_config(
        ureq::config::Config::builder()
            .http_status_as_error(false)
            .build(),
    );

    // Attempt the request, retrying once on transient errors.
    let mut attempt = 0;
    let (status, response_text) = loop {
        attempt += 1;
        let result = agent
            .post(&url)
            .header("Authorization", &api_key.to_string())
            .header("Content-Type", "application/json")
            .send(&body);

        match result {
            Ok(mut resp) => {
                let st = resp.status();
                if debug.is_some() {
                    eprintln!("<<< {st}");
                    for (name, value) in resp.headers() {
                        eprintln!("<<<   {}: {}", name, value.to_str().unwrap_or("<binary>"));
                    }
                }
                let text = resp
                    .body_mut()
                    .read_to_string()
                    .map_err(|e| format!("Failed to read response: {e}"))?;
                if debug.is_some() {
                    eprintln!("<<<");
                    eprintln!("<<< {}", format_debug_body(&text, pretty));
                    eprintln!();
                }

                if attempt == 1 && is_retryable_status(st.as_u16()) {
                    if debug.is_some() {
                        eprintln!(">>> Retrying after 1s (HTTP {st})...");
                        eprintln!();
                    }
                    std::thread::sleep(std::time::Duration::from_secs(1));
                    continue;
                }
                break (st, text);
            }
            Err(e) => {
                if debug.is_some() {
                    eprintln!("<<< ERROR: {e}");
                    eprintln!();
                }
                if attempt == 1 {
                    if debug.is_some() {
                        eprintln!(">>> Retrying after 1s (network error)...");
                        eprintln!();
                    }
                    std::thread::sleep(std::time::Duration::from_secs(1));
                    continue;
                }
                return Err(format!("HTTP request failed: {e}"));
            }
        }
    };

    // Check HTTP status after logging.
    if status.as_u16() >= 400 {
        return Err(format!(
            "HTTP {status}: {}",
            response_text.chars().take(500).collect::<String>()
        ));
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
    fn build_list_query_no_filters() {
        let req = build_list_query(10, &IssueFilters::default(), None);
        assert!(req.query.contains("issues(first: $first"));
        let vars = req.variables.unwrap();
        assert_eq!(vars["first"], 10);
        assert!(vars["filter"].is_null());
    }

    #[test]
    fn build_list_query_with_filters() {
        let filters = IssueFilters {
            assignee_id: Some("user-1".to_string()),
            team_key: Some("ENG".to_string()),
            state_name: Some("In Progress".to_string()),
            ..Default::default()
        };
        let req = build_list_query(5, &filters, None);
        let vars = req.variables.unwrap();
        assert_eq!(vars["first"], 5);
        assert_eq!(vars["filter"]["assignee"]["id"]["eq"], "user-1");
        assert_eq!(vars["filter"]["team"]["key"]["eq"], "ENG");
        assert_eq!(vars["filter"]["state"]["name"]["eq"], "In Progress");
    }

    #[test]
    fn build_list_query_with_priority_filter() {
        let filters = IssueFilters {
            priority: Some(2),
            ..Default::default()
        };
        let req = build_list_query(10, &filters, None);
        let vars = req.variables.unwrap();
        assert_eq!(vars["filter"]["priority"]["eq"], 2);
    }

    #[test]
    fn build_list_query_with_label_filter() {
        let filters = IssueFilters {
            label_name: Some("bug".to_string()),
            ..Default::default()
        };
        let req = build_list_query(10, &filters, None);
        let vars = req.variables.unwrap();
        assert_eq!(vars["filter"]["labels"]["some"]["name"]["eq"], "bug");
    }

    #[test]
    fn build_list_query_with_all_filters() {
        let filters = IssueFilters {
            assignee_id: Some("user-1".to_string()),
            team_key: Some("ENG".to_string()),
            state_name: Some("In Progress".to_string()),
            priority: Some(1),
            label_name: Some("urgent".to_string()),
        };
        let req = build_list_query(5, &filters, None);
        let vars = req.variables.unwrap();
        assert_eq!(vars["filter"]["assignee"]["id"]["eq"], "user-1");
        assert_eq!(vars["filter"]["team"]["key"]["eq"], "ENG");
        assert_eq!(vars["filter"]["state"]["name"]["eq"], "In Progress");
        assert_eq!(vars["filter"]["priority"]["eq"], 1);
        assert_eq!(vars["filter"]["labels"]["some"]["name"]["eq"], "urgent");
    }

    #[test]
    fn build_viewer_id_query_structure() {
        let req = build_viewer_id_query();
        assert!(req.query.contains("viewer"));
        assert!(req.query.contains("id"));
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
                "issues": {
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
                "issues": {
                    "nodes": [],
                    "pageInfo": { "hasNextPage": true }
                }
            }
        });
        let result = parse_list_response(&body, 25).unwrap();
        assert!(result.message.is_some());
        assert!(result.message.unwrap().contains("truncated"));
    }

    #[test]
    fn parse_viewer_id_extracts_id() {
        let body = serde_json::json!({
            "data": { "viewer": { "id": "user-123" } }
        });
        assert_eq!(parse_viewer_id(&body).unwrap(), "user-123");
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

    // ---- Pagination tests ----

    #[test]
    fn build_list_query_with_cursor() {
        let req = build_list_query(10, &IssueFilters::default(), Some("cursor123"));
        assert!(req.query.contains("after: $after"));
        assert!(req.query.contains("$after: String"));
        let vars = req.variables.unwrap();
        assert_eq!(vars["after"], "cursor123");
    }

    #[test]
    fn build_list_query_without_cursor() {
        let req = build_list_query(10, &IssueFilters::default(), None);
        let vars = req.variables.unwrap();
        assert!(vars["after"].is_null());
    }

    #[test]
    fn build_list_query_includes_end_cursor_in_page_info() {
        let req = build_list_query(10, &IssueFilters::default(), None);
        assert!(req.query.contains("endCursor"));
    }

    #[test]
    fn parse_list_response_returns_pagination_fields() {
        let body = serde_json::json!({
            "data": {
                "issues": {
                    "nodes": [],
                    "pageInfo": {
                        "hasNextPage": true,
                        "endCursor": "cursor-abc"
                    }
                }
            }
        });
        let result = parse_list_response(&body, 25).unwrap();
        assert!(result.has_more);
        assert_eq!(result.next_cursor.as_deref(), Some("cursor-abc"));
    }

    #[test]
    fn parse_list_response_no_more_pages() {
        let body = serde_json::json!({
            "data": {
                "issues": {
                    "nodes": [],
                    "pageInfo": {
                        "hasNextPage": false
                    }
                }
            }
        });
        let result = parse_list_response(&body, 25).unwrap();
        assert!(!result.has_more);
        assert!(result.next_cursor.is_none());
    }

    #[test]
    fn parse_list_response_has_more_skips_data_serialization() {
        // has_more and next_cursor should not appear in serialized data
        let body = serde_json::json!({
            "data": {
                "issues": {
                    "nodes": [],
                    "pageInfo": {
                        "hasNextPage": true,
                        "endCursor": "cursor-abc"
                    }
                }
            }
        });
        let result = parse_list_response(&body, 25).unwrap();
        let serialized = serde_json::to_value(&result).unwrap();
        assert!(serialized.get("has_more").is_none());
        assert!(serialized.get("next_cursor").is_none());
    }

    // ---- New field tests ----

    #[test]
    fn parse_issue_with_all_new_fields() {
        let body = serde_json::json!({
            "data": {
                "issue": {
                    "id": "uuid-1",
                    "identifier": "PROJ-1",
                    "title": "Full issue",
                    "state": { "name": "In Progress" },
                    "priority": 2.0,
                    "description": "Desc",
                    "url": "https://linear.app/proj/issue/PROJ-1",
                    "assignee": { "name": "Alice", "email": "alice@example.com" },
                    "team": { "key": "ENG", "name": "Engineering" },
                    "labels": { "nodes": [{ "name": "bug" }, { "name": "urgent" }] },
                    "createdAt": "2026-01-01T00:00:00Z",
                    "updatedAt": "2026-01-02T00:00:00Z"
                }
            }
        });
        let issue = parse_get_response(&body).unwrap();
        let assignee = issue.assignee.unwrap();
        assert_eq!(assignee.name, "Alice");
        assert_eq!(assignee.email.as_deref(), Some("alice@example.com"));
        let team = issue.team.unwrap();
        assert_eq!(team.key, "ENG");
        assert_eq!(team.name, "Engineering");
        let labels = issue.labels.unwrap();
        assert_eq!(labels.nodes.len(), 2);
        assert_eq!(labels.nodes[0].name, "bug");
        assert_eq!(labels.nodes[1].name, "urgent");
        assert_eq!(issue.created_at.as_deref(), Some("2026-01-01T00:00:00Z"));
        assert_eq!(issue.updated_at.as_deref(), Some("2026-01-02T00:00:00Z"));
    }

    #[test]
    fn parse_issue_missing_new_fields_defaults_to_none() {
        let body = serde_json::json!({
            "data": {
                "issue": {
                    "id": "uuid-1",
                    "identifier": "PROJ-1",
                    "title": "Minimal issue",
                    "state": null,
                    "priority": null,
                    "description": null,
                    "url": "https://linear.app/proj/issue/PROJ-1"
                }
            }
        });
        let issue = parse_get_response(&body).unwrap();
        assert!(issue.assignee.is_none());
        assert!(issue.team.is_none());
        assert!(issue.labels.is_none());
        assert!(issue.created_at.is_none());
        assert!(issue.updated_at.is_none());
    }

    #[test]
    fn issue_created_at_serializes_as_snake_case() {
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
            updated_at: Some("2026-01-02T00:00:00Z".to_string()),
        };
        let json = serde_json::to_value(&issue).unwrap();
        // Serializes as snake_case (the Rust field name)
        assert_eq!(json["created_at"], "2026-01-01T00:00:00Z");
        assert_eq!(json["updated_at"], "2026-01-02T00:00:00Z");
        // But also deserializes from camelCase (the API alias)
        let camel_json = serde_json::json!({
            "id": "uuid-1",
            "identifier": "PROJ-1",
            "title": "Test",
            "url": "https://example.com",
            "createdAt": "2026-03-01T00:00:00Z",
            "updatedAt": "2026-03-02T00:00:00Z"
        });
        let from_camel: Issue = serde_json::from_value(camel_json).unwrap();
        assert_eq!(
            from_camel.created_at.as_deref(),
            Some("2026-03-01T00:00:00Z")
        );
        assert_eq!(
            from_camel.updated_at.as_deref(),
            Some("2026-03-02T00:00:00Z")
        );
    }

    #[test]
    fn build_list_query_includes_new_fields() {
        let req = build_list_query(10, &IssueFilters::default(), None);
        assert!(req.query.contains("assignee { name email }"));
        assert!(req.query.contains("team { key name }"));
        assert!(req.query.contains("labels { nodes { name } }"));
        assert!(req.query.contains("createdAt"));
        assert!(req.query.contains("updatedAt"));
    }

    #[test]
    fn build_get_query_includes_new_fields() {
        let req = build_get_query("PROJ-1");
        assert!(req.query.contains("assignee { name email }"));
        assert!(req.query.contains("team { key name }"));
        assert!(req.query.contains("labels { nodes { name } }"));
        assert!(req.query.contains("createdAt"));
        assert!(req.query.contains("updatedAt"));
    }

    #[test]
    fn build_create_mutation_includes_new_fields() {
        let req = build_create_mutation("Test", "team-uuid", None, None);
        assert!(req.query.contains("assignee { name email }"));
        assert!(req.query.contains("team { key name }"));
        assert!(req.query.contains("labels { nodes { name } }"));
        assert!(req.query.contains("createdAt"));
        assert!(req.query.contains("updatedAt"));
    }

    #[test]
    fn build_close_mutation_includes_new_fields() {
        let req = build_close_mutation("issue-uuid", "done-state-uuid");
        assert!(req.query.contains("assignee { name email }"));
        assert!(req.query.contains("team { key name }"));
        assert!(req.query.contains("labels { nodes { name } }"));
        assert!(req.query.contains("createdAt"));
        assert!(req.query.contains("updatedAt"));
    }

    // ---- Retry helper tests ----

    #[test]
    fn is_retryable_status_429() {
        assert!(is_retryable_status(429));
    }

    #[test]
    fn is_retryable_status_500() {
        assert!(is_retryable_status(500));
    }

    #[test]
    fn is_retryable_status_502() {
        assert!(is_retryable_status(502));
    }

    #[test]
    fn is_retryable_status_503() {
        assert!(is_retryable_status(503));
    }

    #[test]
    fn is_retryable_status_200_not_retryable() {
        assert!(!is_retryable_status(200));
    }

    #[test]
    fn is_retryable_status_400_not_retryable() {
        assert!(!is_retryable_status(400));
    }

    #[test]
    fn is_retryable_status_404_not_retryable() {
        assert!(!is_retryable_status(404));
    }

    #[test]
    fn is_retryable_status_499_not_retryable() {
        assert!(!is_retryable_status(499));
    }
}
