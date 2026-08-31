use crate::model::{
    Evidence, JobScope, Provenance, ScheduledJob, SchedulerKind, UnavailableReason, stable_job_id,
};

fn provenance() -> Provenance {
    Provenance {
        adapter: SchedulerKind::Launchd,
        source_reference: "/Library/LaunchAgents/example.plist".into(),
        detail: None,
    }
}

#[test]
fn stable_id_uses_scheduler_native_identifier_and_scope_only() {
    let user_id = stable_job_id(SchedulerKind::Launchd, "com.example.backup", JobScope::User);
    let same_user_id = stable_job_id(SchedulerKind::Launchd, "com.example.backup", JobScope::User);
    let system_id = stable_job_id(
        SchedulerKind::Launchd,
        "com.example.backup",
        JobScope::System,
    );

    assert_eq!(user_id, same_user_id);
    assert_ne!(user_id, system_id);
    assert!(user_id.starts_with("job_"));
}

#[test]
fn unavailable_evidence_serialises_reason_without_a_value() {
    let evidence = Evidence::<String>::Unavailable {
        reason: UnavailableReason::PermissionDenied,
        provenance: provenance(),
    };

    let json = serde_json::to_value(evidence).expect("evidence should serialise");

    assert_eq!(json["availability"], "unavailable");
    assert_eq!(json["reason"], "permissionDenied");
    assert!(json.get("value").is_none());
}

#[test]
fn scheduled_job_contract_has_environment_keys_but_no_value_field() {
    let job = ScheduledJob::example_for_tests();
    let json = serde_json::to_string(&job).expect("job should serialise");

    assert!(json.contains("environmentKeys"));
    assert!(!json.contains("environmentValues"));
    assert!(!json.contains("TOP_SECRET_VALUE"));
}
