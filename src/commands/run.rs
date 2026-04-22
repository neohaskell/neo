use crate::output::OutputMode;
use crate::prereqs;
use crate::config::NeoConfig;

pub async fn run(_watch: bool, output_mode: &mut OutputMode) -> miette::Result<()> {
    prereqs::require_nix().await?;
    prereqs::warn_direnv(output_mode).await;
    
    let _config = NeoConfig::load("neo.json")?;
    
    if output_mode.is_ci() {
        println!("[info] Running run");
    } else {
        println!("Running run");
    }
    
    Ok(())
}
