# Neo IDE — bundled frontend

This directory holds the Vite project for the in-browser IDE that `neo ide`
serves. The Rust binary embeds `dist/` at compile time via
[`rust-embed`](https://docs.rs/rust-embed) (see `src/commands/ide.rs`).

## Layout

- `dist/` — the Vite build output. **Tracked in git**: the Rust binary
  expects this directory to exist at `cargo build` time, and the release
  `nix build` consumes it as-is (the release derivation does not run Vite).
- `dist/index.html` — currently a placeholder. Replace by dropping in a real
  Vite project and running `npm run build` (or `pnpm build`).

## Workflow once the real Vite project is in place

```sh
nix develop              # gives you node + pnpm
cd assets/ide
pnpm install
pnpm build               # populates assets/ide/dist/
cd ../..
nix develop --command cargo build
target/debug/neo ide     # serves the freshly built dist/
```

In debug builds `rust-embed` reads `dist/` from disk on every request, so
you don't need to rebuild the Rust binary while iterating on the frontend —
just `pnpm build` and refresh the browser.
