# Dependency advisory status

Last checked: 2026-08-31 with `cargo-audit` 0.22.2.

JobGlass v0.1.0 has no RustSec vulnerability findings and no npm vulnerabilities at the configured high-or-critical gate. The Rust audit does report 17 allowed warnings from transitive desktop dependencies:

- 10 unmaintained GTK3 binding crates required by Tauri's Linux WebView stack;
- five unmaintained `unic-*` crates and `proc-macro-error`;
- `RUSTSEC-2024-0429`, an unsound iterator implementation in `glib` 0.18.5.

The `glib` advisory affects `VariantStrIter` iterator methods. JobGlass does not depend on `glib` directly or call those methods, but Tauri's Linux dependency tree includes the affected version. That transitive exposure cannot be claimed absent, so Linux packages carry this disclosed upstream risk.

The project does not suppress or allowlist these notices. CI preserves the complete audit output, and releases remain blocked by known vulnerability findings. The notices will be re-evaluated on every dependency update; migrating with Tauri's supported Linux WebView stack is preferred over carrying a private GTK fork.
