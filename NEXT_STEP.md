# Next Step

## Goal
Fix smoke test failures caused by upstream dependency issues.

## Action Items
1.  **Investigate Upstream Failure**:
    - [ ] Analyze the `jose-0.11` compilation error found in the smoke test.
    - [ ] Determine if it requires a version pin in `cabal.project.j2` or if it's a GHC compatibility issue.
2.  **Verify Smoke Test**:
    - [ ] Run `./ralph.sh` and ensure it documents the failure in `NEXT_STEP.md` as per the new rules in `AGENTS.md`.
    - [ ] Fix the issue and ensure the smoke test passes completely.

## Status
- `ralph.sh` updated with Neo-on-Neo smoke test.
- `AGENTS.md` updated with strict verification and documentation rules.
- `flake.nix` template improved with system dependencies.
- Smoke test is currently failing on `jose-0.11` build (upstream dependency).
