# Development

## Toolchains

- Node.js 26.7.0 or later in the 26.x line
- npm 11.19.0
- Rust 1.98.0 with rustfmt and clippy
- platform packages required by [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/)

The exact application dependencies are locked by `package-lock.json` and `src-tauri/Cargo.lock`.

## Set up

```bash
git clone https://github.com/DrewMauldin/jobglass.git
cd jobglass
rustup toolchain install 1.98.0 --component rustfmt,clippy
npm ci
```

Linux development additionally needs WebKitGTK 4.1, AppIndicator, librsvg, and patchelf. Windows needs the Microsoft C++ build tools and WebView2 runtime. macOS needs Xcode Command Line Tools.

## Run

```bash
npm run dev          # browser presentation with deterministic fixture data
npm run tauri:dev    # native read-only scheduler scan
```

The browser build cannot prove native collection. Use it for fast UI work and the packaged/dev desktop app for native authority.

## Verify

```bash
npm run check:fast
npm run check:task
npm run quality -- full
```

`check:fast` covers type, lint, format, docs links, and floor constraints. `check:task` adds frontend tests and the production frontend build. The full quality script adds browser accessibility/performance tests, Rust format/test/clippy/coverage/benchmark, dependency audits, secret scanning when available, and bundle limits.

Run a focused Rust test with:

```bash
cargo test --locked --manifest-path src-tauri/Cargo.toml TEST_NAME
```

## Build a package

```bash
npm run tauri:build
```

Local packages inherit the local machine's signing configuration. A successful package build is not evidence of signing or notarisation; inspect those states separately.

## Project layout

- `src/` — typed React presentation and UI tests
- `src-tauri/src/` — evidence model, safety boundaries, adapters, diagnostics, exports, and Tauri commands
- `fixtures/` — sanitised cross-platform native inputs
- `tests/e2e/` — browser accessibility, responsive, and performance flows
- `docs/` — user, architecture, security, and verification records
- `site/` — dependency-free public landing page
- `scripts/` — deterministic quality and media helpers
