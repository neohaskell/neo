//! Stderr/stdout pattern interpretation for subprocess wraps.
//!
//! The goal is the hard invariant in `.claude/skills/error-messages-instruct-llms/SKILL.md`:
//! when a known child-failure pattern shows up in captured output, emit a `cause` + `fix` recipe
//! that a tiny LLM can act on without re-reading docs.
//!
//! Each `interpret_*` function is pure: it takes a borrowed string slice (joined stdout+stderr)
//! and returns `Some(Interpreted)` if it recognizes the failure, `None` otherwise.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Interpreted {
    pub cause: String,
    pub fix: String,
}

/// Try each interpreter in turn (cabal, then nix, then git, then hurl).
/// First match wins; document order in this function is the precedence.
pub fn interpret_any(captured: &str) -> Option<Interpreted> {
    interpret_cabal(captured)
        .or_else(|| interpret_nix(captured))
        .or_else(|| interpret_git(captured))
        .or_else(|| interpret_hurl(captured))
}

pub fn interpret_cabal(captured: &str) -> Option<Interpreted> {
    if let Some(name) = extract_after(captured, "unknown package: ") {
        let name = strip_trailing_punct(name);
        return Some(Interpreted {
            cause: format!(
                "package `{}` is referenced in `neo.json` but is neither in the NeoPackages registry nor declared as `hackage:`/`git:`/`github:`/`file:`",
                name
            ),
            fix: format!(
                "Edit `neo.json`: replace the entry for `{n}` with `\"hackage:{n}\": \"^…\"` if it is on Hackage, OR use `\"git:<url>#<ref>\"` / `\"github:<owner>/<repo>#<ref>\"` / `\"file:<path>\"` for an explicit source, OR remove it. Then re-run `neo build`.",
                n = name
            ),
        });
    }

    if captured.contains("Could not resolve dependencies") {
        return Some(Interpreted {
            cause: "version constraints in `neo.json` cannot be satisfied simultaneously".to_string(),
            fix: "Loosen the `^`/`~` ranges in `neo.json` so they overlap (e.g. change `\"text\": \"^2.1\"` to `\"text\": \"^2.0\"`), or pin compatible versions for the packages cabal listed as conflicting above. Then re-run `neo build`.".to_string(),
        });
    }

    None
}

pub fn interpret_nix(captured: &str) -> Option<Interpreted> {
    for needle in &["attribute '", "attribute `"] {
        if let Some(rest) = captured.find(needle).map(|i| &captured[i + needle.len()..]) {
            let close = if needle.ends_with('\'') { '\'' } else { '`' };
            if let Some(end) = rest.find(close) {
                let attr = &rest[..end];
                let tail_after = &rest[end + 1..];
                if tail_after.starts_with(" missing") {
                    return Some(Interpreted {
                        cause: format!(
                            "`flake.nix` references attribute `{}` which no longer exists in the resolved flake inputs",
                            attr
                        ),
                        fix: "Run `rm flake.nix cabal.project *.cabal && neo build` to regenerate the build artifacts from the current `neo.json`. Direnv will pick up the new flake automatically.".to_string(),
                    });
                }
            }
        }
    }

    if let Some(rest) = extract_after(captured, "hash '") {
        if let Some(end) = rest.find('\'') {
            let bad_hash = &rest[..end];
            let tail_after = &rest[end + 1..];
            if tail_after.contains("has wrong length") {
                return Some(Interpreted {
                    cause: format!(
                        "the NeoHaskell pin in `flake.nix` resolved to `{}`, which is not a valid git hash — this almost always means `NEO_SKIP_NETWORK=1` was set when `neo build` last ran, so `flake.nix` got the placeholder SHA `deadbeef` baked in",
                        bad_hash
                    ),
                    fix: "Unset `NEO_SKIP_NETWORK` and regenerate the flake: `unset NEO_SKIP_NETWORK && rm flake.nix flake.lock cabal.project *.cabal && neo build`. (Use `NEO_SKIP_NETWORK=1` only for offline scaffolding, never for builds.)".to_string(),
                });
            }
        }
    }

    if captured.contains("error: builder for") && captured.contains("failed with exit code") {
        let drv = extract_after(captured, "builder for ")
            .and_then(|r| r.split_whitespace().next())
            .unwrap_or("<unknown>");
        let drv = drv.trim_matches(|c| c == '\'' || c == '`');
        return Some(Interpreted {
            cause: format!(
                "nix derivation `{}` failed to build — the underlying compiler / cabal / shell step exited non-zero inside the sandbox",
                drv
            ),
            fix: format!(
                "Read the full nix log to find the real error: `nix log {}`. Look for the line beginning with `Error:`, `error:`, or the last `cabal: ` line. Most common causes: (a) a dependency in `neo.json` is misspelled (cabal will say `unknown package: <name>`), (b) version constraints don't overlap (cabal will say `Could not resolve dependencies`), (c) a transitive `git:` dep points at a non-existent ref.",
                drv
            ),
        });
    }

    if captured.contains("Could not download tarball")
        || captured.contains("unable to download")
        || captured.contains("error: getting attributes of path")
    {
        return Some(Interpreted {
            cause: "nix could not download or access a flake input".to_string(),
            fix: "Check your connection (try `curl -I https://github.com`). If you are offline or behind a strict proxy, set `NEO_SKIP_NETWORK=1` only for scaffolding — builds always need real network.".to_string(),
        });
    }

    None
}

pub fn interpret_git(captured: &str) -> Option<Interpreted> {
    if let Some(rest) = captured.find("couldn't find remote ref ").map(|i| &captured[i + "couldn't find remote ref ".len()..]) {
        let ref_name = first_token(rest);
        return Some(Interpreted {
            cause: format!(
                "git dependency in `neo.json` points to ref `{}` which does not exist on the remote",
                ref_name
            ),
            fix: format!(
                "Edit `neo.json`: change `#{r}` (in the `git:<url>#<ref>` or `github:<owner>/<repo>#<ref>` entry) to a real branch, tag, or full SHA on the remote. List remote refs with `git ls-remote <url>`.",
                r = ref_name
            ),
        });
    }

    if let Some(rest) = captured.find("unknown revision or path not in the working tree").map(|_| captured) {
        let _ = rest;
        return Some(Interpreted {
            cause: "git could not resolve the requested revision".to_string(),
            fix: "Check that the `#<ref>` in your `neo.json` `git:` or `github:` dependency exists on the remote (use `git ls-remote <url>`).".to_string(),
        });
    }

    if captured.contains("Repository not found") {
        return Some(Interpreted {
            cause: "the git URL in `neo.json` points to a repository that does not exist or is private".to_string(),
            fix: "Check the spelling of the `git:<url>` / `github:<owner>/<repo>` entry in `neo.json`. If the repo is private, configure a credential helper (`git config --global credential.helper store`) and authenticate once.".to_string(),
        });
    }

    None
}

pub fn interpret_hurl(captured: &str) -> Option<Interpreted> {
    if captured.contains("Connection refused") || captured.contains("Failed to connect") {
        return Some(Interpreted {
            cause: "hurl could not reach the target server".to_string(),
            fix: "Start the server `neo` is testing against before running `neo test` (e.g. in another terminal: `neo run`), or change the host:port in your `.hurl` files to point at a running instance.".to_string(),
        });
    }
    None
}

// ---------------- helpers ----------------

fn extract_after<'a>(haystack: &'a str, needle: &str) -> Option<&'a str> {
    let start = haystack.find(needle)? + needle.len();
    Some(&haystack[start..])
}

fn strip_trailing_punct(s: &str) -> &str {
    let s = s.trim_start();
    let end = s
        .find(|c: char| c == '\n' || c == '\r' || c == ' ' || c == '\t' || c == ',' || c == ';' || c == '.' || c == ':')
        .unwrap_or(s.len());
    &s[..end]
}

fn first_token(s: &str) -> &str {
    let s = s.trim_start();
    let end = s
        .find(|c: char| c == '\n' || c == '\r' || c == ' ' || c == '\t')
        .unwrap_or(s.len());
    &s[..end]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cabal_unknown_package_matched() {
        let captured = "Resolving dependencies...\nError: unknown package: definitely-wrong-pkg\ncabal: Could not build the package.";
        let i = interpret_cabal(captured).expect("should match");
        assert!(i.cause.contains("package `definitely-wrong-pkg`"), "cause: {}", i.cause);
        assert!(i.fix.contains("hackage:definitely-wrong-pkg"), "fix: {}", i.fix);
        assert!(i.fix.contains("neo build"), "fix: {}", i.fix);
    }

    #[test]
    fn cabal_unknown_package_handles_trailing_newline() {
        let captured = "unknown package: foo\nmore output";
        let i = interpret_cabal(captured).unwrap();
        assert!(i.cause.contains("package `foo`"));
    }

    #[test]
    fn cabal_unknown_package_empty_name() {
        let captured = "unknown package: \n";
        let i = interpret_cabal(captured).unwrap();
        assert!(i.cause.contains("package ``"), "cause: {}", i.cause);
    }

    #[test]
    fn cabal_could_not_resolve_matched() {
        let captured = "cabal: Could not resolve dependencies:\n[__0] trying: foo-1.0\n";
        let i = interpret_cabal(captured).unwrap();
        assert!(i.cause.contains("version constraints in `neo.json` cannot be satisfied"));
        assert!(i.fix.contains("Loosen the `^`/`~` ranges"));
    }

    #[test]
    fn nix_attribute_missing_matched_single_quotes() {
        let captured = "error: attribute 'xyz' missing\n       at /nix/store/...";
        let i = interpret_nix(captured).expect("should match");
        assert!(i.cause.contains("`flake.nix` references attribute `xyz`"), "cause: {}", i.cause);
        assert!(i.fix.contains("rm flake.nix cabal.project *.cabal && neo build"), "fix: {}", i.fix);
    }

    #[test]
    fn nix_attribute_missing_matched_backticks() {
        let captured = "error: attribute `abc` missing";
        let i = interpret_nix(captured).expect("should match");
        assert!(i.cause.contains("`abc`"));
    }

    #[test]
    fn git_missing_ref_matched() {
        let captured = "fatal: couldn't find remote ref refs/heads/typo\n";
        let i = interpret_git(captured).expect("should match");
        assert!(i.cause.contains("ref `refs/heads/typo`"), "cause: {}", i.cause);
        assert!(i.fix.contains("`#refs/heads/typo`"), "fix: {}", i.fix);
    }

    #[test]
    fn git_repo_not_found_matched() {
        let i = interpret_git("ERROR: Repository not found.").expect("should match");
        assert!(i.cause.contains("does not exist or is private"));
    }

    #[test]
    fn hurl_connection_refused_matched() {
        let i = interpret_hurl("error: HTTP connection: Connection refused").expect("should match");
        assert!(i.cause.contains("could not reach the target server"));
        assert!(i.fix.contains("neo run"));
    }

    #[test]
    fn returns_none_on_unknown_pattern() {
        assert!(interpret_cabal("some unrelated error").is_none());
        assert!(interpret_nix("some unrelated error").is_none());
        assert!(interpret_git("some unrelated error").is_none());
        assert!(interpret_hurl("some unrelated error").is_none());
    }

    #[test]
    fn returns_none_on_empty() {
        assert!(interpret_cabal("").is_none());
        assert!(interpret_nix("").is_none());
        assert!(interpret_git("").is_none());
        assert!(interpret_hurl("").is_none());
        assert!(interpret_any("").is_none());
    }

    #[test]
    fn interpret_any_prefers_cabal() {
        let captured = "unknown package: foo\nattribute 'bar' missing\ncouldn't find remote ref baz";
        let i = interpret_any(captured).unwrap();
        assert!(i.cause.contains("package `foo`"), "cabal should win, got: {}", i.cause);
    }

    #[test]
    fn unicode_ref_preserved_verbatim() {
        let captured = "fatal: couldn't find remote ref refs/heads/feature/日本語\n";
        let i = interpret_git(captured).unwrap();
        assert!(i.cause.contains("refs/heads/feature/日本語"), "cause: {}", i.cause);
    }
}
