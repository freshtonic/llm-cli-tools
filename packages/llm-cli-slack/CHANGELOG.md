# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]
## [0.1.0] - 2026-04-02

### Bug Fixes

- Skip interactive curl_cmd prompt when stdin is not a TTY
- Show HTTP responses in --debug mode by disabling ureq auto-errors
- Detect empty 1Password credentials and add configurable op_field
- Correct --debug flag argument parsing and add error response logging

### Features

- Add schema subcommand, --input flag, and CLI-level filtering
- Add retry logic and additional API-level filtering
- Differentiate exit codes by error category
- Add cursor-based pagination to all list commands
- Add high-signal output fields and route JSON errors to stdout
- Add shell completions and interactive init wizard
- Add --version/-V flag to dispatcher and all sub-crates
- Add automatic pager support for --human output
- Format --human output as markdown across all crates
- Add curl_cmd debug mode with comma-separated flag support
- Enhance --debug flag with optional pretty-print mode
- Add llm-cli-slack crate for Slack API interaction
