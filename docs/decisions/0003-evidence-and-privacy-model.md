# ADR-0003: Preserve provenance and exclude environment values by construction

## Status

Accepted

## Date

2026-08-31

## Context

Scheduler definitions mix facts, hints and unavailable state. They also commonly contain sensitive paths, arguments and environment values. A dashboard that silently fills gaps or leaks report contents would undermine the product thesis.

## Decision

Every optional model field uses an evidence wrapper with availability, provenance and an optional reason. Environment values are discarded at the native adapter boundary; only validated key names are retained. Command arguments stay in the local model for inspection but exports require a policy decision and default to `[REDACTED]`.

Diagnostic findings include a stable rule ID, severity, explanation, evidence references and suggested manual investigation. They never make changes and never claim execution success without native evidence.

## Alternatives considered

- Nullable fields: rejected because `null` cannot distinguish unsupported, denied, malformed and genuinely absent evidence.
- Regex-only secret detection: retained only as a review warning, not a guarantee. Default redaction is the actual safety boundary.
- AI classification: rejected because v0.1.0 diagnostics must be deterministic, offline and auditable.

## Consequences

- The model is more verbose but honest.
- Exports remain useful without environment values.
- Users can opt into reviewed arguments while seeing likely-secret warnings before generation.
