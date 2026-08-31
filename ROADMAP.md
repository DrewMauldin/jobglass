# Roadmap

JobGlass uses small, evidence-backed releases. Items below are directions, not promises or dates.

## v0.1 — inspect one machine

- [x] Native scheduled-job discovery on macOS, Linux, and Windows.
- [x] Provenance, unavailable states, deterministic diagnostics, and privacy-reviewed exports.
- [x] Reproducible tests, cross-platform packages, checksums, SBOM, provenance, and public documentation.
- [ ] Code signing and macOS notarisation when project-owned signing identities are available.

## Candidate follow-ups

- Broaden sanitised fixtures for uncommon launchd, cron, systemd, and Task Scheduler trigger variants.
- Add real Linux and Windows desktop runtime/visual verification.
- Improve explanations for native timezone and daylight-saving behaviour without inventing cross-platform equivalence.
- Add an opt-in, local-only diff between two user-selected scan exports.

## Explicitly out of scope

Scheduler mutation, executing discovered commands, privilege elevation, remote fleet management, accounts, cloud sync, telemetry, behavioural analytics, AI classification, and unattended sharing are not planned for the v0.x product boundary. Any proposal to change that boundary requires a public specification and threat-model review first.
