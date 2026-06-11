use crate::output::OutputMode;
use crate::errors::NeoError;
use crate::subprocess::interpret;
use std::process::Stdio;
use std::collections::VecDeque;
use tokio::process::Command;
use tokio::io::{AsyncBufReadExt, BufReader};
use crate::theme::Theme;
use crate::tui::spinner::Spinner;
use crate::tui::progress::ProgressBar;
use ratatui::layout::{Layout, Direction, Constraint};
use ratatui::widgets::Paragraph;

pub async fn build(output_mode: &mut OutputMode) -> miette::Result<()> {
    execute("cabal build all", output_mode).await
}

pub async fn run(output_mode: &mut OutputMode) -> miette::Result<()> {
    execute("cabal run all", output_mode).await
}

pub async fn spawn_app() -> miette::Result<tokio::process::Child> {
    let mut cmd = Command::new("nix");
    cmd.args(["develop", "--command", "bash", "-c", "cabal run all"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    #[cfg(unix)]
    cmd.process_group(0);

    cmd.spawn()
        .map_err(|e| NeoError::SubprocessFailed {
            operation: "spawning `nix develop --command bash -c 'cabal run all'`".to_string(),
            cause: format!("could not spawn child process: {}", e),
            fix: "Ensure `nix` is installed and on PATH (run `which nix`). If it is installed, ensure your shell can exec it (a new shell after install often helps). If you are not in a flake-enabled directory, `cd` into one first.".to_string(),
        }.into())
}

pub async fn kill_app(mut child: tokio::process::Child) {
    #[cfg(unix)]
    if let Some(pid) = child.id() {
        let group = format!("-{pid}");
        let _ = std::process::Command::new("kill")
            .args(["-TERM", &group])
            .status();
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        let _ = std::process::Command::new("kill")
            .args(["-KILL", &group])
            .status();
    }
    let _ = child.wait().await;
}

pub async fn test(output_mode: &mut OutputMode) -> miette::Result<()> {
    execute("cabal test all", output_mode).await
}

fn parse_cabal_progress(line: &str) -> Option<(usize, usize)> {
    let line = line.trim();
    if line.starts_with('[') {
        let content = line.split(']').next()?;
        if content.contains("of") {
            let mut parts = content[1..].split("of");
            let current = parts.next()?.trim().parse().ok()?;
            let total = parts.next()?.trim().parse().ok()?;
            return Some((current, total));
        }
    }
    None
}

fn last_meaningful_lines(lines: &[String], n: usize) -> String {
    let _ = n;
    // Prefer the single most-likely-actionable line: an `error: …` / `Error: …` /
    // `error:` line, or the last `cabal:` / `> ` line, or the last non-empty line.
    let interesting = lines
        .iter()
        .rev()
        .find(|l| {
            let t = l.trim();
            t.starts_with("error:")
                || t.starts_with("Error:")
                || t.starts_with("cabal:")
                || (t.starts_with("> ") && t.len() > 2)
        })
        .or_else(|| lines.iter().rev().find(|l| !l.trim().is_empty()));

    match interesting {
        Some(l) => l.trim().to_string(),
        None => "(no output)".to_string(),
    }
}

async fn execute(command_str: &str, output_mode: &mut OutputMode) -> miette::Result<()> {
    let mut terminal = if matches!(output_mode, OutputMode::Interactive) {
        ratatui::crossterm::terminal::enable_raw_mode().unwrap();
        let backend = ratatui::backend::CrosstermBackend::new(std::io::stdout());
        let t = ratatui::Terminal::with_options(
            backend,
            ratatui::TerminalOptions { viewport: ratatui::Viewport::Inline(5) }
        ).unwrap();
        ratatui::crossterm::terminal::disable_raw_mode().unwrap();
        Some(t)
    } else {
        None
    };

    let mut child = Command::new("nix")
        .args(["develop", "--command", "bash", "-c", command_str])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| NeoError::SubprocessFailed {
            operation: format!("spawning `nix develop --command {}`", command_str),
            cause: format!("could not spawn child process: {}", e),
            fix: "Ensure `nix` is installed and on PATH (run `which nix`). Re-open your shell after install. If you are outside a flake-enabled directory, `cd` to one first.".to_string(),
        })?;

    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    let mut stdout_reader = BufReader::new(stdout).lines();
    let mut stderr_reader = BufReader::new(stderr).lines();

    let is_ci = output_mode.is_ci();
    let theme = Theme::neo();
    let mut current_step = 0;
    let mut total_steps = 0;
    let mut last_lines: VecDeque<String> = VecDeque::with_capacity(3);
    let mut frame = 0;
    let mut captured_output = Vec::new();

    let mut stdout_done = false;
    let mut stderr_done = false;

    loop {
        tokio::select! {
            result = stdout_reader.next_line(), if !stdout_done => {
                match result {
                    Ok(Some(line)) => {
                        if let Some((c, t)) = parse_cabal_progress(&line) {
                            current_step = c;
                            total_steps = t;
                        }
                        if last_lines.len() >= 3 {
                            last_lines.pop_front();
                        }
                        last_lines.push_back(line.clone());
                        captured_output.push(line.clone());
                        if is_ci {
                            println!("{}", line);
                        }
                    }
                    Ok(None) => stdout_done = true,
                    Err(e) => return Err(NeoError::SubprocessFailed {
                        operation: format!("reading stdout of `nix develop --command {}`", command_str),
                        cause: format!("OS error while reading child stdout: {}", e),
                        fix: "This usually means the child was killed externally (signal, OOM). Re-run the command. If reproducible, capture `dmesg` and file a bug.".to_string(),
                    }.into()),
                }
            }
            result = stderr_reader.next_line(), if !stderr_done => {
                match result {
                    Ok(Some(line)) => {
                        if last_lines.len() >= 3 {
                            last_lines.pop_front();
                        }
                        last_lines.push_back(line.clone());
                        captured_output.push(line.clone());
                        if is_ci {
                            eprintln!("{}", line);
                        }
                    }
                    Ok(None) => stderr_done = true,
                    Err(e) => return Err(NeoError::SubprocessFailed {
                        operation: format!("reading stderr of `nix develop --command {}`", command_str),
                        cause: format!("OS error while reading child stderr: {}", e),
                        fix: "This usually means the child was killed externally (signal, OOM). Re-run the command. If reproducible, capture `dmesg` and file a bug.".to_string(),
                    }.into()),
                }
            }
            _ = tokio::time::sleep(std::time::Duration::from_millis(80)) => {
                frame += 1;
            }
        }

        if stdout_done && stderr_done {
            break;
        }

        if matches!(output_mode, OutputMode::Interactive) {
            if let Some(t) = &mut terminal {
                t.draw(|f| {
                    let chunks = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([
                            Constraint::Length(1), // Spinner
                            Constraint::Length(1), // Progress bar
                            Constraint::Length(3), // Last 3 lines
                        ])
                        .split(f.area());

                    let spinner = Spinner::new(&theme, frame);
                    f.render_widget(spinner, chunks[0]);

                    if total_steps > 0 {
                        let progress = current_step as f64 / total_steps as f64;
                        let label = format!("Step {}/{}", current_step, total_steps);
                        let bar = ProgressBar::new(&theme, progress)
                            .with_label(&label);
                        f.render_widget(bar, chunks[1]);
                    }

                    let output_text = last_lines.iter().cloned().collect::<Vec<_>>().join("\n");
                    let output = Paragraph::new(output_text).style(theme.style_muted());
                    f.render_widget(output, chunks[2]);
                }).ok();
            }
        }
    }

    let status = child.wait().await
        .map_err(|e| NeoError::SubprocessFailed {
            operation: format!("waiting on `nix develop --command {}`", command_str),
            cause: format!("could not reap child process: {}", e),
            fix: "Re-run the command. If reproducible, your shell may be out of file descriptors (`ulimit -n`).".to_string(),
        })?;

    if !status.success() {
        let joined = captured_output.join("\n");
        if let Some(i) = interpret::interpret_any(&joined) {
            return Err(NeoError::SubprocessFailed {
                operation: format!("`{}`", command_str),
                cause: i.cause,
                fix: i.fix,
            }.into());
        }
        return Err(NeoError::SubprocessRaw {
            operation: format!("`{}`", command_str),
            tail: last_meaningful_lines(&captured_output, 5),
        }.into());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_cabal_progress() {
        assert_eq!(parse_cabal_progress("[1 of 5] Compiling Lib"), Some((1, 5)));
        assert_eq!(parse_cabal_progress("  [ 12 of 100 ] Compiling module"), Some((12, 100)));
        assert_eq!(parse_cabal_progress("Compiling module"), None);
        assert_eq!(parse_cabal_progress("[1 of 5 Compiling"), None);
    }

    #[test]
    fn test_last_meaningful_lines_empty() {
        let v: Vec<String> = vec![];
        assert_eq!(last_meaningful_lines(&v, 5), "(no output)");
    }

    #[test]
    fn test_last_meaningful_lines_all_blank() {
        let v: Vec<String> = vec!["".to_string(), "  ".to_string(), "\t".to_string()];
        assert_eq!(last_meaningful_lines(&v, 5), "(no output)");
    }

    #[test]
    fn test_last_meaningful_lines_picks_last_error_line() {
        let v: Vec<String> = vec![
            "Building...".to_string(),
            "[1 of 5] Compiling Foo".to_string(),
            "error: something broke specifically here".to_string(),
            "       continuation line that should NOT be the headline".to_string(),
        ];
        let tail = last_meaningful_lines(&v, 5);
        assert_eq!(tail, "error: something broke specifically here");
    }

    #[test]
    fn test_last_meaningful_lines_picks_cabal_line_when_no_error() {
        let v: Vec<String> = vec![
            "Resolving deps".to_string(),
            "cabal: unknown package: foo".to_string(),
            "(stray line)".to_string(),
        ];
        let tail = last_meaningful_lines(&v, 5);
        assert_eq!(tail, "cabal: unknown package: foo");
    }

    #[test]
    fn test_last_meaningful_lines_falls_back_to_last_nonempty() {
        let v: Vec<String> = vec![
            "step a".to_string(),
            "step b".to_string(),
            "  ".to_string(),
        ];
        let tail = last_meaningful_lines(&v, 5);
        assert_eq!(tail, "step b");
    }

    #[tokio::test]
    async fn test_nix_not_found() {
        let mut output_mode = OutputMode::Ci;
        let result = execute("ls /non-existent-directory-neo", &mut output_mode).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        if let Some(neo_err) = err.downcast_ref::<NeoError>() {
            match neo_err {
                NeoError::SubprocessRaw { operation, tail } => {
                    assert!(operation.contains("ls /non-existent-directory-neo"));
                    assert!(!tail.is_empty(), "Captured tail should not be empty");
                }
                NeoError::SubprocessFailed { operation, .. } => {
                    // also acceptable if `ls` stderr happens to match an interpreter pattern
                    assert!(operation.contains("ls /non-existent-directory-neo"));
                }
                other => panic!("Expected SubprocessRaw or SubprocessFailed, got {:?}", other),
            }
        } else {
            panic!("Expected NeoError, got {:?}", err);
        }
    }

    #[tokio::test]
    async fn test_spawn_app() {
        let result = spawn_app().await;
        if let Ok(mut child) = result {
            child.kill().await.ok();
        }
    }
}
