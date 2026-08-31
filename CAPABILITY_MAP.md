# Capability Map: JobGlass

| Module id                   | Responsibility                                                                               | Depends on                                                             |
| --------------------------- | -------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------- |
| `native-inventory`          | Read native scheduler definitions and observable runtime state without mutation or elevation | -                                                                      |
| `normalised-model`          | Convert native evidence into one versioned, provenance-rich contract                         | `native-inventory`                                                     |
| `deterministic-diagnostics` | Explain malformed, missing, stale, duplicate and overlapping jobs from the normalised model  | `normalised-model`                                                     |
| `privacy-safe-export`       | Review and redact sensitive fields before JSON, CSV or self-contained HTML export            | `normalised-model`, `deterministic-diagnostics`                        |
| `desktop-experience`        | Present overview, list, timeline, inspector and diagnostics with accessible interaction      | `normalised-model`, `deterministic-diagnostics`, `privacy-safe-export` |
| `release-and-docs`          | Build, verify, package, document and publish reproducible FOSS releases                      | all modules                                                            |

Build order: `native-inventory` -> `normalised-model` -> `deterministic-diagnostics` -> `privacy-safe-export` -> `desktop-experience` -> `release-and-docs`.

The dependency graph is acyclic. Native adapters never depend on presentation or export code. Diagnostics consume normalised data only, so platform-specific uncertainty remains explicit at the boundary.
