# Contributing to JobGlass

Thank you for helping make native scheduler evidence easier to understand.

## Before you start

- Search existing issues and pull requests.
- Use an issue for behaviour changes, new native sources, or changes to the read-only/privacy boundary.
- Keep each change focused. Do not add scheduler mutation, elevation, telemetry, accounts, remote access, or environment values.
- Treat scheduler fixtures as potentially sensitive. Sanitise usernames, paths, arguments, hostnames, identifiers, and environment values before committing them.

Participation is governed by the [Code of Conduct](CODE_OF_CONDUCT.md). Security problems belong in [private vulnerability reporting](SECURITY.md), not public issues.

## Development workflow

1. Fork the repository and create a short-lived branch from `main`.
2. Install the pinned toolchains and dependencies described in [Development](docs/development.md).
3. Add a failing focused test before changing parser, diagnostic, export, or UI behaviour.
4. Make the smallest change that passes the test and preserves the contracts in [SPEC.md](SPEC.md) and [CONSTRAINTS.md](CONSTRAINTS.md).
5. Run `npm run check:task` while working and `npm run quality -- full` before requesting review.
6. Update user, architecture, support, security, or release documentation when the observable contract changes.

## Pull requests

A reviewable pull request:

- explains the user-visible problem and the chosen boundary;
- links its issue when one exists;
- includes fixture or runtime evidence appropriate to each claimed platform;
- reports frontend, Rust, browser, packaging, and native runtime proof separately;
- does not claim signing, notarisation, or a live deployment without direct evidence;
- has no unrelated formatting, refactoring, dependency, or generated-file changes.

Maintainers may ask for a fresh-context security, accessibility, or platform review before merge.

## Native fixtures

Prefer the smallest representative fixture. Keep the native structure needed by the parser, replace machine-specific values, and add an assertion that proves the behaviour. Never include real secrets, environment values, tokens, private keys, or command arguments that reveal personal data.

## License

By contributing, you agree that your contribution is licensed under Apache-2.0, the repository's license.
