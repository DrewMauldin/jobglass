# Privacy

JobGlass is local-only by design. It has no account, backend, cloud sync, telemetry, analytics, advertising, crash reporter, AI service, remote-management channel, or automatic updater.

## Data collected from the machine

Subject to the current user's permissions, a scan may contain:

- scheduler type, native identifier, owner/scope, privilege and enabled state;
- schedule expressions and native explanations;
- observable next/last runs and outcomes;
- executable, command arguments, working directory, triggers, dependencies, and target service;
- environment **key names only**;
- source paths, provenance, and parse/permission warnings.

Environment values never enter the canonical model. JobGlass does not execute discovered commands.

## Storage and network

Scan evidence remains in application memory. JobGlass does not upload it or maintain a scan history. Theme choice may be stored in the WebView's normal local preferences. Export files exist only when the user reviews and saves them.

The packaged application has no product network feature. Repository badges, release downloads, and the public documentation site are ordinary GitHub web resources outside the desktop app.

## Export boundary

Every export requires acknowledgement of a privacy checkpoint. Arguments remain redacted unless separately included. Even a redacted report may expose machine-identifying paths, usernames, labels, commands, owners, schedules, and source references.

Before sharing:

1. keep arguments redacted unless essential;
2. inspect the generated file in a text editor;
3. remove unnecessary machine identifiers outside JobGlass;
4. choose a recipient and storage location appropriate to the data.

## Screenshots and fixtures

The repository's product media comes from a real scan and therefore may show public operating-system labels and system paths. Contributed fixtures and screenshots must be sanitised. Never file a public issue containing real secrets, private arguments, environment values, tokens, or personal paths.
