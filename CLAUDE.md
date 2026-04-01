# CLAUDE.md

## Project overview

A Cargo workspace of agent-first CLI tools for interacting with SaaS APIs (Linear, Discourse, Slack). Each tool is a standalone binary crate under `packages/`. A dispatcher (`llm-cli`) uses git-style subcommand dispatch to exec the others.

## Build and test

```sh
cargo test --workspace    # Run all tests (131 tests across 4 crates)
cargo build --workspace   # Build all crates
./install.sh              # Install all binaries to ~/.cargo/bin
```

## Architecture

- **`packages/llm-cli/`** — Thin dispatcher. No dependencies beyond std. Scans `$PATH` for `llm-cli-*` binaries.
- **`packages/llm-cli-linear/`** — Linear GraphQL client. Auth: `Authorization: <api_key>` (no Bearer prefix).
- **`packages/llm-cli-discourse/`** — Discourse REST client. Auth: `Api-Key` + `Api-Username` headers. Supports multiple instances via `--instance`.
- **`packages/llm-cli-slack/`** — Slack REST client. Auth: `Authorization: Bearer <token>`.

Each API crate follows the same module structure:
- `cli.rs` — clap derive argument parsing
- `config.rs` — TOML config loading from `~/.config/llm-cli/config.toml`
- `credential.rs` — 1Password credential retrieval via `op` CLI
- `api.rs` — HTTP client, request construction, response parsing
- `output.rs` — JSON envelope formatting, human-readable formatting, error types
- `main.rs` — wiring

## Key conventions

- **Edition 2024**, Rust stable
- **JSON to stdout**, diagnostics/debug to stderr
- **`--human` flag** for human-readable output on all API crates
- **`--debug` flag** with comma-separated modes: `compact`, `pretty`, `curl_cmd`
- **`ureq` v3** with `http_status_as_error(false)` — we handle HTTP status ourselves to enable response logging
- **No async** — blocking HTTP, single-threaded, short-lived processes
- **No shared library crate** — duplication across crates is acceptable at this scale
- **`indoc`** crate for multi-line TOML strings in tests
- Config uses `$XDG_CONFIG_HOME` or `$HOME/.config`, not the `dirs` crate

## Design principles

See [PRINCIPLES.md](PRINCIPLES.md) for the full CLI design philosophy (agent-first output, structured errors, named flags, etc.).
