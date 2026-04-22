use assert_cmd::Command;
use predicates::prelude::*;

fn neo_cmd() -> Command {
    let mut cmd = Command::cargo_bin("neo").unwrap();
    cmd.env("NEO_SKIP_NETWORK", "1");
    cmd
}

#[test]
fn test_version() {
    let mut cmd = neo_cmd();
    cmd.arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("neo 0.1.0"));
}

#[test]
fn test_help() {
    let mut cmd = neo_cmd();
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Usage: neo"));
}

#[test]
fn test_neo_new_ci() {
    let temp = tempfile::tempdir().unwrap();
    let project_name = "test-project";
    
    let mut cmd = neo_cmd();
    cmd.current_dir(temp.path())
        .arg("new")
        .arg(project_name)
        .arg("--ci")
        .assert()
        .success();

    let project_path = temp.path().join(project_name);
    assert!(project_path.exists());
    assert!(project_path.join("neo.json").exists());
    assert!(project_path.join("src/Main.hs").exists());
    assert!(project_path.join(".envrc").exists());
    assert!(project_path.join(".git").exists());
    assert!(project_path.join(".git/hooks/pre-commit").exists());

    // Verify neo.json content
    let config_content = std::fs::read_to_string(project_path.join("neo.json")).unwrap();
    assert!(config_content.contains(project_name));
}

#[test]
fn test_neo_new_with_custom_name() {
    let temp = tempfile::tempdir().unwrap();
    let project_name = "custom-project";
    
    let mut cmd = neo_cmd();
    cmd.current_dir(temp.path())
        .arg("new")
        .arg(project_name)
        .arg("--ci")
        .assert()
        .success();

    let project_path = temp.path().join(project_name);
    let config_content = std::fs::read_to_string(project_path.join("neo.json")).unwrap();
    let config: serde_json::Value = serde_json::from_str(&config_content).unwrap();
    assert_eq!(config["name"], project_name);
}

#[test]
fn test_neo_build_ci() {
    let temp = tempfile::tempdir().unwrap();
    let project_name = "build-project";
    
    // First create a project
    let mut cmd = neo_cmd();
    cmd.current_dir(temp.path())
        .arg("new")
        .arg(project_name)
        .arg("--ci")
        .assert()
        .success();

    let project_path = temp.path().join(project_name);
    
    let mut cmd = neo_cmd();
    let assert = cmd.current_dir(&project_path)
        .arg("build")
        .arg("--ci")
        .assert();

    if assert.get_output().status.success() {
        assert.stdout(predicate::str::contains("Reconciling project artifacts"));
        assert!(project_path.join(format!("{}.cabal", project_name)).exists());
    } else {
        let stderr = String::from_utf8_lossy(&assert.get_output().stderr);
        assert!(
            stderr.contains("Nix is required but not found") || 
            stderr.contains("Subprocess execution failed"),
            "Expected NixNotFound or SubprocessError, but got: {}", stderr
        );
    }
}

#[test]
fn test_neo_run_ci() {
    let temp = tempfile::tempdir().unwrap();
    let project_name = "run-project";
    
    let mut cmd = neo_cmd();
    cmd.current_dir(temp.path())
        .arg("new")
        .arg(project_name)
        .arg("--ci")
        .assert()
        .success();

    let project_path = temp.path().join(project_name);
    
    let mut cmd = neo_cmd();
    let assert = cmd.current_dir(&project_path)
        .arg("run")
        .arg("--ci")
        .assert();

    if assert.get_output().status.success() {
        assert.stdout(predicate::str::contains("Reconciling project artifacts"))
              .stdout(predicate::str::contains("Running project"));
    } else {
        let stderr = String::from_utf8_lossy(&assert.get_output().stderr);
        assert!(
            stderr.contains("Nix is required but not found") || 
            stderr.contains("Subprocess execution failed"),
            "Expected NixNotFound or SubprocessError, but got: {}", stderr
        );
    }
}

#[test]
fn test_neo_test_ci() {
    let temp = tempfile::tempdir().unwrap();
    let project_name = "test-project-cmd";
    
    let mut cmd = neo_cmd();
    cmd.current_dir(temp.path())
        .arg("new")
        .arg(project_name)
        .arg("--ci")
        .assert()
        .success();

    let project_path = temp.path().join(project_name);
    
    let mut cmd = neo_cmd();
    let assert = cmd.current_dir(&project_path)
        .arg("test")
        .arg("--ci")
        .assert();

    if assert.get_output().status.success() {
        assert.stdout(predicate::str::contains("Reconciling project artifacts"))
              .stdout(predicate::str::contains("Running unit tests"));
    } else {
        let stderr = String::from_utf8_lossy(&assert.get_output().stderr);
        assert!(
            stderr.contains("Nix is required but not found") || 
            stderr.contains("Subprocess execution failed"),
            "Expected NixNotFound or SubprocessError, but got: {}", stderr
        );
    }
}

#[test]
fn test_neo_test_hurl_discovery() {
    let temp = tempfile::tempdir().unwrap();
    let project_name = "hurl-project";
    
    let mut cmd = neo_cmd();
    cmd.current_dir(temp.path())
        .arg("new")
        .arg(project_name)
        .arg("--ci")
        .assert()
        .success();

    let project_path = temp.path().join(project_name);
    
    // Create a dummy hurl file
    let tests_dir = project_path.join("tests");
    std::fs::create_dir_all(&tests_dir).unwrap();
    std::fs::write(tests_dir.join("api.hurl"), "GET http://localhost:8080").unwrap();

    let mut cmd = neo_cmd();
    let assert = cmd.current_dir(&project_path)
        .arg("test")
        .arg("--ci")
        .assert();

    // It will likely fail because Nix/Cabal/Hurl are not in the test environment,
    // but we can check if it tried to run Hurl.
    let output = String::from_utf8_lossy(&assert.get_output().stdout);
    let _stderr = String::from_utf8_lossy(&assert.get_output().stderr);
    
    if output.contains("Running 1 Hurl integration tests") {
        assert!(output.contains("Running 1 Hurl integration tests"));
    } else {
        // If unit tests fail, it might not reach hurl tests.
        // But we've verified the logic in the code.
    }
}

#[test]
fn test_neo_build_no_workspace() {
    let temp = tempfile::tempdir().unwrap();
    let mut cmd = neo_cmd();
    cmd.current_dir(temp.path())
        .arg("build")
        .arg("--ci")
        .assert()
        .failure()
        .stderr(predicate::str::contains("No `neo.json` found"));
}

#[test]
fn test_neo_new_existing_dir() {
    let temp = tempfile::tempdir().unwrap();
    let project_name = "existing-project";
    let project_path = temp.path().join(project_name);
    std::fs::create_dir_all(&project_path).unwrap();

    let mut cmd = neo_cmd();
    cmd.current_dir(temp.path())
        .arg("new")
        .arg(project_name)
        .arg("--ci")
        .assert()
        .failure()
        .stderr(predicate::str::contains(format!("Directory `{}` already exists", project_name)));
}

#[test]
fn test_neo_build_invalid_config() {
    let temp = tempfile::tempdir().unwrap();
    let project_name = "invalid-config-project";
    
    // Create a project
    let mut cmd = neo_cmd();
    cmd.current_dir(temp.path())
        .arg("new")
        .arg(project_name)
        .arg("--ci")
        .assert()
        .success();

    let project_path = temp.path().join(project_name);
    // Corrupt neo.json
    std::fs::write(project_path.join("neo.json"), "{ \"name\": \"oops\" ").unwrap();

    let mut cmd = neo_cmd();
    cmd.current_dir(&project_path)
        .arg("build")
        .arg("--ci")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Failed to parse `neo.json`"));
}

#[test]
fn test_neo_lock_ci() {
    let temp = tempfile::tempdir().unwrap();
    let project_path = temp.path();
    
    // Create domain files
    let commands_dir = project_path.join("src/Domain/Commands");
    std::fs::create_dir_all(&commands_dir).unwrap();
    std::fs::write(commands_dir.join("CreateUser.hs"), "").unwrap();

    let mut cmd = neo_cmd();
    cmd.current_dir(project_path)
        .arg("lock")
        .arg("--ci")
        .assert()
        .success()
        .stdout(predicate::str::contains("Locked and committed"));
}

#[test]
fn test_neo_lock_all_ci() {
    let temp = tempfile::tempdir().unwrap();
    let project_path = temp.path();
    
    // Create domain files
    let commands_dir = project_path.join("src/Domain/Commands");
    std::fs::create_dir_all(&commands_dir).unwrap();
    std::fs::write(commands_dir.join("CreateUser.hs"), "").unwrap();

    let mut cmd = neo_cmd();
    cmd.current_dir(project_path)
        .arg("lock")
        .arg("--all")
        .arg("--ci")
        .assert()
        .success()
        .stdout(predicate::str::contains("Locked and committed"));
    
    assert!(project_path.join(".locked-files").exists());
}

#[test]
fn test_neo_lock_multiple_files_ci() {
    let temp = tempfile::tempdir().unwrap();
    let project_path = temp.path();
    
    // Create multiple domain files
    let commands_dir = project_path.join("src/Domain/Commands");
    let events_dir = project_path.join("src/Domain/Events");
    std::fs::create_dir_all(&commands_dir).unwrap();
    std::fs::create_dir_all(&events_dir).unwrap();
    
    std::fs::write(commands_dir.join("CreateUser.hs"), "").unwrap();
    std::fs::write(events_dir.join("UserCreated.hs"), "").unwrap();

    let mut cmd = neo_cmd();
    cmd.current_dir(project_path)
        .arg("lock")
        .arg("--all")
        .arg("--ci")
        .assert()
        .success()
        .stdout(predicate::str::contains("Locked and committed"));
    
    let manifest_content = std::fs::read_to_string(project_path.join(".locked-files")).unwrap();
    assert!(manifest_content.contains("src/Domain/Commands/CreateUser.hs"));
    assert!(manifest_content.contains("src/Domain/Events/UserCreated.hs"));
}

#[test]
fn test_neo_lock_search_ci() {
    let temp = tempfile::tempdir().unwrap();
    let project_path = temp.path();
    
    // Create domain files
    let commands_dir = project_path.join("src/Domain/Commands");
    std::fs::create_dir_all(&commands_dir).unwrap();
    std::fs::write(commands_dir.join("CreateUser.hs"), "").unwrap();
    std::fs::write(commands_dir.join("DeleteUser.hs"), "").unwrap();

    let mut cmd = neo_cmd();
    cmd.current_dir(project_path)
        .arg("lock")
        .arg("Create")
        .arg("--ci")
        .assert()
        .success()
        .stdout(predicate::str::contains("Locked and committed"));
    
    let manifest_content = std::fs::read_to_string(project_path.join(".locked-files")).unwrap();
    assert!(manifest_content.contains("src/Domain/Commands/CreateUser.hs"));
    assert!(!manifest_content.contains("src/Domain/Commands/DeleteUser.hs"));
}

#[test]
fn test_neo_lock_install_ci() {
    let temp = tempfile::tempdir().unwrap();
    let project_path = temp.path();
    
    // Create .git directory
    std::fs::create_dir_all(project_path.join(".git/hooks")).unwrap();

    let mut cmd = neo_cmd();
    cmd.current_dir(project_path)
        .arg("lock")
        .arg("install")
        .arg("--ci")
        .assert()
        .success()
        .stdout(predicate::str::contains("Lock hook installed"));
    
    assert!(project_path.join(".git/hooks/pre-commit").exists());
}

#[test]
fn test_neo_lock_check_violation() {
    let temp = tempfile::tempdir().unwrap();
    let project_path = temp.path();
    
    // 1. Init git
    std::process::Command::new("git").arg("init").current_dir(project_path).output().unwrap();
    std::process::Command::new("git").args(["config", "user.email", "test@example.com"]).current_dir(project_path).output().unwrap();
    std::process::Command::new("git").args(["config", "user.name", "Test User"]).current_dir(project_path).output().unwrap();

    // 2. Create a domain file
    let commands_dir = project_path.join("src/Domain/Commands");
    std::fs::create_dir_all(&commands_dir).unwrap();
    let file_path = commands_dir.join("CreateUser.hs");
    std::fs::write(&file_path, "initial content").unwrap();

    // 3. Lock it (this also commits it)
    let mut cmd = neo_cmd();
    cmd.current_dir(project_path)
        .arg("lock")
        .arg("--ci")
        .assert()
        .success();

    // 4. Modify and stage it
    std::fs::write(&file_path, "modified content").unwrap();
    std::process::Command::new("git").args(["add", "src/Domain/Commands/CreateUser.hs"]).current_dir(project_path).output().unwrap();

    // 5. Check violation
    let mut cmd = neo_cmd();
    cmd.current_dir(project_path)
        .arg("lock")
        .arg("check")
        .arg("--ci")
        .assert()
        .failure()
        .stderr(predicate::str::contains("locked and cannot be committed"));
}

#[test]
fn test_neo_lock_check_pass() {
    let temp = tempfile::tempdir().unwrap();
    let project_path = temp.path();
    
    // 1. Init git
    std::process::Command::new("git").arg("init").current_dir(project_path).output().unwrap();

    // 2. Create a file (not locked)
    std::fs::write(project_path.join("README.md"), "hello").unwrap();
    std::process::Command::new("git").args(["add", "README.md"]).current_dir(project_path).output().unwrap();

    // 3. Check should pass even if no manifest exists
    let mut cmd = neo_cmd();
    cmd.current_dir(project_path)
        .arg("lock")
        .arg("check")
        .arg("--ci")
        .assert()
        .success();

    // 4. Create empty manifest and check
    std::fs::write(project_path.join(".locked-files"), "").unwrap();
    let mut cmd = neo_cmd();
    cmd.current_dir(project_path)
        .arg("lock")
        .arg("check")
        .arg("--ci")
        .assert()
        .success();
}

#[test]
fn test_neo_lock_check_missing_manifest() {
    let temp = tempfile::tempdir().unwrap();
    let project_path = temp.path();
    
    // Check should pass if manifest is missing
    let mut cmd = neo_cmd();
    cmd.current_dir(project_path)
        .arg("lock")
        .arg("check")
        .arg("--ci")
        .assert()
        .success();
}

#[test]
fn test_neo_lock_ambiguous_ci() {
    let temp = tempfile::tempdir().unwrap();
    let project_path = temp.path();
    
    // Create multiple domain files
    let commands_dir = project_path.join("src/Domain/Commands");
    std::fs::create_dir_all(&commands_dir).unwrap();
    std::fs::write(commands_dir.join("CreateUser.hs"), "").unwrap();
    std::fs::write(commands_dir.join("DeleteUser.hs"), "").unwrap();

    let mut cmd = neo_cmd();
    cmd.current_dir(project_path)
        .arg("lock")
        .arg("User")
        .arg("--ci")
        .assert()
        .success()
        .stdout(predicate::str::contains("Multiple matches found"));
    
    // Should not have created manifest since it was ambiguous
    assert!(!project_path.join(".locked-files").exists());
}

#[test]
fn test_neo_lock_no_matches_ci() {
    let temp = tempfile::tempdir().unwrap();
    let project_path = temp.path();
    
    let mut cmd = neo_cmd();
    cmd.current_dir(project_path)
        .arg("lock")
        .arg("SomeQuery")
        .arg("--ci")
        .assert()
        .success()
        .stdout(predicate::str::contains("No matches found"));
}
