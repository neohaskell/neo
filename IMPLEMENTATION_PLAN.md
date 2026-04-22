# NeoCLI — Implementation Plan

## 1. Goal

Build NeoCLI, the NeoHaskell CLI tool, in Rust. It must deliver a polished, branded developer experience with the **Neo** mascot, rich terminal output (spinners, progress bars, colored errors), an interactive project scaffolding interview, and a `--ci` headless mode — all backed by robust subprocess management for Nix/Cabal/GHCi.

---

## 2. Technology Stack & Crate Selection

| Concern | Crate | Rationale |
|---|---|---|
| Argument parsing | `clap` (v4, derive) | Industry standard; derive API for type-safe subcommands & global flags |
| Rich terminal output | `ratatui` + `crossterm` | Immediate-mode TUI; full control over layout, color, and rendering |
| Interactive prompts | Custom widgets on `ratatui` | PRD demands branded prompts with Neo mascot inline — `dialoguer` can't do this |
| Spinners / progress | Custom `ratatui` widgets | Unified rendering pipeline; no fighting between `indicatif` and `ratatui` |
| Async runtime | `tokio` | Subprocess spawning, signal handling, concurrent background checks |
| HTTP client | `reqwest` | Tarball download, GitHub API for update checks and tag resolution |
| JSON parsing | `serde` + `serde_json` | `neo.json` config, NeoPackages registry manifest |
| Fuzzy matching | `nucleo` | High-performance fuzzy matcher (powers Helix editor); for `neo lock` |
| File watching | `notify` | Cross-platform filesystem watcher for `--watch` mode |
| Error handling | `miette` | Beautiful diagnostic errors with source spans — fits PRD error style |
| Tracing/logging | `tracing` + `tracing-subscriber` | Structured logging; swap between pretty (interactive) and JSON (CI) |
| Semver | `semver` | Parse and compare version constraints in `neo.json` dependencies |
| ASCII art banner | `tui-big-text` | Ratatui-native large text widget for the Neo mascot banner |
| Template engine | `minijinja` | Generate `.cabal`, `flake.nix`, `cabal.project` from templates |
| Tar extraction | `flate2` + `tar` | Extract starter template tarball |
| Git operations | `gix` (gitoxide) | Pure-Rust git for `git init`, staging, committing, hook installation |

---

## 3. Project Structure

```
neocli/
├── [x] Cargo.toml
├── assets/
│   ├── [x] neo_mascot.txt          # ASCII art for the Neo CRT mascot
│   └── templates/                  # .cabal, flake.nix, cabal.project templates
│       ├── [ ] project.cabal.j2
│       ├── [ ] flake.nix.j2
│       └── [ ] cabal.project.j2
├── src/
│   ├── [x] main.rs                 # Entry point: parse CLI, init runtime, dispatch
│   ├── [x] cli.rs                  # Clap derive structs (Cli, Commands, global flags)
│   ├── [x] app.rs                  # Top-level command dispatcher
│   ├── [ ] config.rs               # neo.json parsing, validation, NeoConfig struct
│   ├── [x] errors.rs               # miette error types (NeoError enum)
│   ├── [x] theme.rs                # Semantic color palette & style definitions
│   ├── [x] output.rs               # OutputMode (Interactive/CI) + rendering context
│   │
│   ├── commands/                   # One module per command
│   │   ├── [x] mod.rs
│   │   ├── [x] new.rs              # neo new (interactive interview + CI) (scaffolded)
│   │   ├── [x] build.rs            # neo build (reconcile + compile) (scaffolded)
│   │   ├── [x] run.rs              # neo run (reconcile + execute) (scaffolded)
│   │   ├── [x] test.rs             # neo test (unit + integration) (scaffolded)
│   │   ├── [x] lock.rs             # neo lock [search] / --all (scaffolded)
│   │   └── [ ] lock_install.rs     # neo lock install
│   │
│   ├── reconcile/                  # Config → build artifact generation
│   │   ├── [x] mod.rs
│   │   ├── [ ] cabal.rs            # .cabal file generation
│   │   ├── [ ] flake.rs            # flake.nix generation
│   │   ├── [ ] cabal_project.rs    # cabal.project generation
│   │   ├── [ ] modules.rs          # src/ module auto-discovery
│   │   └── [ ] resolve.rs          # Dependency resolution (NeoPackages, Hackage, git, file)
│   │
│   ├── tui/                        # All terminal UI components
│   │   ├── [x] mod.rs
│   │   ├── [x] mascot.rs           # Neo mascot ASCII art widget
│   │   ├── [ ] banner.rs           # Top banner with big text + mascot
│   │   ├── [ ] spinner.rs          # Animated spinner widget
│   │   ├── [ ] progress.rs         # Progress bar widget
│   │   ├── [ ] prompt.rs           # Interactive input prompt widget
│   │   ├── [ ] confirm.rs          # Y/n confirmation widget
│   │   ├── [ ] success.rs          # Green success message with mascot
│   │   ├── [ ] error_display.rs    # Red error display with hints
│   │   └── [ ] watch.rs            # Watch-mode full-screen layout
│   │
│   ├── subprocess/                 # Nix/Cabal/GHCi process management
│   │   ├── [x] mod.rs
│   │   ├── [x] nix.rs              # nix develop --command ... wrappers (scaffolded)
│   │   ├── [ ] ghci.rs             # GHCi session management for --watch
│   │   └── [ ] hurl.rs             # hurl test runner integration
│   │
│   ├── [ ] git.rs                  # Git operations (init, commit, hook install)
│   ├── [ ] lock.rs                 # .locked-files manifest + pre-commit hook logic
│   ├── [ ] network.rs              # HTTP: tarball download, registry fetch, update check
│   └── [ ] prereqs.rs              # Prerequisite checks (nix, git, direnv)
│
└── tests/
    ├── integration/
    │   ├── [ ] new_test.rs
    │   ├── [ ] build_test.rs
    │   └── [ ] lock_test.rs
    └── snapshots/                  # insta snapshot tests for TUI output
```

---

## 4. Architecture: The Elm Architecture (TEA) for TUI Rendering

NeoCLI uses a **hybrid architecture**: Clap handles CLI parsing, then each command either runs a **one-shot render** (non-interactive) or enters a **TEA loop** (interactive prompts, watch mode).

### 4.1 Core Loop (for interactive modes)

```rust
/// The main TEA loop used by interactive commands (neo new, neo lock)
pub struct App<S: State> {
    state: S,
    terminal: DefaultTerminal,
    should_quit: bool,
}

impl<S: State> App<S> {
    pub async fn run(&mut self) -> miette::Result<S::Output> {
        loop {
            // VIEW: render current state
            self.terminal.draw(|frame| self.state.view(frame))?;

            // EVENT: collect input
            if crossterm::event::poll(Duration::from_millis(50))? {
                let event = crossterm::event::read()?;
                // UPDATE: produce new state + optional side-effect
                let action = self.state.update(event)?;
                match action {
                    Action::Continue => {}
                    Action::Quit(output) => return Ok(output),
                }
            }

            // TICK: update animations (spinners, etc.)
            self.state.tick();
        }
    }
}

pub trait State {
    type Output;
    fn view(&self, frame: &mut Frame);
    fn update(&mut self, event: Event) -> miette::Result<Action<Self::Output>>;
    fn tick(&mut self);
}
```

### 4.2 One-Shot Renders (CI mode / non-interactive)

For CI mode and simple success/error output, bypass the loop entirely:

```rust
// CI mode: plain text to stdout/stderr
if output_mode.is_ci() {
    eprintln!("error: {}", message);
    std::process::exit(1);
}

// Interactive one-shot: render a single frame (e.g., success message)
terminal.draw(|frame| render_success(frame, &message, &theme))?;
```

---

## 5. CLI Parsing (Clap Derive)

```rust
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "neo", version, about = "The NeoHaskell CLI")]
#[command(propagate_version = true)]
pub struct Cli {
    /// Enable debug-level output
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// Disable interactive prompts, animations, and colors
    #[arg(long, global = true)]
    pub ci: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Scaffold a new NeoHaskell project
    New {
        /// Project name (required in --ci mode)
        project_name: Option<String>,
    },
    /// Reconcile config and build the project
    Build {
        /// Watch mode with GHCi hot-reloading
        #[arg(long)]
        watch: bool,
    },
    /// Reconcile, build, and run the application
    Run {
        /// Watch mode with auto-restart
        #[arg(long)]
        watch: bool,
    },
    /// Run unit tests, then integration tests
    Test {
        /// Watch mode for continuous testing
        #[arg(long)]
        watch: bool,
    },
    /// Lock event-sourced domain files
    Lock(LockArgs),
}

#[derive(clap::Args)]
pub struct LockArgs {
    #[command(subcommand)]
    pub subcommand: Option<LockSubcommand>,

    /// Fuzzy search string to match domain files
    pub search: Option<String>,

    /// Lock all discovered domain files
    #[arg(long)]
    pub all: bool,
}

#[derive(Subcommand)]
pub enum LockSubcommand {
    /// Install the git pre-commit lock hook
    Install,
}
```

### Global Behavior (runs on every invocation)

Implemented in `main.rs` after parsing, before dispatching:

```rust
// 1. Detect CI environment
let ci_mode = cli.ci || std::env::var("CI").is_ok();

// 2. Background checks (non-blocking, best-effort)
let update_handle = tokio::spawn(network::check_for_updates());
let hook_handle = tokio::spawn(git::ensure_lock_hook());

// 3. Dispatch command
let result = app::dispatch(cli.command, output_mode).await;

// 4. Print update notice (if available)
if let Ok(Some(notice)) = update_handle.await { ... }
```

---

## 6. Output Mode & Theme System

### 6.1 OutputMode

```rust
pub enum OutputMode {
    Interactive { terminal: DefaultTerminal },
    Ci,
}

impl OutputMode {
    pub fn is_ci(&self) -> bool { matches!(self, Self::Ci) }
}
```

All command functions accept `&OutputMode` and branch on it:
- **Interactive**: use `ratatui` rendering, spinners, mascot, colors
- **CI**: use `println!`/`eprintln!` with plain structured text, exit codes

### 6.2 Theme (Semantic Color Palette)

```rust
use ratatui::style::{Color, Style, Modifier};

pub struct Theme {
    pub primary:    Color,  // Neo brand — cyan/teal
    pub success:    Color,  // Green
    pub error:      Color,  // Red
    pub warning:    Color,  // Yellow/amber
    pub info:       Color,  // Blue
    pub muted:      Color,  // Dim gray
    pub text:       Color,  // White/light
    pub bg:         Color,  // Terminal default
    pub accent:     Color,  // Highlight — magenta/purple
}

impl Theme {
    pub fn neo() -> Self {
        Self {
            primary:  Color::from_u32(0x0050E0D0), // Teal
            success:  Color::from_u32(0x0066D96E), // Green
            error:    Color::from_u32(0x00FF6B6B), // Soft red
            warning:  Color::from_u32(0x00FFD93D), // Amber
            info:     Color::from_u32(0x006BC5F0), // Sky blue
            muted:    Color::from_u32(0x006C757D), // Gray
            text:     Color::from_u32(0x00F8F9FA), // Near-white
            bg:       Color::Reset,
            accent:   Color::from_u32(0x00BB86FC), // Purple
        }
    }

    pub fn style_error(&self) -> Style {
        Style::default().fg(self.error).add_modifier(Modifier::BOLD)
    }
    pub fn style_success(&self) -> Style {
        Style::default().fg(self.success).add_modifier(Modifier::BOLD)
    }
    // ... etc
}
```

---

## 7. TUI Design Guidelines

These guidelines ensure NeoCLI delivers a **fantastic, polished** terminal experience.

### 7.1 Mascot Rendering

The Neo CRT mascot is stored as ASCII art in `assets/neo_mascot.txt` and embedded via `include_str!`. It is rendered as a `Paragraph` widget with the brand `primary` color.

**Rules:**
- Show mascot on: bare `neo` invocation, `neo new` interview banner, success messages
- Never show mascot in `--ci` mode
- Mascot is rendered in a fixed-width `Rect` — measure the art and constrain layout accordingly
- Use `tui-big-text` for the "NEO" title text beside the mascot

### 7.2 Layout Principles

```
┌─────────────────────────────────────────┐
│  ╔═══╗   N E O                          │  ← Banner area (mascot + big text)
│  ║ :)║   The NeoHaskell CLI             │
│  ╚═══╝                                  │
├─────────────────────────────────────────┤
│  [Content area — prompts, output, etc]  │  ← Main content
│                                          │
├─────────────────────────────────────────┤
│  Neo v0.2.1 → v0.3.0 available          │  ← Footer (update notice, status)
└─────────────────────────────────────────┘
```

- Use `Layout::vertical` with `Constraint::Length` for banner/footer, `Constraint::Fill(1)` for content
- Minimum terminal width: 60 columns. Detect and warn if smaller
- All widgets respect the `Rect` they're given — never overflow

### 7.3 Color & Styling Rules

| Element | Style |
|---|---|
| Errors | `theme.error` + bold + "✗ " prefix |
| Successes | `theme.success` + bold + "✓ " prefix |
| Warnings | `theme.warning` + "⚠ " prefix |
| Spinner text | `theme.primary` + animation char |
| Prompt labels | `theme.text` + bold |
| User input | `theme.accent` |
| Muted/hint text | `theme.muted` + dim |
| File paths in errors | `theme.accent` + underline |
| Keyboard shortcuts | `theme.info` + `[brackets]` |

### 7.4 Animation & Timing

- **Spinners**: Braille pattern cycle (`⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏`), 80ms per frame
- **Progress bars**: `Gauge` widget, updates on each Nix/Cabal output line parsed
- **Tick rate**: 50ms poll interval (20 FPS) — sufficient for smooth animation without CPU waste
- **Transitions**: When switching between interview prompts, use instant clear + redraw (no animation delay)

### 7.5 Interactive Prompts (neo new interview)

Each prompt is a `State` implementation rendered in the TEA loop:

```
  What is the name of your project?
  ❯ my-app█
    ↵ Enter to confirm
```

- Blinking cursor via tick-based toggle
- Input validation inline (e.g., kebab-case check on project name)
- Default values shown as muted placeholder text: `Version? (0.1.0)`
- Press Enter on empty input → accept default
- Arrow keys navigate multi-choice (license selector)

### 7.6 Watch Mode Layout

```
┌─────────────────────────────────────────┐
│  neo build --watch                       │
├─────────────────────────────────────────┤
│                                          │
│  ✓ All good! No errors.                 │
│                                          │
│  --- or on error: ---                    │
│                                          │
│  ✗ src/Counter.hs:42:10                 │
│    Couldn't match type 'Int' with 'Text' │
│                                          │
├─────────────────────────────────────────┤
│  Watching... (Ctrl+C to stop)  12:34:56  │
└─────────────────────────────────────────┘
```

- Full terminal clear + redraw on each file change
- Errors displayed with source location in `theme.error`
- Status bar at bottom shows mode + timestamp

### 7.7 CI Mode Output

All rich formatting is stripped. Output is plain, parseable text:

```
[info] Parsing neo.json...
[info] Resolving neo-version 0.3.0 → SHA abc1234
[info] Generating my-app.cabal
[info] Generating flake.nix
[info] Building project...
[ok] Build succeeded in 42.3s
```

- Prefixed with `[info]`, `[ok]`, `[warn]`, `[error]`
- No ANSI escape codes, no unicode symbols
- Timestamps if `--verbose` is set

---

## 8. Command Implementation Details

### 8.1 `neo new`

**Interactive flow** (TEA loop with `NewProjectState`):
1. Render banner with mascot
2. Step through prompts: name → version → description → author → license
3. On completion, exit loop with collected `ProjectConfig`
4. One-shot: show spinner → download tarball → extract → generate `neo.json` → git init → install hook → success

**CI flow**: Require `project_name` arg, use defaults, skip prompts.

**Cleanup on failure**: If any step fails after directory creation, delete the partial directory.

### 8.2 `neo build`

1. `prereqs::require_nix()?` — check Nix is installed
2. `prereqs::warn_direnv()` — soft warning
3. `config::load("neo.json")?` — parse + validate
4. `reconcile::run(&config)?` — generate .cabal, cabal.project, flake.nix
5. `subprocess::nix::build()?` — run `nix develop --command cabal build all`
6. Display success with build duration

**Watch mode**: After step 4, spawn GHCi via `subprocess::ghci::start()`, enter watch TEA loop with `notify` watcher.

### 8.3 `neo run`

Same as build, but step 5 uses `cabal run all`. The child process runs in foreground. `Ctrl+C` is forwarded via `tokio::signal::ctrl_c()` → `SIGTERM` to child.

### 8.4 `neo test`

1. Build steps 1–4
2. Run unit tests: `nix develop --command cabal test all`
3. Display unit test results
4. Auto-start application (like `neo run`)
5. Discover `.hurl` files in `tests/`
6. Run `hurl` against each file
7. Kill application process
8. Display summary: total/passed/failed/duration

### 8.5 `neo lock [search]`

1. Scan `src/` for files in `Commands/`, `Events/`, `Queries/` dirs
2. If `--all`: present full list → confirm → lock all
3. If search string: fuzzy-match with `nucleo`, present best matches → confirm → lock
4. Lock = append to `.locked-files`, stage via `gix`, commit with message `lock: <path>`

### 8.6 `neo lock install`

Write the pre-commit hook script to `.git/hooks/pre-commit`. The hook reads `.locked-files` and rejects commits that modify any listed file.

---

## 9. Subprocess Management

### 9.1 Standard Commands (build/run/test)

```rust
pub async fn nix_develop_command(args: &[&str], output: &OutputMode) -> miette::Result<ExitStatus> {
    let mut child = tokio::process::Command::new("nix")
        .args(["develop", "--command"])
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    // Stream output lines to the TUI or stdout
    // Handle Ctrl+C gracefully
    tokio::select! {
        status = child.wait() => Ok(status?),
        _ = tokio::signal::ctrl_c() => {
            // Send SIGTERM, wait, then SIGKILL if needed
            graceful_shutdown(&mut child).await
        }
    }
}
```

### 9.2 GHCi Watch Mode

GHCi runs as a long-lived child process. On file change:
1. Send `:reload` to GHCi's stdin
2. Capture stdout/stderr
3. Parse for errors/warnings
4. Redraw TUI with results

---

## 10. Error Handling Strategy

All errors use `miette::Diagnostic` for rich reporting:

```rust
#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum NeoError {
    #[error("No `neo.json` found")]
    #[diagnostic(
        code(neo::no_workspace),
        help("Run `neo new` to create a project, or `cd` into an existing one.")
    )]
    NoWorkspace,

    #[error("Failed to parse `neo.json` at line {line}, column {col}: {reason}")]
    #[diagnostic(code(neo::invalid_config))]
    InvalidConfig { line: usize, col: usize, reason: String },

    #[error("Directory `{name}` already exists")]
    #[diagnostic(
        code(neo::dir_exists),
        help("Choose a different name or delete it first.")
    )]
    DirectoryExists { name: String },

    #[error("Nix is required but not found")]
    #[diagnostic(
        code(neo::nix_missing),
        url("https://nixos.org/download"),
        help("Install Nix: https://nixos.org/download")
    )]
    NixNotFound,

    #[error("Failed to fetch the starter template")]
    #[diagnostic(
        code(neo::network),
        help("Check your internet connection and try again.")
    )]
    NetworkError(#[source] reqwest::Error),
}
```

In CI mode, errors are rendered as plain text. In interactive mode, they're rendered with the `theme.error` style and the mascot showing a sad face.

---

## 11. Terminal Setup & Panic Safety

```rust
fn main() -> miette::Result<()> {
    // Install miette's panic hook for pretty panics
    miette::set_hook(Box::new(|_| {
        Box::new(miette::NarratableReportHandler::new())
    }))?;

    // For interactive mode, use ratatui::init() which handles:
    // - enable_raw_mode()
    // - EnterAlternateScreen (only for full-screen modes like watch)
    // - panic hook that restores terminal
    // For one-shot renders, use inline terminal without alternate screen

    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async_main())
}
```

> [!IMPORTANT]
> Only use the alternate screen for full-screen modes (`--watch`, `neo new` interview). For one-shot renders (build success, errors), render inline so output stays visible in scrollback.

---

## 12. Cargo.toml Dependencies

```toml
[package]
name = "neo"
version = "0.1.0"
edition = "2024"

[dependencies]
# CLI parsing
clap = { version = "4", features = ["derive", "env"] }

# TUI
ratatui = { version = "0.30", features = ["crossterm"] }
crossterm = "0.28"
tui-big-text = "0.7"

# Async
tokio = { version = "1", features = ["full"] }

# HTTP
reqwest = { version = "0.12", features = ["json", "rustls-tls"], default-features = false }

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# Error handling
miette = { version = "7", features = ["fancy"] }
thiserror = "2"

# Logging
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# Fuzzy matching
nucleo = "0.5"

# File watching
notify = "7"

# Semver
semver = "1"

# Templates
minijinja = "2"

# Archive
flate2 = "1"
tar = "0.4"

# Git
gix = { version = "0.70", default-features = false, features = ["basic", "index"] }

[dev-dependencies]
insta = { version = "1", features = ["yaml"] }
assert_cmd = "2"
predicates = "3"
tempfile = "3"
```

---

## 13. Verification Plan

### Automated Tests

1. **Unit tests**: Each module (`config.rs`, `reconcile/*.rs`, `lock.rs`) gets `#[cfg(test)]` modules
2. **Snapshot tests**: Use `insta` to snapshot TUI output rendered to a test buffer
3. **Integration tests**: Use `assert_cmd` to test full CLI invocations
   - `neo --version` prints version
   - `neo new my-app --ci` creates correct directory structure
   - `neo build --ci` in a fixture project produces expected files
   - `neo lock install` creates `.git/hooks/pre-commit`
4. **Error path tests**: Verify each error in §10 triggers the correct exit code and message

### Manual Verification

1. Run `neo` bare → verify mascot banner renders correctly
2. Run `neo new` → step through full interactive interview
3. Run `neo build --watch` → verify terminal clears and shows GHCi output on file save
4. Run `neo test --ci` → verify plain text output and correct exit codes
5. Test in a narrow terminal (< 60 cols) → verify graceful degradation

---

## Resolved Decisions

1. **Mascot art**: Use a placeholder ASCII art CRT monitor embedded via `include_str!`. The user will replace it with final art later.

2. **NeoPackages registry format**: We define the schema ourselves. The registry is a JSON object mapping package names to metadata:
   ```json
   {
     "packages": {
       "some-neo-package": {
         "description": "A useful package",
         "repository": "https://github.com/NeoHaskell/some-neo-package",
         "versions": {
           "1.0.0": { "sha": "abc123...", "tag": "v1.0.0" },
           "1.1.0": { "sha": "def456...", "tag": "v1.1.0" }
         }
       }
     }
   }
   ```

3. **GHCi integration**: Use a straightforward `:set` / `:load` sequence. On startup, scan `src/` for all `.hs` modules, then issue `:set -isrc` followed by `:load *ModuleName` for each discovered module. On file change, issue `:reload`. Parse GHCi's stdout/stderr for error/warning lines matching the standard `file:line:col: error:` pattern.

4. **Hurl runner**: Installed via Nix as part of the `flake.nix` dev shell. The generated `flake.nix` will include `hurl` in `buildInputs` so it's available on PATH inside `nix develop`.

5. **Platform scope**: **Linux + macOS only**. No Windows support — Nix with flakes is the hard dependency and doesn't have first-class Windows support.
