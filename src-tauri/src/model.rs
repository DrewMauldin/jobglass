use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const CONTRACT_VERSION: &str = "1.0";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SchedulerKind {
    Launchd,
    Cron,
    Systemd,
    WindowsTaskScheduler,
}

impl SchedulerKind {
    fn key(self) -> &'static str {
        match self {
            Self::Launchd => "launchd",
            Self::Cron => "cron",
            Self::Systemd => "systemd",
            Self::WindowsTaskScheduler => "windowsTaskScheduler",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum JobScope {
    User,
    System,
}

impl JobScope {
    fn key(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::System => "system",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PrivilegeLevel {
    StandardUser,
    Elevated,
    System,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EnabledState {
    Enabled,
    Disabled,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum UnavailableReason {
    NotReported,
    PermissionDenied,
    Unsupported,
    NotApplicable,
    ParseFailure,
    SourceMissing,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Provenance {
    pub adapter: SchedulerKind,
    pub source_reference: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "availability", rename_all = "camelCase")]
pub enum Evidence<T> {
    Available {
        value: T,
        provenance: Provenance,
    },
    Unavailable {
        reason: UnavailableReason,
        provenance: Provenance,
    },
}

impl<T> Evidence<T> {
    pub fn available(value: T, provenance: Provenance) -> Self {
        Self::Available { value, provenance }
    }

    pub fn unavailable(reason: UnavailableReason, provenance: Provenance) -> Self {
        Self::Unavailable { reason, provenance }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ScheduleKind {
    Calendar,
    Interval,
    Event,
    Boot,
    Manual,
    Composite,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleSpec {
    pub kind: ScheduleKind,
    pub native_expression: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimezoneBasis {
    pub name: String,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunTime {
    pub iso8601: String,
    pub timezone_basis: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OutcomeState {
    Success,
    Failed,
    Running,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LastOutcome {
    pub state: OutcomeState,
    pub native_code: Option<i64>,
    pub explanation: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeSource {
    pub source_type: String,
    pub reference: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Trigger {
    pub kind: String,
    pub expression: String,
    pub explanation: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParseWarning {
    pub code: String,
    pub message: String,
    pub source_reference: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduledJob {
    pub schema_version: String,
    pub id: String,
    pub scheduler: Evidence<SchedulerKind>,
    pub native_identifier: Evidence<String>,
    pub display_name: Evidence<String>,
    pub owner: Evidence<String>,
    pub scope: Evidence<JobScope>,
    pub privilege_level: Evidence<PrivilegeLevel>,
    pub enabled: Evidence<EnabledState>,
    pub schedule: Evidence<ScheduleSpec>,
    pub schedule_explanation: Evidence<String>,
    pub timezone_basis: Evidence<TimezoneBasis>,
    pub next_run: Evidence<RunTime>,
    pub last_run: Evidence<RunTime>,
    pub last_outcome: Evidence<LastOutcome>,
    pub executable: Evidence<String>,
    pub arguments: Evidence<Vec<String>>,
    pub working_directory: Evidence<String>,
    pub environment_keys: Evidence<Vec<String>>,
    pub native_source: Evidence<NativeSource>,
    pub triggers: Evidence<Vec<Trigger>>,
    pub dependencies: Evidence<Vec<String>>,
    pub target_service: Evidence<String>,
    pub parse_warnings: Vec<ParseWarning>,
}

impl ScheduledJob {
    pub fn new(
        scheduler: SchedulerKind,
        native_identifier: impl Into<String>,
        display_name: impl Into<String>,
        scope: JobScope,
        source_reference: impl Into<String>,
    ) -> Self {
        let native_identifier = native_identifier.into();
        let source_reference = source_reference.into();
        let provenance = Provenance {
            adapter: scheduler,
            source_reference: source_reference.clone(),
            detail: None,
        };
        macro_rules! unavailable {
            ($reason:expr) => {
                Evidence::unavailable($reason, provenance.clone())
            };
        }

        Self {
            schema_version: CONTRACT_VERSION.into(),
            id: stable_job_id(scheduler, &native_identifier, scope),
            scheduler: Evidence::available(scheduler, provenance.clone()),
            native_identifier: Evidence::available(native_identifier, provenance.clone()),
            display_name: Evidence::available(display_name.into(), provenance.clone()),
            owner: unavailable!(UnavailableReason::NotReported),
            scope: Evidence::available(scope, provenance.clone()),
            privilege_level: Evidence::available(
                match scope {
                    JobScope::User => PrivilegeLevel::StandardUser,
                    JobScope::System => PrivilegeLevel::System,
                },
                provenance.clone(),
            ),
            enabled: Evidence::available(EnabledState::Unknown, provenance.clone()),
            schedule: unavailable!(UnavailableReason::NotReported),
            schedule_explanation: unavailable!(UnavailableReason::NotReported),
            timezone_basis: unavailable!(UnavailableReason::NotReported),
            next_run: unavailable!(UnavailableReason::NotReported),
            last_run: unavailable!(UnavailableReason::NotReported),
            last_outcome: unavailable!(UnavailableReason::NotReported),
            executable: unavailable!(UnavailableReason::NotReported),
            arguments: unavailable!(UnavailableReason::NotReported),
            working_directory: unavailable!(UnavailableReason::NotReported),
            environment_keys: Evidence::available(Vec::new(), provenance.clone()),
            native_source: Evidence::available(
                NativeSource {
                    source_type: "nativeDefinition".into(),
                    reference: source_reference,
                },
                provenance.clone(),
            ),
            triggers: Evidence::available(Vec::new(), provenance.clone()),
            dependencies: Evidence::available(Vec::new(), provenance.clone()),
            target_service: unavailable!(UnavailableReason::NotApplicable),
            parse_warnings: Vec::new(),
        }
    }
}

pub fn stable_job_id(scheduler: SchedulerKind, native_identifier: &str, scope: JobScope) -> String {
    let mut hasher = Sha256::new();
    hasher.update(scheduler.key().as_bytes());
    hasher.update([0]);
    hasher.update(native_identifier.as_bytes());
    hasher.update([0]);
    hasher.update(scope.key().as_bytes());
    let digest = hasher.finalize();
    let short_hash = digest[..12]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("job_{short_hash}")
}

#[cfg(test)]
impl ScheduledJob {
    pub(crate) fn example_for_tests() -> Self {
        let provenance = Provenance {
            adapter: SchedulerKind::Launchd,
            source_reference: "/Library/LaunchAgents/example.plist".into(),
            detail: None,
        };
        macro_rules! available {
            ($value:expr) => {
                Evidence::Available {
                    value: $value,
                    provenance: provenance.clone(),
                }
            };
        }
        macro_rules! unavailable {
            ($reason:expr) => {
                Evidence::Unavailable {
                    reason: $reason,
                    provenance: provenance.clone(),
                }
            };
        }

        Self {
            schema_version: CONTRACT_VERSION.into(),
            id: stable_job_id(SchedulerKind::Launchd, "com.example.backup", JobScope::User),
            scheduler: available!(SchedulerKind::Launchd),
            native_identifier: available!("com.example.backup".into()),
            display_name: available!("Example backup".into()),
            owner: available!("example-user".into()),
            scope: available!(JobScope::User),
            privilege_level: available!(PrivilegeLevel::StandardUser),
            enabled: available!(EnabledState::Enabled),
            schedule: available!(ScheduleSpec {
                kind: ScheduleKind::Interval,
                native_expression: "3600".into(),
            }),
            schedule_explanation: available!("Every hour".into()),
            timezone_basis: available!(TimezoneBasis {
                name: "local".into(),
                source: "scheduler default".into(),
            }),
            next_run: unavailable!(UnavailableReason::NotReported),
            last_run: unavailable!(UnavailableReason::NotReported),
            last_outcome: unavailable!(UnavailableReason::NotReported),
            executable: available!("/usr/bin/example".into()),
            arguments: available!(vec!["--quiet".into()]),
            working_directory: unavailable!(UnavailableReason::NotReported),
            environment_keys: available!(vec!["PATH".into()]),
            native_source: available!(NativeSource {
                source_type: "propertyList".into(),
                reference: "/Library/LaunchAgents/example.plist".into(),
            }),
            triggers: available!(vec![]),
            dependencies: available!(vec![]),
            target_service: unavailable!(UnavailableReason::NotApplicable),
            parse_warnings: vec![],
        }
    }
}
