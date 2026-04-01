//! Credential retrieval from 1Password via the `op` CLI.
//!
//! Calls `op item get <op_item_id> --field credential` to retrieve the
//! Discourse API key at call time. No caching -- each invocation calls `op`.

use std::process::Command;

/// Errors that can occur when retrieving credentials.
#[derive(Debug)]
pub enum CredentialError {
    /// The `op` binary was not found on PATH.
    OpNotFound,
    /// The `op` command failed (non-zero exit).
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

/// Retrieve an API key from 1Password using the given item ID.
pub fn get_api_key(op_item_id: &str) -> Result<String, CredentialError> {
    let output = Command::new("op")
        .args(["item", "get", op_item_id, "--field", "credential"])
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
    Ok(key)
}
