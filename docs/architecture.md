# Architecture

JobGlass is a static React interface over a small Rust evidence core inside Tauri. The architecture keeps native authority in Rust and presentation authority in the WebView.

```text
allowlisted files + fixed native queries
                  │
                  ▼
       bounded platform adapters
                  │
                  ▼
 ScheduledJob + provenance + warnings
                  │
          deterministic diagnostics
                  │
        two read-only Tauri commands
                  │
                  ▼
       React views and local exports
```

## Rust core

Platform adapters parse launchd property lists/runtime tables, cron variants, systemd timer properties, and Task Scheduler XML/runtime output. All inputs cross size, type, encoding, path, timeout, and count boundaries before normalisation.

The canonical `ScheduledJob` represents source, stable native identity, ownership/scope, enabled state, schedule, next/last run, last outcome, executable, optional arguments, working directory, environment key names, triggers, dependencies, target service, native source, provenance, and parse warnings. Unknown data is an unavailable value with a reason.

Diagnostics are pure and deterministic over a completed scan bundle. Export rendering is also pure after the UI supplies a reviewed redaction policy.

## IPC boundary

The Tauri builder registers only:

- `scan_jobs` — collect and return one read-only `ScanBundle`;
- `render_export` — render a supplied bundle with an explicit reviewed policy and format.

The capability file does not grant general shell, filesystem, HTTP, clipboard, updater, or process access to the WebView. Content Security Policy limits the packaged frontend to its local resources.

## React interface

React owns loading, error, empty, overview, findings, list, timeline, inspector, theme, and privacy-review states. Native HTML controls and semantic regions preserve keyboard and accessibility behaviour. Large lists page by 25 rows.

## Decisions and contracts

- [Tauri, Rust, and React](decisions/0001-tauri-rust-react.md)
- [Read-only native adapters](decisions/0002-read-only-native-adapters.md)
- [Evidence and privacy model](decisions/0003-evidence-and-privacy-model.md)
- [Specification](../SPEC.md)
- [Constraints](../CONSTRAINTS.md)
- [Capability map](../CAPABILITY_MAP.md)
