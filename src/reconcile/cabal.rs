use minijinja::{context, Environment};
use std::fs;
use crate::reconcile::resolve::{ResolvedConfig, DependencySource};
use crate::errors::NeoError;

use std::path::Path;

pub fn generate<P: AsRef<Path>>(
    project_dir: P,
    env: &Environment,
    config: &ResolvedConfig,
    modules: &[String],
) -> miette::Result<()> {
    let template = env.get_template("project.cabal")
        .map_err(|e| NeoError::TemplateError(e.to_string()))?;
    
    let dependencies: Vec<(String, String)> = config.dependencies.iter().map(|dep| {
        let version = match &dep.source {
            DependencySource::Hackage(v) => v.clone(),
            _ => ">= 0".to_string(), // For git/file, we just need a valid constraint
        };
        (dep.name.clone(), version)
    }).collect();

    let rendered = template.render(context! {
        name => config.name,
        version => config.version,
        description => config.description,
        license => config.license,
        author => config.author,
        modules => modules,
        dependencies => dependencies,
    }).map_err(|e| NeoError::TemplateError(e.to_string()))?;

    let filename = format!("{}.cabal", config.name);
    fs::write(project_dir.as_ref().join(filename), rendered).map_err(NeoError::IoError)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use crate::reconcile::resolve::{ResolvedConfig, ResolvedDependency, DependencySource};

    #[test]
    fn test_generate_cabal_with_dependencies() {
        let dir = tempdir().unwrap();

        let mut env = Environment::new();
        env.add_template("project.cabal", "name: {{name}}\n{% for dep, ver in dependencies %}{{dep}} {{ver}}\n{% endfor %}").unwrap();

        let config = ResolvedConfig {
            name: "test-deps".to_string(),
            version: "0.1.0".to_string(),
            neo_version: "main".to_string(),
            neo_sha: "abc".to_string(),
            description: None,
            author: None,
            license: "MIT".to_string(),
            dependencies: vec![
                ResolvedDependency {
                    name: "base".to_string(),
                    source: DependencySource::Hackage(">= 4.14".to_string()),
                },
                ResolvedDependency {
                    name: "relude".to_string(),
                    source: DependencySource::Hackage(">= 1.0".to_string()),
                },
            ],
        };

        generate(dir.path(), &env, &config, &[]).unwrap();
        
        let content = fs::read_to_string(dir.path().join("test-deps.cabal")).unwrap();
        assert!(content.contains("base >= 4.14"));
        assert!(content.contains("relude >= 1.0"));
    }

    #[test]
    fn test_generate_cabal_with_modules() {
        let dir = tempdir().unwrap();

        let mut env = Environment::new();
        env.add_template("project.cabal", "name: {{name}}\nexposed-modules: {% for mod in modules %}{{mod}}{% if not loop.last %}, {% endif %}{% endfor %}").unwrap();

        let config = ResolvedConfig {
            name: "test-modules".to_string(),
            version: "0.1.0".to_string(),
            neo_version: "main".to_string(),
            neo_sha: "abc".to_string(),
            description: None,
            author: None,
            license: "MIT".to_string(),
            dependencies: vec![],
        };

        generate(dir.path(), &env, &config, &["Lib".to_string(), "App.Server".to_string()]).unwrap();
        
        let content = fs::read_to_string(dir.path().join("test-modules.cabal")).unwrap();
        assert!(content.contains("exposed-modules: Lib, App.Server"));
    }
}
