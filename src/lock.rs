use std::path::{Path, PathBuf};
use std::fs;
use miette::{IntoDiagnostic, Result};
use walkdir::WalkDir;
// Using nucleo-matcher types that are re-exported by nucleo or available in the crate
use nucleo::Config;
use nucleo::pattern::{Pattern, CaseMatching, Normalization, AtomKind};

pub const LOCK_MANIFEST: &str = ".locked-files";

pub struct LockManifest {
    pub path: PathBuf,
    pub locked_files: Vec<String>,
}

impl LockManifest {
    pub fn load() -> Result<Self> {
        Self::load_from(Path::new(LOCK_MANIFEST))
    }

    pub fn load_from(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self { 
                path: path.to_path_buf(),
                locked_files: Vec::new() 
            });
        }

        let content = fs::read_to_string(path).into_diagnostic()?;
        let locked_files = content
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect();

        Ok(Self { 
            path: path.to_path_buf(),
            locked_files 
        })
    }

    pub fn save(&self) -> Result<()> {
        let content = self.locked_files.join("\n");
        fs::write(&self.path, content).into_diagnostic()?;
        Ok(())
    }

    pub fn add(&mut self, file: String) {
        if !self.locked_files.contains(&file) {
            self.locked_files.push(file);
            self.locked_files.sort();
        }
    }

    pub fn is_locked(&self, path: &str) -> bool {
        self.locked_files.iter().any(|f| f == path)
    }
}

pub fn discover_domain_files() -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let src_dir = Path::new("src");
    
    if !src_dir.exists() {
        return Ok(files);
    }

    for entry in WalkDir::new(src_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path();
        let components: Vec<_> = path.components().collect();
        
        // Look for src/**/Commands/*.hs, src/**/Events/*.hs, src/**/Queries/*.hs
        let is_domain = components.iter().any(|c| {
            let s = c.as_os_str().to_string_lossy();
            s == "Commands" || s == "Events" || s == "Queries"
        });

        if is_domain {
            files.push(path.to_path_buf());
        }
    }

    Ok(files)
}

pub fn fuzzy_match(query: &str, candidates: &[PathBuf]) -> Vec<PathBuf> {
    if query.is_empty() {
        return candidates.to_vec();
    }

    let mut matcher = nucleo::Matcher::new(Config::DEFAULT);
    let pattern = Pattern::new(query, CaseMatching::Ignore, Normalization::Smart, AtomKind::Fuzzy);
    
    let candidate_strs: Vec<String> = candidates.iter().map(|p| p.to_string_lossy().to_string()).collect();
    
    let matches = pattern.match_list(candidate_strs.iter().map(|s| s.as_str()), &mut matcher);
    
    matches.into_iter()
        .map(|(s, _)| PathBuf::from(s))
        .collect()
}

pub fn get_staged_files() -> Result<Vec<String>> {
    let output = std::process::Command::new("git")
        .args(["diff", "--cached", "--name-only"])
        .output()
        .into_diagnostic()?;
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout.lines().map(|l| l.to_string()).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use crate::test_utils::tests::TEST_MUTEX;

    #[test]
    fn test_lock_manifest() {
        let dir = tempdir().unwrap();
        let manifest_path = dir.path().join(".locked-files");

        let mut manifest = LockManifest::load_from(&manifest_path).unwrap();
        assert!(manifest.locked_files.is_empty());

        manifest.add("src/Commands/User.hs".to_string());
        manifest.add("src/Events/UserCreated.hs".to_string());
        manifest.save().unwrap();

        let manifest2 = LockManifest::load_from(&manifest_path).unwrap();
        assert_eq!(manifest2.locked_files.len(), 2);
        assert!(manifest2.is_locked("src/Commands/User.hs"));
        assert!(manifest2.is_locked("src/Events/UserCreated.hs"));
        assert!(!manifest2.is_locked("src/Queries/User.hs"));
    }

    #[test]
    fn test_lock_manifest_no_duplicates() {
        let mut manifest = LockManifest {
            path: PathBuf::from("dummy"),
            locked_files: Vec::new(),
        };
        manifest.add("file.hs".to_string());
        manifest.add("file.hs".to_string());
        assert_eq!(manifest.locked_files.len(), 1);
    }

    #[test]
    fn test_lock_manifest_sorting() {
        let mut manifest = LockManifest {
            path: PathBuf::from("dummy"),
            locked_files: Vec::new(),
        };
        manifest.add("b.hs".to_string());
        manifest.add("a.hs".to_string());
        assert_eq!(manifest.locked_files, vec!["a.hs", "b.hs"]);
    }

    #[test]
    fn test_discover_domain_files() {
        let _lock = TEST_MUTEX.lock().unwrap();
        // We still need set_current_dir for discover_domain_files as it uses Path::new("src")
        // But we'll run it in a way that minimizes interference if possible
        let dir = tempdir().unwrap();
        let src = dir.path().join("src");
        let commands = src.join("Domain").join("Commands");
        let events = src.join("Domain").join("Events");
        
        fs::create_dir_all(&commands).unwrap();
        fs::create_dir_all(&events).unwrap();
        
        fs::write(commands.join("CreateUser.hs"), "").unwrap();
        fs::write(events.join("UserCreated.hs"), "").unwrap();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir.path()).unwrap();
        
        let files = discover_domain_files().unwrap();
        
        // Restore dir immediately
        std::env::set_current_dir(original_dir).unwrap();
        
        assert_eq!(files.len(), 2);
    }

    #[test]
    fn test_fuzzy_match() {
        let candidates = vec![
            PathBuf::from("src/Domain/Commands/CreateUser.hs"),
            PathBuf::from("src/Domain/Events/UserCreated.hs"),
            PathBuf::from("src/Domain/Queries/GetUser.hs"),
        ];

        // "Create" matches "CreateUser" and "UserCreated" (fuzzy)
        let matches = fuzzy_match("Create", &candidates);
        assert!(matches.len() >= 1);
        assert!(matches.contains(&PathBuf::from("src/Domain/Commands/CreateUser.hs")));

        // "Queries" should only match "GetUser.hs" via Queries component
        let matches = fuzzy_match("Queries", &candidates);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0], PathBuf::from("src/Domain/Queries/GetUser.hs"));

        // Empty query should return all candidates
        let matches = fuzzy_match("", &candidates);
        assert_eq!(matches.len(), 3);

        // Query with no matches
        let matches = fuzzy_match("NonExistent", &candidates);
        assert!(matches.is_empty());
    }
}
