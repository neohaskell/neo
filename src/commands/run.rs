use crate::output::OutputMode;
use crate::prereqs;
use crate::config::NeoConfig;
use crate::subprocess::nix;
use crate::commands::watch_common;

pub async fn run(watch: bool, output_mode: &mut OutputMode) -> miette::Result<()> {
    prereqs::require_nix().await?;
    prereqs::warn_direnv(output_mode).await;
    
    let config = NeoConfig::load("neo.json")?;
    
    if output_mode.is_ci() {
        println!("[info] Reconciling project artifacts...");
    }
    crate::reconcile::run(".", &config).await?;
    
    if watch {
        watch_common::run_watch("run", output_mode).await?;
    } else {
        if output_mode.is_ci() {
            println!("[info] Running project...");
        }
        nix::run(output_mode).await?;
    }
    
    Ok(())
}
