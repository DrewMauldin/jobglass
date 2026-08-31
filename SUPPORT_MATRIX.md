# Platform support matrix

JobGlass reports only evidence visible to the current user. It never requests elevation.

| Platform      | Source                            | Definitions                                                                               | Runtime evidence                                                  | Permission behaviour                                               | v0.1.0 evidence                     |
| ------------- | --------------------------------- | ----------------------------------------------------------------------------------------- | ----------------------------------------------------------------- | ------------------------------------------------------------------ | ----------------------------------- |
| macOS 13+     | launchd user agents               | `~/Library/LaunchAgents/*.plist`                                                          | `launchctl print gui/<uid>/<label>` where observable              | User-readable by default                                           | Real Mac runtime plus fixtures      |
| macOS 13+     | launchd global agents and daemons | `/Library/LaunchAgents`, `/Library/LaunchDaemons`, readable `/System/Library` definitions | `launchctl print gui/<uid>` or `system/<label>` where observable  | Unreadable definitions or state become permission-limited findings | Real Mac runtime plus fixtures      |
| Linux         | per-user cron                     | `crontab -l` through a fixed read-only invocation                                         | Cron usually exposes no portable last/next outcome                | Missing utility or denied access is explicit                       | Fixtures and hosted build/test only |
| Linux         | system cron                       | `/etc/crontab`, `/etc/cron.d`, periodic directories when readable                         | Definition evidence only                                          | Unreadable files/directories are explicit                          | Fixtures and hosted build/test only |
| Linux         | systemd user/system timers        | `systemctl list-unit-files` and machine-readable `systemctl show`                         | Next/last trigger, active/load/result properties where exposed    | User and system managers queried separately without elevation      | Fixtures and hosted build/test only |
| Windows 10/11 | Task Scheduler                    | `schtasks /query /xml` for local definitions                                              | Documented query/runtime fields; access-denied tasks are explicit | Current-token visibility only; no remote credentials or elevation  | Fixtures and hosted build/test only |

## Honest limitations

- Cron has no standard portable success history. JobGlass does not infer one from unrelated logs.
- launchd and Task Scheduler may withhold system definitions or runtime fields from an unelevated process.
- systemd timer semantics can combine wall-clock and monotonic triggers. JobGlass preserves the native expression and native next-run evidence rather than pretending every expression can be converted to cron.
- v0.1.0 does not read remote machines, container schedulers, Kubernetes CronJobs, CI schedules or application-internal schedulers.
- A successful hosted build proves compilation and fixture behaviour on that runner, not real desktop appearance or local policy visibility.
