# Quick start

## 1. Read the scan state

JobGlass scans automatically on launch. The heading identifies the native scheduler and timestamp. A **Partial visibility** badge means one or more definitions or runtime records could not be observed with the current user's permissions. The app does not elevate to fill the gap.

The summary distinguishes:

- scheduled jobs discovered;
- jobs with error-severity findings;
- jobs with an observable next run;
- scheduler scopes with visibility limits.

## 2. Find a job

Search matches job names, native identifiers, commands, and schedule text. Use the scheduler filter on platforms with multiple sources. Search and filter apply to both **List** and **Timeline**.

The list intentionally begins with 100 rows. Choose **Show 100 more jobs** to page through a large machine without rendering thousands of rows at once.

## 3. Read evidence, not guesses

Select a job to open the evidence inspector. Every value comes from a named native definition or runtime query. **Unavailable** and **Unknown** are meaningful states; JobGlass does not synthesise a next run, last outcome, owner, or working directory when the platform did not provide one.

JobGlass shows environment key names only. It never places environment values in the model or interface.

## 4. Review findings

Open **Findings** to review deterministic configuration checks. Findings explain the condition and supporting evidence. They are not malware classifications and do not establish that a command is safe or unsafe.

JobGlass cannot fix a finding. Apply changes through the operating system's documented scheduler tools, then relaunch JobGlass to collect fresh evidence.

## 5. Export carefully

Choose **Export report** to open the privacy checkpoint. Before any format is prepared, acknowledge the summary and intended destination.

- Arguments are redacted by default.
- Including arguments requires a separate explicit choice.
- Environment values are impossible to include.
- Paths, usernames, commands, owners, labels, schedules, and source references can still identify a machine.

JSON is best for tools, CSV for tabular review, and self-contained HTML for a human-readable offline report. The app prepares the content locally; use the platform's download/save surface to choose its destination.

## Keyboard and motion

Use normal Tab and Shift-Tab navigation. The skip link appears when focused and moves directly to scheduled jobs. Buttons, tabs, search, filters, checkboxes, and the export dialog expose accessible names. JobGlass follows the operating system's reduced-motion preference and supports system, light, and dark themes.
