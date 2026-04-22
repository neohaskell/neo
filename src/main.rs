mod app;
mod cli;
mod commands;
mod errors;
mod output;
mod config;
mod prereqs;
mod theme;
mod tui;
mod lock;
mod network;
mod git;
mod reconcile;
mod subprocess;
mod test_utils;

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
    
    let mut output_mode = if is_ci {
        OutputMode::Ci
    } else {
        OutputMode::Interactive
    };
    
    let update_status = std::sync::Arc::new(std::sync::Mutex::new(None));
    let update_status_clone = update_status.clone();
    
    // Background update check (non-blocking)
    let _update_handle = tokio::spawn(async move {
        if let Ok(Some(latest_version)) = network::check_for_updates().await {
            let mut status = update_status_clone.lock().unwrap();
            *status = Some(latest_version);
        }
    });

    let result = app::dispatch(cli.command, &mut output_mode, update_status.clone()).await;

    if let Err(e) = &result {
        if matches!(output_mode, OutputMode::Interactive) {
            // Just use miette's default formatting which is already nice
            // We don't need to spin up a terminal just for the error box
            // that disappears after 3 seconds.
            // miette::set_hook handles the pretty printing automatically
            // when main returns Err(e).
        }
    }

    // Show update notice if available at the end as well (for non-interactive or short-lived commands)
    let final_update_status = update_status.lock().unwrap();
    if let Some(latest_version) = &*final_update_status {
        if is_ci {
            println!("\n[info] A new version of NeoCLI is available: v{}", latest_version);
        } else {
            // Only print if not already shown in TUI or for consistency
            println!("\n  NeoCLI v{} is available! Run `neo update` to install.", latest_version);
        }
    }

    result
}
