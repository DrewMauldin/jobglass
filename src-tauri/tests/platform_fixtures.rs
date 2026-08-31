use chrono::{DateTime, Utc};
use jobglass_lib::adapters::{cron, launchd, systemd, windows};
use jobglass_lib::diagnostics::{Visibility, VisibilityStatus, diagnose};
use jobglass_lib::model::{
    EnabledState, Evidence, JobScope, OutcomeState, ParseWarning, ScheduleKind,
};
use std::path::PathBuf;

fn fixture(path: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("fixtures")
        .join(path);
    std::fs::read_to_string(path).expect("fixture should be readable")
}

fn fixture_bytes(path: &str) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("fixtures")
        .join(path);
    std::fs::read(path).expect("fixture should be readable")
}

fn value<T>(evidence: &Evidence<T>) -> &T {
    match evidence {
        Evidence::Available { value, .. } => value,
        Evidence::Unavailable { reason, .. } => panic!("expected evidence, got {reason:?}"),
    }
}

fn available_like<T>(value: T, evidence: &Evidence<T>) -> Evidence<T> {
    let provenance = match evidence {
        Evidence::Available { provenance, .. } | Evidence::Unavailable { provenance, .. } => {
            provenance.clone()
        }
    };
    Evidence::available(value, provenance)
}

#[test]
fn launchd_fixture_normalises_and_runtime_evidence_enriches_it() {
    let mut job = launchd::parse_plist(
        fixture("macos/launchd-backup.plist").as_bytes(),
        "/Library/LaunchAgents/backup.plist",
        JobScope::User,
    )
    .expect("valid launchd fixture");

    assert_eq!(value(&job.native_identifier), "com.jobglass.fixture.backup");
    assert_eq!(value(&job.schedule).kind, ScheduleKind::Composite);
    assert_eq!(value(&job.environment_keys), &["BACKUP_TOKEN", "PATH"]);
    assert!(
        !serde_json::to_string(&job)
            .expect("serialise launchd job")
            .contains("TOP_SECRET_VALUE")
    );

    launchd::enrich_launchctl(&mut job, &fixture("macos/launchctl-backup.txt"));
    assert_eq!(value(&job.last_outcome).state, OutcomeState::Success);
    let domain = launchd::parse_launchctl_domain(&fixture("macos/launchctl-domain.txt"));
    assert_eq!(domain.len(), 2);
    launchd::enrich_launchctl_domain(&mut job, &domain);
    assert_eq!(value(&job.last_outcome).state, OutcomeState::Success);
    assert!(
        launchd::parse_plist(
            fixture("macos/malformed.plist").as_bytes(),
            "malformed.plist",
            JobScope::User,
        )
        .is_err()
    );
}

#[test]
fn launchd_binary_plist_is_bounded_and_supported() {
    let xml = fixture("macos/launchd-backup.plist");
    let plist_value = plist::Value::from_reader_xml(xml.as_bytes()).expect("XML fixture value");
    let mut binary = Vec::new();
    plist_value
        .to_writer_binary(&mut binary)
        .expect("binary fixture encoding");

    let job = launchd::parse_plist(&binary, "binary fixture", JobScope::User)
        .expect("valid binary launchd fixture");

    assert_eq!(value(&job.native_identifier), "com.jobglass.fixture.backup");
}

#[test]
fn cron_variants_keep_environment_keys_and_report_bad_lines() {
    let user = cron::parse_crontab(
        &fixture("linux/cron/user.crontab"),
        "user crontab",
        JobScope::User,
        Some("fixture-user"),
        false,
    );
    assert_eq!(user.jobs.len(), 2);
    assert_eq!(value(&user.jobs[0].environment_keys), &["MAILTO", "PATH"]);
    assert_eq!(
        value(&user.jobs[0].executable),
        "/usr/local/bin/refresh-cache"
    );

    let system = cron::parse_crontab(
        &fixture("linux/cron/system.crontab"),
        "/etc/crontab",
        JobScope::System,
        None,
        true,
    );
    assert_eq!(system.jobs.len(), 1);
    assert_eq!(value(&system.jobs[0].owner), "backup");
    assert_eq!(system.warnings.len(), 1);

    let periodic = cron::periodic_job(
        "/etc/cron.daily/jobglass-fixture",
        "daily",
        JobScope::System,
    );
    assert_eq!(
        value(&periodic.executable),
        "/etc/cron.daily/jobglass-fixture"
    );
    assert_eq!(value(&periodic.schedule).native_expression, "@daily");
}

#[test]
fn cron_rejects_out_of_range_fields_and_unknown_nicknames() {
    let result = cron::parse_crontab(
        "99 99 99 99 99 /bin/true\n@bogus /bin/true\n",
        "malformed fixture",
        JobScope::User,
        Some("fixture-user"),
        false,
    );

    assert!(result.jobs.is_empty());
    assert_eq!(result.warnings.len(), 2);
    assert_ne!(
        result.warnings[0].source_reference,
        result.warnings[1].source_reference
    );
}

#[test]
fn systemd_timer_preserves_calendar_and_monotonic_evidence() {
    let job = systemd::parse_timer_show(
        &fixture("linux/systemd/backup.timer.show"),
        JobScope::System,
    )
    .expect("valid systemd timer fixture");

    assert_eq!(value(&job.native_identifier), "backup.timer");
    assert_eq!(value(&job.schedule).kind, ScheduleKind::Composite);
    assert_eq!(value(&job.target_service), "backup.service");
    assert!(value(&job.dependencies).contains(&"network-online.target".into()));
    assert!(DateTime::parse_from_rfc3339(&value(&job.next_run).iso8601).is_ok());
    assert!(DateTime::parse_from_rfc3339(&value(&job.last_run).iso8601).is_ok());
    assert_eq!(value(&job.last_outcome).state, OutcomeState::Success);
    assert!(systemd::valid_timer_identifier("backup.timer"));
    assert!(!systemd::valid_timer_identifier("--host=remote.timer"));
    assert!(!systemd::valid_timer_identifier("../../remote.timer"));
}

#[test]
fn windows_namespaced_xml_normalises_actions_principal_and_state() {
    let job = windows::parse_task_xml(
        fixture("windows/backup-task.xml").as_bytes(),
        "Task Scheduler XML",
    )
    .expect("valid Windows task fixture");

    assert_eq!(value(&job.enabled), &EnabledState::Disabled);
    assert_eq!(value(&job.owner), "fixture-user");
    assert_eq!(value(&job.executable), r"C:\Tools\backup.exe");
    assert_eq!(value(&job.arguments), &["--quiet", "--retain", "7"]);
    assert!(
        windows::parse_task_xml(
            fixture("windows/malformed-task.xml").as_bytes(),
            "malformed XML",
        )
        .is_err()
    );
}

#[test]
fn windows_collection_preserves_each_task_scope_and_runtime() {
    let mut result = windows::parse_task_xml_collection(
        &fixture_bytes("windows/multiple-tasks.xml"),
        "Task Scheduler collection",
    );
    assert!(result.warnings.is_empty(), "{:?}", result.warnings);
    assert_eq!(result.jobs.len(), 2);
    assert_eq!(value(&result.jobs[0].scope), &JobScope::User);
    assert_eq!(value(&result.jobs[1].scope), &JobScope::System);
    assert_ne!(result.jobs[0].id, result.jobs[1].id);

    windows::enrich_runtime_json(
        &mut result.jobs,
        &fixture("windows/runtime.json"),
        "Task Scheduler runtime query",
    )
    .expect("valid invariant runtime JSON");

    assert_eq!(
        value(&result.jobs[0].last_outcome).state,
        OutcomeState::Success
    );
    assert_eq!(
        value(&result.jobs[1].last_outcome).state,
        OutcomeState::Failed
    );
    assert!(DateTime::parse_from_rfc3339(&value(&result.jobs[0].next_run).iso8601).is_ok());
}

#[test]
fn windows_parser_accepts_actual_utf16_bytes() {
    let bytes = fixture_bytes("windows/multiple-tasks.xml");
    assert!(matches!(&bytes[..2], [0xff, 0xfe] | [0xfe, 0xff]));

    let result = windows::parse_task_xml_collection(&bytes, "UTF-16 fixture");
    assert!(result.warnings.is_empty(), "{:?}", result.warnings);
    assert_eq!(result.jobs.len(), 2);
}

#[test]
fn every_platform_fixture_reaches_fault_and_visibility_diagnostics() {
    let jobs = vec![
        launchd::parse_plist(
            fixture("macos/launchd-backup.plist").as_bytes(),
            "launchd category fixture",
            JobScope::User,
        )
        .expect("launchd fixture"),
        cron::parse_crontab(
            &fixture("linux/cron/user.crontab"),
            "cron category fixture",
            JobScope::User,
            Some("fixture-user"),
            false,
        )
        .jobs
        .remove(0),
        systemd::parse_timer_show(
            &fixture("linux/systemd/backup.timer.show"),
            JobScope::System,
        )
        .expect("systemd fixture"),
        windows::parse_task_xml(
            fixture("windows/backup-task.xml").as_bytes(),
            "Windows category fixture",
        )
        .expect("Windows fixture"),
    ];

    for (index, mut job) in jobs.into_iter().enumerate() {
        job.executable = available_like(format!("/fixture/missing-{index}"), &job.executable);
        job.arguments = available_like(Vec::new(), &job.arguments);
        job.enabled = available_like(EnabledState::Disabled, &job.enabled);
        let mut duplicate = job.clone();
        duplicate.id = format!("duplicate_{index}");
        let scheduler = *value(&job.scheduler);
        let scope = *value(&job.scope);
        let warnings = [ParseWarning {
            code: "fixture.malformed".into(),
            message: "Malformed platform category fixture".into(),
            source_reference: format!("fixture:{scheduler:?}"),
        }];
        let visibility = [Visibility {
            scheduler,
            scope,
            status: VisibilityStatus::PermissionLimited,
            explanation: "Permission-limited platform category fixture".into(),
        }];
        let findings = diagnose(
            &[job, duplicate],
            &warnings,
            &visibility,
            Utc::now(),
            |_| false,
            |_| true,
        );
        let codes = findings
            .iter()
            .map(|finding| finding.code.as_str())
            .collect::<Vec<_>>();

        for expected in [
            "disabledJob",
            "duplicateCommand",
            "duplicateIdentifier",
            "likelyOverlap",
            "malformedDefinition",
            "missingExecutable",
            "permissionLimited",
        ] {
            assert!(
                codes.contains(&expected),
                "{scheduler:?} fixture missed {expected}: {codes:?}"
            );
        }
    }
}
