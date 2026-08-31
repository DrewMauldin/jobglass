#!/bin/sh
set -eu

mode=${1:-full}

case "$mode" in
  fast|task|full) ;;
  *)
    printf 'usage: scripts/quality.sh [fast|task|full]\n' >&2
    exit 64
    ;;
esac

npm run check:fast
cargo fmt --check --manifest-path src-tauri/Cargo.toml
cargo check --locked --manifest-path src-tauri/Cargo.toml --all-targets --all-features

if [ "$mode" = fast ]; then
  exit 0
fi

npm test
npm run build
cargo test --locked --manifest-path src-tauri/Cargo.toml --all-features
cargo clippy --locked --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings

if [ "$mode" = task ]; then
  exit 0
fi

npm run test:coverage
npm run test:e2e
npm run size
npm audit --audit-level=high
cargo llvm-cov --locked --manifest-path src-tauri/Cargo.toml --all-features --fail-under-lines 80
cargo audit --deny warnings --file src-tauri/Cargo.lock
cargo bench --locked --manifest-path src-tauri/Cargo.toml --bench diagnostics
gitleaks git --redact --no-banner
