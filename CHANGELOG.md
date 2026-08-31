# Changelog

All notable changes to JobGlass are recorded here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and releases use semantic versioning.

## [0.1.0] - 2026-08-31

### Added

- Read-only macOS launchd, Linux cron/systemd, and Windows Task Scheduler adapters.
- Provenance-aware canonical job model with explicit unavailable evidence.
- Searchable list and timeline views, evidence inspector, and deterministic findings.
- Privacy-reviewed JSON, CSV, and self-contained HTML exports with arguments redacted by default.
- Light, dark, reduced-motion, responsive, keyboard, and screen-reader-oriented interface.
- Cross-platform CI packaging, checksums, CycloneDX SBOM, and GitHub build provenance.
- Public documentation, installation guidance, support boundaries, and release verification record.

### Security

- Bounded no-follow native file reads, fixed native tool invocations, output and timeout limits, a 10,000-job cap, and restrictive Tauri capabilities.
- No network services, telemetry, accounts, updater, privilege elevation, or scheduler mutation.

### Known limitations

- v0.1.0 packages are unsigned and not notarised.
- macOS has real runtime and visual evidence; Linux and Windows have fixture, hosted test, compilation, and package evidence only.
- The inherited GTK/WebKit dependency graph contains upstream advisories documented in [dependency advisories](docs/security/dependency-advisories.md).

[0.1.0]: https://github.com/DrewMauldin/jobglass/releases/tag/v0.1.0
