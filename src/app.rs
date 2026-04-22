use crate::cli::Commands;
use crate::commands;
use crate::output::OutputMode;
use ratatui::{crossterm::event::Event, Frame};

pub enum Action<T> {
    Continue,
    Quit(T),
}

pub trait State {
    type Output;
    fn view(&self, frame: &mut Frame);
    fn update(&mut self, event: Event) -> miette::Result<Action<Self::Output>>;
    fn tick(&mut self);
}

pub async fn dispatch(command: Option<Commands>, mut output_mode: OutputMode) -> miette::Result<()> {
    match command {
        Some(Commands::New { project_name }) => {
            commands::new::run(project_name, &mut output_mode).await?;
        }
        Some(Commands::Build { watch }) => {
            commands::build::run(watch, &mut output_mode).await?;
        }
        Some(Commands::Run { watch }) => {
            commands::run::run(watch, &mut output_mode).await?;
        }
        Some(Commands::Test { watch }) => {
            commands::test::run(watch, &mut output_mode).await?;
        }
        Some(Commands::Lock(args)) => {
            commands::lock::run(args, &mut output_mode).await?;
        }
        None => {
            // Default behavior if no command: show the mascot and help
            if let OutputMode::Interactive { terminal } = &mut output_mode {
                let mascot = include_str!("../assets/neo_mascot.txt");
                println!("{}", mascot);
                println!("The NeoHaskell CLI. Run `neo --help` for commands.");
            } else {
                println!("The NeoHaskell CLI. Run `neo --help` for commands.");
            }
        }
    }
    
    Ok(())
}
