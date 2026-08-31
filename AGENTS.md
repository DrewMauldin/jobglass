# Agent instructions

Read `SPEC.md`, `CONSTRAINTS.md`, `SUPPORT_MATRIX.md` and the active item in `tasks/todo.md` before writing code. Do not weaken `CONSTRAINTS.md` to make a change pass.

JobGlass v0.1.0 is strictly read-only. Never add scheduler mutation, execution, elevation, remote management, telemetry, accounts, cloud sync or AI. Treat native definitions and command output as hostile input. Never expose environment values.

Keep commits atomic and conventional. Run focused tests for each slice and the full gate before merge. Record platform evidence honestly.
