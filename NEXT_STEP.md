# Next Step

## Goal
Fix the issue where `neo build` fails because the `.cabal` file is untracked/gitignored.

## Tasks
- [x] Add warning comment to `project.cabal.j2`
- [x] Ensure `neo new` calls `reconcile::run` before initial commit
- [x] Safeguard `.gitignore` against ignoring `*.cabal`
- [x] Verify fix with a new project and `neo build`
