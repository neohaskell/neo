use crate::output::OutputMode;
use crate::errors::NeoError;
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
    Command::new("nix")
        .args(["develop", "--command", "bash", "-c", "cabal run all"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| NeoError::SubprocessError { 
            command: "nix develop --command bash -c cabal run all".to_string(),
            output: format!("Failed to spawn app: {}", e)
        }.into())
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
        .map_err(|e| NeoError::SubprocessError { 
            command: command_str.to_string(), 
            output: format!("Failed to spawn nix develop: {}", e) 
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
                    Err(e) => return Err(NeoError::SubprocessError { 
                        command: command_str.to_string(), 
                        output: format!("Error reading stdout: {}", e) 
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
                    Err(e) => return Err(NeoError::SubprocessError { 
                        command: command_str.to_string(), 
                        output: format!("Error reading stderr: {}", e) 
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
        .map_err(|e| NeoError::SubprocessError { 
            command: command_str.to_string(), 
            output: format!("Failed to wait for nix develop: {}", e) 
        })?;

    if !status.success() {
        let output_text = captured_output.join("\n");
        if matches!(output_mode, OutputMode::Interactive) {
            // Print the captured output so the user can see what happened
            println!("\n--- Command Output ---");
            println!("{}", output_text);
            println!("--- End of Output ---\n");
        }
        return Err(NeoError::SubprocessError { 
            command: command_str.to_string(), 
            output: output_text 
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

    #[tokio::test]
    async fn test_nix_not_found() {
        let mut output_mode = OutputMode::Ci;
        let result = execute("ls /non-existent-directory-neo", &mut output_mode).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        let err_str = err.to_string();
        assert!(err_str.contains("Subprocess execution failed"));
        // The output should contain the error from ls
        if let Some(neo_err) = err.downcast_ref::<NeoError>() {
            match neo_err {
                NeoError::SubprocessError { command, output } => {
                    assert_eq!(command, "ls /non-existent-directory-neo");
                    // Just ensure we got SOME output from the failure
                    assert!(!output.is_empty(), "Captured output should not be empty");
                }
                _ => panic!("Expected SubprocessError"),
            }
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
