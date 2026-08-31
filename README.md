# JobGlass

> **See what runs next.**

[![CI](https://github.com/DrewMauldin/jobglass/actions/workflows/ci.yml/badge.svg)](https://github.com/DrewMauldin/jobglass/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/DrewMauldin/jobglass?display_name=tag)](https://github.com/DrewMauldin/jobglass/releases/latest)
[![License](https://img.shields.io/github/license/DrewMauldin/jobglass)](LICENSE)

JobGlass is a free, local-first desktop inspector for native scheduled jobs. It brings launchd, cron, systemd timers, and Windows Task Scheduler into one searchable evidence view—without accounts, telemetry, cloud services, privilege elevation, or scheduler mutations.

![JobGlass showing a real macOS launchd scan in light and dark themes](docs/media/jobglass-tour.gif)

## What it does

- Normalises native job definitions while preserving their source and unavailable fields.
- Shows search, scheduler filters, list and timeline views, detailed evidence, and deterministic findings.
- Flags duplicate identifiers or commands, likely overlaps, missing paths, invalid working directories, stale or disabled jobs, environment-key differences, malformed definitions, and visibility limits.
- Exports deterministic JSON, CSV, or self-contained HTML after an explicit privacy review.
- Redacts command arguments by default and never places environment values in the data model.

JobGlass is intentionally an inspector, not a scheduler. It cannot create, edit, delete, enable, disable, run, stop, or repair a job.

## Why JobGlass exists

Native schedulers are powerful, but each exposes a different vocabulary and a different set of blind spots. A launchd property list, a cron line, a systemd timer, and a Task Scheduler XML document do not answer the same questions in the same way. JobGlass gives them one interface without pretending their evidence is equivalent.

It is useful when you need to answer questions such as:

- What is expected to run next on this machine?
- Which native definition produced this job?
- Did the scheduler expose a last run or result, or is that information genuinely unavailable?
- Are two definitions targeting the same command or schedule?
- Is a path missing, a definition malformed, or visibility limited by the current user's permissions?
- Can I share a report without automatically including command arguments or environment values?

JobGlass does not monitor execution in the background and does not replace the operating system's scheduler tools. Every scan is an on-demand, read-only snapshot of evidence available to the current user.

## How the evidence model works

Every normalised field is either **available** with provenance or **unavailable** with a typed reason. Scope is not treated as proof of privilege, missing runtime history is not converted into success, and a permission error is not presented as an empty scheduler.

The canonical model records:

- scheduler, native identifier, display name, owner, scope, privilege evidence, and enabled state;
- native schedule expression, plain-language explanation, timezone basis, next run, last run, and last outcome when observable;
- executable, arguments, working directory, environment key names, triggers, dependencies, and activated service;
- native source reference, field-level provenance, and parse warnings.

Stable IDs are derived from scheduler type, native identifier, and scope. Display names and other mutable presentation fields do not change identity.

## Deterministic findings

Findings are rules over the collected model—not AI classifications, malware verdicts, or guarantees that a command is safe.

| Finding family       | What JobGlass checks                                                                       |
| -------------------- | ------------------------------------------------------------------------------------------ |
| Identity and overlap | Duplicate native identifiers, duplicate commands, and matching native schedule expressions |
| Definition health    | Malformed or ambiguous native definitions and invalid native state values                  |
| Local paths          | Missing absolute executables and invalid working directories, using no-follow local probes |
| Runtime evidence     | Disabled jobs, failed observable outcomes, and last runs older than 30 days                |
| Environment          | Commands that depend on `PATH` and differences between privacy-safe environment key sets   |
| Visibility           | Permission-limited or unavailable scheduler scopes                                         |

The same input bundle produces the same findings and stable finding IDs.

## Install

Download the package for your platform from the [latest GitHub release](https://github.com/DrewMauldin/jobglass/releases/latest), then verify it against `SHA256SUMS` before opening it.

The v0.1.0 community packages are **unsigned**. macOS Gatekeeper and Windows SmartScreen may warn or block them. Read the platform-specific [installation guide](docs/install.md) before bypassing a warning, and do not treat an unsigned package as notarised or publisher-verified.

To build from source instead:

```bash
git clone https://github.com/DrewMauldin/jobglass.git
cd jobglass
npm ci
npm run tauri:build
```

See [development](docs/development.md) for the required Node, npm, Rust, and operating-system dependencies.

## Quick start

1. Open JobGlass. The first scan is read-only and stays on the machine.
2. Review the visibility banner; inaccessible system definitions remain explicit rather than inferred.
3. Search or filter the job list, then select a job to inspect its native evidence.
4. Open **Findings** for deterministic configuration diagnostics.
5. Choose **Export report** only when you have reviewed the paths and identifiers that may leave the machine.

The full walkthrough is in [Quick start](docs/quick-start.md).

## Platform behaviour

### macOS

JobGlass reads regular `.plist` definitions from the current user's LaunchAgents directory and the standard global launchd agent/daemon roots when accessible. It correlates bounded `launchctl print` output for the current GUI and system domains. A protected file, absent runtime record, or unsupported field stays explicit.

### Linux

JobGlass reads `/etc/crontab`, `/etc/cron.d`, executable periodic directories, and the current user's direct cron spool file in supported `/var/spool/cron*` layouts when readable. It never invokes the privilege-bearing `crontab` helper. User and system systemd managers are queried separately with fixed `systemctl` arguments; native calendar and monotonic expressions are preserved.

### Windows

JobGlass reads local Task Scheduler definitions and runtime information through fixed `schtasks` and ScheduledTasks PowerShell queries using the current token. It does not accept remote hosts, credentials, or arbitrary commands. Runtime records are bounded and joined to definitions by validated native identifier.

## Platform evidence

| Platform      | Sources                                                  | v0.1.0 evidence                                          |
| ------------- | -------------------------------------------------------- | -------------------------------------------------------- |
| macOS 13+     | launchd agents and daemons, observable `launchctl` state | Real Apple silicon runtime, fixtures, tests, and package |
| Linux         | user/system cron and systemd timers                      | Fixtures, tests, hosted compilation, and package         |
| Windows 10/11 | local Task Scheduler definitions and runtime fields      | Fixtures, tests, hosted compilation, and package         |

A hosted build proves compilation and fixture behaviour, not native desktop appearance or local policy visibility. See the detailed [support matrix](SUPPORT_MATRIX.md).

## Export formats

All exports require the user to open a review dialog and acknowledge that paths and identifiers may identify a machine.

- **JSON** preserves the versioned evidence contract and deterministic findings.
- **CSV** provides a compact job table and neutralises spreadsheet-formula prefixes.
- **HTML** produces a self-contained, escaped report with a restrictive Content Security Policy.

Command arguments are replaced with `<redacted>` by default, including argument text repeated inside finding evidence. Including arguments is a separate explicit reviewed choice. Environment values never enter the model, so no export mode can reveal them.

## Privacy and security

JobGlass has no network feature, account, analytics, telemetry, AI, updater, or remote-management path. The WebView receives only a bounded, serialisable evidence model through two custom read-only commands. Native inputs, outputs, time, paths, and job counts are capped.

Native scheduler files are opened beneath allowlisted roots with component-relative no-follow semantics. Special files are opened nonblocking and rejected by type. Native command output, runtime-record counts, and total command duration are bounded; timed-out process trees are terminated. JobGlass never executes a command discovered in a scheduler definition.

Exports can still reveal usernames, paths, commands, owners, schedules, and source references. Read [Privacy](docs/privacy.md), [Permissions](docs/permissions.md), and the [security policy and threat model](SECURITY.md) before sharing a report. Report vulnerabilities through GitHub private vulnerability reporting, not a public issue.

## Architecture

```text
allowlisted files + fixed native queries
                  │
                  ▼
        bounded Rust adapters
                  │
                  ▼
   versioned evidence model + findings
                  │
          ┌───────┴────────┐
          ▼                ▼
  read-only Tauri IPC   reviewed exports
          │          JSON · CSV · HTML
          ▼
   React evidence interface
```

The Rust core owns native access, parsing, normalisation, diagnostics, and export generation. Tauri exposes only scan and export commands. React renders the serialised model and cannot browse the filesystem or start processes. See [Architecture](docs/architecture.md) and the decision records under [docs/decisions](docs/decisions/).

## Build and verify from source

Prerequisites and platform packages are documented in [Development](docs/development.md). The short path is:

```bash
git clone https://github.com/DrewMauldin/jobglass.git
cd jobglass
npm ci
npm run quality -- full
npm run tauri:build
```

The full quality path checks TypeScript, ESLint, formatting, documentation links, source-floor rules, unit tests, changed-line and overall coverage, production bundle size, Playwright/axe flows at four window sizes, Rust formatting, Clippy with warnings denied, all Rust tests, coverage, the 5,000-job diagnostic benchmark, dependency audits, and committed-history secret scanning. GitHub CI and release jobs additionally validate workflow syntax with actionlint.

Platform packages are built natively on macOS, Ubuntu, and Windows. Release automation binds an annotated version tag to the exact `main` commit, rebuilds the packages, generates checksums and a CycloneDX SBOM, records GitHub provenance, re-downloads the draft assets, verifies them, and only then publishes the release.

## Release trust

v0.1.0 is a deliberately **unsigned community release**:

- macOS has no Developer ID signature or notarisation;
- Windows has no Authenticode publisher signature;
- Linux packages are unsigned;
- release checksums, SBOM, and GitHub build provenance verify build identity and bytes, not a paid platform publisher identity.

If unsigned packages do not meet your trust requirements, build the tagged source yourself. The [verification record](docs/verification/v0.1.0.md) keeps reviewed source, local tests, real macOS runtime, hosted compilation, package validation, signing state, release publication, and live documentation as separate evidence gates.

## Documentation

- [Installation](docs/install.md) · [Quick start](docs/quick-start.md) · [Troubleshooting](docs/troubleshooting.md)
- [Architecture](docs/architecture.md) · [Permissions](docs/permissions.md) · [Privacy](docs/privacy.md)
- [Contributing](CONTRIBUTING.md) · [Development](docs/development.md) · [Release process](docs/release.md)
- [Specification](SPEC.md) · [Constraints](CONSTRAINTS.md) · [Capability map](CAPABILITY_MAP.md)
- [Changelog](CHANGELOG.md) · [Roadmap](ROADMAP.md) · [Code of conduct](CODE_OF_CONDUCT.md)

![JobGlass export privacy checkpoint](docs/media/jobglass-export-review.png)

## Contributing

Bug reports, fixture improvements, documentation fixes, and focused platform patches are welcome. Please read [CONTRIBUTING.md](CONTRIBUTING.md) and [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) before opening a pull request.

## License

Copyright 2026 Drew Mauldin and JobGlass contributors. Licensed under the [Apache License 2.0](LICENSE); see [NOTICE](NOTICE).
