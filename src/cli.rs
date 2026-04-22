use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "neo", version, about = "The NeoHaskell CLI")]
#[command(propagate_version = true)]
pub struct Cli {
    /// Enable debug-level output
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// Disable interactive prompts, animations, and colors
    #[arg(long, global = true)]
    pub ci: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Scaffold a new NeoHaskell project
    New {
        /// Project name (required in --ci mode)
        project_name: Option<String>,
    },
    /// Reconcile config and build the project
    Build {
        /// Watch mode with GHCi hot-reloading
        #[arg(long)]
        watch: bool,
    },
    /// Reconcile, build, and run the application
    Run {
        /// Watch mode with auto-restart
        #[arg(long)]
        watch: bool,
    },
    /// Run unit tests, then integration tests
    Test {
        /// Watch mode for continuous testing
        #[arg(long)]
        watch: bool,
    },
    /// Lock event-sourced domain files
    Lock(LockArgs),
}

#[derive(clap::Args)]
pub struct LockArgs {
    #[command(subcommand)]
    pub subcommand: Option<LockSubcommand>,

    /// Fuzzy search string to match domain files
    pub search: Option<String>,

    /// Lock all discovered domain files
    #[arg(long)]
    pub all: bool,
}

#[derive(Subcommand)]
pub enum LockSubcommand {
    /// Install the git pre-commit lock hook
    Install,
}
