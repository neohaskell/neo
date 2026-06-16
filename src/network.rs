use std::collections::HashMap;
use std::path::Path;
use miette::IntoDiagnostic;
use miette::WrapErr;
use serde::Deserialize;
use semver::Version;
use crate::errors::NeoError;
use crate::interpret::{self, Kind};

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
}



pub async fn fetch_neo_sha(version: &str) -> miette::Result<String> {
    if std::env::var("NEO_SKIP_NETWORK").is_ok() {
        return Ok("deadbeef".to_string());
    }

    let target = if version == "latest" || version == "main" {
        "main"
    } else {
        version
    };

    let output = tokio::process::Command::new("git")
        .args(["ls-remote", "https://github.com/NeoHaskell/neohaskell", target])
        .output()
        .await
        .map_err(|e| NeoError::SubprocessFailed {
            operation: format!("spawning `git ls-remote https://github.com/NeoHaskell/neohaskell {}`", target),
            cause: format!("could not run git: {}", e),
            fix: "Ensure `git` is installed and on PATH (`which git`). If installed, open a new shell.".to_string(),
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        if let Some(i) = interpret::match_kind(Kind::Git, &stderr) {
            return Err(NeoError::SubprocessFailed {
                operation: format!("`git ls-remote https://github.com/NeoHaskell/neohaskell {}`", target),
                cause: i.cause,
                fix: i.fix,
            }.into());
        }
        return Err(NeoError::SubprocessFailed {
            operation: format!("`git ls-remote https://github.com/NeoHaskell/neohaskell {}`", target),
            cause: format!("git exited non-zero: {}", stderr.trim()),
            fix: "Check that https://github.com/NeoHaskell/neohaskell is reachable (try `curl -I https://github.com/NeoHaskell/neohaskell`). If you are behind a proxy or offline, set `NEO_SKIP_NETWORK=1` to skip the lookup (uses a placeholder SHA — fine for tests, not for real builds).".to_string(),
        }.into());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let sha = stdout
        .split_whitespace()
        .next()
        .ok_or_else(|| NeoError::SubprocessFailed {
            operation: format!("resolving NeoHaskell version `{}` against https://github.com/NeoHaskell/neohaskell", version),
            cause: format!("`git ls-remote` returned no SHA for ref `{}`", target),
            fix: format!("Edit `neo.json` or your `neo` invocation: the NeoHaskell version `{}` does not resolve to any branch, tag, or SHA on https://github.com/NeoHaskell/neohaskell. Use `main`, `latest`, or a published tag (list them with `git ls-remote https://github.com/NeoHaskell/neohaskell`).", version),
        })?;

    Ok(sha.to_string())
}

#[derive(Debug, Clone, Deserialize)]
pub struct NeoPackages {
    pub packages: HashMap<String, NeoPackageMetadata>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NeoPackageMetadata {
    #[allow(dead_code)]
    pub description: String,
    pub repository: String,
}

/// A git tag discovered on a NeoPackages-listed repository, together with the
/// commit SHA it resolves to. Versions in NeoPackages come from upstream git
/// tags (enumerated via `git ls-remote --tags`), not from the registry JSON.
#[derive(Debug, Clone)]
pub struct PackageTag {
    pub tag: String,
    pub sha: String,
}

pub async fn fetch_package_registry() -> miette::Result<NeoPackages> {
    if std::env::var("NEO_SKIP_NETWORK").is_ok() {
        return Ok(NeoPackages {
            packages: HashMap::new(),
        });
    }

    let url = "https://raw.githubusercontent.com/NeoHaskell/neopackages/main/registry.json";

    let client = reqwest::Client::builder()
        .user_agent("NeoCLI")
        .build()
        .map_err(|e| NeoError::NetworkError { url: url.to_string(), source: e })?;

    let response = client.get(url).send().await
        .map_err(|e| NeoError::NetworkError { url: url.to_string(), source: e })?;

    if !response.status().is_success() {
        return Ok(NeoPackages { packages: HashMap::new() });
    }

    let registry: NeoPackages = response.json().await
        .into_diagnostic()
        .wrap_err_with(|| format!(
            "parsing the NeoPackages registry JSON downloaded from `{}`. \
             Expected: an object `{{ \"packages\": {{ \"<name>\": {{ \"description\": \"...\", \"repository\": \"https://...\" }}, ... }} }}` — versions come from the upstream git tags, not the registry. \
             Fix: if you are pointing at a non-default registry URL, verify it serves valid JSON in this shape. \
             If you maintain the registry, validate it with `check-jsonschema --schemafile registry.schema.json registry.json`. \
             Otherwise re-run with `NEO_SKIP_NETWORK=1` to skip registry resolution.",
            url
        ))?;
    Ok(registry)
}

/// Run `git ls-remote --tags <repo_url>` and return one `PackageTag` per
/// upstream tag. Annotated tags (with a `^{}` peeled line) resolve to the
/// peeled commit SHA, never the tag-object SHA.
pub async fn fetch_package_tags(repo_url: &str) -> miette::Result<Vec<PackageTag>> {
    if std::env::var("NEO_SKIP_NETWORK").is_ok() {
        return Ok(Vec::new());
    }

    let output = tokio::process::Command::new("git")
        .args(["ls-remote", "--tags", repo_url])
        .output()
        .await
        .map_err(|e| NeoError::SubprocessFailed {
            operation: format!("spawning `git ls-remote --tags {}` to enumerate versions", repo_url),
            cause: format!("could not run git: {}", e),
            fix: "Ensure `git` is installed and on PATH (`which git`). If installed, open a new shell.".to_string(),
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        if let Some(i) = interpret::match_kind(Kind::Git, &stderr) {
            return Err(NeoError::SubprocessFailed {
                operation: format!("`git ls-remote --tags {}`", repo_url),
                cause: i.cause,
                fix: i.fix,
            }.into());
        }
        return Err(NeoError::SubprocessFailed {
            operation: format!("`git ls-remote --tags {}`", repo_url),
            cause: format!("git exited non-zero: {}", stderr.trim()),
            fix: format!("Check that `{}` is reachable and is a valid git repository (try `git ls-remote --tags {}` yourself). If the registry points at the wrong repo, fix it in `registry.json` (https://github.com/NeoHaskell/neopackages).", repo_url, repo_url),
        }.into());
    }

    Ok(parse_ls_remote_tags(&String::from_utf8_lossy(&output.stdout)))
}

fn parse_ls_remote_tags(stdout: &str) -> Vec<PackageTag> {
    // `git ls-remote --tags` lines look like:
    //   <sha>\trefs/tags/<tag>
    //   <sha>\trefs/tags/<tag>^{}     (peeled: annotated-tag → commit)
    // For an annotated tag the un-peeled line is the tag object's SHA and the
    // peeled line is the commit's SHA. We always want the commit, so the peeled
    // value wins when both are present.
    let mut by_tag: std::collections::BTreeMap<String, String> = std::collections::BTreeMap::new();
    for line in stdout.lines() {
        let Some((sha, refname)) = line.split_once('\t') else { continue };
        let Some(rest) = refname.strip_prefix("refs/tags/") else { continue };
        let (tag, peeled) = match rest.strip_suffix("^{}") {
            Some(t) => (t, true),
            None => (rest, false),
        };
        if tag.is_empty() {
            continue;
        }
        if peeled || !by_tag.contains_key(tag) {
            by_tag.insert(tag.to_string(), sha.to_string());
        }
    }
    by_tag
        .into_iter()
        .map(|(tag, sha)| PackageTag { tag, sha })
        .collect()
}

pub async fn check_for_updates() -> miette::Result<Option<String>> {
    if std::env::var("NEO_SKIP_NETWORK").is_ok() {
        return Ok(None);
    }

    let url = "https://api.github.com/repos/NeoHaskell/neocli/releases/latest";

    let client = reqwest::Client::builder()
        .user_agent("NeoCLI")
        .build()
        .map_err(|e| NeoError::NetworkError { url: url.to_string(), source: e })?;

    let response = client.get(url).send().await
        .map_err(|e| NeoError::NetworkError { url: url.to_string(), source: e })?;
    if !response.status().is_success() {
        return Ok(None);
    }

    let release: GitHubRelease = response.json().await
        .into_diagnostic()
        .wrap_err_with(|| format!("parsing the GitHub Releases JSON response from `{}` while checking for `neo` updates", url))?;
    let latest_version_str = release.tag_name.trim_start_matches('v');
    let latest_version = Version::parse(latest_version_str)
        .into_diagnostic()
        .wrap_err_with(|| format!("parsing the latest `neo` release tag `{}` from GitHub as semver", release.tag_name))?;

    let current_version = Version::parse(env!("CARGO_PKG_VERSION"))
        .into_diagnostic()
        .wrap_err_with(|| format!("parsing the built-in `neo` version `{}` as semver (this is a build-time bug)", env!("CARGO_PKG_VERSION")))?;

    if latest_version > current_version {
        Ok(Some(latest_version.to_string()))
    } else {
        Ok(None)
    }
}

pub async fn fetch_starter_template(dest: &Path) -> miette::Result<()> {
    if std::env::var("NEO_SKIP_NETWORK").is_ok() {
        // Create a dummy structure for tests
        let src_dir = dest.join("src");
        std::fs::create_dir_all(&src_dir)
            .map_err(|e| NeoError::io_at("creating offline-stub `src/` directory at", &src_dir, e))?;
        let stub_app = src_dir.join("App.hs");
        std::fs::write(
            &stub_app,
            "module App where\n\nrun :: IO ()\nrun = putStrLn \"Hello from NeoHaskell!\"\n",
        )
        .map_err(|e| NeoError::io_at("writing offline-stub `src/App.hs` to", &stub_app, e))?;

        let launcher_dir = dest.join("launcher");
        std::fs::create_dir_all(&launcher_dir)
            .map_err(|e| NeoError::io_at("creating offline-stub `launcher/` directory at", &launcher_dir, e))?;
        let stub_launcher = launcher_dir.join("Launcher.hs");
        std::fs::write(
            &stub_launcher,
            "module Main where\n\nimport App\n\nmain :: IO ()\nmain = App.run\n",
        )
        .map_err(|e| NeoError::io_at("writing offline-stub `launcher/Launcher.hs` to", &stub_launcher, e))?;

        return Ok(());
    }

    let url = "https://github.com/NeoHaskell/neo-starter/archive/refs/heads/main.tar.gz";

    let client = reqwest::Client::builder()
        .user_agent("NeoCLI")
        .build()
        .map_err(|e| NeoError::NetworkError { url: url.to_string(), source: e })?;

    let response = client.get(url).send().await
        .map_err(|e| NeoError::NetworkError { url: url.to_string(), source: e })?;
    let bytes = response.bytes().await
        .map_err(|e| NeoError::NetworkError { url: url.to_string(), source: e })?;

    let tar_gz = flate2::read::GzDecoder::new(&bytes[..]);
    let mut archive = tar::Archive::new(tar_gz);

    // The tarball has a top-level directory like "neo-starter-main/"
    // We want to extract its contents into dest
    let temp_dir = tempfile::tempdir()
        .map_err(|e| NeoError::io_at("creating a temp dir to unpack the starter tarball into", &std::path::PathBuf::from(std::env::temp_dir()), e))?;
    archive.unpack(temp_dir.path())
        .map_err(|e| NeoError::io_at("unpacking the starter tarball into", &temp_dir.path().to_path_buf(), e))?;

    let entries = std::fs::read_dir(temp_dir.path())
        .map_err(|e| NeoError::io_at("listing the unpacked starter tarball at", &temp_dir.path().to_path_buf(), e))?;
    let first_entry = entries.into_iter().next().ok_or_else(|| miette::miette!(
        help = format!("Re-run with network connectivity (try `curl -I {}`), or set `NEO_SKIP_NETWORK=1` to use the offline stub starter (fine for tests, not for real builds).", url),
        "Downloaded starter template tarball from `{}` was empty after unpack into `{}` — no top-level directory found.",
        url,
        temp_dir.path().display()
    ))?
    .map_err(|e| NeoError::io_at("reading the first entry of the unpacked starter tarball at", &temp_dir.path().to_path_buf(), e))?;
    let root_path = first_entry.path();

    let inner_entries = std::fs::read_dir(&root_path)
        .map_err(|e| NeoError::io_at("listing files inside the unpacked starter at", &root_path, e))?;
    for entry in inner_entries {
        let entry = entry
            .map_err(|e| NeoError::io_at("reading a file entry inside the unpacked starter at", &root_path, e))?;
        let file_name = entry.file_name();
        let dest_path = dest.join(file_name);
        std::fs::rename(entry.path(), &dest_path)
            .map_err(|e| NeoError::io_at("moving an unpacked starter file into the project dir at", &dest_path, e))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_check_for_updates() {
        unsafe { std::env::set_var("NEO_SKIP_NETWORK", "1"); }
        let _ = check_for_updates().await;
    }

    #[tokio::test]
    async fn test_fetch_starter_template() {
        unsafe { std::env::set_var("NEO_SKIP_NETWORK", "1"); }
        let dir = tempdir().unwrap();
        fetch_starter_template(dir.path()).await.unwrap();
        assert!(dir.path().join("src/App.hs").exists());
        assert!(dir.path().join("launcher/Launcher.hs").exists());
    }

    #[test]
    fn parse_ls_remote_tags_lightweight() {
        let stdout = "abc1234567890abc1234567890abc1234567890a\trefs/tags/0.1.0\n\
                      def1234567890def1234567890def1234567890d\trefs/tags/v1.2.3\n";
        let tags = parse_ls_remote_tags(stdout);
        assert_eq!(tags.len(), 2);
        let by: std::collections::HashMap<_, _> =
            tags.iter().map(|t| (t.tag.clone(), t.sha.clone())).collect();
        assert_eq!(by.get("0.1.0").unwrap(), "abc1234567890abc1234567890abc1234567890a");
        assert_eq!(by.get("v1.2.3").unwrap(), "def1234567890def1234567890def1234567890d");
    }

    #[test]
    fn parse_ls_remote_tags_annotated_prefers_peeled() {
        // Annotated tag: first line is tag-object SHA, second line is the peeled commit SHA.
        let stdout = "tagobjsha000000000000000000000000000000000\trefs/tags/v2.0.0\n\
                      commitsha00000000000000000000000000000000\trefs/tags/v2.0.0^{}\n";
        let tags = parse_ls_remote_tags(stdout);
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].tag, "v2.0.0");
        assert_eq!(tags[0].sha, "commitsha00000000000000000000000000000000");
    }

    #[test]
    fn parse_ls_remote_tags_ignores_unrelated_refs() {
        let stdout = "abc1234567890abc1234567890abc1234567890a\trefs/heads/main\n\
                      def1234567890def1234567890def1234567890d\trefs/tags/0.1.0\n\
                      garbage-without-tab\n";
        let tags = parse_ls_remote_tags(stdout);
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].tag, "0.1.0");
    }

    #[test]
    fn parse_ls_remote_tags_empty() {
        assert!(parse_ls_remote_tags("").is_empty());
    }
}
