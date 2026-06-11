use std::path::PathBuf;
use miette::{Diagnostic, NamedSource, SourceSpan};
use thiserror::Error;

#[derive(Debug, Error, Diagnostic)]
pub enum NeoError {
    #[error("No `neo.json` found in the current directory")]
    #[diagnostic(
        code(neo::no_workspace),
        url(docsrs),
        help("Run `neo new <name>` to create a project, or `cd` into an existing project directory that contains a `neo.json`.")
    )]
    NoWorkspace,

    #[error("Failed to parse `neo.json`: {reason}")]
    #[diagnostic(
        code(neo::invalid_config),
        url(docsrs),
        help("Expected JSON object syntax — `\"key\": value` pairs separated by commas, no trailing comma before `}}`. Open `neo.json` at the line and column underlined above, fix the syntax (e.g. remove a trailing comma, close a missing brace/quote, end an unterminated string), save, then re-run.")
    )]
    InvalidConfig {
        reason: String,
        #[source_code]
        src: NamedSource<String>,
        #[label("syntax error here")]
        bad_bit: SourceSpan,
    },

    #[error("Directory `{name}` already exists in the current directory")]
    #[diagnostic(
        code(neo::dir_exists),
        url(docsrs),
        help("Pick a different project name (e.g. `neo new {name}-2`), or remove the existing directory first with `rm -rf {name}` and re-run `neo new {name}`.")
    )]
    DirectoryExists { name: String },

    #[error("Nix is required but was not found on PATH")]
    #[diagnostic(
        code(neo::nix_missing),
        url("https://nixos.org/download"),
        help("Install Nix from https://nixos.org/download (Determinate Systems installer recommended on macOS), then open a new shell and re-run.")
    )]
    NixNotFound,

    #[error("Git is required but was not found on PATH")]
    #[diagnostic(
        code(neo::git_missing),
        url("https://git-scm.com/downloads"),
        help("Install Git from https://git-scm.com/downloads (or via your OS package manager, e.g. `brew install git`), then open a new shell and re-run.")
    )]
    GitNotFound,

    #[error("Failed to fetch `{url}` over the network: {source}")]
    #[diagnostic(
        code(neo::network),
        url(docsrs),
        help("Check your internet connection (try `curl -I {url}`). If you intentionally want to skip network I/O (tests, offline dev), set `NEO_SKIP_NETWORK=1` — `neo` will use a local stub instead of downloading the starter template.")
    )]
    NetworkError {
        url: String,
        #[source]
        source: reqwest::Error,
    },

    #[error("I/O error while {operation} `{path}`: {source}")]
    #[diagnostic(
        code(neo::io_error),
        url(docsrs),
        help("Check that the path exists and that you have read/write permission. Run `ls -la {path}` to inspect, `df -h` to check disk space. If the parent directory does not exist, create it with `mkdir -p $(dirname {path})`.")
    )]
    IoErrorAt {
        operation: String,
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("git {subcommand} failed: {reason}")]
    #[diagnostic(
        code(neo::git_error),
        url(docsrs),
        help("{fix}")
    )]
    GitError {
        subcommand: String,
        reason: String,
        fix: String,
    },

    #[error("Failed to render template `{template}`: {reason}")]
    #[diagnostic(
        code(neo::template_error),
        url(docsrs),
        help("This is an internal templating error in `neo`. Re-run with `RUST_BACKTRACE=1`. If it reproduces on a fresh `neo new` checkout, file a bug at https://github.com/NeoHaskell/neocli/issues with the full backtrace — the bundled `{template}` template should not fail to render with valid input.")
    )]
    TemplateError { template: String, reason: String },

    #[error("{operation} failed: {cause}")]
    #[diagnostic(
        code(neo::subprocess),
        url(docsrs),
        help("{fix}")
    )]
    SubprocessFailed {
        operation: String,
        cause: String,
        fix: String,
    },

    #[error("{operation} failed — `neo` could not extract an actionable cause from the child output.\n\nLast meaningful line from the child:\n  {tail}")]
    #[diagnostic(
        code(neo::subprocess_raw),
        url(docsrs),
        help("Scroll up to read the full child output, or re-run with `--verbose`. If you can identify the real failure line in there, add a match for it in `src/subprocess/interpret.rs` (`interpret_cabal` / `interpret_nix` / `interpret_git`) so future runs surface a concrete fix recipe instead of raw output.")
    )]
    SubprocessRaw {
        operation: String,
        tail: String,
    },

    #[error("Invalid dependency `{key}` = `{value}`: {reason}")]
    #[diagnostic(
        code(neo::invalid_dep),
        url(docsrs),
        help("Dependency values use npm-style semver (e.g. `^1.2.3`, `~2.0`, `*`). Use prefix `hackage:`, `git:`, `github:`, or `file:` for explicit sources. Example valid entries in `neo.json`: `\"text\": \"^2.0\"`, `\"hackage:aeson\": \"^2.1\"`, `\"github:owner/repo\": \"git:#main\"`, `\"file:../sibling\": \"file:../sibling\"`.")
    )]
    InvalidDependency {
        key: String,
        value: String,
        reason: String,
        #[source_code]
        src: Option<NamedSource<String>>,
        #[label("from this entry")]
        span: Option<SourceSpan>,
    },
}

impl NeoError {
    pub fn io_at(operation: impl Into<String>, path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        NeoError::IoErrorAt {
            operation: operation.into(),
            path: path.into().display().to_string(),
            source,
        }
    }
}

/// Find the byte-offset span of `"<key>"` inside a JSON-like source.
///
/// Returns the span covering the quoted key (including the surrounding `"`).
/// Returns `None` if the literal substring `"<key>"` does not occur.
///
/// Limitation: first match wins. If the same key name appears earlier in the
/// file inside a string value, that earlier occurrence is what gets pointed at.
/// Acceptable for `neo.json` where keys are typically distinctive.
pub fn key_span(content: &str, key: &str) -> Option<SourceSpan> {
    let needle = format!("\"{}\"", key);
    let offset = content.find(&needle)?;
    Some(SourceSpan::new(offset.into(), needle.len()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use miette::{GraphicalReportHandler, GraphicalTheme};

    fn render(diag: &dyn Diagnostic) -> String {
        let mut buf = String::new();
        GraphicalReportHandler::new_themed(GraphicalTheme::unicode_nocolor())
            .with_width(120)
            .with_links(false)
            .with_urls(false)
            .render_report(&mut buf, diag)
            .unwrap();
        buf
    }

    fn render_with_urls(diag: &dyn Diagnostic, links: bool, urls: bool) -> String {
        let mut buf = String::new();
        GraphicalReportHandler::new_themed(GraphicalTheme::unicode_nocolor())
            .with_width(120)
            .with_links(links)
            .with_urls(urls)
            .render_report(&mut buf, diag)
            .unwrap();
        buf
    }

    fn stub_invalid_config() -> NeoError {
        NeoError::InvalidConfig {
            reason: "unexpected comma".to_string(),
            src: NamedSource::new("neo.json", "x".to_string()),
            bad_bit: SourceSpan::new(0usize.into(), 1usize),
        }
    }

    fn stub_invalid_dep() -> NeoError {
        NeoError::InvalidDependency {
            key: "k".to_string(),
            value: "v".to_string(),
            reason: "r".to_string(),
            src: None,
            span: None,
        }
    }

    #[test]
    fn test_error_messages() {
        let err = NeoError::NoWorkspace;
        assert!(err.to_string().contains("No `neo.json` found"));

        let err = NeoError::DirectoryExists { name: "test".to_string() };
        assert!(err.to_string().contains("Directory `test` already exists"));

        let err = stub_invalid_config();
        assert!(err.to_string().contains("Failed to parse `neo.json`"));
        assert!(err.to_string().contains("unexpected comma"));

        let err = NeoError::NixNotFound;
        assert!(err.to_string().contains("Nix is required"));

        let err = NeoError::SubprocessFailed {
            operation: "cabal build".to_string(),
            cause: "package `foo` not found".to_string(),
            fix: "edit `neo.json` and add a source prefix".to_string(),
        };
        let rendered = err.to_string();
        assert!(rendered.contains("cabal build failed"));
        assert!(rendered.contains("package `foo` not found"));

        let err = NeoError::SubprocessRaw {
            operation: "nix develop".to_string(),
            tail: "(no output)".to_string(),
        };
        let rendered = err.to_string();
        assert!(rendered.contains("nix develop failed"), "rendered: {}", rendered);
        assert!(rendered.contains("could not extract an actionable cause"), "rendered: {}", rendered);

        let err = NeoError::GitError {
            subcommand: "ls-remote".to_string(),
            reason: "repository not found".to_string(),
            fix: "check the URL".to_string(),
        };
        assert!(err.to_string().contains("git ls-remote failed"));
        assert!(err.to_string().contains("repository not found"));
    }

    #[test]
    fn invalid_config_help_no_longer_repeats_line_col() {
        // The line/col now live in the snippet block; the help text should explain
        // how to fix without re-stating coordinates already shown above.
        let err = stub_invalid_config();
        let help = err.help().map(|h| h.to_string()).unwrap_or_default();
        assert!(help.contains("underlined above"), "help: {}", help);
        assert!(help.contains("re-run"), "help: {}", help);
    }

    #[test]
    fn test_network_error_mentions_offline_env_var() {
        let err = NeoError::NetworkError {
            url: "https://example.invalid".to_string(),
            source: reqwest::Client::new()
                .get("not a url")
                .build()
                .unwrap_err(),
        };
        let help = err.help().map(|h| h.to_string()).unwrap_or_default();
        assert!(help.contains("NEO_SKIP_NETWORK=1"), "help missing env var: {}", help);
    }

    #[test]
    fn test_io_error_carries_path() {
        let err = NeoError::io_at(
            "writing `neo.json`".to_string(),
            std::path::PathBuf::from("/tmp/x/neo.json"),
            std::io::Error::new(std::io::ErrorKind::PermissionDenied, "no perm"),
        );
        let display = err.to_string();
        assert!(display.contains("writing `neo.json`"), "missing op: {}", display);
        assert!(display.contains("/tmp/x/neo.json"), "missing path: {}", display);
        assert!(display.contains("no perm"), "missing source: {}", display);

        let help = err.help().map(|h| h.to_string()).unwrap_or_default();
        assert!(help.contains("ls -la"), "help missing ls -la: {}", help);
    }

    #[test]
    fn test_subprocess_failed_carries_fix() {
        let err = NeoError::SubprocessFailed {
            operation: "cabal build".to_string(),
            cause: "X".to_string(),
            fix: "Y".to_string(),
        };
        let display = err.to_string();
        let help = err.help().map(|h| h.to_string()).unwrap_or_default();
        assert!(display.contains("cabal build failed: X"), "bad display: {}", display);
        assert_eq!(help, "Y", "bad help: {}", help);
    }

    // ---------------- Tier 2a: url() on every variant ----------------

    fn all_variants() -> Vec<NeoError> {
        vec![
            NeoError::NoWorkspace,
            stub_invalid_config(),
            NeoError::DirectoryExists { name: "x".to_string() },
            NeoError::NixNotFound,
            NeoError::GitNotFound,
            NeoError::NetworkError {
                url: "u".to_string(),
                source: reqwest::Client::new().get("not a url").build().unwrap_err(),
            },
            NeoError::IoErrorAt {
                operation: "o".to_string(),
                path: "p".to_string(),
                source: std::io::Error::other("io"),
            },
            NeoError::GitError {
                subcommand: "g".to_string(),
                reason: "r".to_string(),
                fix: "f".to_string(),
            },
            NeoError::TemplateError { template: "t".to_string(), reason: "r".to_string() },
            NeoError::SubprocessFailed { operation: "o".to_string(), cause: "c".to_string(), fix: "f".to_string() },
            NeoError::SubprocessRaw { operation: "o".to_string(), tail: "t".to_string() },
            stub_invalid_dep(),
        ]
    }

    #[test]
    fn every_variant_has_url() {
        for v in all_variants() {
            let url = v.url().map(|u| u.to_string());
            assert!(
                url.is_some() && !url.as_deref().unwrap().is_empty(),
                "variant {:?} has no url()",
                v
            );
        }
    }

    #[test]
    fn url_renders_as_link_when_links_on() {
        let rendered = render_with_urls(&NeoError::NoWorkspace, true, true);
        assert!(rendered.contains("\x1b]8;"), "expected OSC-8 sentinel in: {:?}", rendered);
    }

    #[test]
    fn url_omitted_when_with_urls_false() {
        let rendered = render_with_urls(&NeoError::NoWorkspace, false, false);
        assert!(!rendered.contains("\x1b]8;"), "OSC-8 should be absent: {:?}", rendered);
    }

    // ---------------- Tier 2b: InvalidConfig source span ----------------

    fn invalid_config_with_content(content: &str, line: usize, col: usize, reason: &str) -> NeoError {
        let offset = miette::SourceOffset::from_location(content, line, col);
        NeoError::InvalidConfig {
            reason: reason.to_string(),
            src: NamedSource::new("neo.json", content.to_string()),
            bad_bit: SourceSpan::new(offset, 1usize),
        }
    }

    #[test]
    fn invalid_config_renders_with_caret_block() {
        // Trailing comma on line 3 col 18.
        let content = "{\n  \"name\": \"x\",\n  \"author\": \"y\",,\n}";
        let err = invalid_config_with_content(content, 3, 18, "trailing comma");
        let rendered = render(&err);
        assert!(rendered.contains("Failed to parse `neo.json`"), "headline missing: {}", rendered);
        assert!(rendered.contains("trailing comma"), "reason missing: {}", rendered);
        assert!(rendered.contains("syntax error here"), "label missing: {}", rendered);
        // Snippet block opens with either unicode `╭` or ASCII `,-` depending on theme.
        // We picked unicode_nocolor in `render()`, so this should be unicode.
        assert!(
            rendered.contains("╭") || rendered.contains(",-"),
            "snippet block open missing: {}",
            rendered
        );
    }

    #[test]
    fn invalid_config_renders_deterministically() {
        let err = stub_invalid_config();
        assert_eq!(render(&err), render(&err));
    }

    #[test]
    fn invalid_config_with_unicode_content_does_not_panic() {
        let content = "{\n  \"name\": \"日本語\",,\n}";
        let err = invalid_config_with_content(content, 2, 22, "trailing comma after unicode");
        let _ = render(&err); // must not panic
    }

    #[test]
    fn invalid_config_at_eof_does_not_panic() {
        let content = "{ \"name\": ";
        let err = invalid_config_with_content(content, 1, 11, "unexpected end of input");
        let _ = render(&err);
    }

    #[test]
    fn invalid_config_empty_content_does_not_panic() {
        let err = invalid_config_with_content("", 1, 1, "empty input");
        let _ = render(&err);
    }

    // ---------------- Tier 2c: key_span helper ----------------

    #[test]
    fn key_span_finds_quoted_key() {
        let content = "{\"foo\": 1, \"bar\": 2}";
        let span = key_span(content, "bar").expect("should find bar");
        assert_eq!(span.offset(), 11);
        assert_eq!(span.len(), 5); // "bar" with surrounding quotes = 5 bytes
    }

    #[test]
    fn key_span_returns_none_when_missing() {
        assert!(key_span("{\"x\": 1}", "nope").is_none());
    }

    #[test]
    fn key_span_first_match_wins() {
        // The key `a` appears twice — once as a key, once as a value. We get the first.
        let content = "{\"a\":\"a\"}";
        let span = key_span(content, "a").expect("should find a");
        assert_eq!(span.offset(), 1, "should point at the first occurrence");
    }

    #[test]
    fn key_span_handles_unicode_key() {
        let content = "{\"日本語\": 1}";
        let span = key_span(content, "日本語").expect("should find unicode key");
        // 1 byte for `{`, then 1 byte for opening `"`, span starts there
        assert_eq!(span.offset(), 1);
    }

    // ---------------- Tier 2c: InvalidDependency rendering ----------------

    #[test]
    fn invalid_dep_renders_with_span_when_source_attached() {
        let content = "{\n  \"dependencies\": {\n    \"foo\": \"^9.9.9\"\n  }\n}";
        let err = NeoError::InvalidDependency {
            key: "foo".to_string(),
            value: "^9.9.9".to_string(),
            reason: "package `foo` not found in the NeoPackages registry".to_string(),
            src: Some(NamedSource::new("neo.json", content.to_string())),
            span: key_span(content, "foo"),
        };
        let rendered = render(&err);
        assert!(rendered.contains("Invalid dependency `foo`"), "headline: {}", rendered);
        assert!(rendered.contains("from this entry"), "label: {}", rendered);
        assert!(rendered.contains("neo.json"), "filename: {}", rendered);
    }

    #[test]
    fn invalid_dep_renders_without_span_when_no_source() {
        let err = NeoError::InvalidDependency {
            key: "foo".to_string(),
            value: "^9.9.9".to_string(),
            reason: "r".to_string(),
            src: None,
            span: None,
        };
        let rendered = render(&err);
        assert!(rendered.contains("Invalid dependency `foo`"), "headline: {}", rendered);
        // No snippet block (no source attached).
        assert!(!rendered.contains("from this entry"), "label should not appear: {}", rendered);
    }
}
