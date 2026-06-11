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
        .map_err(|e| NeoError::TemplateError { template: "project.cabal".to_string(), reason: e.to_string() })?;
    
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
    }).map_err(|e| NeoError::TemplateError { template: "project.cabal".to_string(), reason: e.to_string() })?;

    let filename = format!("{}.cabal", config.name);
    let out_path = project_dir.as_ref().join(filename);
    fs::write(&out_path, rendered).map_err(|e| NeoError::io_at("writing generated cabal file at", &out_path, e))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use crate::reconcile::resolve::{ResolvedConfig, ResolvedDependency, DependencySource};

    fn rc(deps: Vec<ResolvedDependency>) -> ResolvedConfig {
        ResolvedConfig {
            name: "p".to_string(),
            version: "0.1.0".to_string(),
            neo_version: "main".to_string(),
            neo_sha: "abc".to_string(),
            description: None,
            author: None,
            license: "MIT".to_string(),
            dependencies: deps,
        }
    }

    fn dep_env() -> Environment<'static> {
        let mut env = Environment::new();
        env.add_template(
            "project.cabal",
            "name: {{name}}\nbuild-depends: base\n{% for dep, ver in dependencies %}    , {{dep}} {{ver}}\n{% endfor %}",
        ).unwrap();
        env
    }

    #[test]
    fn cabal_emits_hackage_dep_with_constraint() {
        let dir = tempdir().unwrap();
        let env = dep_env();
        let mut config = rc(vec![
            ResolvedDependency {
                name: "aeson".to_string(),
                source: DependencySource::Hackage(">=2.0 && <3.0".to_string()),
            },
        ]);
        config.name = "test-deps".to_string();
        generate(dir.path(), &env, &config, &[]).unwrap();
        let content = fs::read_to_string(dir.path().join("test-deps.cabal")).unwrap();
        assert!(content.contains(", aeson >=2.0 && <3.0"), "got: {}", content);
    }

    #[test]
    fn cabal_emits_hackage_dep_empty_constraint() {
        let dir = tempdir().unwrap();
        let env = dep_env();
        let mut config = rc(vec![
            ResolvedDependency {
                name: "base".to_string(),
                source: DependencySource::Hackage(String::new()),
            },
        ]);
        config.name = "test-empty".to_string();
        generate(dir.path(), &env, &config, &[]).unwrap();
        let content = fs::read_to_string(dir.path().join("test-empty.cabal")).unwrap();
        // Empty version: line ends with the name and a trailing space (no constraint).
        assert!(content.contains(", base "), "got: {}", content);
    }

    #[test]
    fn cabal_emits_or_range_parenthesized() {
        let dir = tempdir().unwrap();
        let env = dep_env();
        let mut config = rc(vec![
            ResolvedDependency {
                name: "foo".to_string(),
                source: DependencySource::Hackage("(>=1.0.0 && <2.0.0) || (>=3.0.0)".to_string()),
            },
        ]);
        config.name = "test-or".to_string();
        generate(dir.path(), &env, &config, &[]).unwrap();
        let content = fs::read_to_string(dir.path().join("test-or.cabal")).unwrap();
        assert!(content.contains("(>=1.0.0 && <2.0.0) || (>=3.0.0)"), "got: {}", content);
    }

    #[test]
    fn cabal_omits_git_deps_from_build_depends_constraint() {
        let dir = tempdir().unwrap();
        let env = dep_env();
        let mut config = rc(vec![
            ResolvedDependency {
                name: "my-git-pkg".to_string(),
                source: DependencySource::Git {
                    url: "https://github.com/me/g".to_string(),
                    rev: "main".to_string(),
                },
            },
            ResolvedDependency {
                name: "my-file-pkg".to_string(),
                source: DependencySource::File("../local".to_string()),
            },
        ]);
        config.name = "test-non-hackage".to_string();
        generate(dir.path(), &env, &config, &[]).unwrap();
        let content = fs::read_to_string(dir.path().join("test-non-hackage.cabal")).unwrap();
        // Both packages appear with a placeholder constraint, not as Hackage versions.
        assert!(content.contains(", my-git-pkg >= 0"), "got: {}", content);
        assert!(content.contains(", my-file-pkg >= 0"), "got: {}", content);
    }

    #[test]
    fn cabal_no_dependencies() {
        let dir = tempdir().unwrap();
        let env = dep_env();
        let mut config = rc(vec![]);
        config.name = "test-empty-deps".to_string();
        generate(dir.path(), &env, &config, &[]).unwrap();
        let content = fs::read_to_string(dir.path().join("test-empty-deps.cabal")).unwrap();
        assert!(content.contains("build-depends: base"));
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
