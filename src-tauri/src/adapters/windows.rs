use roxmltree::{Document, Node};

use crate::adapters::warning;
use crate::input::decode_bounded;
use crate::model::{
    EnabledState, Evidence, JobScope, ParseWarning, PrivilegeLevel, Provenance, RunTime,
    ScheduleKind, ScheduleSpec, ScheduledJob, SchedulerKind, TimezoneBasis, Trigger,
};

pub fn parse_task_xml(input: &[u8], source: &str) -> Result<ScheduledJob, ParseWarning> {
    let text = decode_bounded(input)
        .map_err(|error| warning("windows.input", error.to_string(), source))?;
    let document =
        Document::parse(text).map_err(|error| warning("windows.xml", error.to_string(), source))?;
    let identifier = text_of(&document, "URI")
        .ok_or_else(|| warning("windows.uri", "task URI is missing", source))?;
    let provenance = provenance(source);
    let mut job = ScheduledJob::new(
        SchedulerKind::WindowsTaskScheduler,
        identifier,
        identifier.rsplit('\\').next().unwrap_or(identifier),
        JobScope::User,
        source,
    );

    if let Some(owner) = text_of(&document, "UserId") {
        job.owner = Evidence::available(owner.into(), provenance.clone());
    }
    if let Some(run_level) = text_of(&document, "RunLevel") {
        job.privilege_level = Evidence::available(
            if run_level.eq_ignore_ascii_case("HighestAvailable") {
                PrivilegeLevel::Elevated
            } else {
                PrivilegeLevel::StandardUser
            },
            provenance.clone(),
        );
    }
    let enabled = descendants(&document, "Enabled")
        .filter_map(|node| node.text())
        .last()
        .map(|value| !value.eq_ignore_ascii_case("false"))
        .unwrap_or(true);
    job.enabled = Evidence::available(
        if enabled {
            EnabledState::Enabled
        } else {
            EnabledState::Disabled
        },
        provenance.clone(),
    );

    let command = text_of(&document, "Command")
        .ok_or_else(|| warning("windows.command", "Exec Command is missing", source))?;
    job.executable = Evidence::available(command.into(), provenance.clone());
    job.arguments = Evidence::available(
        text_of(&document, "Arguments")
            .map(|arguments| arguments.split_whitespace().map(str::to_owned).collect())
            .unwrap_or_default(),
        provenance.clone(),
    );
    if let Some(directory) = text_of(&document, "WorkingDirectory") {
        job.working_directory = Evidence::available(directory.into(), provenance.clone());
    }

    let triggers = document
        .descendants()
        .filter(|node| {
            node.is_element()
                && matches!(
                    node.tag_name().name(),
                    "CalendarTrigger" | "TimeTrigger" | "BootTrigger" | "LogonTrigger"
                )
        })
        .map(|node| task_trigger(node))
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

pub fn enrich_runtime(job: &mut ScheduledJob, next_run: Option<&str>, last_run: Option<&str>) {
    let provenance = provenance("Task Scheduler runtime query");
    if let Some(next_run) = next_run {
        job.next_run = Evidence::available(
            RunTime {
                iso8601: next_run.into(),
                timezone_basis: "Task Scheduler local output".into(),
            },
            provenance.clone(),
        );
    }
    if let Some(last_run) = last_run {
        job.last_run = Evidence::available(
            RunTime {
                iso8601: last_run.into(),
                timezone_basis: "Task Scheduler local output".into(),
            },
            provenance,
        );
    }
}

fn task_trigger(node: Node<'_, '_>) -> Trigger {
    let kind = match node.tag_name().name() {
        "CalendarTrigger" | "TimeTrigger" => "calendar",
        "BootTrigger" => "boot",
        "LogonTrigger" => "logon",
        _ => "event",
    };
    let start = node
        .descendants()
        .find(|child| child.has_tag_name("StartBoundary"))
        .and_then(|child| child.text())
        .unwrap_or(node.tag_name().name());
    Trigger {
        kind: kind.into(),
        expression: start.into(),
        explanation: format!("Windows {kind} trigger"),
    }
}

fn text_of<'a>(document: &'a Document<'a>, name: &str) -> Option<&'a str> {
    descendants(document, name)
        .find_map(|node| node.text())
        .map(str::trim)
}

fn descendants<'a>(
    document: &'a Document<'a>,
    name: &str,
) -> impl Iterator<Item = Node<'a, 'a>> + 'a {
    let name = name.to_owned();
    document
        .descendants()
        .filter(move |node| node.is_element() && node.tag_name().name() == name)
}

fn provenance(source: &str) -> Provenance {
    Provenance {
        adapter: SchedulerKind::WindowsTaskScheduler,
        source_reference: source.into(),
        detail: None,
    }
}
