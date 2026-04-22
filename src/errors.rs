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

    #[error("Git is required but not found")]
    #[diagnostic(
        code(neo::git_missing),
        url("https://git-scm.com/downloads"),
        help("Install Git: https://git-scm.com/downloads")
    )]
    GitNotFound,

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

    #[error("Template error: {0}")]
    #[diagnostic(code(neo::template_error))]
    TemplateError(String),
    
    #[error("Subprocess execution failed: {command}")]
    #[diagnostic(
        code(neo::subprocess),
        help("Check the output above for more details.")
    )]
    SubprocessError { command: String, output: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_messages() {
        let err = NeoError::NoWorkspace;
        assert!(err.to_string().contains("No `neo.json` found"));

        let err = NeoError::DirectoryExists { name: "test".to_string() };
        assert!(err.to_string().contains("Directory `test` already exists"));

        let err = NeoError::InvalidConfig { line: 10, col: 5, reason: "unexpected comma".to_string() };
        assert_eq!(err.to_string(), "Failed to parse `neo.json` at line 10, column 5: unexpected comma");

        let err = NeoError::NixNotFound;
        assert_eq!(err.to_string(), "Nix is required but not found");

        let err = NeoError::SubprocessError { 
            command: "fail".to_string(), 
            output: "some output".to_string() 
        };
        assert_eq!(err.to_string(), "Subprocess execution failed: fail");

        let err = NeoError::GitError("git failed".to_string());
        assert_eq!(err.to_string(), "Git error: git failed");
    }
}
