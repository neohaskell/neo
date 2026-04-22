# NeoCLI — PRD

## 1. Overview
NeoCLI is the official command-line interface for NeoHaskell. It abstracts away the complexity of Cabal and Nix, allowing developers to scaffold, build, run, test, and lock event-sourced NeoHaskell applications purely through a unified configuration file (`neo.json`) and a set of intuitive commands. It delivers a seamless, zero-boilerplate developer experience modeled after the ergonomics of `npm`, with terminal output polish inspired by `ratatui` and `charm.sh` tooling. The CLI features a mascot named **Neo** — a CRT monitor with a face — that appears during interactive sessions to give the tool personality and warmth.

## 2. Core Concepts
- **Workspace**: A NeoHaskell project directory containing a `neo.json` file and a `src/` directory.
- **`neo.json`**: The single declarative configuration file for a NeoHaskell project, analogous to `package.json` in Node.js. Describes the project name, version, NeoHaskell version, dependencies, and optional metadata. All other build configuration (`.cabal`, `flake.nix`, `cabal.project`) is generated from it.
- **Reconciliation**: The process of automatically generating valid Cabal, `cabal.project`, and `flake.nix` configurations from the declarative `neo.json` state. Generated files are pure build artifacts — never hand-edited, always overwritten, always gitignored.
- **NeoPackages**: A curated package registry hosted at `github.com/NeoHaskell/neopackages` containing a JSON manifest of NeoHaskell-compatible packages with metadata pointing at source repositories. Analogous to Elm's package registry.
- **Locking**: The process of making event-sourced domain files (Commands, Events, Queries) immutable via a `.locked-files` manifest and a git pre-commit hook. Locked files cannot be modified — only new versioned files can be created. This enforces the append-only nature of event sourcing.
- **Neo (Mascot)**: A CRT monitor character with a face that appears in interactive CLI sessions, providing personality during prompts, progress feedback, and success/error messages.

## 3. Actors & Environments
- **Local Developer**: Uses the CLI interactively to scaffold projects via `neo new`, run dev servers with hot-reloading (`--watch`), lock domain files, and iterate on the application. Sees the Neo mascot, spinners, progress bars, and rich terminal output.
- **CI/CD Pipeline**: Runs the CLI headlessly using the `--ci` flag to predictably build and test the project without interactive prompts, terminal animations, or mascot output. All commands output plain structured logs and exit with standard codes.
- **Coding Agents**: AI assistants that will connect to a future `neo mcp` server (Phase 2) to accurately navigate and author NeoHaskell code.

## 4. CLI Interface (Commands & Flags)

### Global Behavior
- `neo` with no arguments displays help text with the Neo mascot banner and a list of available commands.
- `neo --help` / `neo help [command]` displays contextual help.
- `neo --version` / `neo -V` prints the installed NeoCLI version.
- `--verbose` / `-v` (global flag) enables debug-level output on any command.
- `--ci` (global flag on applicable commands) disables all interactive prompts, animations, mascot output, and colored formatting. Outputs plain structured logs.
- On every invocation, `neo` performs two background checks:
  1. **Lock hook check**: If the current directory is a git-initialized workspace and the `.git/hooks/pre-commit` lock hook is missing, it silently installs it.
  2. **Update check**: Queries the NeoCLI GitHub repo for the latest release tag. If a newer version exists, prints a single-line notice after the command completes (e.g., "Neo v0.3.0 is available. You're on v0.2.1.").

---

- **`neo new [project-name]`**
  - **Purpose**: Scaffolds a new NeoHaskell project from the official starter template.
  - **Required Arguments**: None. If `project-name` is omitted, the CLI enters a full interactive interview (see User Flow 7.1).
  - **Key Flags**:
    - `--ci`: Requires `project-name` as a positional argument. Uses defaults for all other fields (version `0.1.0`, no description, no author, `Apache-2.0` license). Fails if `project-name` is missing.
  - **Side Effects**:
    - Downloads the latest tarball from the `NeoHaskell/neohaskell-starter` GitHub repository.
    - Creates the project directory and extracts the template into it.
    - Generates a `neo.json` populated with the user's answers.
    - Initializes a git repository (`git init`).
    - Installs the lock pre-commit hook.
    - Does **not** generate `.cabal`, `flake.nix`, or `cabal.project` — those are created on first `neo build`.

- **`neo build`**
  - **Purpose**: Reconciles `neo.json` into Nix/Cabal build configuration and compiles the project.
  - **Required Arguments**: None.
  - **Key Flags**:
    - `--watch`: Spawns GHCi with injected module loading (similar to ghcide) for fast typechecking and hot-reloading on file changes. Clears the terminal and re-displays results on each save.
    - `--ci`: Runs headlessly with plain log output. No spinners, no colors, no mascot.
  - **Reconciliation Steps** (run on every `neo build`):
    1. Parses `neo.json`.
    2. Resolves `neo-version` to a commit SHA by looking up the corresponding git tag on `NeoHaskell/neohaskell`.
    3. Resolves all dependencies from the NeoPackages registry (or Hackage for `hackage:` prefixed deps, or git/file remotes).
    4. Scans `src/` recursively to auto-discover all `.hs` modules.
    5. Generates `<project-name>.cabal` with the discovered modules, hardcoded `default-extensions`, hardcoded `ghc-options`, and resolved dependencies. `nhcore` and `nhintegrations` are always implicitly included as build-depends.
    6. Generates `cabal.project` with source-repository-package entries for `nhcore` and `nhintegrations` pinned to the resolved NeoHaskell commit SHA.
    7. Generates `flake.nix` with haskell.nix overlay and the NeoHaskell input pinned to the same SHA.
    8. Updates `flake.lock` via Nix (the user commits this file).
  - **Build Step**: Runs `nix develop --command cabal build all`.
  - **Output**: Compiled binary at the conventional Cabal output path (e.g., `dist-newstyle/`). Does **not** run the application.

- **`neo run`**
  - **Purpose**: Reconciles, builds, and executes the application.
  - **Required Arguments**: None.
  - **Key Flags**:
    - `--watch`: Rebuilds and restarts the application on file changes via GHCi.
    - `--ci`: Headless execution with plain logs.
  - **Side Effects**: Runs `nix develop --command cabal run all`. The application process runs in the foreground.

- **`neo test`**
  - **Purpose**: Runs the project's full test suite in order: unit tests, then integration tests.
  - **Required Arguments**: None.
  - **Key Flags**:
    - `--watch`: Runs tests in watch mode via GHCi for continuous feedback on unit tests.
    - `--ci`: Disables interactive output. Fails with exit code `1` on any test failure.
  - **Test Execution Order**:
    1. **Unit tests**: Runs `nix develop --command cabal test all`.
    2. **Integration tests**: Auto-starts the application (equivalent of `neo run`), then runs all `.hurl` files discovered in the `tests/` directory tree using the `hurl` test runner. Shuts down the application after tests complete.
  - **Output**: Displays test results with pass/fail counts, durations, and failure details.

- **`neo lock [search-string]`**
  - **Purpose**: Locks event-sourced domain files (Commands, Events, Queries) to make them immutable, enforcing the append-only nature of event sourcing.
  - **Required Arguments**: None (with `--all` flag). Otherwise, a search string for fuzzy-matching against command/event/query files.
  - **Key Flags**:
    - `--all`: Discovers and locks all Command, Event, and Query files currently in the project. Presents the full list to the user for confirmation before locking.
  - **Behavior**:
    1. User runs `neo lock CounterCreated` (or a partial/fuzzy string).
    2. CLI scans the project for files in `Commands/`, `Events/`, and `Queries/` directories.
    3. CLI finds the best match(es) and presents them interactively: "Lock `src/Starter/Counter/Events/CounterCreated.hs`? [Y/n]".
    4. On confirmation, the file path is appended to `.locked-files`, the manifest is staged, and a commit is created (`lock: <filepath>`).
  - **Side Effects**: Modifies `.locked-files`, stages it, and creates a git commit.

- **`neo lock install`**
  - **Purpose**: Explicitly installs the git pre-commit hook that enforces file locking.
  - **Note**: This is also done automatically by any `neo` command when the hook is missing, but this subcommand allows manual installation.

## 5. Output & Developer Experience (DX)

### Mascot & Branding
- The Neo mascot (CRT monitor with a face) is displayed as ASCII art in the CLI banner when running `neo` with no arguments, during `neo new` interactive prompts, and on major success messages. The mascot is suppressed when `--ci` is active.

### Success/Error Reporting
- **Errors** are displayed in red with actionable hints. Examples:
  - "Failed to parse `neo.json` at line 4, column 12: unexpected character. Missing comma?"
  - "No `neo.json` found. Run `neo new` to create a project, or `cd` into an existing one."
  - "Directory `my-app` already exists. Choose a different name or delete it first."
  - "`src/Counter/Events/CounterCreated.hs` is not tracked by git. Stage and commit it first."
- **Success** messages are green and celebratory, often accompanied by the Neo mascot.
- **Warnings** are yellow. Used for soft prerequisite notices (e.g., "direnv is not installed. Install it for automatic HLS integration in your editor: https://direnv.net").

### Interactivity
- `neo new` runs a full interactive interview (name, version, description, author, license) when the project name is not provided as a positional argument. Uses smart defaults (version defaults to `0.1.0`, license defaults to `Apache-2.0`).
- `neo lock` presents fuzzy-match results for interactive confirmation.
- `neo lock --all` presents the full file list before committing.

### Progress Indicators
- Spinners are used during long-running tasks (fetching starter template tarball, resolving dependencies, reconciling Nix/Cabal).
- Progress bars track build compilation and test suite completion.
- In `--watch` mode, the terminal is cleared on each rebuild/recheck and fresh results are displayed.
- All animations, colors, and mascot output are disabled when `--ci` is active.

### Prerequisite Notices
- **Nix not installed** (hard blocker): Red error with installation link: "Nix is required but not found. Install it: https://nixos.org/download".
- **direnv not installed** (soft warning): Yellow notice: "direnv is not installed. Install it for automatic HLS integration in your editor."
- **Newer NeoCLI version available** (info): Single line after command output: "Neo v0.3.0 is available. You're on v0.2.1."

## 6. Side Effects

### Filesystem
- `neo new` creates a new directory populated with the starter template, a generated `neo.json`, a `.gitignore` (which includes `*.cabal`, `flake.nix`, `cabal.project`, `dist-newstyle/`, `result`, `.direnv/`), and initializes a git repository.
- `neo build`, `neo run`, and `neo test` generate/overwrite `<name>.cabal`, `cabal.project`, and `flake.nix` from `neo.json` on every invocation. These files are gitignored build artifacts.
- `neo lock` modifies `.locked-files`, stages it, and creates a git commit.

### Network
- `neo new` downloads a tarball from `github.com/NeoHaskell/neohaskell-starter`.
- `neo build` resolves dependencies from `github.com/NeoHaskell/neopackages` and fetches Nix inputs.
- `neo` checks `github.com/NeoHaskell/neocli` (or equivalent) for newer release tags on every invocation (non-blocking, best-effort).

### System
- `neo build --watch` and `neo run --watch` spawn GHCi processes.
- `neo build`/`neo run`/`neo test` invoke `nix develop --command cabal ...` as a subprocess.
- `neo test` spawns the application process for integration tests and terminates it after.
- `neo lock install` (and lazy auto-install) writes to `.git/hooks/pre-commit`.

## 7. User Flows

### 7.1 Scaffolding a New Project (Interactive)
1. User runs `neo new`.
2. CLI displays the Neo mascot banner.
3. CLI prompts: "What is the name of your project?" — user enters `my-app`.
4. CLI prompts: "Version?" — user presses Enter to accept default `0.1.0`.
5. CLI prompts: "Description?" — user enters "My first NeoHaskell app" or presses Enter to skip.
6. CLI prompts: "Author?" — user enters their name or presses Enter to skip.
7. CLI prompts: "License?" — user presses Enter to accept default `Apache-2.0`.
8. CLI displays a spinner: "Fetching starter template...".
9. CLI downloads and extracts the starter template tarball into `my-app/`.
10. CLI generates `neo.json` with the user's answers.
11. CLI runs `git init` in the new directory and installs the lock pre-commit hook.
12. CLI outputs a green success message with the Neo mascot: "Project my-app is ready! Run `cd my-app && neo build` to get started."

### 7.2 Scaffolding a New Project (CI)
1. CI script runs `neo new my-app --ci`.
2. CLI skips all prompts, uses defaults for version/description/author/license.
3. CLI fetches the template, generates `neo.json`, initializes git.
4. CLI outputs plain log lines and exits with code `0`.

### 7.3 First Build
1. User runs `neo build` inside a freshly scaffolded project.
2. CLI parses `neo.json` and resolves the `neo-version` tag to a commit SHA.
3. CLI resolves dependencies from NeoPackages.
4. CLI scans `src/` to discover all `.hs` modules.
5. CLI generates `my-app.cabal`, `cabal.project`, and `flake.nix` for the first time.
6. CLI displays a spinner: "Building project...".
7. CLI runs `nix develop --command cabal build all`.
8. CLI outputs a green success message with build time.

### 7.4 Local Development Loop
1. User runs `neo build --watch`.
2. CLI reconciles `neo.json` into Nix/Cabal configuration files.
3. CLI spawns GHCi with injected module loading and begins watching the filesystem.
4. CLI outputs: "Watching for changes... (press Ctrl+C to stop)".
5. Upon saving a file, the CLI clears the terminal and outputs the fast typechecking results from GHCi.
6. On `Ctrl+C`, CLI cleanly shuts down GHCi and prints "Stopped watching."

### 7.5 Running the Application
1. User runs `neo run`.
2. CLI reconciles and runs `nix develop --command cabal run all`.
3. The application starts in the foreground.
4. On `Ctrl+C`, CLI forwards the signal and shuts down the application.

### 7.6 Running Tests
1. User runs `neo test`.
2. CLI reconciles configuration.
3. CLI runs unit tests via `nix develop --command cabal test all`.
4. CLI displays unit test results.
5. CLI auto-starts the application for integration tests.
6. CLI runs all `.hurl` files discovered in `tests/` using the `hurl` runner.
7. CLI shuts down the application.
8. CLI displays a summary: total tests, passed, failed, duration.

### 7.7 Running Tests in CI
1. CI script runs `neo test --ci`.
2. CLI skips all spinners and UI animations.
3. CLI runs unit tests, then integration tests (with auto-start/stop of the application).
4. CLI outputs raw standard logs.
5. CLI exits with code `0` on success or `1` on failure.

### 7.8 Locking a Domain File
1. User runs `neo lock CounterCreated`.
2. CLI scans `src/` for files in `Commands/`, `Events/`, and `Queries/` directories.
3. CLI finds `src/Starter/Counter/Events/CounterCreated.hs` as the best match.
4. CLI prompts: "Lock `src/Starter/Counter/Events/CounterCreated.hs`? [Y/n]".
5. User confirms.
6. CLI appends the path to `.locked-files`, stages it, and commits: "lock: src/Starter/Counter/Events/CounterCreated.hs".
7. CLI outputs a green success message.

### 7.9 Locking All Domain Files
1. User runs `neo lock --all`.
2. CLI scans and discovers all Command, Event, and Query files.
3. CLI presents the full list: "The following 9 files will be locked: ..." and asks for confirmation.
4. User confirms.
5. CLI appends all paths to `.locked-files`, stages, and commits.

### 7.10 Error: No Workspace Found
1. User runs `neo build` in a directory without `neo.json`.
2. CLI outputs a red error: "No `neo.json` found. Run `neo new` to create a project, or `cd` into an existing one."
3. CLI exits with code `1`.

### 7.11 Error: Invalid Configuration
1. User runs `neo build` with a malformed `neo.json`.
2. CLI outputs a red error with line/column: "Failed to parse `neo.json` at line 4, column 12: unexpected character."
3. CLI exits with code `1`.

### 7.12 Error: Compilation Failure in Watch Mode
1. User is in `neo build --watch` and saves a file with a type error.
2. CLI clears the terminal and displays the GHCi error in red with the source location.
3. CLI remains running and prints: "Fix the error and save to retry."
4. GHCi stays alive — no restart needed.

### 7.13 Error: Directory Already Exists
1. User runs `neo new my-app` but `my-app/` already exists.
2. CLI outputs a red error: "Directory `my-app` already exists. Choose a different name or delete it first."
3. CLI exits with code `1`.

### 7.14 Error: Nix Not Installed
1. User runs any `neo` command that requires Nix (`build`, `run`, `test`).
2. CLI outputs a red error: "Nix is required but not found. Install it: https://nixos.org/download"
3. CLI exits with code `1`.

### 7.15 Error: Network Unavailable During Scaffold
1. User runs `neo new my-app` but the network is unreachable.
2. CLI displays a red error: "Failed to fetch the starter template. Check your internet connection and try again."
3. CLI exits with code `1`. No partial directory is left behind.

## 8. Configuration & Environment

### `neo.json` — Project Configuration
All project configuration is managed via a single `neo.json` file at the root of the project. There is no global configuration or dotfile.

**Representative example:**
```json
{
  "name": "my-app",
  "version": "0.1.0",
  "neo-version": "0.3.0",
  "description": "My first NeoHaskell app",
  "author": "Jane Doe",
  "license": "Apache-2.0",
  "dependencies": {
    "some-neo-package": "^1.0.0",
    "hackage:aeson": ">=2.0 && <3.0",
    "my-local-lib": "file:../libs/my-local-lib",
    "experimental": "git+https://github.com/user/repo.git#main"
  }
}
```

**Field reference:**
- `name` (required): Project name. Kebab-case.
- `version` (required): Project version. Semver.
- `neo-version` (required): NeoHaskell version to use. Resolved to a git tag on `NeoHaskell/neohaskell`.
- `description` (optional): One-line project description.
- `author` (optional): Author name.
- `license` (optional): SPDX license identifier. Defaults to `Apache-2.0`.
- `dependencies` (optional): Map of package names to version constraints.
  - **NeoPackages** (default): `"package-name": "^1.0.0"` — resolved from the NeoPackages registry at `github.com/NeoHaskell/neopackages`. Uses npm-style semver ranges (`^`, `~`, `>=`, `*`, etc.).
  - **Hackage**: `"hackage:package-name": ">=2.0 && <3.0"` — prefixed with `hackage:`, pulled from Hackage.
  - **Local**: `"name": "file:../path/to/lib"` — local filesystem dependency.
  - **Git**: `"name": "git+https://github.com/user/repo.git#branch-or-tag"` — git remote dependency.

**Implicit dependencies** (always included, never declared by the user): `nhcore`, `nhintegrations`, `base`.

**Hardcoded build settings** (non-overridable, generated into `.cabal`):
- GHC options: `-Wall -Wno-orphans -threaded -fno-warn-partial-type-signatures -fno-warn-name-shadowing -Werror`
- Default extensions: `NoImplicitPrelude`, `OverloadedStrings`, `Strict`, `TemplateHaskell`, `TypeFamilies`, and all other extensions from the NeoHaskell standard set.
- Source directory: Always `src/`.
- Module discovery: Automatic — all `.hs` files in `src/` are included.

### Generated Files (Build Artifacts)
The following files are generated by reconciliation and must **never** be hand-edited. They are included in `.gitignore` by `neo new`:
- `<name>.cabal`
- `cabal.project`
- `flake.nix`

The following generated file **must** be committed by the user:
- `flake.lock` — the Nix lockfile ensuring reproducible builds.

### Environment Variables
- `CI=true`: Acts as an implicit `--ci` flag. When detected, all commands behave as if `--ci` were passed.

### Prerequisite Software
- **Nix** (with flakes enabled): Hard requirement. `neo` checks for it before any build/run/test command and exits with an actionable error if missing.
- **direnv**: Soft requirement. `neo` emits a yellow warning if not found, recommending installation for HLS (Haskell Language Server) integration in text editors. The `.envrc` file generated by `neo new` contains `use flake`.
- **Git**: Hard requirement. Needed for `neo new` (initializes repo), `neo lock` (commits manifest), and the lock hook system.

## 9. Phased Roadmap
- **Phase 1 (MVP)**: `neo new`, `neo build`, `neo run`, `neo test`, `neo lock`, `neo lock install`. Full interactive DX with Neo mascot, `--ci` mode, prerequisite checks, and update notifications.
- **Phase 2**: `neo mcp` — Model Context Protocol server exposing NeoHaskell project context and idioms to AI coding agents.
- **Phase 3**: `neo ide` — Graphical event modeling canvas and web UI control plane for visualizing event models, inspecting state, and triggering queries.
- **Future**: Infrastructure/services management (e.g., auto-starting Docker dependencies), shell completion generation (`neo completions bash/zsh/fish`), `--quiet` flag.

## 10. Out of Scope
- Hand-managing or directly editing Cabal, `cabal.project`, or Nix files (these are treated as pure build artifacts).
- Supporting non-NeoHaskell standard project structures or custom source directory layouts.
- Deployments or cloud provisioning.
- Overriding GHC options or language extensions via `neo.json`.
- Infrastructure/services management (Docker, databases) — deferred to a future phase.
- Monorepo or multi-package workspace support.
- MCP server and IDE web UI — deferred to Phase 2 and Phase 3 respectively.
