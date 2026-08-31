import type { Evidence, ExportFormat, ExportPolicy, ScanBundle } from "./types";

export function renderBrowserExport(
  bundle: ScanBundle,
  format: ExportFormat,
  policy: ExportPolicy,
): string {
  if (!policy.reviewed) throw new Error("export privacy review is required");
  const jobs = bundle.jobs.map((job) => ({
    ...job,
    arguments:
      policy.includeArguments || job.arguments.availability === "unavailable"
        ? job.arguments
        : { ...job.arguments, value: ["<redacted>"] },
  }));
  const argumentsToRedact = policy.includeArguments
    ? []
    : [
        ...new Set(
          bundle.jobs.flatMap((job) =>
            job.arguments.availability === "available"
              ? job.arguments.value
                  .filter(Boolean)
                  .flatMap((argument) => [argument, JSON.stringify(argument)])
              : [],
          ),
        ),
      ].sort((left, right) => right.length - left.length);
  const findings = bundle.findings.map((finding) => ({
    ...finding,
    evidence: finding.evidence.map((item) =>
      argumentsToRedact.reduce(
        (redacted, argument) => redacted.replaceAll(argument, "<redacted>"),
        item,
      ),
    ),
  }));
  if (format === "json")
    return JSON.stringify({ schemaVersion: "1.0", jobs, findings }, null, 2);
  if (format === "csv") {
    const rows = jobs.map((job) => {
      const findingCount = bundle.findings.filter((finding) =>
        finding.jobIds.includes(job.id),
      ).length;
      return [
        job.id,
        exportEvidenceText(job.scheduler, String),
        exportEvidenceText(job.displayName, String),
        exportEvidenceText(job.schedule, (value) => value.nativeExpression),
        exportEvidenceText(job.executable, String),
        exportEvidenceText(job.arguments, (value) => value.join(" ")),
        String(findingCount),
      ]
        .map(csvEscape)
        .join(",");
    });
    return [
      "id,scheduler,name,schedule,executable,arguments,finding_count",
      ...rows,
      "",
    ].join("\n");
  }
  const rows = jobs
    .map((job) => {
      const fields = [
        job.id,
        exportEvidenceText(job.scheduler, String),
        exportEvidenceText(job.displayName, String),
        exportEvidenceText(job.schedule, (value) => value.nativeExpression),
        exportEvidenceText(job.executable, String),
        exportEvidenceText(job.arguments, (value) => value.join(" ")),
      ];
      return `<tr>${fields.map((field) => `<td>${htmlEscape(field)}</td>`).join("")}</tr>`;
    })
    .join("");
  const findingCount = String(bundle.findings.length);
  const findingLabel = bundle.findings.length === 1 ? "finding" : "findings";
  return `<!doctype html><html lang="en"><head><meta charset="utf-8"><meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'unsafe-inline'"><meta name="viewport" content="width=device-width"><title>JobGlass report</title><style>body{font:15px system-ui;margin:2rem;color:#17201c}table{border-collapse:collapse;width:100%}th,td{border:1px solid #cad1cc;padding:.6rem;text-align:left;vertical-align:top}th{background:#edf2ee}caption{font-size:1.5rem;font-weight:700;text-align:left;margin-bottom:1rem}</style></head><body><main><table><caption>JobGlass scheduler report</caption><thead><tr><th>ID</th><th>Scheduler</th><th>Name</th><th>Schedule</th><th>Executable</th><th>Arguments</th></tr></thead><tbody>${rows}</tbody></table><p>${findingCount} diagnostic ${findingLabel}. Environment values are never represented.</p></main></body></html>`;
}

function exportEvidenceText<T>(
  field: Evidence<T>,
  format: (value: T) => string,
): string {
  return field.availability === "available"
    ? format(field.value)
    : `Unavailable: ${unavailableReasonLabel(field.reason)}`;
}

function unavailableReasonLabel(reason: string): string {
  return reason.replace(/([a-z])([A-Z])/g, "$1 $2").toLocaleLowerCase();
}

function csvEscape(value: string): string {
  const safe = /^[\t\r ]*[=+@-]/.test(value) ? `'${value}` : value;
  return `"${safe.replaceAll('"', '""')}"`;
}

function htmlEscape(value: string): string {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#39;");
}
