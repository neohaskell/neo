use minijinja::{context, Environment};
use std::fs;
use crate::reconcile::resolve::{ResolvedConfig, DependencySource};
use crate::errors::NeoError;

use std::path::Path;

pub fn generate<P: AsRef<Path>>(
    project_dir: P,
    env: &Environment,
    config: &ResolvedConfig,
) -> miette::Result<()> {
    let template = env.get_template("cabal.project")
        .map_err(|e| NeoError::TemplateError(e.to_string()))?;

    let mut git_dependencies = Vec::new();
    let mut file_dependencies = Vec::new();

    for dep in &config.dependencies {
        match &dep.source {
            DependencySource::Git { url, rev } => {
                git_dependencies.push(context! { url => url, rev => rev });
            }
            DependencySource::File(path) => {
                file_dependencies.push(context! { path => path });
            }
            _ => {}
        }
    }

    let rendered = template.render(context! {
        git_dependencies => git_dependencies,
        file_dependencies => file_dependencies,
        neo_sha => config.neo_sha,
    }).map_err(|e| NeoError::TemplateError(e.to_string()))?;

    fs::write(project_dir.as_ref().join("cabal.project"), rendered).map_err(NeoError::IoError)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use crate::reconcile::resolve::{ResolvedConfig, ResolvedDependency, DependencySource};

    #[test]
    fn test_generate_cabal_project() {
        let dir = tempdir().unwrap();

        let mut env = Environment::new();
        env.add_template("cabal.project", "packages: .\ngit: {{ git_dependencies | length }}\nfile: {{ file_dependencies | length }}\nsha: {{ neo_sha }}").unwrap();

        let config = ResolvedConfig {
            name: "test-project".to_string(),
            version: "0.1.0".to_string(),
            neo_version: "0.1.0".to_string(),
            neo_sha: "abc1234".to_string(),
            description: None,
            author: None,
            license: "MIT".to_string(),
            dependencies: vec![
                ResolvedDependency {
                    name: "my-git-dep".to_string(),
                    source: DependencySource::Git { url: "https://github.com/user/repo".to_string(), rev: "main".to_string() },
                },
                ResolvedDependency {
                    name: "my-file-dep".to_string(),
                    source: DependencySource::File("../local-pkg".to_string()),
                },
            ],
        };

        generate(dir.path(), &env, &config).unwrap();
        
        let content = fs::read_to_string(dir.path().join("cabal.project")).unwrap();
        assert!(content.contains("packages: ."));
        assert!(content.contains("git: 1"));
        assert!(content.contains("file: 1"));
        assert!(content.contains("sha: abc1234"));
    }
}
