use jobglass_lib::adapters::{cron, launchd, systemd, windows};
use jobglass_lib::model::{EnabledState, Evidence, JobScope, OutcomeState, ScheduleKind};
use std::path::PathBuf;

fn fixture(path: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("fixtures")
        .join(path);
    std::fs::read_to_string(path).expect("fixture should be readable")
}

fn value<T>(evidence: &Evidence<T>) -> &T {
    match evidence {
        Evidence::Available { value, .. } => value,
        Evidence::Unavailable { reason, .. } => panic!("expected evidence, got {reason:?}"),
    }
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
    launchd::enrich_launchctl_domain(&mut job, &fixture("macos/launchctl-domain.txt"));
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
