use std::path::Path;
use std::process::Command;
use crate::errors::NeoError;

pub fn init(path: &Path) -> miette::Result<()> {
    let output = Command::new("git")
        .arg("init")
        .current_dir(path)
        .output()
        .map_err(|e| NeoError::GitError(format!("Failed to execute git init: {}", e)))?;
        
    if !output.status.success() {
        return Err(NeoError::GitError(String::from_utf8_lossy(&output.stderr).to_string()).into());
    }
    
    // Configure local user for tests/CI if not set
    let _ = Command::new("git").args(["config", "user.email", "neo@example.com"]).current_dir(path).output();
    let _ = Command::new("git").args(["config", "user.name", "NeoCLI"]).current_dir(path).output();
    
    Ok(())
}

pub fn add_all(path: &Path) -> miette::Result<()> {
    let output = Command::new("git")
        .arg("add")
        .arg(".")
        .current_dir(path)
        .output()
        .map_err(|e| NeoError::GitError(format!("Failed to execute git add: {}", e)))?;
        
    if !output.status.success() {
        return Err(NeoError::GitError(String::from_utf8_lossy(&output.stderr).to_string()).into());
    }
    
    Ok(())
}

pub fn commit(path: &Path, message: &str) -> miette::Result<()> {
    let output = Command::new("git")
        .arg("commit")
        .arg("--no-verify")
        .arg("-m")
        .arg(message)
        .current_dir(path)
        .output()
        .map_err(|e| NeoError::GitError(format!("Failed to execute git commit: {}", e)))?;
        
    if !output.status.success() {
        return Err(NeoError::GitError(String::from_utf8_lossy(&output.stderr).to_string()).into());
    }
    
    Ok(())
}

pub fn install_lock_hook(path: &Path) -> miette::Result<()> {
    let hooks_dir = path.join(".git").join("hooks");
    std::fs::create_dir_all(&hooks_dir).map_err(NeoError::IoError)?;
    
    let hook_path = hooks_dir.join("pre-commit");
    let hook_content = r#"#!/bin/sh
# NeoHaskell Lock Hook
# This hook prevents committing changes to locked files.
neo lock check
"#;
    
    std::fs::write(&hook_path, hook_content).map_err(NeoError::IoError)?;
    
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&hook_path).map_err(NeoError::IoError)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&hook_path, perms).map_err(NeoError::IoError)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_git_init() {
        let dir = tempdir().unwrap();
        init(dir.path()).expect("Failed to init git");
        assert!(dir.path().join(".git").exists());
    }

    #[test]
    fn test_install_lock_hook() {
        let dir = tempdir().unwrap();
        // Create .git dir first as install_lock_hook expects it (though it creates hooks dir)
        std::fs::create_dir_all(dir.path().join(".git")).unwrap();
        
        install_lock_hook(dir.path()).expect("Failed to install lock hook");
        let hook_path = dir.path().join(".git/hooks/pre-commit");
        assert!(hook_path.exists());
        
        let content = std::fs::read_to_string(hook_path).unwrap();
        assert!(content.contains("neo lock check"));
        
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let metadata = std::fs::metadata(dir.path().join(".git/hooks/pre-commit")).unwrap();
            assert!(metadata.permissions().mode() & 0o111 != 0);
        }
    }
}
