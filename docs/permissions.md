# Permissions and native access

JobGlass uses the current user's existing read access. It has no privileged helper, `sudo` path, UAC prompt, remote credential, Accessibility control, Full Disk Access request, or scheduler mutation command.

## macOS

JobGlass reads regular property-list files from these allowlisted roots when accessible:

- `~/Library/LaunchAgents`
- `/Library/LaunchAgents`
- `/Library/LaunchDaemons`
- `/System/Library/LaunchAgents`
- `/System/Library/LaunchDaemons`

It runs fixed, bounded `launchctl print` queries for the current GUI and system domains. It does not execute a discovered program or interpolate a job value into a shell. Unreadable files and missing runtime records become warnings or unavailable evidence.

## Linux

JobGlass reads allowlisted system cron definitions and makes fixed read-only `systemctl` queries. It deliberately does not invoke the privilege-bearing `crontab` helper, so user-crontab visibility is reported as unavailable. User and system systemd managers are queried separately. It does not inspect unrelated logs to invent cron success history.

## Windows

JobGlass invokes local, fixed Task Scheduler query commands with the current token and parses bounded XML/output. It does not request remote hosts, credentials, or an elevated console. Access-denied definitions remain explicit.

## File boundary

Native inputs must be regular files under an expected scheduler root. Symlinks are rejected rather than followed. Each file, native command output, overall job count, and command duration is capped. These controls protect the parser and do not expand the operating system access granted to JobGlass.

See [Security](../SECURITY.md) for the threat model and [Support matrix](../SUPPORT_MATRIX.md) for source-by-source limitations.
