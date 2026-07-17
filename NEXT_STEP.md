# Next Step

## Goal
Expand Neo IDE collaboration beyond the verified move-only slice with typed structural board commands and deterministic reconnect repair.

## Tasks
- [x] Add warning comment to `project.cabal.j2`
- [x] Ensure `neo new` calls `reconcile::run` before initial commit
- [x] Safeguard `.gitignore` against ignoring `*.cabal`
- [x] Verify fix with a new project and `neo build`

## Collaboration follow-up
- [ ] Add typed add/remove/rename/connect/group commands with host validation and tests
- [ ] Bound host command deduplication and signed-wire/snapshot resource usage for long-running or adversarial sessions
- [x] Add missing-sequence detection and explicit snapshot repair after a lagged notification
- [ ] Add read-only share capabilities distinct from edit tickets
- [ ] Add stable collaborator identity/display-name configuration

## Verification blocker observed 2026-07-17
- `cargo test` did not complete after more than 11 minutes because `test_neo_build_ci`, `test_neo_test_ci`, and `test_neo_test_hurl_discovery` remained inside their external Nix/CLI subprocesses. No failure was reported; `test_neo_run_ci` eventually passed. The run was terminated to avoid leaving unbounded background work.
- Collaboration tests, frontend tests/build, and the remaining Rust suites must remain green independently while this pre-existing long-running integration-test behavior is investigated.
