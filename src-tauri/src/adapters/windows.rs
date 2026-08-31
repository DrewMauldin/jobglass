use std::collections::HashMap;

use chrono::DateTime;
use roxmltree::{Document, Node};
use serde::Deserialize;

use crate::adapters::{AdapterResult, warning};
use crate::input::{MAX_JOBS, validate_bounded_bytes};
use crate::model::{
    EnabledState, Evidence, JobScope, LastOutcome, OutcomeState, ParseWarning, PrivilegeLevel,
    Provenance, RunTime, ScheduleKind, ScheduleSpec, ScheduledJob, SchedulerKind, TimezoneBasis,
    Trigger,
};

pub fn parse_task_xml(input: &[u8], source: &str) -> Result<ScheduledJob, ParseWarning> {
    let mut result = parse_task_xml_collection(input, source);
    result.jobs.drain(..).next().ok_or_else(|| {
        result.warnings.into_iter().next().unwrap_or_else(|| {
            warning(
                "windows.task",
                "Task Scheduler XML contained no task definitions",
                source,
            )
        })
    })
}

pub fn parse_task_xml_collection(input: &[u8], source: &str) -> AdapterResult {
    let text = match decode_xml(input) {
        Ok(text) => text,
        Err(error) => {
            return AdapterResult {
                jobs: Vec::new(),
                warnings: vec![warning("windows.input", error, source)],
            };
        }
    };
    let document = match Document::parse(&text) {
        Ok(document) => document,
        Err(error) => {
            return AdapterResult {
                jobs: Vec::new(),
                warnings: vec![warning("windows.xml", error.to_string(), source)],
            };
        }
    };
    let task_nodes = document
        .descendants()
        .filter(|node| node.is_element() && node.tag_name().name() == "Task")
        .collect::<Vec<_>>();
    let mut result = AdapterResult::default();
    for (index, task) in task_nodes.iter().take(MAX_JOBS).enumerate() {
        let task_source = format!("{source}:task {}", index + 1);
        match parse_task_node(*task, &task_source) {
            Ok(job) => result.jobs.push(job),
            Err(parse_warning) => result.warnings.push(parse_warning),
        }
    }
    if task_nodes.len() > MAX_JOBS {
        result.warnings.push(warning(
            "windows.jobLimit",
            format!("job limit of {MAX_JOBS} reached"),
            source,
        ));
    }
    result
}

fn parse_task_node(task: Node<'_, '_>, source: &str) -> Result<ScheduledJob, ParseWarning> {
    let identifier = text_of(task, "URI")
        .ok_or_else(|| warning("windows.uri", "task URI is missing", source))?;
    let owner = principal_text(task, "UserId").or_else(|| principal_text(task, "GroupId"));
    let scope = if owner.as_deref().is_some_and(is_system_principal) {
        JobScope::System
    } else {
        JobScope::User
    };
    let source = format!("{source}:{identifier}");
    let provenance = provenance(&source);
    let display_name = identifier
        .rsplit('\\')
        .next()
        .unwrap_or(&identifier)
        .to_owned();
    let mut job = ScheduledJob::new(
        SchedulerKind::WindowsTaskScheduler,
        identifier,
        display_name,
        scope,
        &source,
    );

    if let Some(owner) = owner {
        job.owner = Evidence::available(owner, provenance.clone());
    }
    if scope == JobScope::System {
        job.privilege_level = Evidence::available(PrivilegeLevel::System, provenance.clone());
    }
    if scope == JobScope::User
        && let Some(run_level) = principal_text(task, "RunLevel")
    {
        let privilege = if run_level.eq_ignore_ascii_case("HighestAvailable") {
            PrivilegeLevel::Elevated
        } else if run_level.eq_ignore_ascii_case("LeastPrivilege") {
            PrivilegeLevel::StandardUser
        } else {
            job.parse_warnings.push(warning(
                "windows.runLevel",
                "RunLevel has an invalid value",
                &source,
            ));
            PrivilegeLevel::Unknown
        };
        job.privilege_level = Evidence::available(privilege, provenance.clone());
    }
    let enabled_value = task
        .descendants()
        .find(|node| node.is_element() && node.tag_name().name() == "Settings")
        .and_then(|settings| direct_child_text(settings, "Enabled"));
    let enabled = match enabled_value.as_deref() {
        None => EnabledState::Enabled,
        Some(value) if value.eq_ignore_ascii_case("true") => EnabledState::Enabled,
        Some(value) if value.eq_ignore_ascii_case("false") => EnabledState::Disabled,
        Some(_) => {
            job.parse_warnings.push(warning(
                "windows.enabled",
                "Enabled has an invalid value",
                &source,
            ));
            EnabledState::Unknown
        }
    };
    job.enabled = Evidence::available(enabled, provenance.clone());

    let exec_actions = task
        .descendants()
        .filter(|node| node.is_element() && node.tag_name().name() == "Exec")
        .collect::<Vec<_>>();
    let action = exec_actions
        .first()
        .copied()
        .ok_or_else(|| warning("windows.command", "Exec action is missing", &source))?;
    let command = text_of(action, "Command")
        .ok_or_else(|| warning("windows.command", "Exec Command is missing", &source))?;
    job.executable = Evidence::available(command, provenance.clone());
    job.arguments = Evidence::available(
        text_of(action, "Arguments")
            .map(|arguments| arguments.split_whitespace().map(str::to_owned).collect())
            .unwrap_or_default(),
        provenance.clone(),
    );
    if let Some(directory) = text_of(action, "WorkingDirectory") {
        job.working_directory = Evidence::available(directory, provenance.clone());
    }
    if exec_actions.len() > 1 {
        job.parse_warnings.push(warning(
            "windows.multipleExecActions",
            "Only the first Exec action is represented by the canonical executable field",
            &source,
        ));
    }

    let triggers = task
        .descendants()
        .filter(|node| {
            node.is_element()
                && matches!(
                    node.tag_name().name(),
                    "CalendarTrigger" | "TimeTrigger" | "BootTrigger" | "LogonTrigger"
                )
        })
        .map(task_trigger)
        .collect::<Vec<_>>();
    if !triggers.is_empty() {
        let kind = if triggers.len() > 1 {
            ScheduleKind::Composite
        } else if triggers[0].kind == "calendar" {
            ScheduleKind::Calendar
        } else if triggers[0].kind == "boot" {
            ScheduleKind::Boot
        } else {
            ScheduleKind::Event
        };
        job.schedule = Evidence::available(
            ScheduleSpec {
                kind,
                native_expression: triggers
                    .iter()
                    .map(|trigger| trigger.expression.as_str())
                    .collect::<Vec<_>>()
                    .join("; "),
            },
            provenance.clone(),
        );
        job.schedule_explanation = Evidence::available(
            triggers
                .iter()
                .map(|trigger| trigger.explanation.as_str())
                .collect::<Vec<_>>()
                .join("; "),
            provenance.clone(),
        );
        job.triggers = Evidence::available(triggers, provenance.clone());
    }
    job.timezone_basis = Evidence::available(
        TimezoneBasis {
            name: "task definition offset or local timezone".into(),
            source: "Task Scheduler XML".into(),
        },
        provenance,
    );
    Ok(job)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeEvidence {
    identifier: String,
    next_run_time: Option<String>,
    last_run_time: Option<String>,
    last_task_result: Option<i64>,
    state: String,
}

pub fn enrich_runtime_json(
    jobs: &mut [ScheduledJob],
    input: &str,
    source: &str,
) -> Result<(), ParseWarning> {
    let records = serde_json::from_str::<Vec<RuntimeEvidence>>(input)
        .map_err(|error| warning("windows.runtimeJson", error.to_string(), source))?;
    if records.len() > MAX_JOBS {
        return Err(warning(
            "windows.runtimeLimit",
            format!("runtime record limit of {MAX_JOBS} exceeded"),
            source,
        ));
    }
    let mut job_indexes = HashMap::new();
    for (index, job) in jobs.iter().enumerate() {
        if let Evidence::Available { value, .. } = &job.native_identifier {
            job_indexes.entry(value.clone()).or_insert(index);
        }
    }
    for record in records {
        let Some(index) = job_indexes.get(&record.identifier).copied() else {
            continue;
        };
        let job = &mut jobs[index];
        let provenance = provenance(source);
        if let Some(next_run_time) = record.next_run_time.as_deref() {
            if let Some(next_run) = normalise_runtime(next_run_time) {
                job.next_run = Evidence::available(next_run, provenance.clone());
            } else {
                job.parse_warnings.push(warning(
                    "windows.nextRunTime",
                    "NextRunTime was not valid RFC3339",
                    source,
                ));
            }
        }
        let last_run = record.last_run_time.as_deref().and_then(normalise_runtime);
        if record.last_run_time.is_some() && last_run.is_none() {
            job.parse_warnings.push(warning(
                "windows.lastRunTime",
                "LastRunTime was not valid RFC3339",
                source,
            ));
        }
        let had_last_run = last_run.is_some();
        if let Some(last_run) = last_run {
            job.last_run = Evidence::available(last_run, provenance.clone());
        }
        let running = record.state.eq_ignore_ascii_case("running");
        if running || had_last_run {
            job.last_outcome = Evidence::available(
                LastOutcome {
                    state: if running {
                        OutcomeState::Running
                    } else if record.last_task_result == Some(0) {
                        OutcomeState::Success
                    } else if record.last_task_result.is_some() {
                        OutcomeState::Failed
                    } else {
                        OutcomeState::Unknown
                    },
                    native_code: record.last_task_result,
                    explanation: format!(
                        "Task Scheduler state={} result={}",
                        record.state,
                        record
                            .last_task_result
                            .map_or_else(|| "not reported".into(), |value| value.to_string())
                    ),
                },
                provenance,
            );
        }
    }
    Ok(())
}

fn normalise_runtime(value: &str) -> Option<RunTime> {
    let parsed = DateTime::parse_from_rfc3339(value).ok()?;
    Some(RunTime {
        iso8601: parsed.to_rfc3339(),
        timezone_basis: "UTC from Task Scheduler runtime query".into(),
    })
}

fn decode_xml(input: &[u8]) -> Result<String, String> {
    validate_bounded_bytes(input).map_err(|error| error.to_string())?;
    if input.starts_with(&[0xff, 0xfe]) {
        return decode_utf16(&input[2..], u16::from_le_bytes);
    }
    if input.starts_with(&[0xfe, 0xff]) {
        return decode_utf16(&input[2..], u16::from_be_bytes);
    }
    let looks_utf16le = input.len() >= 4
        && input.len().is_multiple_of(2)
        && input
            .iter()
            .skip(1)
            .step_by(2)
            .take(32)
            .any(|byte| *byte == 0);
    if looks_utf16le {
        return decode_utf16(input, u16::from_le_bytes);
    }
    std::str::from_utf8(input)
        .map(str::to_owned)
        .map_err(|error| error.to_string())
}

fn decode_utf16(bytes: &[u8], decode: impl Fn([u8; 2]) -> u16) -> Result<String, String> {
    if !bytes.len().is_multiple_of(2) {
        return Err("UTF-16 input had an odd byte count".into());
    }
    char::decode_utf16(bytes.as_chunks::<2>().0.iter().map(|pair| decode(*pair)))
        .collect::<Result<String, _>>()
        .map_err(|error| error.to_string())
}

fn is_system_principal(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_uppercase().as_str(),
        "SYSTEM"
            | "LOCAL SYSTEM"
            | "LOCALSYSTEM"
            | "NT AUTHORITY\\SYSTEM"
            | "S-1-5-18"
            | "LOCAL SERVICE"
            | "NT AUTHORITY\\LOCAL SERVICE"
            | "NETWORK SERVICE"
            | "NT AUTHORITY\\NETWORK SERVICE"
    )
}

fn task_trigger(node: Node<'_, '_>) -> Trigger {
    let kind = match node.tag_name().name() {
        "CalendarTrigger" | "TimeTrigger" => "calendar",
        "BootTrigger" => "boot",
        "LogonTrigger" => "logon",
        _ => "event",
    };
    let start = text_of(node, "StartBoundary").unwrap_or_else(|| node.tag_name().name().into());
    Trigger {
        kind: kind.into(),
        expression: start,
        explanation: format!("Windows {kind} trigger"),
    }
}

fn principal_text(task: Node<'_, '_>, name: &str) -> Option<String> {
    let principal = task
        .descendants()
        .find(|node| node.is_element() && node.tag_name().name() == "Principal")?;
    text_of(principal, name)
}

fn direct_child_text(node: Node<'_, '_>, name: &str) -> Option<String> {
    node.children()
        .find(|child| child.is_element() && child.tag_name().name() == name)
        .and_then(|child| child.text())
        .map(|value| value.trim().to_owned())
}

fn text_of(node: Node<'_, '_>, name: &str) -> Option<String> {
    node.descendants()
        .find(|child| child.is_element() && child.tag_name().name() == name)
        .and_then(|child| child.text())
        .map(|value| value.trim().to_owned())
}

fn provenance(source: &str) -> Provenance {
    Provenance {
        adapter: SchedulerKind::WindowsTaskScheduler,
        source_reference: source.into(),
        detail: None,
    }
}
