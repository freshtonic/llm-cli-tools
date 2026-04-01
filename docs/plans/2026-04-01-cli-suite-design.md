# CLI Suite Design

## Architecture

Four crates in a Cargo workspace, plus a thin dispatcher:

- **`llm-cli`** — git-style dispatcher that execs `llm-cli-<subcommand>`
- **`llm-cli-linear`** — Linear API client (GraphQL)
- **`llm-cli-discourse`** — Discourse API client (REST)
- **`llm-cli-slack`** — Slack API client (REST)

All crates live under `packages/`.

## Output

JSON to stdout by default. `--human` flag on each tool for readable output. Diagnostics to stderr.

Success responses:

```json
{
  "success": true,
  "data": { ... }
}
```

Error responses:

```json
{
  "success": false,
  "error": {
    "code": "OP_NOT_FOUND",
    "message": "Could not retrieve API key from 1Password. Is the 1Password app running?",
    "suggestion": "Ensure the 1Password desktop app is running and unlocked"
  }
}
```

With `--human`, errors print as plain text to stderr, data as formatted text to stdout.

## Configuration

Path: `$XDG_CONFIG_HOME/llm-cli/config.toml` (defaults to `~/.config/llm-cli/config.toml`).

```toml
[linear]
api_url = "https://api.linear.app"  # optional, has default
op_item_id = "abc123-some-uuid"
op_field = "credential"             # optional, defaults to "credential"

[discourse.my-forum]
base_url = "https://forum.example.com"
op_item_id = "def456-some-uuid"
op_field = "credential"             # optional, defaults to "credential"
api_username = "james"

[slack]
op_item_id = "ghi789-some-uuid"
op_field = "credential"             # optional, defaults to "credential"
```

## Authentication

All tools retrieve API keys from 1Password at call time via `op item get <op_item_id> --field <op_field> --reveal`. The item ID and field name come from config. No caching — each invocation calls `op` directly. Empty credentials are detected and reported as errors.

## Debugging

All API crates support `--debug` with comma-separated modes:

- `--debug` — compact request/response logging to stderr (auth redacted)
- `--debug=pretty` — pretty-printed JSON bodies; GraphQL queries printed as readable text blocks
- `--debug=curl_cmd` — prints reproducible curl commands with unredacted secrets (prompts for confirmation)
- `--debug=pretty,curl_cmd` — both

HTTP error responses (4xx/5xx) are logged with full status, headers, and body before being reported as errors.

## `llm-cli` Dispatcher

Minimal binary with no dependencies beyond std.

- `llm-cli linear <args...>` — finds `llm-cli-linear` on `$PATH`, execs it with `<args...>`
- `llm-cli --help` or no args — lists available subcommands by scanning `$PATH` for `llm-cli-*` binaries
- Unknown subcommand — stderr error with available subcommands, exit 1

Uses `std::os::unix::process::CommandExt::exec` to replace the process (no child process management).

No flags of its own beyond `--help`. No config. No auth.

## `llm-cli-linear`

### Subcommands

- `issues list` — issues assigned to the authenticated user. Default 25 results, `--limit` to adjust. Truncated results include a message steering toward narrower filters.
- `issues get --id <issue-id>` — fetch a single issue
- `issues create --title <title> --team <team> [--description <desc>] [--priority <1-4>]` — create an issue
- `issues close --id <issue-id>` — close an issue (sets state to "Done")

### API

GraphQL at `{api_url}/graphql`. Auth header: `Authorization: <api_key>` (no Bearer prefix).

### Config

```toml
[linear]
api_url = "https://api.linear.app"  # optional, has default
op_item_id = "abc123-some-uuid"
op_field = "credential"             # optional, defaults to "credential"
```

## `llm-cli-discourse`

### Subcommands

- `posts latest` — list the latest posts across all topics
- `posts get --id <post-id>` — fetch a single post/topic
- `posts create --title <title> --category <category> [--raw <body>]` — create a new topic
- `posts delete --id <post-id>` — delete a topic
- `comments create --post-id <post-id> --raw <body>` — reply to a topic
- `comments delete --id <comment-id>` — delete a comment/reply

### Common flags

- `--human` — human-readable output
- `--instance <name>` — which Discourse instance to use (maps to `[discourse.<name>]` in config). Required when multiple instances configured; if only one exists, used automatically.

### API

REST/JSON. Auth via `Api-Key` and `Api-Username` headers. Category lookup by name is supported (case-insensitive).

### Config

```toml
[discourse.my-forum]
base_url = "https://forum.example.com"
op_item_id = "def456-some-uuid"
op_field = "credential"             # optional, defaults to "credential"
api_username = "james"
```

## `llm-cli-slack`

### Subcommands

- `messages send --channel <ch> --text <t> [--thread-ts <ts>]` — send to channel or thread
- `messages read --channel <ch> [--limit <n>]` — read recent channel history
- `messages dm --user <u> --text <t>` — send a direct message (opens DM channel automatically)
- `messages mentions [--limit <n>]` — search messages mentioning the authenticated user
- `summary --channel <ch> [--oldest <date>] [--latest <date>]` — Slack AI channel summary (defaults to yesterday + today)

### API

REST at `https://slack.com/api/*`. Auth via `Authorization: Bearer <token>` header. Uses `search.messages` for mentions and `conversations.requestSummarize` for AI summaries.

### Config

```toml
[slack]
op_item_id = "ghi789-some-uuid"
op_field = "credential"             # optional, defaults to "credential"
```

## Error Handling

Common error scenarios across all tools:

- `op` not found or not running — clear error with setup instructions
- API key retrieval failed — include the item ID in the error message
- API key field empty — explicit error naming the field and item
- Config file missing or malformed — error with expected config path and example
- API request failed — pass through HTTP status and response body

## Dependencies

**`llm-cli`:** no dependencies (std only)

**`llm-cli-linear`, `llm-cli-discourse`, `llm-cli-slack`:**

- `clap` — argument parsing (derive)
- `serde` / `serde_json` — JSON serialization
- `toml` — config parsing
- `ureq` — HTTP client (blocking, no async runtime)

No async runtime. Short-lived CLI invocations making one or two HTTP calls.
