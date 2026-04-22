use crate::errors::NeoError;
use crate::output::OutputMode;
use crossterm::style::Stylize;

pub async fn require_nix() -> miette::Result<()> {
    if tokio::process::Command::new("nix").arg("--version").output().await.is_err() {
        return Err(NeoError::NixNotFound.into());
    }
    Ok(())
}

pub async fn warn_direnv(output_mode: &OutputMode) {
    if tokio::process::Command::new("direnv").arg("--version").output().await.is_err() {
        let msg = "direnv is not installed. Install it for automatic HLS integration in your editor.";
        if output_mode.is_ci() {
            println!("[warn] {}", msg);
        } else {
            println!("{} {}", "⚠".yellow(), msg.yellow());
        }
    }
}
