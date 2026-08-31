# ADR-0001: Use Tauri 2 with a Rust core and React WebView

## Status

Accepted

## Date

2026-08-31

## Context

JobGlass needs one desktop interface on macOS, Linux and Windows while keeping native scheduler access typed, local and least-privileged. The application must parse hostile local definitions, package for three operating systems and expose no generic filesystem or shell API to the UI.

## Decision

Use Tauri 2.11 with a Rust 1.98 core and a React 19.2 TypeScript frontend built by Vite 8.

Only application-defined read-only commands are registered. No Tauri shell, filesystem, HTTP, updater or logging plugin is exposed to the WebView. The default capability names the main window and the minimum core permission required to render it.

## Alternatives considered

### Electron

Electron has mature cross-platform packaging, but would ship a larger runtime and make Node-capability isolation an additional burden. JobGlass does not need a bundled Chromium or Node APIs in the renderer.

### Native UI per platform

Native SwiftUI, WinUI and GTK applications could fit each platform well but would triple presentation code and make deterministic cross-platform UI behaviour harder to maintain for the initial FOSS release.

### Browser-only local server

A local server would add a listening network surface and lifecycle complexity without helping native scheduler access.

## Consequences

- Platform adapters and diagnostics are compiled, tested Rust modules.
- The frontend is static and portable but real visual verification remains platform-specific.
- Linux builds require WebKitGTK development packages; Windows builds require WebView2 and the MSVC toolchain.
- Tauri's IPC boundary must be kept narrow and audited on every command change.

## Official sources

- Tauri overview and architecture: https://v2.tauri.app/start/
- Tauri prerequisites by platform: https://v2.tauri.app/start/prerequisites/
- Tauri project structure: https://v2.tauri.app/start/project-structure/
- Tauri IPC commands: https://v2.tauri.app/concept/inter-process-communication/
- Tauri command scopes and deny precedence: https://v2.tauri.app/security/scope/
- Tauri GitHub build matrix: https://v2.tauri.app/distribute/pipelines/github/
- Tauri 2.11.1 remote-origin ACL security fix: https://v2.tauri.app/release/tauri/v2.11.1/
