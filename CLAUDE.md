# NeoCLI agent guide

See `AGENTS.md` for the full Ralph-loop workflow (`STATE.md` → `NEXT_STEP.md` → `IMPLEMENTATION_PLAN.md`). This file documents harness-specific testing details.

## Test layers

| Layer | Command | Network | Speed | Notes |
|---|---|---|---|---|
| Unit + integration | `cargo test` | Stubbed (`NEO_SKIP_NETWORK=1` inside tests) | seconds | Default suite. `tests/integration_tests.rs` uses `assert_cmd::Command::cargo_bin("neo")` — runs a cargo-built debug binary. |
| End-to-end (shell-level) | `cargo test --test e2e -- --ignored --test-threads=1` | Real | minutes (Haskell builds inside) | `tests/e2e.rs` shells out to the **nix-built** `result/bin/neo` against per-scenario sandbox dirs under `target/e2e-sandbox/`. |
| Neo-on-Neo smoke | `./ralph.sh` | Real | minutes | Bash loop driven by the Ralph agent; not wired to `cargo test`. |

## Running the e2e suite

```sh
nix build                                                           # produces result/bin/neo
cargo test --test e2e -- --ignored --test-threads=1 --nocapture
```

Prerequisites (all present inside `nix develop`):

- `result/bin/neo` must exist; the helper panics with a clear hint if missing.
- `nix`, `git`, `pgrep`, `timeout` must be on `PATH`.
- Real network is required — the suite intentionally does not set `NEO_SKIP_NETWORK`. It calls real `git ls-remote https://github.com/NeoHaskell/neohaskell` and downloads the real starter tarball.
- Single-threaded: `--test-threads=1` is mandatory (real network rate limits + shared nix-store lock during builds).

### Env knobs

- `NEO_E2E_KEEP=1` — preserve sandbox directories after each test (success or failure). Default behavior preserves sandboxes only on failure.

### When happy-path scenarios go red

The `build` / `run` / `test` happy-path scenarios in `tests/e2e.rs` (groups D, F, G) require the generated NeoHaskell project to actually compile. The two recurring sources of breakage are:

1. **Starter template drift against upstream `neohaskell` API.** `neo new` tarballs `github.com/NeoHaskell/neo-starter@main` and then `neo build` updates `flake.lock` to the latest `neohaskell` `main`. When upstream renames or removes a module the starter imports (recent example: `Service.Query.Auth` → `Service.AccessControl`, `QueryAuthError` → `AccessError`), the generated project fails GHC compile. Fix in `neo-starter` and push to `main`.
2. **A transitive Haskell dep refusing to build under plain cabal.** Historical example: `jose` needing native crypto paths that only `haskell.nix`'s `hix.project` supplies — fixed in commit `87dde77` by templating the right `flake.nix`.

Either way, do not mask or `#[ignore]` these scenarios — they are the intended signal that the starter ↔ upstream contract is broken.

## When changing CLI behavior

If you change any subcommand surface, error message, output prefix (`[info]` / `[ok]` / `[error]` / `[fail]`), or the generated project layout, look for the affected assertions in both `tests/integration_tests.rs` and `tests/e2e.rs` and update them in the same change.

## Files

- `tests/e2e.rs` — scenarios (each `#[test] #[ignore]`)
- `tests/common/mod.rs` — `Sandbox`, `neo_bin()`, `cmd` wrappers, isolated `HOME` / git identity, prepended `PATH` so the installed pre-commit hook can resolve `neo`
- `tests/integration_tests.rs` — fast, network-stubbed CLI tests via `cargo_bin`
- `ralph.sh` — Ralph-driven smoke loop
