mod init;

use std::os::unix::process::CommandExt;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(args) {
        Ok(()) => {}
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
}

/// The result of parsing CLI arguments.
enum Action {
    /// Show help/usage (no subcommand given, or --help).
    Help,
    /// Show version.
    Version,
    /// Generate shell completions.
    Completions { shell: String },
    /// Run the interactive setup wizard.
    Init,
    /// Dispatch to a subcommand binary with the given remaining args.
    Dispatch {
        subcommand: String,
        args: Vec<String>,
    },
}

/// Parse raw CLI arguments into an action.
fn parse_args(args: Vec<String>) -> Action {
    match args.first().map(|s| s.as_str()) {
        None | Some("--help") | Some("-h") => Action::Help,
        Some("--version") | Some("-V") => Action::Version,
        Some("init") => Action::Init,
        Some("completions") => {
            let shell = args
                .get(1)
                .filter(|a| *a == "--shell")
                .and_then(|_| args.get(2))
                .cloned()
                .unwrap_or_default();
            Action::Completions { shell }
        }
        Some(_) => {
            let mut iter = args.into_iter();
            let subcommand = iter.next().unwrap();
            let args = iter.collect();
            Action::Dispatch { subcommand, args }
        }
    }
}

/// Scan `$PATH` for binaries matching `llm-cli-*` and return sorted subcommand names.
fn find_subcommand_binaries() -> Vec<String> {
    let prefix = "llm-cli-";
    let path_var = std::env::var("PATH").unwrap_or_default();
    let mut names: Vec<String> = std::env::split_paths(&path_var)
        .filter_map(|dir| std::fs::read_dir(dir).ok())
        .flatten()
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let file_name = entry.file_name().to_string_lossy().into_owned();
            file_name.strip_prefix(prefix).map(|s| s.to_string())
        })
        .collect();
    names.sort();
    names.dedup();
    names
}

/// Format help text listing available subcommands.
fn format_help(subcommands: &[String]) -> String {
    let mut out = String::from("usage: llm-cli <subcommand> [args...]\n\n");
    if subcommands.is_empty() {
        out.push_str("No subcommands found on PATH.\n");
    } else {
        out.push_str("Available subcommands:\n");
        for name in subcommands {
            out.push_str(&format!("  {name}\n"));
        }
    }
    out.push_str("\nBuilt-in commands:\n");
    out.push_str("  init           Generate config file interactively\n");
    out.push_str("  completions    Generate shell completions\n");
    out
}

/// Format an error message for an unknown subcommand.
fn format_not_found_error(subcommand: &str, available: &[String]) -> String {
    let mut out = format!("error: subcommand '{subcommand}' not found\n");
    if !available.is_empty() {
        out.push_str("\nAvailable subcommands:\n");
        for name in available {
            out.push_str(&format!("  {name}\n"));
        }
    }
    out
}

/// Run `llm-cli-<name> completions --shell <shell>` and return stdout, or None on failure.
fn get_subcommand_completions(name: &str, shell: &str) -> Option<String> {
    let binary = format!("llm-cli-{name}");
    let output = std::process::Command::new(&binary)
        .args(["completions", "--shell", shell])
        .output()
        .ok()?;
    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        None
    }
}

/// Collect completions from all installed subcommand binaries.
fn collect_subcommand_completions(subcommands: &[String], shell: &str) -> String {
    let mut out = String::new();
    for name in subcommands {
        if let Some(completions) = get_subcommand_completions(name, shell) {
            out.push_str(&completions);
            out.push('\n');
        }
    }
    out
}

/// Generate bash completions for the dispatcher and all subcommands.
fn generate_bash_completions(subcommands: &[String]) -> String {
    let mut out = collect_subcommand_completions(subcommands, "bash");

    let subcmds = subcommands.join(" ");
    out.push_str(&format!(
        r#"_llm_cli() {{
    local cur prev
    cur="${{COMP_WORDS[COMP_CWORD]}}"
    prev="${{COMP_WORDS[COMP_CWORD-1]}}"

    if [[ $COMP_CWORD -eq 1 ]]; then
        COMPREPLY=($(compgen -W "{subcmds} init completions --help --version" -- "$cur"))
        return
    fi

    local subcmd="${{COMP_WORDS[1]}}"
    case "$subcmd" in
        completions)
            if [[ "$prev" == "--shell" ]]; then
                COMPREPLY=($(compgen -W "bash zsh fish" -- "$cur"))
            else
                COMPREPLY=($(compgen -W "--shell" -- "$cur"))
            fi
            return
            ;;
    esac

    # Delegate to subcommand's completion function by rewriting COMP_WORDS
    local func="_llm-cli-$subcmd"
    if declare -F "$func" &>/dev/null; then
        # Rewrite COMP_WORDS: replace "llm-cli <subcmd>" with "llm-cli-<subcmd>"
        local binary="llm-cli-$subcmd"
        COMP_WORDS=("$binary" "${{COMP_WORDS[@]:2}}")
        COMP_CWORD=$((COMP_CWORD - 1))
        COMP_LINE="${{COMP_LINE/#llm-cli $subcmd/$binary}}"
        COMP_POINT=$((COMP_POINT - ${{#subcmd}} - 1 + ${{#binary}}))
        "$func"
    fi
}}
complete -F _llm_cli llm-cli
"#
    ));
    out
}

/// Generate zsh completions for the dispatcher and all subcommands.
fn generate_zsh_completions(subcommands: &[String]) -> String {
    let mut out = collect_subcommand_completions(subcommands, "zsh");

    let mut subcmd_lines = String::new();
    for name in subcommands {
        subcmd_lines.push_str(&format!("        '{name}:llm-cli-{name} subcommand'\n"));
    }
    out.push_str(&format!(
        r#"#compdef llm-cli

_llm_cli() {{
    local -a subcommands
    subcommands=(
{subcmd_lines}        'init:Generate config file interactively'
        'completions:Generate shell completions'
    )

    if (( CURRENT == 2 )); then
        _describe 'subcommand' subcommands
        return
    fi

    local subcmd="${{words[2]}}"
    case "$subcmd" in
        completions)
            _arguments '--shell[Shell to generate completions for]:shell:(bash zsh fish)'
            return
            ;;
    esac

    # Delegate to subcommand's completion function
    local func="_llm-cli-$subcmd"
    if (( $+functions[$func] )); then
        words=("llm-cli-$subcmd" "${{words[@]:2}}")
        (( CURRENT-- ))
        _comps[llm-cli-$subcmd]="$func"
        "$func"
    fi
}}

_llm_cli "$@"
"#
    ));
    out
}

/// Generate fish completions for the dispatcher and all subcommands.
fn generate_fish_completions(subcommands: &[String]) -> String {
    let mut out = collect_subcommand_completions(subcommands, "fish");

    out.push_str("# Completions for the llm-cli dispatcher\n");
    out.push_str("complete -c llm-cli -f\n");
    for name in subcommands {
        out.push_str(&format!(
            "complete -c llm-cli -n '__fish_use_subcommand' -a '{name}' -d 'llm-cli-{name} subcommand'\n"
        ));
        // Wrap: when subcommand is selected, inherit completions from the sub-binary
        out.push_str(&format!(
            "complete -c llm-cli -n '__fish_seen_subcommand_from {name}' -w llm-cli-{name}\n"
        ));
    }
    out.push_str(
        "complete -c llm-cli -n '__fish_use_subcommand' -a 'init' -d 'Generate config file interactively'\n\
         complete -c llm-cli -n '__fish_use_subcommand' -a 'completions' -d 'Generate shell completions'\n\
         complete -c llm-cli -n '__fish_use_subcommand' -l help -d 'Show help'\n\
         complete -c llm-cli -n '__fish_use_subcommand' -l version -d 'Show version'\n\
         complete -c llm-cli -n '__fish_seen_subcommand_from completions' -l shell -d 'Shell to generate completions for' -ra 'bash zsh fish'\n",
    );
    out
}

/// Run the CLI dispatcher. Returns `Ok(())` only if `exec` replaces the process.
/// In practice, `exec` never returns on success, so `Ok` is only returned for help output.
fn run(args: Vec<String>) -> Result<(), String> {
    match parse_args(args) {
        Action::Help => {
            let subcommands = find_subcommand_binaries();
            print!("{}", format_help(&subcommands));
            Ok(())
        }
        Action::Version => {
            println!("llm-cli {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Action::Init => {
            let subcommands = find_subcommand_binaries();
            init::run(&subcommands)?;
            Ok(())
        }
        Action::Completions { shell } => {
            let subcommands = find_subcommand_binaries();
            let output = match shell.as_str() {
                "bash" => generate_bash_completions(&subcommands),
                "zsh" => generate_zsh_completions(&subcommands),
                "fish" => generate_fish_completions(&subcommands),
                "" => {
                    return Err(
                        "error: missing --shell argument\n\nUsage: llm-cli completions --shell <bash|zsh|fish>"
                            .to_string(),
                    );
                }
                other => {
                    return Err(format!(
                        "error: unsupported shell '{other}'\n\nSupported shells: bash, zsh, fish"
                    ));
                }
            };
            print!("{output}");
            Ok(())
        }
        Action::Dispatch { subcommand, args } => {
            let binary = format!("llm-cli-{subcommand}");
            let err = std::process::Command::new(&binary).args(&args).exec();
            // exec() only returns on error
            if err.kind() == std::io::ErrorKind::NotFound {
                let available = find_subcommand_binaries();
                Err(format_not_found_error(&subcommand, &available))
            } else {
                Err(format!("error: failed to exec '{binary}': {err}"))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- parse_args tests ----

    #[test]
    fn parse_args_no_args_returns_help() {
        let action = parse_args(vec![]);
        assert!(matches!(action, Action::Help));
    }

    #[test]
    fn parse_args_help_flag_returns_help() {
        let action = parse_args(vec!["--help".to_string()]);
        assert!(matches!(action, Action::Help));
    }

    #[test]
    fn parse_args_h_flag_returns_help() {
        let action = parse_args(vec!["-h".to_string()]);
        assert!(matches!(action, Action::Help));
    }

    #[test]
    fn parse_args_version_flag_returns_version() {
        let action = parse_args(vec!["--version".to_string()]);
        assert!(matches!(action, Action::Version));
    }

    #[test]
    fn parse_args_v_flag_returns_version() {
        let action = parse_args(vec!["-V".to_string()]);
        assert!(matches!(action, Action::Version));
    }

    #[test]
    fn parse_args_subcommand_with_no_args() {
        let action = parse_args(vec!["linear".to_string()]);
        match action {
            Action::Dispatch { subcommand, args } => {
                assert_eq!(subcommand, "linear");
                assert!(args.is_empty());
            }
            _ => panic!("Expected Dispatch"),
        }
    }

    #[test]
    fn parse_args_subcommand_with_remaining_args() {
        let action = parse_args(vec![
            "linear".to_string(),
            "issues".to_string(),
            "list".to_string(),
            "--status".to_string(),
            "open".to_string(),
        ]);
        match action {
            Action::Dispatch { subcommand, args } => {
                assert_eq!(subcommand, "linear");
                assert_eq!(args, vec!["issues", "list", "--status", "open"]);
            }
            _ => panic!("Expected Dispatch"),
        }
    }

    // ---- parse_args init test ----

    #[test]
    fn parse_args_init_returns_init() {
        let action = parse_args(vec!["init".to_string()]);
        assert!(matches!(action, Action::Init));
    }

    // ---- parse_args completions tests ----

    #[test]
    fn parse_args_completions_with_shell() {
        let action = parse_args(vec![
            "completions".to_string(),
            "--shell".to_string(),
            "bash".to_string(),
        ]);
        match action {
            Action::Completions { shell } => assert_eq!(shell, "bash"),
            _ => panic!("Expected Completions"),
        }
    }

    #[test]
    fn parse_args_completions_without_shell() {
        let action = parse_args(vec!["completions".to_string()]);
        match action {
            Action::Completions { shell } => assert_eq!(shell, ""),
            _ => panic!("Expected Completions"),
        }
    }

    // ---- format_help tests ----

    #[test]
    fn format_help_with_no_subcommands() {
        let help = format_help(&[]);
        assert!(help.contains("usage:"));
        assert!(help.contains("No subcommands found"));
    }

    #[test]
    fn format_help_lists_subcommands() {
        let subcommands = vec!["discourse".to_string(), "linear".to_string()];
        let help = format_help(&subcommands);
        assert!(help.contains("discourse"));
        assert!(help.contains("linear"));
        assert!(help.contains("usage:"));
    }

    // ---- format_not_found_error tests ----

    #[test]
    fn format_not_found_error_includes_subcommand_name() {
        let err = format_not_found_error("foobar", &[]);
        assert!(err.contains("foobar"));
        assert!(err.contains("not found"));
    }

    #[test]
    fn format_not_found_error_lists_available_subcommands() {
        let available = vec!["discourse".to_string(), "linear".to_string()];
        let err = format_not_found_error("foobar", &available);
        assert!(err.contains("discourse"));
        assert!(err.contains("linear"));
    }

    // ---- completions generation tests ----

    #[test]
    fn generate_bash_completions_includes_subcommands() {
        let subs = vec!["discourse".to_string(), "linear".to_string()];
        let out = generate_bash_completions(&subs);
        assert!(out.contains("discourse linear"));
        assert!(out.contains("complete -F _llm_cli llm-cli"));
    }

    #[test]
    fn generate_zsh_completions_includes_subcommands() {
        let subs = vec!["discourse".to_string(), "linear".to_string()];
        let out = generate_zsh_completions(&subs);
        assert!(out.contains("#compdef llm-cli"));
        assert!(out.contains("discourse"));
        assert!(out.contains("linear"));
    }

    #[test]
    fn generate_fish_completions_includes_subcommands() {
        let subs = vec!["discourse".to_string(), "linear".to_string()];
        let out = generate_fish_completions(&subs);
        assert!(out.contains("complete -c llm-cli"));
        assert!(out.contains("discourse"));
        assert!(out.contains("linear"));
    }

    // ---- find_subcommand_binaries tests ----

    #[test]
    fn find_subcommand_binaries_returns_sorted_unique_names() {
        // This test exercises the real PATH. We cannot guarantee specific results,
        // but we can verify the return is sorted and contains no duplicates.
        let results = find_subcommand_binaries();
        for window in results.windows(2) {
            assert!(window[0] < window[1], "results must be sorted and unique");
        }
        // None of the returned names should contain the "llm-cli-" prefix.
        for name in &results {
            assert!(
                !name.starts_with("llm-cli-"),
                "returned names should be stripped subcommand names, got: {name}"
            );
        }
    }
}
