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
- **Tests**: No unit or integration tests have been written yet.

## Recent Log
- **2026-04-22**: Implemented `src/config.rs` with `NeoConfig` to parse `neo.json`. Implemented `src/prereqs.rs` with `require_nix` and `warn_direnv`. Wired these prerequisite checks and config loading into `build`, `run`, and `test` commands. Verified code compilation with `cargo check`.
