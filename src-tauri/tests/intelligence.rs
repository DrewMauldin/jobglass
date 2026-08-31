use chrono::{TimeZone, Utc};
use jobglass_lib::adapters::{cron, launchd, systemd, windows};
use jobglass_lib::diagnostics::{Visibility, VisibilityStatus, diagnose};
use jobglass_lib::export::{ExportError, ExportPolicy, export_csv, export_html, export_json};
use jobglass_lib::model::{Evidence, JobScope, ParseWarning, RunTime, SchedulerKind};
use std::path::PathBuf;

fn fixture(path: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("fixtures")
        .join(path);
    std::fs::read_to_string(path).expect("fixture should be readable")
}

fn available<T>(value: T, existing: &Evidence<T>) -> Evidence<T> {
    Evidence::Available {
        value,
        provenance: match existing {
            Evidence::Available { provenance, .. } | Evidence::Unavailable { provenance, .. } => {
                provenance.clone()
            }
        },
    }
}

#[test]
fn diagnostics_are_deterministic_and_cover_seeded_faults() {
    let mut first = launchd::parse_plist(
        fixture("macos/launchd-backup.plist").as_bytes(),
        "fixture-one.plist",
        JobScope::User,
    )
    .expect("first job");
    let mut second = first.clone();
    second.id = "job_second".into();
    second.environment_keys = available(vec!["HOME".into()], &second.environment_keys);
    first.last_run = available(
        RunTime {
            iso8601: "2025-01-01T00:00:00Z".into(),
            timezone_basis: "UTC".into(),
        },
        &first.last_run,
    );
    let warnings = vec![ParseWarning {
        code: "fixture.malformed".into(),
        message: "A fixture definition was malformed".into(),
        source_reference: "fixture-bad".into(),
    }];
    let visibility = vec![Visibility {
        scheduler: SchedulerKind::Cron,
        scope: JobScope::System,
        status: VisibilityStatus::PermissionLimited,
        explanation: "System crontab was not readable".into(),
    }];
    let now = Utc.with_ymd_and_hms(2026, 8, 31, 0, 0, 0).unwrap();

    let findings = diagnose(
        &[first.clone(), second.clone()],
        &warnings,
        &visibility,
        now,
        |_| false,
        |_| false,
    );
    let repeated = diagnose(
        &[first, second],
        &warnings,
        &visibility,
        now,
        |_| false,
        |_| false,
    );
    let codes = findings
        .iter()
        .map(|finding| finding.code.as_str())
        .collect::<Vec<_>>();

    assert_eq!(findings, repeated);
    for expected in [
        "duplicateIdentifier",
        "duplicateCommand",
        "likelyOverlap",
        "malformedDefinition",
        "missingExecutable",
        "invalidWorkingDirectory",
        "staleJob",
        "environmentDifference",
        "permissionLimited",
    ] {
        assert!(codes.contains(&expected), "missing finding {expected}");
    }
}

#[test]
fn malformed_definition_findings_have_unique_stable_ids() {
    let warnings = vec![
        ParseWarning {
            code: "cron.malformedLine".into(),
            message: "line 1 is malformed".into(),
            source_reference: "fixture:1".into(),
        },
        ParseWarning {
            code: "cron.malformedLine".into(),
            message: "line 2 is malformed".into(),
            source_reference: "fixture:1".into(),
        },
    ];
    let findings = diagnose(&[], &warnings, &[], Utc::now(), |_| true, |_| true);

    assert_eq!(findings.len(), 2);
    assert_ne!(findings[0].id, findings[1].id);
}

#[test]
fn exports_require_review_redact_arguments_and_escape_html() {
    let mut job = windows::parse_task_xml(
        fixture("windows/backup-task.xml").as_bytes(),
        "fixture task",
    )
    .expect("Windows fixture");
    job.display_name = available(
        "</script><img src=x onerror=alert(1)>".into(),
        &job.display_name,
    );

    assert_eq!(
        export_json(&[job.clone()], &[], ExportPolicy::default()),
        Err(ExportError::ReviewRequired)
    );
    let reviewed = ExportPolicy {
        reviewed: true,
        include_arguments: false,
    };
    let json = export_json(&[job.clone()], &[], reviewed).expect("reviewed JSON export");
    let csv = export_csv(&[job.clone()], &[], reviewed).expect("reviewed CSV export");
    let html = export_html(&[job], &[], reviewed).expect("reviewed HTML export");

    assert!(json.contains("<redacted>"));
    assert!(!json.contains("--retain"));
    assert!(csv.contains("<redacted>"));
    assert!(html.contains("&lt;/script&gt;&lt;img"));
    assert!(!html.contains("<img src=x"));
    assert!(html.contains("Content-Security-Policy"));
}

#[test]
fn default_export_redacts_arguments_repeated_in_finding_evidence() {
    let result = cron::parse_crontab(
        "0 1 * * * /usr/bin/backup --token fixture-secret\n0 2 * * * /usr/bin/backup --token fixture-secret\n",
        "sensitive fixture",
        JobScope::User,
        Some("fixture-user"),
        false,
    );
    let findings = diagnose(&result.jobs, &[], &[], Utc::now(), |_| true, |_| true);
    let redacted = export_json(
        &result.jobs,
        &findings,
        ExportPolicy {
            reviewed: true,
            include_arguments: false,
        },
    )
    .expect("redacted export");
    let included = export_json(
        &result.jobs,
        &findings,
        ExportPolicy {
            reviewed: true,
            include_arguments: true,
        },
    )
    .expect("reviewed argument export");

    assert!(!redacted.contains("fixture-secret"));
    assert!(included.contains("fixture-secret"));
}

#[test]
fn csv_export_neutralises_spreadsheet_formulas() {
    let mut job = windows::parse_task_xml(
        fixture("windows/backup-task.xml").as_bytes(),
        "fixture task",
    )
    .expect("Windows fixture");
    job.display_name = available("\t =2+2".into(), &job.display_name);

    let csv = export_csv(
        &[job],
        &[],
        ExportPolicy {
            reviewed: true,
            include_arguments: false,
        },
    )
    .expect("reviewed CSV export");

    assert!(csv.contains("\"'\t =2+2\""));
}

#[test]
fn including_arguments_is_an_explicit_reviewed_choice() {
    let result = cron::parse_crontab(
        &fixture("linux/cron/user.crontab"),
        "fixture crontab",
        JobScope::User,
        Some("fixture-user"),
        false,
    );
    let export = export_json(
        &result.jobs,
        &[],
        ExportPolicy {
            reviewed: true,
            include_arguments: true,
        },
    )
    .expect("reviewed argument export");

    assert!(export.contains("--quiet"));
}

#[test]
fn mixed_platform_bundle_reaches_diagnostics_and_export() {
    let launchd_job = launchd::parse_plist(
        fixture("macos/launchd-backup.plist").as_bytes(),
        "mixed launchd",
        JobScope::User,
    )
    .expect("launchd fixture");
    let cron_job = cron::parse_crontab(
        &fixture("linux/cron/user.crontab"),
        "mixed cron",
        JobScope::User,
        Some("fixture-user"),
        false,
    )
    .jobs
    .remove(0);
    let windows_job =
        windows::parse_task_xml(fixture("windows/backup-task.xml").as_bytes(), "mixed task")
            .expect("Windows fixture");
    let systemd_job = systemd::parse_timer_show(
        &fixture("linux/systemd/backup.timer.show"),
        JobScope::System,
    )
    .expect("systemd fixture");
    let jobs = vec![launchd_job, cron_job, systemd_job, windows_job];
    let findings = diagnose(&jobs, &[], &[], Utc::now(), |_| true, |_| true);
    let export = export_json(
        &jobs,
        &findings,
        ExportPolicy {
            reviewed: true,
            include_arguments: false,
        },
    )
    .expect("mixed export");

    assert!(export.contains("launchd"));
    assert!(export.contains("cron"));
    assert!(export.contains("windowsTaskScheduler"));
    assert!(export.contains("systemd"));
    assert!(!export.contains("TOP_SECRET_VALUE"));
}
