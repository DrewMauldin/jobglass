# Security policy and threat model

## Supported versions

Security fixes are provided for the latest published release. During the v0.x series, upgrades may include small contract changes documented in the changelog.

## Reporting a vulnerability

Use GitHub private vulnerability reporting for this repository. Do not open a public issue containing exploit details, native job contents, command arguments, environment values or local paths.

## Security boundary

JobGlass is deliberately read-only. The Rust core may read allowlisted native scheduler locations and invoke fixed local query utilities. The WebView receives a serialisable normalised model through explicitly registered commands. It has no general shell, filesystem, HTTP, clipboard, updater or process capability.

### Assets

- Local job definitions, which may reveal usernames, paths, commands or secret-bearing arguments.
- Environment values present in native scheduler definitions.
- Integrity of scheduler state: JobGlass must never change or execute it.
- User trust in exported reports and diagnostic evidence.

### Trust boundaries and abuse cases

| Boundary                     | Threats                                                                        | Required controls                                                                                                              |
| ---------------------------- | ------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------ |
| Native files -> Rust parser  | Malicious XML/text, huge files, invalid encodings, symlink escape, path tricks | Size caps, type checks, no-follow metadata checks, bounded decoding, allowlisted roots, warnings instead of panics             |
| Native tools -> Rust adapter | Localised or malformed output, hangs, attacker-controlled identifiers          | Fixed executable and argument arrays, no shell, validated identifiers, timeouts, output caps, typed parsing                    |
| Rust -> WebView IPC          | Over-broad privilege or unintended sensitive fields                            | Two custom read-only commands, restrictive capability, serialisable allowlisted DTOs, environment keys only                    |
| Model -> UI                  | Stored XSS or misleading evidence                                              | React text escaping, no raw HTML, provenance displayed, unavailable values labelled                                            |
| Model -> exports             | Secret leakage through arguments, paths or environment                         | Mandatory review policy, arguments redacted by default, environment values absent by construction, self-contained escaped HTML |
| Release artifacts -> users   | Tampering, dependency compromise, unsigned binary confusion                    | Lockfiles, audits, checksums, SBOM, provenance where available, explicit signing status                                        |

### STRIDE summary

- Spoofing: stable identifiers include scheduler and scope; provenance names the native source.
- Tampering: source references and parse warnings preserve evidence; release artifacts carry checksums.
- Repudiation: local scan summaries include a random correlation ID and counts, never sensitive contents.
- Information disclosure: environment values never enter the model; export arguments are redacted by default.
- Denial of service: file, command-output, job-count and execution-time bounds prevent unbounded parsing.
- Elevation of privilege: no helper, installer, sudo prompt, remote credential or mutation command exists.

## Non-goals

JobGlass is not an antivirus product and cannot prove a job command is safe. Diagnostics are deterministic configuration findings, not malware verdicts.
