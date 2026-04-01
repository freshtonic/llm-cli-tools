//! Automatic pager support for human-readable output.
//!
//! Pipes output through a pager (default: `less -R`) when:
//! - stdout is a TTY
//! - `--human` mode is active
//! - output exceeds the terminal height
//! - `--debug` is not active
//! - `NO_PAGER` env var is not set
//! - `PAGER` is not set to "" or "cat"

use std::io::{IsTerminal, Write};

/// Write output to stdout, piping through a pager if appropriate.
pub fn print_with_pager(output: &str, human: bool, debug: bool) {
    if should_page(output, human, debug) {
        if pipe_to_pager(output).is_err() {
            // Fallback: print directly if pager fails.
            print!("{output}");
        }
    } else {
        print!("{output}");
    }
}

fn should_page(output: &str, human: bool, debug: bool) -> bool {
    if !human || debug {
        return false;
    }

    if std::env::var("NO_PAGER").is_ok() {
        return false;
    }

    if let Ok(pager) = std::env::var("PAGER") {
        let trimmed = pager.trim();
        if trimmed.is_empty() || trimmed == "cat" {
            return false;
        }
    }

    if !std::io::stdout().is_terminal() {
        return false;
    }

    let term_height = terminal_height().unwrap_or(24);
    let line_count = output.lines().count();
    line_count > term_height
}

fn terminal_height() -> Option<usize> {
    // Use the LINES env var if set, otherwise try ioctl.
    if let Ok(lines) = std::env::var("LINES")
        && let Ok(n) = lines.parse()
    {
        return Some(n);
    }

    #[cfg(unix)]
    {
        use std::mem::MaybeUninit;
        unsafe {
            let mut ws = MaybeUninit::<libc::winsize>::uninit();
            if libc::ioctl(1, libc::TIOCGWINSZ, ws.as_mut_ptr()) == 0 {
                let ws = ws.assume_init();
                if ws.ws_row > 0 {
                    return Some(ws.ws_row as usize);
                }
            }
        }
    }

    None
}

fn pager_command() -> (String, Vec<String>) {
    if let Ok(pager) = std::env::var("PAGER") {
        let parts: Vec<String> = pager.split_whitespace().map(String::from).collect();
        if let Some((cmd, args)) = parts.split_first() {
            return (cmd.clone(), args.to_vec());
        }
    }
    ("less".to_string(), vec!["-R".to_string()])
}

fn pipe_to_pager(output: &str) -> Result<(), std::io::Error> {
    let (cmd, args) = pager_command();
    let mut child = std::process::Command::new(&cmd)
        .args(&args)
        .stdin(std::process::Stdio::piped())
        .spawn()?;

    if let Some(ref mut stdin) = child.stdin {
        // Ignore broken pipe — user may quit the pager early.
        let _ = stdin.write_all(output.as_bytes());
    }
    drop(child.stdin.take());
    child.wait()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_not_page_json_output() {
        assert!(!should_page("some output\n", false, false));
    }

    #[test]
    fn should_not_page_with_debug() {
        assert!(!should_page("some output\n", true, true));
    }

    #[test]
    fn should_not_page_short_output() {
        let short = "line\n".repeat(5);
        // Even if human and TTY, short output shouldn't page.
        // (This test may pass trivially in CI where stdout isn't a TTY.)
        assert!(!should_page(&short, true, false));
    }
}
