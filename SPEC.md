# Spec: JobGlass v0.1.0

## Objective

JobGlass is a free, open-source, local-first desktop control surface for native scheduled jobs. Its product line is **See what runs next.** It helps developers, system administrators, support engineers and power users understand what is scheduled on one machine, when it should run, whether native evidence says it succeeded, and where schedules may overlap.

JobGlass is an inspector, not a scheduler. Version 0.1.0 MUST NOT create, edit, delete, enable, disable, run, stop, repair or remotely manage a job. It MUST NOT elevate privileges or imply that unavailable native evidence exists.

### Acceptance criteria

- macOS: read user and system launchd property lists, correlate observable `launchctl` state, and expose last-exit evidence when available.
- Linux: read user and system cron variants plus systemd timers and their activated services.
- Windows: read local Task Scheduler definitions and runtime information through documented query interfaces.
- Every job maps to the canonical model defined below, with permission-limited and parse-warning states.
- Deterministic diagnostics identify duplicate identifiers or commands, likely overlaps, malformed definitions, missing executables, invalid working directories, disabled or stale jobs, PATH/environment differences and permission-limited visibility.
- The desktop interface provides overview, search, filters, list and timeline views, a detail inspector, diagnostic findings, theme controls and meaningful loading, empty, error and permission states.
- JSON, CSV and self-contained HTML exports require an explicit privacy review. Environment values are never exported. Command arguments are redacted by default unless the user explicitly includes reviewed values.
- No account, service, cloud sync, analytics, telemetry or AI is present.
- Fixtures and tests cover normal, overlapping, malformed, disabled, missing-executable and permission-limited cases for every platform.
- Release evidence distinguishes local checks, hosted CI, real platform runtime, packaging, signing/notarisation and live documentation.

## Canonical model

The versioned `ScheduledJob` contract contains:

- scheduler source and stable native identifier;
- owner, scope, privilege level and enabled state;
- normalised schedule, plain-English explanation and timezone basis;
- next and last run, plus last outcome, only when observable;
- executable, arguments, working directory and privacy-safe environment key names;
- native source reference, triggers, dependencies and target service;
- provenance for every field and parse warnings for ambiguity or unavailable evidence.

Unknown values are represented as unavailable with a reason, never as invented defaults. Stable identifiers are derived from scheduler type, native identifier and scope, not mutable display text.

## Tech stack

- Rust 1.98 stable core with platform-specific adapters behind one `SchedulerAdapter` interface.
- Tauri 2.11 desktop shell with custom commands only and a restrictive capability file.
- React 19.2, TypeScript 6.0 and Vite 8 for a static WebView frontend.
- Vitest and Testing Library for frontend behaviour; Rust unit, integration and property-style tests for parsers and diagnostics; Playwright plus axe-core for browser runtime and accessibility verification.

The selected versions are pinned in repository manifests and lockfiles. See `docs/decisions/0001-tauri-rust-react.md` for rationale and official sources.

## Commands

```bash
npm ci
npm run dev
npm run test
npm run test:coverage
npm run test:e2e
npm run lint
npm run typecheck
npm run build
npm run tauri:dev
npm run tauri:build
npm run check:fast
npm run check:task
npm run check:full
```

Rust commands use the pinned toolchain:

```bash
cargo fmt --check --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml --all-features
cargo audit
```

## Project structure

```text
src/                         React presentation and export review UI
src/components/              Focused accessible UI components
src/lib/                     Pure filtering, timeline and export formatting
src-tauri/src/               Rust core, adapters, normalisation and diagnostics
src-tauri/tests/             Cross-platform fixtures and integration tests
fixtures/{macos,linux,windows}/  Sanitised native evidence bundles
docs/                        User, contributor, security and release documentation
docs/decisions/              Architecture decision records
scripts/                     Reproducible quality and release helpers
site/                        Source for the public static documentation site
tasks/                       Dependency-ordered plan and execution checklist
```

## Code style

Rust boundaries return typed outcomes with warnings instead of panicking on untrusted scheduler input:

```rust
pub fn parse_job(input: &[u8], source: &NativeSource) -> ParseOutcome<ScheduledJob> {
    let definition = validate_size_and_decode(input)?;
    normalise_definition(definition, source)
}
```

TypeScript presentation components receive typed immutable data and use native HTML controls:

```tsx
export function ViewSelector({ value, onChange }: ViewSelectorProps) {
  return (
    <label>
      View
      <select
        value={value}
        onChange={(event) => onChange(event.target.value as ViewMode)}
      >
        <option value="list">List</option>
        <option value="timeline">Timeline</option>
      </select>
    </label>
  );
}
```

Rust uses `cargo fmt`; TypeScript uses ESLint and Prettier. Names describe native evidence precisely. Comments explain non-obvious safety decisions, not syntax.

## Testing strategy

- Test-first for model, parser, diagnostic, redaction and UI behaviour.
- Unit tests for pure parsing, schedule explanation, diagnostics, filters and exports.
- Integration tests load fixture bundles for every platform and assert deterministic normalised output.
- Property-style tests bound parser input and assert no panic across generated malformed cases.
- Browser tests verify seeded findings reach list, timeline, inspector and exports with zero serious or critical axe findings.
- On macOS, a read-only live scan is compared with a privacy-safe representative native sample.
- Changed-line coverage target: at least 80% where measurable. Rust and frontend coverage are reported separately.

## Boundaries

### Always

- Validate size, encoding, type, path and native command output at adapter boundaries.
- Invoke native tools with argument arrays, fixed commands, bounded timeouts and no shell interpolation.
- Preserve provenance and explicit unavailable states.
- Keep environment values out of the model, logs, screenshots and default exports.
- Run focused tests before each atomic commit and the full gate before merge.

### Ask first

- Any state-changing scheduler action, privilege elevation, remote management, network service, telemetry, account, auto-update channel or new sensitive data category.
- Any relaxation of `CONSTRAINTS.md`.

### Never

- Execute a discovered job command.
- Build a mutation API hidden behind an unused UI.
- Read environment values into the frontend or export layer.
- Follow symlinks outside allowlisted native scheduler roots during directory scans.
- Use shell interpolation for job identifiers, paths or command text.
- Claim Linux or Windows visual runtime without real evidence from those platforms.

## Performance budgets

- Warm launch to usable overview: under 1.5 seconds on the reference Apple silicon Mac.
- Normalise and diagnose 5,000 fixture jobs: under 500 ms in a release-mode benchmark on the reference Mac.
- Initial frontend JavaScript: under 200 KiB gzip.
- No frontend long task over 50 ms during filtering, view switching or privacy review with 5,000 fixture jobs.

## Success criteria

- All acceptance criteria and `CONSTRAINTS.md` gates pass.
- A fresh-context review finds no unresolved critical or required issues.
- `feat/production-v1` merges to `main` only after local and available hosted gates are truthfully recorded.
- The public repository, v0.1.0 release, artifacts, checksums, SBOM, signing status and public documentation URL are verifiably live.
- The repository is clean and a rollback to the previous release or source-only state is documented.

## Open questions resolved for v0.1.0

- Unsigned artifacts may be published only when clearly labelled with platform warnings. This is a release review state, not signed-production completion.
- Job arguments default to redacted in exports. The user must explicitly review and include them.
- System scopes that require elevation remain permission-limited; JobGlass never requests elevation.
