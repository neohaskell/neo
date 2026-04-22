use crate::output::OutputMode;
use std::process::Stdio;
use tokio::process::Command;

pub async fn build(_output_mode: &OutputMode) -> miette::Result<()> {
    let status = Command::new("echo")
        .arg("Running nix develop --command cabal build all")
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|e| miette::miette!("Failed to spawn process: {}", e))?
        .wait()
        .await
        .map_err(|e| miette::miette!("Failed to wait for process: {}", e))?;

    if !status.success() {
        miette::bail!("Build failed");
    }
    
    Ok(())
}
