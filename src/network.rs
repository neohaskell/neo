use std::collections::HashMap;
use std::path::Path;
use miette::IntoDiagnostic;
use serde::Deserialize;
use semver::Version;
use crate::errors::NeoError;

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
        .map_err(|e| NeoError::GitError(format!("Failed to run git ls-remote: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(NeoError::GitError(format!("git ls-remote failed: {}", stderr)).into());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let sha = stdout
        .split_whitespace()
        .next()
        .ok_or_else(|| NeoError::GitError(format!("No SHA found for version {}", version)))?;

    Ok(sha.to_string())
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct NeoPackages {
    pub packages: HashMap<String, NeoPackageMetadata>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct NeoPackageMetadata {
    pub description: String,
    pub repository: String,
    pub versions: HashMap<String, NeoPackageVersion>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct NeoPackageVersion {
    pub sha: String,
    pub tag: String,
}

#[allow(dead_code)]
pub async fn fetch_package_registry() -> miette::Result<NeoPackages> {
    if std::env::var("NEO_SKIP_NETWORK").is_ok() {
        return Ok(NeoPackages {
            packages: HashMap::new(),
        });
    }

    let client = reqwest::Client::builder()
        .user_agent("NeoCLI")
        .build()
        .map_err(NeoError::NetworkError)?;

    let url = "https://raw.githubusercontent.com/NeoHaskell/packages/main/registry.json";
    let response = client.get(url).send().await.map_err(NeoError::NetworkError)?;
    
    if !response.status().is_success() {
        return Ok(NeoPackages { packages: HashMap::new() });
    }

    let registry: NeoPackages = response.json().await.into_diagnostic()?;
    Ok(registry)
}

pub async fn check_for_updates() -> miette::Result<Option<String>> {
    if std::env::var("NEO_SKIP_NETWORK").is_ok() {
        return Ok(None);
    }

    let client = reqwest::Client::builder()
        .user_agent("NeoCLI")
        .build()
        .map_err(NeoError::NetworkError)?;

    let url = "https://api.github.com/repos/NeoHaskell/neocli/releases/latest";
    let response = client.get(url).send().await.map_err(NeoError::NetworkError)?;
    if !response.status().is_success() {
        return Ok(None);
    }

    let release: GitHubRelease = response.json().await.into_diagnostic()?;
    let latest_version_str = release.tag_name.trim_start_matches('v');
    let latest_version = Version::parse(latest_version_str).into_diagnostic()?;
    
    let current_version = Version::parse(env!("CARGO_PKG_VERSION")).into_diagnostic()?;

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
        std::fs::create_dir_all(&src_dir).into_diagnostic()?;
        std::fs::write(
            src_dir.join("App.hs"),
            "module App where\n\nrun :: IO ()\nrun = putStrLn \"Hello from NeoHaskell!\"\n",
        )
        .into_diagnostic()?;

        let launcher_dir = dest.join("launcher");
        std::fs::create_dir_all(&launcher_dir).into_diagnostic()?;
        std::fs::write(
            launcher_dir.join("Launcher.hs"),
            "module Main where\n\nimport App\n\nmain :: IO ()\nmain = App.run\n",
        )
        .into_diagnostic()?;

        return Ok(());
    }

    let url = "https://github.com/NeoHaskell/neo-starter/archive/refs/heads/main.tar.gz";
    
    let client = reqwest::Client::builder()
        .user_agent("NeoCLI")
        .build()
        .map_err(NeoError::NetworkError)?;

    let response = client.get(url).send().await.map_err(NeoError::NetworkError)?;
    let bytes = response.bytes().await.into_diagnostic()?;
    
    let tar_gz = flate2::read::GzDecoder::new(&bytes[..]);
    let mut archive = tar::Archive::new(tar_gz);
    
    // The tarball has a top-level directory like "neo-starter-main/"
    // We want to extract its contents into dest
    let temp_dir = tempfile::tempdir().into_diagnostic()?;
    archive.unpack(temp_dir.path()).into_diagnostic()?;
    
    // Move contents from temp_dir/neo-starter-main/* to dest/
    let entries = std::fs::read_dir(temp_dir.path()).into_diagnostic()?;
    let first_entry = entries.into_iter().next().ok_or_else(|| miette::miette!("Empty tarball"))?.into_diagnostic()?;
    let root_path = first_entry.path();
    
    for entry in std::fs::read_dir(root_path).into_diagnostic()? {
        let entry = entry.into_diagnostic()?;
        let file_name = entry.file_name();
        let dest_path = dest.join(file_name);
        std::fs::rename(entry.path(), dest_path).into_diagnostic()?;
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
}
