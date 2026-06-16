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
        /// Scaffold a library project (no launcher folder, no executable cabal stanza)
        #[arg(long)]
        library: bool,
    },
    /// Reconcile config and build the project
    #[command(long_about = "Automatically generate Nix and Cabal files from neo.json and build the project. \
                            If --watch is used, it starts a GHCi session for instant feedback on file changes.")]
    Build {
        /// Watch mode with GHCi hot-reloading
        #[arg(long)]
        watch: bool,
        /// Skip the pre-build check that aborts when locked files have been modified
        #[arg(long)]
        skip_lock_check: bool,
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
    /// Launch the bundled in-browser NeoHaskell IDE
    #[command(long_about = "Start a local HTTP server that serves the bundled NeoHaskell IDE \
                            (the Vite app embedded into the `neo` binary). Open the printed URL \
                            in your browser. Press Ctrl-C to stop. Defaults to binding 127.0.0.1 \
                            (loopback only). Pass `--host 0.0.0.0` to make the IDE reachable from \
                            other machines on your network.")]
    Ide {
        /// IP address to bind on (e.g. `127.0.0.1`, `0.0.0.0`, `::1`).
        /// Hostnames are not accepted — pass a literal IPv4 or IPv6 address.
        #[arg(long, default_value_t = std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST))]
        host: std::net::IpAddr,
        /// TCP port to bind
        #[arg(long, default_value_t = 2323)]
        port: u16,
    },
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
            Some(Commands::New { project_name, library }) => {
                assert_eq!(project_name, Some("my-project".into()));
                assert!(!library);
            }
            _ => panic!("Expected New command"),
        }
    }

    #[test]
    fn test_parse_new_library() {
        let cli = Cli::try_parse_from(["neo", "new", "my-lib", "--library"]).unwrap();
        match cli.command {
            Some(Commands::New { project_name, library }) => {
                assert_eq!(project_name, Some("my-lib".into()));
                assert!(library);
            }
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
    fn test_parse_build_skip_lock_check() {
        let cli = Cli::try_parse_from(["neo", "build", "--skip-lock-check"]).unwrap();
        match cli.command {
            Some(Commands::Build { watch, skip_lock_check }) => {
                assert!(!watch);
                assert!(skip_lock_check);
            }
            _ => panic!("Expected Build command"),
        }
    }

    #[test]
    fn test_parse_build_default_lock_check() {
        let cli = Cli::try_parse_from(["neo", "build"]).unwrap();
        match cli.command {
            Some(Commands::Build { watch, skip_lock_check }) => {
                assert!(!watch);
                assert!(!skip_lock_check);
            }
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

    #[test]
    fn test_parse_ide_defaults_to_loopback_and_2323() {
        let cli = Cli::try_parse_from(["neo", "ide"]).unwrap();
        match cli.command {
            Some(Commands::Ide { host, port }) => {
                assert_eq!(host, std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)));
                assert_eq!(port, 2323);
            }
            _ => panic!("Expected Ide command"),
        }
    }

    #[test]
    fn test_parse_ide_custom_port() {
        let cli = Cli::try_parse_from(["neo", "ide", "--port", "8080"]).unwrap();
        match cli.command {
            Some(Commands::Ide { host, port }) => {
                assert_eq!(host, std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST));
                assert_eq!(port, 8080);
            }
            _ => panic!("Expected Ide command"),
        }
    }

    #[test]
    fn test_parse_ide_custom_host_any_v4() {
        let cli = Cli::try_parse_from(["neo", "ide", "--host", "0.0.0.0"]).unwrap();
        match cli.command {
            Some(Commands::Ide { host, port }) => {
                assert_eq!(host, std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED));
                assert_eq!(port, 2323);
            }
            _ => panic!("Expected Ide command"),
        }
    }

    #[test]
    fn test_parse_ide_custom_host_v6() {
        let cli = Cli::try_parse_from(["neo", "ide", "--host", "::1", "--port", "9000"]).unwrap();
        match cli.command {
            Some(Commands::Ide { host, port }) => {
                assert_eq!(host, std::net::IpAddr::V6(std::net::Ipv6Addr::LOCALHOST));
                assert_eq!(port, 9000);
            }
            _ => panic!("Expected Ide command"),
        }
    }

    #[test]
    fn test_parse_ide_rejects_out_of_range_port() {
        // 99999 is outside u16 range; clap must refuse to parse.
        let result = Cli::try_parse_from(["neo", "ide", "--port", "99999"]);
        assert!(result.is_err(), "expected clap to reject port 99999");
    }

    #[test]
    fn test_parse_ide_rejects_hostname() {
        // `localhost` is a hostname, not an IP address. We require an IP literal so the
        // bind interface is unambiguous (no DNS, no v4-vs-v6 surprise).
        let result = Cli::try_parse_from(["neo", "ide", "--host", "localhost"]);
        assert!(result.is_err(), "expected clap to reject `localhost`");
    }

    #[test]
    fn test_parse_ide_rejects_garbage_host() {
        let result = Cli::try_parse_from(["neo", "ide", "--host", "not-an-ip"]);
        assert!(result.is_err(), "expected clap to reject `not-an-ip`");
    }
}
