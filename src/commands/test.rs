use crate::output::OutputMode;
use crate::prereqs;
use crate::config::NeoConfig;
use crate::commands::watch_common;
use crate::subprocess::hurl;
use crate::subprocess::nix;
use std::time::Duration;
use tokio::time::sleep;
use crate::tui::spinner::Spinner;
use crate::tui::progress::ProgressBar;
use crate::theme::Theme;
use ratatui::layout::{Layout, Direction, Constraint};

pub async fn run(watch: bool, output_mode: &mut OutputMode) -> miette::Result<()> {
    prereqs::require_nix().await?;
    prereqs::require_git().await?;
    prereqs::warn_direnv(output_mode).await;
    
    let config = NeoConfig::load("neo.json")?;
    
    if output_mode.is_ci() {
        println!("[info] Reconciling project artifacts...");
    }
    crate::reconcile::run(".", &config).await?;
    
    if watch {
        watch_common::run_watch("test", output_mode).await?;
    } else {
        if output_mode.is_ci() {
            println!("[info] Running unit tests...");
        }
        
        let unit_test_result = nix::test(output_mode).await;
        
        if let Err(e) = unit_test_result {
            if output_mode.is_ci() {
                eprintln!("[error] Unit tests failed: {}", e);
            }
            return Err(e);
        }

        if output_mode.is_ci() {
            println!("[ok] Unit tests passed");
        }

        // Integration tests (Hurl)
        let hurl_tests = hurl::discover_tests(None).await?;
        if !hurl_tests.is_empty() {
            if output_mode.is_ci() {
                println!("[info] Running {} Hurl integration tests...", hurl_tests.len());
            }

            // Start the app in the background
            let mut app_child = nix::spawn_app().await?;
            
            // Wait for app to start (heuristic: 2 seconds)
            sleep(Duration::from_secs(2)).await;

            let mut passed = 0;
            let mut failed = 0;
            let start_time = std::time::Instant::now();
            let total_tests = hurl_tests.len();
            let theme = Theme::neo();
            let mut frame = 0;

            let mut terminal: Option<ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>> = None;

            for (i, test_path) in hurl_tests.iter().enumerate() {
                if matches!(output_mode, OutputMode::Interactive) {
                    if terminal.is_none() {
                        ratatui::crossterm::terminal::enable_raw_mode().unwrap();
                        let backend = ratatui::backend::CrosstermBackend::new(std::io::stdout());
                        terminal = Some(ratatui::Terminal::with_options(
                            backend,
                            ratatui::TerminalOptions { viewport: ratatui::Viewport::Inline(3) }
                        ).unwrap());
                        ratatui::crossterm::terminal::disable_raw_mode().unwrap();
                    }
                    if let Some(t) = &mut terminal {
                        t.draw(|f| {
                        let chunks = Layout::default()
                            .direction(Direction::Vertical)
                            .constraints([
                                Constraint::Length(1), // Spinner
                                Constraint::Length(1), // Progress bar
                                Constraint::Min(0),
                            ])
                            .split(f.area());

                        let label = format!("Running: {}", test_path.display());
                        let spinner = Spinner::new(&theme, frame).with_label(&label);
                        f.render_widget(spinner, chunks[0]);

                        let progress = i as f64 / total_tests as f64;
                        let bar_label = format!("Test {}/{}", i + 1, total_tests);
                        let bar = ProgressBar::new(&theme, progress)
                            .with_label(&bar_label);
                        f.render_widget(bar, chunks[1]);
                    }).ok();
                    }
                    frame += 1;
                }

                match hurl::run_test(test_path, output_mode).await {
                    Ok(result) => {
                        if result.success {
                            passed += 1;
                            if output_mode.is_ci() {
                                println!("[ok] {} passed ({:?})", test_path.display(), result.duration);
                            }
                        } else {
                            failed += 1;
                            if output_mode.is_ci() {
                                println!("[fail] {} failed", test_path.display());
                            }
                        }
                    }
                    Err(e) => {
                        failed += 1;
                        if output_mode.is_ci() {
                            eprintln!("[error] Failed to run test {}: {}", test_path.display(), e);
                        }
                    }
                }
            }

            // Kill the app
            let _ = app_child.kill().await;

            let total_duration = start_time.elapsed();

            if output_mode.is_ci() {
                println!("\nTest Summary:");
                println!("  Passed:   {}", passed);
                println!("  Failed:   {}", failed);
                println!("  Duration: {:?}", total_duration);
            } else if matches!(output_mode, OutputMode::Interactive) {
                if terminal.is_none() {
                    ratatui::crossterm::terminal::enable_raw_mode().unwrap();
                    let backend = ratatui::backend::CrosstermBackend::new(std::io::stdout());
                    terminal = Some(ratatui::Terminal::with_options(
                        backend,
                        ratatui::TerminalOptions { viewport: ratatui::Viewport::Inline(3) }
                    ).unwrap());
                    ratatui::crossterm::terminal::disable_raw_mode().unwrap();
                }
                if let Some(t) = &mut terminal {
                    t.draw(|f| {
                        use ratatui::widgets::Paragraph;
                        let summary = format!(
                            "Tests: {} passed, {} failed\nDuration: {:?}",
                            passed, failed, total_duration
                        );
                        f.render_widget(Paragraph::new(summary).style(theme.style_success()), f.area());
                    }).ok();
                }
                tokio::time::sleep(Duration::from_secs(3)).await;
            }

            if failed > 0 {
                return Err(crate::errors::NeoError::SubprocessError { 
                    command: "hurl tests".to_string(), 
                    output: format!("{} integration tests failed", failed) 
                }.into());
            }
        } else {
            if output_mode.is_ci() {
                println!("[info] No Hurl integration tests found in tests/");
            }
        }
    }
    
    Ok(())
}
