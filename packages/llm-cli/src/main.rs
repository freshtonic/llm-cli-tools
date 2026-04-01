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
