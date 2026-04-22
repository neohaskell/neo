use crate::cli::LockArgs;
use crate::output::OutputMode;

pub async fn run(_args: LockArgs, _output_mode: &mut OutputMode) -> miette::Result<()> {
    println!("Running lock");
    Ok(())
}
