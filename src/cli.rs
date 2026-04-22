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
    /// Check if any locked files are being committed (used by pre-commit hook)
    Check,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn test_parse_new() {
        let cli = Cli::try_parse_from(["neo", "new", "my-project"]).unwrap();
        match cli.command {
            Some(Commands::New { project_name }) => assert_eq!(project_name, Some("my-project".into())),
            _ => panic!("Expected New command"),
        }
    }

    #[test]
    fn test_parse_ci_flag() {
        let cli = Cli::try_parse_from(["neo", "--ci", "build"]).unwrap();
        assert!(cli.ci);
        match cli.command {
            Some(Commands::Build { .. }) => (),
            _ => panic!("Expected Build command"),
        }
    }

    #[test]
    fn test_parse_run() {
        let cli = Cli::try_parse_from(["neo", "run", "--watch"]).unwrap();
        match cli.command {
            Some(Commands::Run { watch }) => assert!(watch),
            _ => panic!("Expected Run command"),
        }
    }

    #[test]
    fn test_parse_test() {
        let cli = Cli::try_parse_from(["neo", "test"]).unwrap();
        match cli.command {
            Some(Commands::Test { watch }) => assert!(!watch),
            _ => panic!("Expected Test command"),
        }
    }

    #[test]
    fn test_parse_lock() {
        let cli = Cli::try_parse_from(["neo", "lock", "MyDomain"]).unwrap();
        match cli.command {
            Some(Commands::Lock(args)) => {
                assert_eq!(args.search, Some("MyDomain".to_string()));
            }
            _ => panic!("Expected Lock command"),
        }
    }
}
