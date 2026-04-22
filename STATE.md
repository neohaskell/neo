# NeoCLI Project State

## Overview
The NeoCLI project has completed its initial scaffolding phase. The foundational architecture for CLI parsing, TUI rendering, error handling, and asynchronous task execution has been established and verifies successfully with `cargo check`.

## Scaffolded Components
- **Workspace**: Project initialized with `Cargo.toml` and dependencies.
- **CLI Parser (`src/cli.rs`)**: Subcommands and arguments are structurally defined using `clap`.
- **App Entrypoint (`src/main.rs`, `src/app.rs`)**: The `tokio` runtime is configured, and a dispatcher pattern routes commands. The basic TEA loop framework is present.
- **Diagnostics (`src/errors.rs`)**: `miette` error variants are defined.
- **Theming & Output (`src/theme.rs`, `src/output.rs`)**: The Neo brand colors and CI/Interactive output modes are implemented.
- **Commands**: All major subcommands (`new`, `build`, `run`, `test`, `lock`) have basic function signatures inside `src/commands/`.
- **Subprocesses & Reconcile**: The module structure and some minimal stub functions are present in `src/subprocess/` and `src/reconcile/`.
- **TUI**: The mascot ASCII art and a simple `Mascot` widget are implemented in `src/tui/mascot.rs`.

## Missing / Pending Components
- **Domain Logic**: 
  - Implementation of config parsing (`src/config.rs`).
  - Network operations for downloading templates and updates (`src/network.rs`).
  - Git integration for lockhooks and repo initialization (`src/git.rs`).
  - System checks (`src/prereqs.rs`).
- **TUI Widgets**: Missing core interactive widgets such as banners, spinners, prompts, progress bars, and watch-mode layouts.
- **File Generation**: Missing template rendering for `.cabal`, `flake.nix`, etc., in `src/reconcile/`.
- **Subprocesses**: Missing the interactive GHCi session wrapper (`src/subprocess/ghci.rs`) and Hurl integration (`src/subprocess/hurl.rs`).
- **Tests**: Comprehensive unit tests and integration tests have been implemented for all scaffolded components and the `neo new` command.

## Recent Log
- **2026-04-22**: Implemented the `neo new` command with a full TEA-based interactive interview. Created TUI widgets for banners, prompts, and selection menus. Implemented scaffolding logic including `neo.json` generation, placeholder template fetching, and git initialization with a pre-commit lock hook. Verified functionality in CI mode.
- 2026-04-22: Expanded unit and integration test coverage. Fixed `neo lock` integration tests and added new cases for hook installation and locking all files. Improved GHCi prompt detection tests and NeoError display tests. Verified all 54 tests pass with `cargo test`.
- 2026-04-22: Implemented the `reconcile` module for generating project artifacts (`.cabal`, `flake.nix`, `cabal.project`). Integrated Jinja2 templates via `minijinja`. Implemented Haskell module discovery. Integrated `reconcile::run()` into `build`, `run`, and `test` commands. Verified with unit and integration tests.
- 2026-04-22: Implemented the `subprocess` module including `nix` and `ghci` submodules. `nix::build()` and `nix::run()` now execute real commands via `nix develop`. `ghci::GhciSession` provides basic session management for watch mode. Finalized `neo build` and `neo run` to use these subprocesses. Verified with integration tests.
- 2026-04-22: Implemented full watch mode with `notify` integration. Created `WatchState` and `WatchStatus` for TUI rendering. Implemented a consolidated watch loop in `watch_common.rs` used by `build`, `run`, and `test` commands. Refined `GhciSession` for robust prompt detection. Verified with unit and integration tests.
- 2026-04-22: Implemented `neo lock` command for domain file locking. Added domain file discovery, fuzzy matching using `nucleo`, and lock manifest management. Implemented `neo lock check` for pre-commit hooks and `neo lock install` to install the hook. Added TUI selection for multiple matches. Verified with unit tests.
- 2026-04-22: Implemented network operations and update checks. Added GitHub API integration to check for latest NeoCLI release. Implemented starter template downloading and extraction for `neo new`. Integrated background update check into the main execution flow. Created a TUI `Footer` widget to display update notifications. Verified with unit and integration tests (using network-skipping mocks).
- 2026-04-22: Implemented system prerequisite checks (`nix`, `git`, `direnv`) in `src/prereqs.rs`. Added Hurl integration in `src/subprocess/hurl.rs` for automated integration testing. Refined `neo test` to run both unit tests and Hurl integration tests with a consolidated summary. Verified with unit and integration tests.
- 2026-04-22: Verified comprehensive test coverage for all implemented features. Added missing unit tests for `app::dispatch` and `subprocess::nix::spawn_app`. Fixed race conditions in tests involving `std::env::set_current_dir` by implementing a centralized `TEST_MUTEX`. Verified all 76 tests (59 unit, 17 integration) pass successfully.
- 2026-04-22: Implemented remaining TUI widgets: `Spinner`, `ProgressBar`, `SuccessDisplay`, `ErrorDisplay`, and `Confirm`. Added unit tests for each new widget. Verified all 82 tests (65 unit, 17 integration) pass successfully.
- 2026-04-22: Implemented advanced dependency resolution and `neo-version` pinning. Added GitHub API integration to resolve NeoHaskell versions to pinned SHAs. Enhanced `reconcile` module to handle `hackage:`, `git+`, and `file:` dependency prefixes. Updated `flake.nix` and `cabal.project` templates to use pinned SHAs and resolved dependencies. Integrated `.envrc` generation for `direnv` and HLS support. Verified all 93 tests pass.

- 2026-04-22: Refactored subprocess execution to capture and display full output on failure. Updated `NeoError` to include captured output in `SubprocessError`. Improved `nix::execute` to robustly read both stdout and stderr until completion. Verified that error output is displayed even in interactive mode. Added unit tests for output capture.
