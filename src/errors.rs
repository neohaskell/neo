use miette::Diagnostic;
use thiserror::Error;

#[derive(Debug, Error, Diagnostic)]
pub enum NeoError {
    #[error("No `neo.json` found")]
    #[diagnostic(
        code(neo::no_workspace),
        help("Run `neo new` to create a project, or `cd` into an existing one.")
    )]
    NoWorkspace,

    #[error("Failed to parse `neo.json` at line {line}, column {col}: {reason}")]
    #[diagnostic(code(neo::invalid_config))]
    InvalidConfig { line: usize, col: usize, reason: String },

    #[error("Directory `{name}` already exists")]
    #[diagnostic(
        code(neo::dir_exists),
        help("Choose a different name or delete it first.")
    )]
    DirectoryExists { name: String },

    #[error("Nix is required but not found")]
    #[diagnostic(
        code(neo::nix_missing),
        url("https://nixos.org/download"),
        help("Install Nix: https://nixos.org/download")
    )]
    NixNotFound,

    #[error("Failed to fetch the starter template")]
    #[diagnostic(
        code(neo::network),
        help("Check your internet connection and try again.")
    )]
    NetworkError(#[source] reqwest::Error),

    #[error("I/O error: {0}")]
    #[diagnostic(code(neo::io_error))]
    IoError(#[from] std::io::Error),

    #[error("Git error: {0}")]
    #[diagnostic(code(neo::git_error))]
    GitError(String),
    
    #[error("Subprocess execution failed: {0}")]
    #[diagnostic(code(neo::subprocess))]
    SubprocessError(String),
}
