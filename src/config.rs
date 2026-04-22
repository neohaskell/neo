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
