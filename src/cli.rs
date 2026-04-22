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
    #[command(long_about = "Scaffold a new NeoHaskell project with a full interactive interview. \
                            This command will guide you through project naming, versioning, \
                            and license selection. In --ci mode, it uses defaults unless args are provided.")]
    New {
        /// Project name (required in --ci mode)
        project_name: Option<String>,
    },
    /// Reconcile config and build the project
    #[command(long_about = "Automatically generate Nix and Cabal files from neo.json and build the project. \
                            If --watch is used, it starts a GHCi session for instant feedback on file changes.")]
    Build {
        /// Watch mode with GHCi hot-reloading
        #[arg(long)]
        watch: bool,
    },
    /// Reconcile, build, and run the application
    #[command(long_about = "Build the project and execute the application. \
                            Use --watch to automatically rebuild and restart when source files change.")]
    Run {
        /// Watch mode with auto-restart
        #[arg(long)]
        watch: bool,
    },
    /// Run unit tests, then integration tests
    #[command(long_about = "Execute all unit tests via Cabal, followed by integration tests using Hurl. \
                            In --watch mode, tests are re-run on every file modification.")]
    Test {
        /// Watch mode for continuous testing
        #[arg(long)]
        watch: bool,
    },
    /// Lock event-sourced domain files
    #[command(long_about = "Search for and lock event-sourced domain files to prevent accidental modification. \
                            Locked files are added to .locked-files and verified by the pre-commit hook.")]
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
