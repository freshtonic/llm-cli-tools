//! Credential retrieval from 1Password via the `op` CLI.
//!
//! Calls `op item get <op_item_id> --field credential --reveal` to retrieve
//! the Slack API token at call time.

use std::process::Command;

#[derive(Debug)]
pub enum CredentialError {
    OpNotFound,
    OpFailed(String),
}

impl std::fmt::Display for CredentialError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CredentialError::OpNotFound => write!(f, "1Password CLI (op) not found on PATH"),
            CredentialError::OpFailed(msg) => write!(f, "1Password CLI failed: {msg}"),
        }
    }
}

pub fn get_api_key(op_item_id: &str, field: &str) -> Result<String, CredentialError> {
    let output = Command::new("op")
        .args(["item", "get", op_item_id, "--field", field, "--reveal"])
        .output()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                CredentialError::OpNotFound
            } else {
                CredentialError::OpFailed(e.to_string())
            }
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CredentialError::OpFailed(stderr.trim().to_string()));
    }

    let key = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if key.is_empty() {
        return Err(CredentialError::OpFailed(format!(
            "field '{field}' in item '{op_item_id}' is empty"
        )));
    }

    Ok(key)
}
