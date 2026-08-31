# ADR-0002: Query native schedulers through read-only adapters

## Status

Accepted

## Date

2026-08-31

## Context

Each supported operating system represents schedules and runtime state differently. Parsing only files misses current state, while invoking broad management interfaces would violate the product boundary.

## Decision

Implement one conditional adapter per platform. Adapters may read documented definition locations and invoke only fixed query operations with argument arrays. They may not invoke a discovered executable or any scheduler mutation verb.

- macOS parses bounded property lists and correlates `launchctl print` output where permitted.
- Linux parses bounded cron files and uses `systemctl show` for computer-parsable timer and service properties.
- Windows uses `schtasks /query /xml`, which Microsoft documents as listing local tasks and outputting task definitions as XML.

Permission failures become data: a `PermissionLimited` warning with source and scope. The application never retries with elevation.

## Alternatives considered

- Direct private scheduler databases: rejected because formats and locking are unsupported.
- General shell plugin: rejected because it would expose unnecessary process capability to the WebView.
- Elevating helper: rejected because useful unelevated operation is a core product constraint.

## Consequences

- Runtime evidence varies by platform and version; the support matrix is part of the public contract.
- Native output parsers require sanitised fixtures and output bounds.
- Some fields remain explicitly unavailable.

## Official sources

- Apple launchd job creation and schedule semantics: https://developer.apple.com/library/archive/documentation/MacOSX/Conceptual/BPSystemStartup/Chapters/CreatingLaunchdJobs.html
- systemd timer semantics: https://github.com/systemd/systemd/blob/main/man/systemd.timer.xml
- `systemctl show` computer-parsable output: https://github.com/systemd/systemd/blob/main/man/systemctl.xml
- Linux cron file semantics: https://man7.org/linux/man-pages/man5/crontab.5.html
- Microsoft `schtasks /query`: https://learn.microsoft.com/en-us/windows-server/administration/windows-commands/schtasks-query
