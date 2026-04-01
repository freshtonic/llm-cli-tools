#!/usr/bin/env bash
set -euo pipefail

# Install all binary crates in the workspace.
# Discovers crates by looking for Cargo.toml files under packages/.

repo_root="$(cd "$(dirname "$0")" && pwd)"

for manifest in "$repo_root"/packages/*/Cargo.toml; do
    crate_dir="$(dirname "$manifest")"
    # Only install if the crate has a src/main.rs (binary crate).
    if [[ -f "$crate_dir/src/main.rs" ]]; then
        echo "Installing $(basename "$crate_dir")..."
        cargo install --path "$crate_dir"
    fi
done
