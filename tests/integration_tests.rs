use assert_cmd::Command;
use predicates::prelude::*;

fn neo_cmd() -> Command {
    Command::cargo_bin("neo").unwrap()
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
fn bare_neo_prints_single_line_hint_no_mascot() {
    let mut cmd = neo_cmd();
    cmd.arg("--ci")
        .assert()
        .success()
        .stdout(predicate::str::contains("The NeoHaskell CLI"))
        .stdout(predicate::str::contains("neo --help"))
        // Mascot art must not appear anywhere.
        .stdout(predicate::str::contains("╔═══╗").not())
        .stdout(predicate::str::contains("║ :)║").not())
        .stdout(predicate::str::contains("╚═══╝").not());
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
    assert!(project_path.join("src/App.hs").exists());
    assert!(project_path.join("launcher/Launcher.hs").exists());
    assert!(project_path.join(".envrc").exists());
    assert!(project_path.join(".git").exists());
    assert!(project_path.join(".git/hooks/pre-commit").exists());

    // Verify neo.json content
    let config_content = std::fs::read_to_string(project_path.join("neo.json")).unwrap();
    assert!(config_content.contains(project_name));
    assert!(config_content.contains("\"neo-version\": \"main\""));

    // Verify git commit exists
    let git_log = std::process::Command::new("git")
        .args(["log", "--oneline"])
        .current_dir(&project_path)
        .output()
        .unwrap();
    let log_stdout = String::from_utf8_lossy(&git_log.stdout);
    assert!(log_stdout.contains("Initial commit from NeoCLI"));
}

#[test]
fn test_neo_new_library_ci() {
    // `--library` should produce a project with no launcher/Launcher.hs file
    // and a generated .cabal without the `executable <name>` stanza.
    // The neo.json file should record `"type": "library"`.
    let temp = tempfile::tempdir().unwrap();
    let project_name = "test-lib";

    let mut cmd = neo_cmd();
    cmd.current_dir(temp.path())
        .arg("new")
        .arg(project_name)
        .arg("--library")
        .arg("--ci")
        .assert()
        .success();

    let project_path = temp.path().join(project_name);
    assert!(project_path.exists());
    assert!(project_path.join("neo.json").exists());
    assert!(project_path.join("src/App.hs").exists());

    // No launcher folder
    assert!(
        !project_path.join("launcher").exists(),
        "library project must not have a launcher/ directory"
    );
    assert!(
        !project_path.join("launcher/Launcher.hs").exists(),
        "library project must not have launcher/Launcher.hs"
    );

    // neo.json records type: library
    let config_content = std::fs::read_to_string(project_path.join("neo.json")).unwrap();
    let config: serde_json::Value = serde_json::from_str(&config_content).unwrap();
    assert_eq!(config["type"], "library", "neo.json should record type=library, got: {}", config_content);

    // Generated .cabal has no executable stanza
    let cabal_path = project_path.join(format!("{}.cabal", project_name));
    assert!(cabal_path.exists(), "{}.cabal should be generated", project_name);
    let cabal = std::fs::read_to_string(&cabal_path).unwrap();
    assert!(
        !cabal.contains(&format!("executable {}", project_name)),
        "library .cabal must not declare an executable stanza:\n{}",
        cabal
    );
    assert!(
        !cabal.contains("main-is: Launcher.hs"),
        "library .cabal must not reference Launcher.hs:\n{}",
        cabal
    );
    assert!(
        cabal.contains("library"),
        "library .cabal must keep the library stanza:\n{}",
        cabal
    );
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

    // Without the IOHK + NeoHaskell binary caches wired into the generated flake,
    // `neo build` would compile GHC and haskell.nix infrastructure from source —
    // the "takes hours instead of minutes" failure mode. Verify the template
    // configured both substituters before we attempt to build.
    let flake = std::fs::read_to_string(project_path.join("flake.nix")).unwrap();
    assert!(
        flake.contains("https://cache.iog.io"),
        "generated flake.nix is missing the `cache.iog.io` substituter — neo build would rebuild GHC from source"
    );
    assert!(
        flake.contains("https://neohaskell.cachix.org"),
        "generated flake.nix is missing the `neohaskell.cachix.org` substituter — neo build would rebuild project deps from source"
    );
    assert!(
        flake.contains("hydra.iohk.io:f/Ea+s+dFdN+3Y/G+FDgSq+a5NEWhJGzdjvKNGv0/EQ="),
        "generated flake.nix is missing the IOHK trusted-public-key — substituter URL alone won't trust the cache"
    );
    assert!(
        flake.contains("neohaskell.cachix.org-1:mo2cLaGbwqbrxs9xhqKK8jeNsn3osi7t6XoAmxSZssc="),
        "generated flake.nix is missing the NeoHaskell trusted-public-key"
    );

    let mut cmd = neo_cmd();
    cmd.current_dir(&project_path)
        .arg("build")
        .arg("--ci")
        .assert()
        .success()
        .stdout(predicate::str::contains("Reconciling project artifacts"));
    assert!(project_path.join(format!("{}.cabal", project_name)).exists());
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

    // `neo run --ci` launches the starter executable; the default starter is
    // server-style and runs forever, so a bare `assert_cmd`-style invocation
    // would hang the whole `cargo test` session. Wrap with coreutils
    // `timeout` (mirrors `run_ci_completes_or_runs_for_fresh_starter` in
    // tests/e2e.rs): accept exit 0 (finite program) OR 124 (SIGTERM by
    // timeout), and require both reconcile + run markers in stdout as
    // evidence we got past every interesting stage.
    let neo = assert_cmd::cargo::cargo_bin("neo");
    let out = std::process::Command::new("timeout")
        .args(["--signal=TERM", "180"])
        .arg(&neo)
        .args(["run", "--ci"])
        .current_dir(&project_path)
        .output()
        .expect("spawn `timeout` + neo failed (is coreutils `timeout` on PATH?)");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    let code = out.status.code().unwrap_or(-1);

    assert!(
        stdout.contains("Reconciling project artifacts"),
        "missing reconcile marker; stdout=`{}` stderr=`{}` code={}",
        stdout, stderr, code
    );
    assert!(
        stdout.contains("Running project"),
        "missing run marker; stdout=`{}` stderr=`{}` code={}",
        stdout, stderr, code
    );
    assert!(
        out.status.success() || code == 124,
        "unexpected exit code {} (stderr: {})",
        code, stderr
    );
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
    cmd.current_dir(&project_path)
        .arg("test")
        .arg("--ci")
        .assert()
        .success()
        .stdout(predicate::str::contains("Reconciling project artifacts"))
        .stdout(predicate::str::contains("Running unit tests"));
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
    std::fs::write(tests_dir.join("api.hurl"), "GET http://localhost:8080\nHTTP *\n").unwrap();

    let mut cmd = neo_cmd();
    cmd.current_dir(&project_path)
        .arg("test")
        .arg("--ci")
        .assert()
        .stdout(predicate::str::contains("Running 1 Hurl integration tests"));
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

// ============================================================
// Dependency-grammar input validation
// (fast: reconcile fails before cabal is invoked)
// ============================================================

fn write_minimal_project(dir: &std::path::Path, name: &str, deps_json: &str) {
    let neo_json = format!(
        "{{\n  \"name\": \"{}\",\n  \"version\": \"0.1.0\",\n  \"neo-version\": \"main\",\n  \"license\": \"MIT\",\n  \"dependencies\": {}\n}}\n",
        name, deps_json
    );
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(dir.join("src/App.hs"), "module App where\n").unwrap();
    std::fs::create_dir_all(dir.join("launcher")).unwrap();
    std::fs::write(
        dir.join("launcher/Launcher.hs"),
        "module Main where\nmain :: IO ()\nmain = pure ()\n",
    )
    .unwrap();
    std::fs::write(dir.join("neo.json"), neo_json).unwrap();
}

#[test]
fn integration_build_invalid_semver_errors() {
    let temp = tempfile::tempdir().unwrap();
    write_minimal_project(temp.path(), "p", r#"{"foo":"not-a-version"}"#);
    neo_cmd()
        .current_dir(temp.path())
        .env("NEO_SKIP_NETWORK", "1")
        .arg("build")
        .arg("--ci")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Invalid dependency"));
}

#[test]
fn integration_build_unknown_protocol_errors() {
    let temp = tempfile::tempdir().unwrap();
    write_minimal_project(temp.path(), "p", r#"{"foo":"npm:bar"}"#);
    neo_cmd()
        .current_dir(temp.path())
        .env("NEO_SKIP_NETWORK", "1")
        .arg("build")
        .arg("--ci")
        .assert()
        .failure()
        .stderr(predicate::str::contains("unknown protocol"));
}

#[test]
fn integration_build_conflicting_protocols_errors() {
    let temp = tempfile::tempdir().unwrap();
    write_minimal_project(temp.path(), "p", r#"{"hackage:foo":"git:host/r.git"}"#);
    neo_cmd()
        .current_dir(temp.path())
        .env("NEO_SKIP_NETWORK", "1")
        .arg("build")
        .arg("--ci")
        .assert()
        .failure()
        .stderr(predicate::str::contains("both key and value"));
}

#[test]
fn integration_build_github_too_many_slashes_errors() {
    let temp = tempfile::tempdir().unwrap();
    write_minimal_project(temp.path(), "p", r#"{"foo":"github:owner/repo/sub"}"#);
    neo_cmd()
        .current_dir(temp.path())
        .env("NEO_SKIP_NETWORK", "1")
        .arg("build")
        .arg("--ci")
        .assert()
        .failure()
        .stderr(predicate::str::contains("owner/repo"));
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
        .stderr(predicate::str::contains("Failed to parse `neo.json`"))
        // The new GraphicalReportHandler renders a source-pointer block:
        // either with unicode `╭─[neo.json:` (TTY) or ASCII `,-[neo.json:` (pipe).
        // assert_cmd pipes stderr, so we get the ASCII fallback.
        .stderr(predicate::str::contains("neo.json:").and(
            predicate::str::contains("syntax error here")
        ));
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

    // 5. Check violation — railguard wording: explainer + V-bump recipe +
    //    worked example. The escape hatches (`neo lock --remove`,
    //    `--skip-lock-check`) must NOT appear; they live in `--help` for
    //    humans who already understand the model.
    let mut cmd = neo_cmd();
    cmd.current_dir(project_path)
        .arg("lock")
        .arg("check")
        .arg("--ci")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Build refused"))
        .stderr(predicate::str::contains("src/Domain/Commands/CreateUser.hs"))
        .stderr(predicate::str::contains("event-sourced"))
        .stderr(predicate::str::contains("CreateUserV2.hs"))
        .stderr(predicate::str::contains("neo lock --remove").not())
        .stderr(predicate::str::contains("--skip-lock-check").not());
}

#[test]
fn test_neo_lock_check_unstaged_violation() {
    // Widened semantics: `neo lock check` now catches unstaged modifications
    // too, not just staged ones. A user editing a locked file should see the
    // violation immediately, before they get a chance to `git add`.
    let temp = tempfile::tempdir().unwrap();
    let project_path = temp.path();

    std::process::Command::new("git").arg("init").current_dir(project_path).output().unwrap();
    std::process::Command::new("git").args(["config", "user.email", "test@example.com"]).current_dir(project_path).output().unwrap();
    std::process::Command::new("git").args(["config", "user.name", "Test User"]).current_dir(project_path).output().unwrap();

    let commands_dir = project_path.join("src/Domain/Commands");
    std::fs::create_dir_all(&commands_dir).unwrap();
    let file_path = commands_dir.join("CreateUser.hs");
    std::fs::write(&file_path, "initial content").unwrap();

    let mut cmd = neo_cmd();
    cmd.current_dir(project_path).arg("lock").arg("--ci").assert().success();

    // Modify WITHOUT staging.
    std::fs::write(&file_path, "modified content").unwrap();

    let mut cmd = neo_cmd();
    cmd.current_dir(project_path)
        .arg("lock")
        .arg("check")
        .arg("--ci")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Build refused"))
        .stderr(predicate::str::contains("src/Domain/Commands/CreateUser.hs"));
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

// ---- Pre-build lock check ----
//
// These tests exercise the `--skip-lock-check` flag and the gate that aborts
// `neo build` when a locked file has been modified. The lock check fires
// after `NeoConfig::load` and before reconcile/nix-build, so violation tests
// fail fast (no Haskell compile in the loop). The "skip flag proceeds" test
// runs through real reconcile + nix build and is therefore as slow as
// `test_neo_build_ci`.

/// Hand-roll a minimal NeoHaskell workspace (no `neo new`) so violation tests
/// don't pay the starter-template download. Initializes git, writes the
/// minimal `neo.json` that `NeoConfig::load` accepts, and configures a git
/// identity so subsequent commits work.
fn minimal_workspace(project_path: &std::path::Path) {
    use std::process::Command as Cmd;
    Cmd::new("git").arg("init").current_dir(project_path).output().unwrap();
    Cmd::new("git").args(["config", "user.email", "test@example.com"]).current_dir(project_path).output().unwrap();
    Cmd::new("git").args(["config", "user.name", "Test User"]).current_dir(project_path).output().unwrap();
    std::fs::write(
        project_path.join("neo.json"),
        r#"{"name":"locktest","version":"0.1.0","neo-version":"0.1.0"}"#,
    )
    .unwrap();
}

#[test]
fn test_neo_build_refuses_modified_locked() {
    let temp = tempfile::tempdir().unwrap();
    let project_path = temp.path();
    minimal_workspace(project_path);

    // Create + lock + commit a domain file.
    let commands_dir = project_path.join("src/Domain/Commands");
    std::fs::create_dir_all(&commands_dir).unwrap();
    let file_path = commands_dir.join("CreateUser.hs");
    std::fs::write(&file_path, "initial").unwrap();

    let mut cmd = neo_cmd();
    cmd.current_dir(project_path).arg("lock").arg("--ci").assert().success();

    // Modify and stage the locked file.
    std::fs::write(&file_path, "modified").unwrap();
    std::process::Command::new("git")
        .args(["add", "src/Domain/Commands/CreateUser.hs"])
        .current_dir(project_path)
        .output()
        .unwrap();

    let mut cmd = neo_cmd();
    cmd.current_dir(project_path)
        .arg("build")
        .arg("--ci")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Build refused"))
        .stderr(predicate::str::contains("src/Domain/Commands/CreateUser.hs"))
        .stderr(predicate::str::contains("event-sourced"))
        .stderr(predicate::str::contains("CreateUserV2.hs"))
        .stderr(predicate::str::contains("byte-identical"))
        .stderr(predicate::str::contains("--skip-lock-check").not())
        .stderr(predicate::str::contains("neo lock --remove").not())
        .stderr(predicate::str::contains("git checkout --").not());
}

#[test]
fn test_neo_build_unstaged_locked_modification_refused() {
    // Proves the widened semantics: unstaged edits to a locked file also
    // abort the build, not just staged ones.
    let temp = tempfile::tempdir().unwrap();
    let project_path = temp.path();
    minimal_workspace(project_path);

    let commands_dir = project_path.join("src/Domain/Commands");
    std::fs::create_dir_all(&commands_dir).unwrap();
    let file_path = commands_dir.join("CreateUser.hs");
    std::fs::write(&file_path, "initial").unwrap();

    let mut cmd = neo_cmd();
    cmd.current_dir(project_path).arg("lock").arg("--ci").assert().success();

    // Modify the file but do NOT stage.
    std::fs::write(&file_path, "modified").unwrap();

    let mut cmd = neo_cmd();
    cmd.current_dir(project_path)
        .arg("build")
        .arg("--ci")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Build refused"))
        .stderr(predicate::str::contains("src/Domain/Commands/CreateUser.hs"));
}

#[test]
fn test_neo_build_untracked_path_in_manifest_refused() {
    // Exercises the `??` porcelain status code: a path listed in
    // `.locked-files` exists on disk but is untracked. The lock check should
    // still flag it.
    let temp = tempfile::tempdir().unwrap();
    let project_path = temp.path();
    minimal_workspace(project_path);

    let commands_dir = project_path.join("src/Domain/Commands");
    std::fs::create_dir_all(&commands_dir).unwrap();
    let ghost = commands_dir.join("Ghost.hs");
    std::fs::write(&ghost, "untracked").unwrap();
    std::fs::write(
        project_path.join(".locked-files"),
        "src/Domain/Commands/Ghost.hs",
    )
    .unwrap();

    let mut cmd = neo_cmd();
    cmd.current_dir(project_path)
        .arg("build")
        .arg("--ci")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Build refused"))
        .stderr(predicate::str::contains("src/Domain/Commands/Ghost.hs"));
}

#[test]
fn test_neo_build_skip_lock_check_bypasses_check() {
    // The flag must let the build proceed past the lock-check stage even
    // with a modified locked file. We don't assert on overall success — the
    // hand-rolled workspace has no flake.nix or source code, so reconcile/
    // nix-build will fail downstream. What we DO assert is that the failure
    // is NOT the lock-violation diagnostic.
    let temp = tempfile::tempdir().unwrap();
    let project_path = temp.path();
    minimal_workspace(project_path);

    let commands_dir = project_path.join("src/Domain/Commands");
    std::fs::create_dir_all(&commands_dir).unwrap();
    let file_path = commands_dir.join("CreateUser.hs");
    std::fs::write(&file_path, "initial").unwrap();

    let mut cmd = neo_cmd();
    cmd.current_dir(project_path).arg("lock").arg("--ci").assert().success();

    // Modify the locked file.
    std::fs::write(&file_path, "modified").unwrap();

    let mut cmd = neo_cmd();
    cmd.current_dir(project_path)
        .arg("build")
        .arg("--ci")
        .arg("--skip-lock-check")
        .assert()
        // Build may pass or fail downstream — we don't care. The point is
        // that the lock check did not block.
        .stderr(predicate::str::contains("Build refused").not())
        .stderr(predicate::str::contains("neo::lock_violation").not());
}
