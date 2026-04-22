mod app;
mod cli;
mod commands;
mod errors;
mod output;
mod config;
mod prereqs;
mod theme;
mod tui;

use clap::Parser;
use cli::Cli;
use output::OutputMode;

#[tokio::main]
async fn main() -> miette::Result<()> {
    miette::set_hook(Box::new(|_| {
        Box::new(miette::NarratableReportHandler::new())
    }))?;

    let cli = Cli::parse();
    
    // Detect CI environment
    let is_ci = cli.ci || std::env::var("CI").is_ok();
    
    let output_mode = if is_ci {
        OutputMode::Ci
    } else {
        // We initialize ratatui inline for non-fullscreen mode
        // For commands that need fullscreen (like --watch or new), they will handle EnterAlternateScreen
        let terminal = ratatui::init();
        OutputMode::Interactive { terminal }
    };
    
    let result = app::dispatch(cli.command, output_mode).await;

    if !is_ci {
        ratatui::restore();
    }

    result
}
