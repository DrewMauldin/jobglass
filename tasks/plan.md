# Implementation plan: JobGlass v0.1.0

## Overview

Build JobGlass in dependency order through small, independently verified slices. High-risk parser, security and IPC boundaries land before visual polish. The branch remains releasable after every checkpoint.

## Dependency graph

```text
Native source contracts
  -> canonical model
    -> macOS/Linux/Windows adapters
      -> diagnostics
        -> privacy-safe exports
          -> accessible desktop experience
            -> packaging, site and release
```

## Architecture decisions

- Tauri 2 WebView with custom read-only commands only. See ADR-0001.
- Conditional native adapters using fixed read-only operations. See ADR-0002.
- Provenance-rich model with environment values excluded at ingestion. See ADR-0003.
- Static GitHub Pages documentation from `/docs`; no server or account system.

## Task list

Detailed acceptance criteria, verification and file scope live in `tasks/todo.md`.

### Phase 1: Foundation and contracts

1. Initialise pinned manifests and fail-closed dependency policy.
2. Define and test the canonical evidence model.
3. Define and test bounded native input utilities.

### Phase 2: Native evidence slices

4. Implement the launchd fixture parser.
5. Correlate launchctl evidence and perform a live privacy-safe Mac scan.
6. Implement cron fixture parsing and permission states.
7. Implement systemd timer/service fixture parsing.
8. Implement Windows Task Scheduler XML fixture parsing.

### Phase 3: Product intelligence

9. Implement deterministic diagnostics.
10. Implement privacy review and export generators.
11. Add cross-platform bundle and performance regression tests.

### Phase 4: Desktop experience

12. Build the accessible application shell and overview.
13. Add search, filtering, list and timeline views.
14. Add inspector, findings and permission/error states.
15. Add export review flow and theme/responsive polish.
16. Verify browser and macOS desktop runtime, accessibility and performance.

### Phase 5: FOSS and release

17. Add CI, security scans and packaging matrix.
18. Complete contributor, operator, security and release documentation.
19. Capture real screenshots and hero media, then publish the static site.
20. Run fresh-context review, simplify, merge and publish v0.1.0.

## Checkpoints

- Foundation: manifests locked, floor guard green, contract tests pass.
- Native evidence: all fixture adapters pass and live Mac sample matches native evidence.
- Product intelligence: seeded warnings reach model and exports deterministically.
- Desktop: zero console errors, zero serious/critical axe findings, keyboard and VoiceOver spot checks recorded.
- Release: local full gate green, hosted state truthfully classified, artifacts checksum-verified, site and release links live.

## Risks and mitigations

| Risk | Impact | Mitigation |
|---|---|---|
| Native formats vary or are localised | High | Prefer structured files and computer-parsable properties; preserve warnings and fixtures |
| System scope denied | Medium | Never elevate; show explicit partial visibility |
| Definition contains secrets | High | Discard environment values; redact arguments by default; review gate before export |
| Huge or malicious definitions | High | File/output/job caps, no-follow checks, bounded timeouts, property tests |
| Cross-platform compilation drifts | High | Conditional modules and hosted three-OS matrix |
| GitHub Actions account gate | Medium | Keep reproducible local proof; inspect whether hosted jobs ran before claiming green |
| Signing credentials unavailable | Medium | Publish clearly labelled unsigned artifacts only when safe; leave project task in review |

## Open questions

None block implementation. Any signing or hosted-infrastructure limitation is classified during release rather than guessed now.
