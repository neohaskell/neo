use minijinja::Environment;
use crate::config::NeoConfig;
use crate::errors::NeoError;

pub mod cabal;
pub mod cabal_project;
pub mod flake;
pub mod modules;
pub mod resolve;

use std::path::Path;

pub async fn run<P: AsRef<Path>>(project_dir: P, config: &NeoConfig) -> miette::Result<()> {
    let project_dir = project_dir.as_ref();
    let mut env = Environment::new();

    env.add_template(
        "project.cabal",
        include_str!("../../assets/templates/project.cabal.j2"),
    )
    .map_err(|e| NeoError::TemplateError(e.to_string()))?;
    env.add_template(
        "flake.nix",
        include_str!("../../assets/templates/flake.nix.j2"),
    )
    .map_err(|e| NeoError::TemplateError(e.to_string()))?;
    env.add_template(
        "cabal.project",
        include_str!("../../assets/templates/cabal.project.j2"),
    )
    .map_err(|e| NeoError::TemplateError(e.to_string()))?;

    let resolved = resolve::resolve(config).await?;
    let modules = modules::discover(project_dir.join("src"));

    cabal::generate(project_dir, &env, &resolved, &modules)?;
    flake::generate(project_dir, &env, &resolved)?;
    cabal_project::generate(project_dir, &env, &resolved)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;
    use crate::config::NeoConfig;

    #[tokio::test]
    async fn test_reconcile_full() {
        let dir = tempdir().unwrap();
        let project_dir = dir.path();
        
        // Create a src directory
        fs::create_dir_all(project_dir.join("src")).unwrap();
        fs::write(project_dir.join("src/Main.hs"), "").unwrap();
        fs::write(project_dir.join("src/Lib.hs"), "").unwrap();

        let config = NeoConfig {
            name: "my-project".to_string(),
            version: "0.1.0".to_string(),
            neo_version: "main".to_string(),
            description: Some("A test project".to_string()),
            author: Some("Neo".to_string()),
            license: "Apache-2.0".to_string(),
            dependencies: [("text".to_string(), "^>=2.0".to_string())].into_iter().collect(),
        };

        unsafe { std::env::set_var("NEO_SKIP_NETWORK", "1"); }
        run(project_dir, &config).await.unwrap();

        assert!(project_dir.join("my-project.cabal").exists());
        assert!(project_dir.join("flake.nix").exists());
        assert!(project_dir.join("cabal.project").exists());

        let cabal_content = fs::read_to_string(project_dir.join("my-project.cabal")).unwrap();
        assert!(cabal_content.contains("name:               my-project"));
        assert!(cabal_content.contains("Lib"));
        assert!(cabal_content.contains("text ^>=2.0"));

        let flake_content = fs::read_to_string(project_dir.join("flake.nix")).unwrap();
        assert!(flake_content.contains("description = \"A test project\""));
    }
}
