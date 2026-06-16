#!/usr/bin/env bash
# scripts/build.sh — full pipeline: frontend (TS/Vite) then backend (Rust/cargo).
#
# Run from anywhere; the script chdirs to the repo root.
# Must be inside `nix develop` (provides cargo, rustc, node, npm).
#
# Idempotent:
#   - `npm install` runs only when `assets/ide/node_modules` is absent.
#   - `npm run build` always runs (cheap when nothing changed; Vite caches).
#   - `cargo build` always runs (cargo's own incremental compile is the brake).
#
# Forwards extra args to cargo, so e.g. `scripts/build.sh --release` works.

set -euo pipefail

# repo root = parent of this script's dir
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

for tool in cargo npm node; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        echo "[error] required tool \`$tool\` not found on PATH" >&2
        echo "[info]  run this script from inside \`nix develop\` — the dev shell provides everything" >&2
        exit 1
    fi
done

echo "[info] === step 1/2: frontend (TS + Vite) ==="
cd assets/ide
if [ ! -d node_modules ]; then
    echo "[info] node_modules missing — running \`npm install\` (first-time, ~30s+)"
    npm install
else
    echo "[info] node_modules present — skipping \`npm install\` (rerun manually if package.json changed)"
fi
echo "[info] running \`npm run build\` (tsc -b && vite build)"
npm run build
cd "$REPO_ROOT"

echo ""
echo "[info] === step 2/2: backend (Rust + cargo) ==="
echo "[info] running \`cargo build $*\`"
cargo build "$@"

echo ""
echo "[ok] full build complete"
echo "[info]   frontend artifacts: assets/ide/dist/   (embedded into the binary at compile time)"
case " $* " in
    *" --release "*) echo "[info]   binary:             target/release/neo" ;;
    *)               echo "[info]   binary:             target/debug/neo" ;;
esac
