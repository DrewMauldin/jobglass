use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::model::{EnabledState, Evidence, JobScope, ParseWarning, ScheduledJob, SchedulerKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FindingSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    pub id: String,
    pub code: String,
    pub severity: FindingSeverity,
    pub title: String,
    pub explanation: String,
    pub job_ids: Vec<String>,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VisibilityStatus {
    Complete,
    PermissionLimited,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Visibility {
    pub scheduler: SchedulerKind,
    pub scope: JobScope,
    pub status: VisibilityStatus,
    pub explanation: String,
}

pub fn diagnose(
    jobs: &[ScheduledJob],
    warnings: &[ParseWarning],
    visibility: &[Visibility],
    now: DateTime<Utc>,
    path_exists: impl Fn(&str) -> bool,
) -> Vec<Finding> {
    let mut findings = Vec::new();
    add_group_findings(
        &mut findings,
        jobs,
        "duplicateIdentifier",
        "Duplicate native identifier",
        "Multiple definitions use the same scheduler and native identifier.",
        |job| {
            Some(format!(
                "{:?}:{}",
                available(&job.scheduler)?,
                available(&job.native_identifier)?
            ))
        },
    );
    add_group_findings(
        &mut findings,
        jobs,
        "duplicateCommand",
        "Duplicate command",
        "Multiple jobs launch the same executable with the same arguments.",
        |job| {
            Some(format!(
                "{} {:?}",
                available(&job.executable)?,
                available(&job.arguments)?
            ))
        },
    );
    add_group_findings(
        &mut findings,
        jobs,
        "likelyOverlap",
        "Likely schedule overlap",
        "Multiple jobs expose the same native schedule expression.",
        |job| Some(available(&job.schedule)?.native_expression.clone()),
    );

    for warning in warnings {
        findings.push(finding(
            "malformedDefinition",
            FindingSeverity::Warning,
            "Malformed scheduler definition",
            &warning.message,
            Vec::new(),
            vec![warning.source_reference.clone(), warning.code.clone()],
        ));
    }
    for job in jobs {
        if let Some(executable) = available(&job.executable)
            && is_path(executable)
            && !path_exists(executable)
        {
            findings.push(finding(
                "missingExecutable",
                FindingSeverity::Error,
                "Executable is missing",
                "The configured executable was not present when JobGlass checked it.",
                vec![job.id.clone()],
                vec![executable.clone()],
            ));
        }
        if let Some(directory) = available(&job.working_directory)
            && !path_exists(directory)
        {
            findings.push(finding(
                "invalidWorkingDirectory",
                FindingSeverity::Error,
                "Working directory is missing",
                "The configured working directory was not present when JobGlass checked it.",
                vec![job.id.clone()],
                vec![directory.clone()],
            ));
        }
        if available(&job.enabled) == Some(&EnabledState::Disabled) {
            findings.push(finding(
                "disabledJob",
                FindingSeverity::Info,
                "Job is disabled",
                "The native scheduler definition reports this job as disabled.",
                vec![job.id.clone()],
                Vec::new(),
            ));
        }
        if let Some(last_run) = available(&job.last_run)
            && DateTime::parse_from_rfc3339(&last_run.iso8601).is_ok_and(|last| {
                now.signed_duration_since(last.with_timezone(&Utc)) > Duration::days(30)
            })
        {
            findings.push(finding(
                "staleJob",
                FindingSeverity::Warning,
                "Job has not run recently",
                "The last observable run is more than 30 days old.",
                vec![job.id.clone()],
                vec![last_run.iso8601.clone()],
            ));
        }
        if let Some(executable) = available(&job.executable)
            && !is_path(executable)
        {
            findings.push(finding(
                "pathDependentCommand",
                FindingSeverity::Warning,
                "Command depends on PATH",
                "The executable is not an absolute path, so scheduler PATH differences may change what runs.",
                vec![job.id.clone()],
                vec![executable.clone()],
            ));
        }
    }

    let environment_sets = jobs
        .iter()
        .filter_map(|job| available(&job.environment_keys))
        .map(|keys| keys.iter().cloned().collect::<BTreeSet<_>>())
        .collect::<BTreeSet<_>>();
    if environment_sets.len() > 1 {
        findings.push(finding(
            "environmentDifference",
            FindingSeverity::Warning,
            "Scheduler environments differ",
            "Jobs expose different environment key sets; environment values remain private.",
            jobs.iter().map(|job| job.id.clone()).collect(),
            environment_sets
                .into_iter()
                .map(|keys| keys.into_iter().collect::<Vec<_>>().join(", "))
                .collect(),
        ));
    }
    for item in visibility {
        if item.status != VisibilityStatus::Complete {
            findings.push(finding(
                "permissionLimited",
                FindingSeverity::Warning,
                "Scheduler visibility is limited",
                &item.explanation,
                Vec::new(),
                vec![format!("{:?} {:?}", item.scheduler, item.scope)],
            ));
        }
    }

    findings.sort_by(|left, right| {
        left.code
            .cmp(&right.code)
            .then_with(|| left.id.cmp(&right.id))
    });
    findings
}

fn add_group_findings(
    findings: &mut Vec<Finding>,
    jobs: &[ScheduledJob],
    code: &str,
    title: &str,
    explanation: &str,
    key: impl Fn(&ScheduledJob) -> Option<String>,
) {
    let mut groups = BTreeMap::<String, Vec<String>>::new();
    for job in jobs {
        if let Some(key) = key(job) {
            groups.entry(key).or_default().push(job.id.clone());
        }
    }
    for (evidence, mut job_ids) in groups.into_iter().filter(|(_, ids)| ids.len() > 1) {
        job_ids.sort();
        findings.push(finding(
            code,
            FindingSeverity::Warning,
            title,
            explanation,
            job_ids,
            vec![evidence],
        ));
    }
}

fn finding(
    code: &str,
    severity: FindingSeverity,
    title: &str,
    explanation: &str,
    mut job_ids: Vec<String>,
    mut evidence: Vec<String>,
) -> Finding {
    job_ids.sort();
    evidence.sort();
    let mut hasher = Sha256::new();
    for part in std::iter::once(code)
        .chain(job_ids.iter().map(String::as_str))
        .chain(evidence.iter().map(String::as_str))
    {
        hasher.update(part.as_bytes());
        hasher.update([0]);
    }
    let id = hasher.finalize()[..10]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    Finding {
        id: format!("finding_{id}"),
        code: code.into(),
        severity,
        title: title.into(),
        explanation: explanation.into(),
        job_ids,
        evidence,
    }
}

fn available<T>(evidence: &Evidence<T>) -> Option<&T> {
    match evidence {
        Evidence::Available { value, .. } => Some(value),
        Evidence::Unavailable { .. } => None,
    }
}

fn is_path(executable: &str) -> bool {
    executable.starts_with('/')
        || executable.starts_with("./")
        || executable.as_bytes().get(1) == Some(&b':')
        || executable.starts_with("\\\\")
}
