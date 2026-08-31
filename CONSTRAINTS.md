# Constraints

Last reviewed: 2026-08-31 by project owner brief

## Floor

- No new suppression comments such as `@ts-ignore`, `eslint-disable`, `#[allow]`, `# noqa`, coverage ignores or security-scan allow markers.
- No unimplemented stubs, placeholder UI, empty catches, commented-out code or unresolved TODO/FIXME markers.
- No skipped, weakened or deleted tests without an explicit reviewed exception.
- No secrets or environment values in source, fixtures, logs, screenshots or exports.
- This file MUST NOT be weakened to make a change pass.

## Enforced with numbers

| Dimension                  | Rule and rationale                                                                  | Checked by                                                                                      | Runs at             |
| -------------------------- | ----------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------- | ------------------- |
| Rust format                | Zero formatting drift                                                               | `cargo fmt --check --manifest-path src-tauri/Cargo.toml`                                        | edit, task, CI      |
| Rust lint                  | Zero Clippy warnings; warnings often hide real portability faults                   | `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings` | task, CI            |
| TypeScript types           | Zero type errors                                                                    | `npm run typecheck`                                                                             | edit, task, CI      |
| Frontend lint/format       | Zero errors or warnings                                                             | `npm run lint` and `npm run format:check`                                                       | edit, task, CI      |
| Tests                      | Zero failed or skipped tests                                                        | `npm run test` and `cargo test --manifest-path src-tauri/Cargo.toml --all-features`             | task, CI            |
| Changed-line coverage      | At least 80%; high enough to require evidence without punishing configuration       | `npm run test:coverage` and `cargo llvm-cov --fail-under-lines 80`                              | task, CI            |
| Dependency security        | Zero known high or critical vulnerabilities                                         | `npm audit --audit-level=high` and `cargo audit`                                                | task, CI, release   |
| Secret scanning            | Zero findings; values are always redacted                                           | `gitleaks git --redact`                                                                         | commit, CI, release |
| Accessibility              | Zero serious or critical axe findings                                               | `npm run test:e2e`                                                                              | task, CI            |
| Warm launch                | Under 1.5 seconds on reference Mac; hosted CI uses explicit smoke and scan ceilings | `scripts/measure-launch.sh` locally and `scripts/measure-launch.sh --hosted` in release CI      | release             |
| Fixture scan               | 5,000 jobs under 500 ms in release mode                                             | `cargo bench --manifest-path src-tauri/Cargo.toml`                                              | task, CI            |
| Frontend bundle            | Initial JavaScript under 200 KiB gzip                                               | `npm run size`                                                                                  | task, CI            |
| Interaction responsiveness | No task over 50 ms with 5,000 fixture jobs                                          | Playwright performance assertion                                                                | task, CI            |
| Fast hand-back             | `check:task` under 90 seconds locally                                               | `scripts/quality.sh task`                                                                       | task                |

## Architecture constraints

- The frontend imports no Node built-ins and has no direct filesystem, shell, HTTP or process capability.
- Only typed read-only Tauri commands cross the IPC boundary.
- Platform modules compile conditionally and expose the same normalised contract.
- Untrusted native inputs are size-bounded and parsed without command execution.
- Export generators accept reviewed `ExportPolicy` values; they cannot access native sources directly.

## Measured, not yet enforced

| Metric                   | Baseline         | Direction                                        |
| ------------------------ | ---------------- | ------------------------------------------------ |
| Release artifact sizes   | Record at v0.1.0 | Must not grow without documented user value      |
| Full repository coverage | Record at v0.1.0 | Must not fall by more than 0.5 percentage points |

## Exceptions

None.

Informational, unmaintained, and unsound RustSec notices remain visible in full audit output and the release verification record. They are never allowlisted or suppressed, but they are classified separately from known high or critical vulnerabilities.
