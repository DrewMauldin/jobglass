# JobGlass v0.1.0 task checklist

## Task 1: Pin manifests and dependency policy

**Acceptance criteria:**

- [x] Rust, npm and Tauri versions are pinned with one lockfile per ecosystem.
- [x] Dependency lifecycle scripts default-deny and required exceptions are version-pinned.
- [x] Fast, task and full quality commands are reproducible.

**Verification:** `npm ci`, `npm run check:fast`, `cargo metadata --locked --manifest-path src-tauri/Cargo.toml`
**Dependencies:** None
**Files:** `package.json`, `package-lock.json`, `.npmrc`, `rust-toolchain.toml`, `src-tauri/Cargo.toml`
**Scope:** Medium

## Task 2: Define the canonical evidence model

**Acceptance criteria:**

- [x] Contract represents all required fields, unavailable reasons, provenance and warnings.
- [x] Environment values cannot be represented.

**Verification:** focused Rust model tests and TypeScript contract typecheck
**Dependencies:** Task 1
**Files:** `src-tauri/src/model.rs`, `src-tauri/src/lib.rs`, `src/types.ts`, related tests
**Scope:** Medium

## Task 3: Bound native inputs

**Acceptance criteria:**

- [x] Files, outputs and job counts are capped; invalid encoding and symlinks fail safely.
- [x] Native commands use fixed argument arrays, output caps and timeouts.

**Verification:** property-style malformed input and boundary tests
**Dependencies:** Task 2
**Files:** `src-tauri/src/input.rs`, `src-tauri/src/process.rs`, focused tests
**Scope:** Medium

## Checkpoint: Foundation

- [x] Full Rust and frontend checks pass.
- [x] Floor guard reports clean.

## Task 4: Parse launchd definitions

**Acceptance criteria:**

- [x] Normal and malformed fixture plists produce deterministic jobs or warnings.
- [x] StartInterval, StartCalendarInterval, KeepAlive and path triggers are explained honestly.

**Verification:** macOS fixture integration tests
**Dependencies:** Task 3
**Files:** `src-tauri/src/adapters/launchd.rs`, `fixtures/macos/*`, integration test
**Scope:** Medium

## Task 5: Correlate launchctl evidence

**Acceptance criteria:**

- [x] Observable state and last exit code enrich matching jobs.
- [x] Denied or missing state remains explicitly unavailable.

**Verification:** launchctl fixture tests and live privacy-safe representative comparison
**Dependencies:** Task 4
**Files:** launchd adapter and integration evidence script/docs
**Scope:** Small

## Task 6: Parse cron variants

**Acceptance criteria:**

- [x] User-format fixtures, `/etc/crontab`, `/etc/cron.d` and periodic sources normalise correctly; native user-crontab access remains explicitly unavailable.
- [x] Environment key names persist while values are discarded.

**Verification:** Linux cron fixture integration tests
**Dependencies:** Task 3
**Files:** `src-tauri/src/adapters/cron.rs`, `fixtures/linux/cron/*`, integration test
**Scope:** Medium

## Task 7: Parse systemd timer evidence

**Acceptance criteria:**

- [x] Timer and target service fields, triggers, dependencies and runtime properties normalise.
- [x] Wall-clock and monotonic expressions remain distinguishable.

**Verification:** Linux systemd fixture integration tests
**Dependencies:** Task 3
**Files:** `src-tauri/src/adapters/systemd.rs`, `fixtures/linux/systemd/*`, integration test
**Scope:** Medium

## Task 8: Parse Windows Task Scheduler evidence

**Acceptance criteria:**

- [x] Namespaced XML actions, principals, settings and triggers normalise.
- [x] Disabled, malformed and access-denied cases are explicit.

**Verification:** Windows fixture integration tests and Windows hosted compilation
**Dependencies:** Task 3
**Files:** `src-tauri/src/adapters/windows.rs`, `fixtures/windows/*`, integration test
**Scope:** Medium

## Checkpoint: Native evidence

- [x] Every platform fixture category passes.
- [x] Live Mac sample matches privacy-safe native evidence.

## Task 9: Add deterministic diagnostics

**Acceptance criteria:**

- [x] Every required diagnostic has a stable ID, explanation and evidence.
- [x] Seeded overlaps and path faults produce deterministic ordering.

**Verification:** focused diagnostics tests and 5,000-job benchmark
**Dependencies:** Tasks 4-8
**Files:** `src-tauri/src/diagnostics.rs`, benchmark and tests
**Scope:** Medium

## Task 10: Add privacy-safe exports

**Acceptance criteria:**

- [x] JSON, CSV and escaped self-contained HTML are deterministic.
- [x] Arguments redact by default and environment values are impossible to export.

**Verification:** golden export tests and injection/redaction cases
**Dependencies:** Task 9
**Files:** `src-tauri/src/export.rs`, export tests
**Scope:** Small

## Task 11: Add bundle regression coverage

**Acceptance criteria:**

- [x] Mixed-platform bundle proves diagnostics and exports end to end.
- [x] Performance stays inside the 5,000-job budget.

**Verification:** full Rust suite and benchmark
**Dependencies:** Tasks 9-10
**Files:** fixture bundle, integration test, benchmark
**Scope:** Medium

## Checkpoint: Product intelligence

- [x] Seeded required findings reach normalised output and exports.
- [x] No sensitive fixture values appear in generated reports.

## Task 12: Build accessible overview shell

**Acceptance criteria:**

- [x] Loading, empty, error and populated overview states use semantic HTML.
- [x] Summary makes visibility limits clear.

**Verification:** Testing Library and axe component tests
**Dependencies:** Task 9
**Files:** app shell, overview component, styles, tests
**Scope:** Medium

## Task 13: Add search, list and timeline

**Acceptance criteria:**

- [x] Keyboard-operable filters and search update both views.
- [x] Timeline represents unknown next runs separately.

**Verification:** component behaviour tests and Playwright flow
**Dependencies:** Task 12
**Files:** view components, pure selectors, tests
**Scope:** Medium

## Task 14: Add inspector and findings

**Acceptance criteria:**

- [x] Details expose provenance, unavailable reasons and warnings.
- [x] Findings are understandable without relying on colour.

**Verification:** component and accessibility tests
**Dependencies:** Task 13
**Files:** inspector, findings panel, tests
**Scope:** Medium

## Task 15: Add export review and themes

**Acceptance criteria:**

- [x] Export cannot proceed until a redaction policy is reviewed.
- [x] Light, dark, reduced-motion and responsive layouts are polished.

**Verification:** export interaction tests, theme screenshots, responsive Playwright matrix
**Dependencies:** Tasks 10 and 14
**Files:** export dialog, theme control, styles, tests
**Scope:** Medium

## Task 16: Verify desktop runtime

**Acceptance criteria:**

- [x] macOS app is visually inspected at multiple sizes and both themes.
- [ ] Keyboard, VoiceOver spot checks, console and performance evidence are recorded.

**Verification:** signed-off `docs/verification/v0.1.0.md` evidence
**Dependencies:** Task 15
**Files:** verification record and real media only
**Scope:** Small

## Checkpoint: Desktop

- [x] Zero serious or critical accessibility findings.
- [x] No console warnings; performance budgets pass.

## Task 17: Add CI and packaging

**Acceptance criteria:**

- [ ] macOS, Ubuntu and Windows matrices run format, lint, tests and builds.
- [ ] Release workflow produces artifacts, checksums, SBOM and available provenance.

**Verification:** actionlint plus hosted run inspection
**Dependencies:** Task 16
**Files:** workflows, Dependabot and release scripts
**Scope:** Medium

## Task 18: Complete FOSS documentation

**Acceptance criteria:**

- [x] Install, quick start, permissions, architecture, privacy, contributing, development, troubleshooting and release docs are complete.
- [x] Templates, conduct, licence, roadmap and discoverability metadata are ready.

**Verification:** docs link checker and fresh-clone commands
**Dependencies:** Task 17
**Files:** focused documentation files
**Scope:** Medium per documentation commit

## Task 19: Capture real media and publish docs

**Acceptance criteria:**

- [x] README screenshots and hero animation come from the actual app.
- [ ] Public HTTPS site routes and release/download links resolve.

**Verification:** browser screenshots and public URL checks
**Dependencies:** Tasks 16 and 18
**Files:** media assets, README, static site
**Scope:** Medium

## Task 20: Review, merge and release

**Acceptance criteria:**

- [ ] Independent correctness, security, UX, test, packaging and maintainability review is resolved.
- [ ] Main receives the reviewed feature history without force push; v0.1.0 release state is truthful.

**Verification:** clean diff, full local gate, hosted evidence, re-downloaded checksum verification
**Dependencies:** Tasks 17-19
**Files:** changelog, verification and release record
**Scope:** Medium

## Checkpoint: Release

- [ ] All acceptance criteria and the standing Definition of Done pass.
- [ ] Source, local checks, hosted CI, artifacts, signing and docs are reported separately.
