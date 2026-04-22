use serde::{Deserialize, Serialize};
use crate::config::NeoConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DependencySource {
    Hackage(String),
    Git { url: String, rev: String },
    File(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedDependency {
    pub name: String,
    pub source: DependencySource,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedConfig {
    pub name: String,
    pub version: String,
    pub neo_version: String,
    pub neo_sha: String,
    pub description: Option<String>,
    pub author: Option<String>,
    pub license: String,
    pub dependencies: Vec<ResolvedDependency>,
}

pub async fn resolve(config: &NeoConfig) -> miette::Result<ResolvedConfig> {
    // 1. Resolve neo-version to SHA
    let neo_sha = resolve_neo_sha(&config.neo_version).await?;

    // 2. Resolve dependencies
    let mut resolved_deps = Vec::new();
    for (name, version) in &config.dependencies {
        resolved_deps.push(resolve_dependency(name, version).await?);
    }

    Ok(ResolvedConfig {
        name: config.name.clone(),
        version: config.version.clone(),
        neo_version: config.neo_version.clone(),
        neo_sha,
        description: config.description.clone(),
        author: config.author.clone(),
        license: config.license.clone(),
        dependencies: resolved_deps,
    })
}

async fn resolve_neo_sha(version: &str) -> miette::Result<String> {
    if std::env::var("NEO_SKIP_NETWORK").is_ok() {
        return Ok("deadbeef".to_string());
    }

    // For now, let's assume "latest" or a version maps to a commit on NeoHaskell/neo
    // In a real scenario, we'd query GitHub API
    // Let's mock it for now as instructed by "mocked or pointing to a real URL"
    
    // Actually, the task says: "Fetch the latest git tag/commit SHA for NeoHaskell from GitHub."
    // I'll try to implement it in network.rs and call it here.
    crate::network::fetch_neo_sha(version).await
}

async fn resolve_dependency(name: &str, constraint: &str) -> miette::Result<ResolvedDependency> {
    let source = if constraint.starts_with("git+") {
        let url = constraint.trim_start_matches("git+").to_string();
        // Extract revision if present: git+https://github.com/user/repo@branch
        if let Some((url, rev)) = url.split_once('@') {
            DependencySource::Git { url: url.to_string(), rev: rev.to_string() }
        } else {
            DependencySource::Git { url, rev: "main".to_string() }
        }
    } else if constraint.starts_with("file:") {
        DependencySource::File(constraint.trim_start_matches("file:").to_string())
    } else if constraint.starts_with("hackage:") {
        DependencySource::Hackage(constraint.trim_start_matches("hackage:").to_string())
    } else {
        // Default to Hackage if no prefix
        DependencySource::Hackage(constraint.to_string())
    };

    Ok(ResolvedDependency {
        name: name.to_string(),
        source,
    })
}
