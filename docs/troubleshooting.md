# Troubleshooting

## The app is blocked at first launch

The v0.1.0 packages are unsigned. Verify the package checksum and GitHub provenance, then follow the platform-specific steps in [Install](install.md). JobGlass does not claim signing or notarisation.

## Some jobs or fields are unavailable

This is expected when the current user cannot read a definition or native runtime record. The summary and findings expose the limit. Do not launch JobGlass as root or administrator merely to hide the warning; that changes the evidence and privacy boundary.

## A cron job has no last result

Cron has no portable standard success-history interface. JobGlass does not infer success from unrelated system logs. The value remains unavailable.

## A systemd next run differs from a cron calculator

systemd supports calendar and monotonic triggers with semantics that are not equivalent to cron. JobGlass preserves the native expression and uses native next-trigger evidence when available.

## A finding looks like a security verdict

Findings are deterministic configuration checks, not malware analysis. A missing executable, duplicate command, stale outcome, or overlap deserves review but does not establish intent or safety.

## Export buttons are disabled

You must acknowledge the privacy summary and intended destination. Arguments have a separate opt-in. Environment values are never included.

## The interface is empty in a browser build

`npm run dev` uses deterministic fixture data and has no native scheduler authority. The Tauri desktop runtime performs the real scan. If a native scan fails, retain the non-sensitive error text and open a bug report without attaching raw scheduler files.

## Report a problem

Use the bug template and include operating system version, JobGlass version, package type, the affected scheduler, and a sanitised description. Never attach secrets, environment values, private command arguments, tokens, or unredacted exports. Security vulnerabilities belong in private vulnerability reporting.
