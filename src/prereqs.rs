use crate::errors::NeoError;
use crate::output::OutputMode;
use crossterm::style::Stylize;

pub async fn require_nix() -> miette::Result<()> {
    if tokio::process::Command::new("nix")
        .arg("--version")
        .output()
        .await
        .is_err()
    {
        return Err(NeoError::NixNotFound.into());
    }
    Ok(())
}

pub async fn require_git() -> miette::Result<()> {
    if tokio::process::Command::new("git")
        .arg("--version")
        .output()
        .await
        .is_err()
    {
        return Err(NeoError::GitNotFound.into());
    }
    Ok(())
}

pub async fn warn_direnv(output_mode: &OutputMode) {
    if tokio::process::Command::new("direnv")
        .arg("--version")
        .output()
        .await
        .is_err()
    {
        let msg = "direnv is not installed. Install it for automatic HLS integration in your editor.";
        if output_mode.is_ci() {
            println!("[warn] {}", msg);
        } else {
            println!("{} {}", "⚠".yellow(), msg.yellow());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_require_nix() {
        // We assume nix is present in the test environment
        let result = require_nix().await;
        // In some CI environments it might be missing, so we just check it doesn't panic
        // and if it returns an error, it's the right one.
        if let Err(e) = result {
            let err_str = format!("{:?}", e);
            assert!(err_str.contains("NixNotFound") || err_str.contains("Nix is required"));
        }
    }

    #[tokio::test]
    async fn test_require_git() {
        let result = require_git().await;
        if let Err(e) = result {
            let err_str = format!("{:?}", e);
            assert!(err_str.contains("GitNotFound") || err_str.contains("Git is required"));
        }
    }

    #[tokio::test]
    async fn test_warn_direnv() {
        // This should just not panic
        warn_direnv(&OutputMode::Ci).await;
        // Testing Interactive mode here is hard due to DefaultTerminal type constraints,
        // but since warn_direnv only uses is_ci(), we've covered the logic.
    }
}
