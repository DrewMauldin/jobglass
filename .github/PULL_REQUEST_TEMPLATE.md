## Problem and scope

<!-- What user-visible or maintenance problem does this solve? What is deliberately out of scope? -->

## Evidence

- [ ] Focused tests added or updated
- [ ] `npm run check:task` passes
- [ ] `npm run quality -- full` passes, or unavailable gates are explained
- [ ] Claimed native runtime/platform evidence is attached without sensitive data
- [ ] User, architecture, support, security, or release docs are updated when needed

## Boundary review

- [ ] The change remains read-only and does not execute discovered commands
- [ ] No elevation, remote access, telemetry, account, updater, or environment value was added
- [ ] Inputs and native outputs remain bounded and shell-free
- [ ] The diff contains no unrelated cleanup or generated artifacts

## Privacy

<!-- Confirm fixtures, logs, screenshots, and exports are sanitised. -->
