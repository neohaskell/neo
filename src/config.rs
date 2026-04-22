use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use crate::errors::NeoError;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct NeoConfig {
    pub name: String,
    pub version: String,
    pub neo_version: String,
    pub description: Option<String>,
    pub author: Option<String>,
    #[serde(default = "default_license")]
    pub license: String,
    #[serde(default)]
    pub dependencies: HashMap<String, String>,
}

fn default_license() -> String {
    "Apache-2.0".to_string()
}

impl NeoConfig {
    pub fn load<P: AsRef<Path>>(path: P) -> miette::Result<Self> {
        let path = path.as_ref();
        if !path.exists() {
            return Err(NeoError::NoWorkspace.into());
        }

        let content = std::fs::read_to_string(path).map_err(NeoError::IoError)?;
        
        serde_json::from_str(&content).map_err(|e| {
            NeoError::InvalidConfig {
                line: e.line(),
                col: e.column(),
                reason: e.to_string(),
            }.into()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_load_valid_config() {
        let mut file = NamedTempFile::new().unwrap();
        let json = r#"{
            "name": "test-project",
            "version": "0.1.0",
            "neo-version": "0.1.0",
            "description": "A test project",
            "author": "NeoHaskell Team",
            "license": "MIT",
            "dependencies": {
                "base": ">= 4.17",
                "aeson": "^>= 2.1"
            }
        }"#;
        file.write_all(json.as_bytes()).unwrap();

        let config = NeoConfig::load(file.path()).unwrap();
        assert_eq!(config.name, "test-project");
        assert_eq!(config.version, "0.1.0");
        assert_eq!(config.neo_version, "0.1.0");
        assert_eq!(config.description, Some("A test project".to_string()));
        assert_eq!(config.author, Some("NeoHaskell Team".to_string()));
        assert_eq!(config.license, "MIT");
        assert_eq!(config.dependencies.len(), 2);
        assert_eq!(config.dependencies.get("base").unwrap(), ">= 4.17");
        assert_eq!(config.dependencies.get("aeson").unwrap(), "^>= 2.1");
    }

    #[test]
    fn test_load_default_license() {
        let mut file = NamedTempFile::new().unwrap();
        let json = r#"{
            "name": "test-project",
            "version": "0.1.0",
            "neo-version": "0.1.0"
        }"#;
        file.write_all(json.as_bytes()).unwrap();

        let config = NeoConfig::load(file.path()).unwrap();
        assert_eq!(config.license, "Apache-2.0");
        assert!(config.dependencies.is_empty());
    }

    #[test]
    fn test_load_ignore_unknown_fields() {
        let mut file = NamedTempFile::new().unwrap();
        let json = r#"{
            "name": "test-project",
            "version": "0.1.0",
            "neo-version": "0.1.0",
            "unknown_field": "some value"
        }"#;
        file.write_all(json.as_bytes()).unwrap();

        let config = NeoConfig::load(file.path()).unwrap();
        assert_eq!(config.name, "test-project");
    }

    #[test]
    fn test_load_missing_file() {
        let result = NeoConfig::load("non_existent.json");
        assert!(result.is_err());
    }

    #[test]
    fn test_load_invalid_json() {
        let mut file = NamedTempFile::new().unwrap();
        let json = r#"{ "name": "test-project", "version": "#;
        file.write_all(json.as_bytes()).unwrap();

        let result = NeoConfig::load(file.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_load_missing_fields() {
        let mut file = NamedTempFile::new().unwrap();
        let json = r#"{ "name": "test-project" }"#;
        file.write_all(json.as_bytes()).unwrap();

        let result = NeoConfig::load(file.path());
        assert!(result.is_err());
    }
}
