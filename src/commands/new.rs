use crate::output::OutputMode;

pub async fn run(_project_name: Option<String>, _output_mode: &mut OutputMode) -> miette::Result<()> {
    println!("Running new");
    Ok(())
}
