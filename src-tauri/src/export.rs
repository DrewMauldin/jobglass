use serde::Serialize;
use thiserror::Error;

use crate::diagnostics::Finding;
use crate::model::{Evidence, ScheduledJob, SchedulerKind};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ExportPolicy {
    pub reviewed: bool,
    pub include_arguments: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ExportError {
    #[error("export privacy review is required")]
    ReviewRequired,
    #[error("export serialisation failed: {0}")]
    Serialisation(String),
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ExportDocument<'a> {
    schema_version: &'static str,
    jobs: &'a [ScheduledJob],
    findings: &'a [Finding],
}

pub fn export_json(
    jobs: &[ScheduledJob],
    findings: &[Finding],
    policy: ExportPolicy,
) -> Result<String, ExportError> {
    require_review(policy)?;
    let jobs = prepared_jobs(jobs, policy);
    serde_json::to_string_pretty(&ExportDocument {
        schema_version: "1.0",
        jobs: &jobs,
        findings,
    })
    .map_err(|error| ExportError::Serialisation(error.to_string()))
}

pub fn export_csv(
    jobs: &[ScheduledJob],
    findings: &[Finding],
    policy: ExportPolicy,
) -> Result<String, ExportError> {
    require_review(policy)?;
    let jobs = prepared_jobs(jobs, policy);
    let mut output =
        String::from("id,scheduler,name,schedule,executable,arguments,finding_count\n");
    for job in &jobs {
        let finding_count = findings
            .iter()
            .filter(|finding| finding.job_ids.contains(&job.id))
            .count();
        let fields = [
            job.id.clone(),
            evidence_text(&job.scheduler, |value| format!("{value:?}")),
            evidence_text(&job.display_name, Clone::clone),
            evidence_text(&job.schedule, |value| value.native_expression.clone()),
            evidence_text(&job.executable, Clone::clone),
            evidence_text(&job.arguments, |value| value.join(" ")),
            finding_count.to_string(),
        ];
        output.push_str(
            &fields
                .into_iter()
                .map(|field| csv_escape(&field))
                .collect::<Vec<_>>()
                .join(","),
        );
        output.push('\n');
    }
    Ok(output)
}

pub fn export_html(
    jobs: &[ScheduledJob],
    findings: &[Finding],
    policy: ExportPolicy,
) -> Result<String, ExportError> {
    require_review(policy)?;
    let jobs = prepared_jobs(jobs, policy);
    let mut rows = String::new();
    for job in &jobs {
        let fields = [
            job.id.clone(),
            evidence_text(&job.scheduler, scheduler_name),
            evidence_text(&job.display_name, Clone::clone),
            evidence_text(&job.schedule, |value| value.native_expression.clone()),
            evidence_text(&job.executable, Clone::clone),
            evidence_text(&job.arguments, |value| value.join(" ")),
        ];
        rows.push_str("<tr>");
        for field in fields {
            rows.push_str("<td>");
            rows.push_str(&html_escape(&field));
            rows.push_str("</td>");
        }
        rows.push_str("</tr>");
    }
    Ok(format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; style-src 'unsafe-inline'\"><meta name=\"viewport\" content=\"width=device-width\"><title>JobGlass report</title><style>body{{font:15px system-ui;margin:2rem;color:#17201c}}table{{border-collapse:collapse;width:100%}}th,td{{border:1px solid #cad1cc;padding:.6rem;text-align:left;vertical-align:top}}th{{background:#edf2ee}}caption{{font-size:1.5rem;font-weight:700;text-align:left;margin-bottom:1rem}}</style></head><body><main><table><caption>JobGlass scheduler report</caption><thead><tr><th>ID</th><th>Scheduler</th><th>Name</th><th>Schedule</th><th>Executable</th><th>Arguments</th></tr></thead><tbody>{rows}</tbody></table><p>{} diagnostic findings. Environment values are never represented.</p></main></body></html>",
        findings.len()
    ))
}

fn require_review(policy: ExportPolicy) -> Result<(), ExportError> {
    if policy.reviewed {
        Ok(())
    } else {
        Err(ExportError::ReviewRequired)
    }
}

fn prepared_jobs(jobs: &[ScheduledJob], policy: ExportPolicy) -> Vec<ScheduledJob> {
    let mut jobs = jobs.to_vec();
    if !policy.include_arguments {
        for job in &mut jobs {
            if let Evidence::Available { provenance, .. } = &job.arguments {
                job.arguments = Evidence::available(vec!["<redacted>".into()], provenance.clone());
            }
        }
    }
    jobs
}

fn evidence_text<T>(evidence: &Evidence<T>, render: impl FnOnce(&T) -> String) -> String {
    match evidence {
        Evidence::Available { value, .. } => render(value),
        Evidence::Unavailable { reason, .. } => format!("Unavailable: {reason:?}"),
    }
}

fn scheduler_name(scheduler: &SchedulerKind) -> String {
    format!("{scheduler:?}")
}

fn csv_escape(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
