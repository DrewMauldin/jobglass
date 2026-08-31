use std::collections::BTreeMap;

use crate::adapters::warning;
use crate::model::{
    EnabledState, Evidence, JobScope, ParseWarning, Provenance, RunTime, ScheduleKind,
    ScheduleSpec, ScheduledJob, SchedulerKind, TimezoneBasis, Trigger,
};

pub fn parse_timer_show(input: &str, scope: JobScope) -> Result<ScheduledJob, ParseWarning> {
    let properties = input
        .lines()
        .filter_map(|line| line.split_once('='))
        .map(|(key, value)| (key.trim(), value.trim()))
        .collect::<BTreeMap<_, _>>();
    let identifier = properties
        .get("Id")
        .filter(|value| value.ends_with(".timer"))
        .ok_or_else(|| {
            warning(
                "systemd.id",
                "timer Id is missing or invalid",
                "systemctl show",
            )
        })?;
    let source = format!("systemctl show {identifier}");
    let provenance = provenance(&source);
    let mut job = ScheduledJob::new(
        SchedulerKind::Systemd,
        *identifier,
        *identifier,
        scope,
        &source,
    );

    let unit_file_state = properties.get("UnitFileState").copied().unwrap_or("");
    job.enabled = Evidence::available(
        match unit_file_state {
            "enabled" | "enabled-runtime" | "static" => EnabledState::Enabled,
            "disabled" | "masked" => EnabledState::Disabled,
            _ => EnabledState::Unknown,
        },
        provenance.clone(),
    );
    if let Some(service) = properties.get("Unit").filter(|value| !value.is_empty()) {
        job.target_service = Evidence::available((*service).into(), provenance.clone());
    }

    let calendar = properties
        .get("TimersCalendar")
        .filter(|value| !value.is_empty())
        .copied();
    let monotonic = properties
        .get("TimersMonotonic")
        .filter(|value| !value.is_empty())
        .copied();
    let mut triggers = Vec::new();
    if let Some(expression) = calendar {
        triggers.push(Trigger {
            kind: "calendar".into(),
            expression: expression.into(),
            explanation: "systemd wall-clock calendar trigger".into(),
        });
    }
    if let Some(expression) = monotonic {
        triggers.push(Trigger {
            kind: "monotonic".into(),
            expression: expression.into(),
            explanation: "systemd monotonic interval trigger".into(),
        });
    }
    if !triggers.is_empty() {
        let kind = match (calendar.is_some(), monotonic.is_some()) {
            (true, true) => ScheduleKind::Composite,
            (true, false) => ScheduleKind::Calendar,
            (false, true) => ScheduleKind::Interval,
            (false, false) => unreachable!(),
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
            name: "systemd manager timezone".into(),
            source: "systemctl show".into(),
        },
        provenance.clone(),
    );
    if let Some(next) = properties
        .get("NextElapseUSecRealtime")
        .filter(|value| !value.is_empty())
    {
        job.next_run = Evidence::available(
            RunTime {
                iso8601: (*next).into(),
                timezone_basis: "native systemd text".into(),
            },
            provenance.clone(),
        );
    }
    if let Some(last) = properties
        .get("LastTriggerUSec")
        .filter(|value| !value.is_empty())
    {
        job.last_run = Evidence::available(
            RunTime {
                iso8601: (*last).into(),
                timezone_basis: "native systemd text".into(),
            },
            provenance.clone(),
        );
    }
    let mut dependencies = ["Wants", "Requires"]
        .into_iter()
        .filter_map(|key| properties.get(key))
        .flat_map(|value| value.split_whitespace())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    dependencies.sort();
    dependencies.dedup();
    job.dependencies = Evidence::available(dependencies, provenance);
    Ok(job)
}

fn provenance(source: &str) -> Provenance {
    Provenance {
        adapter: SchedulerKind::Systemd,
        source_reference: source.into(),
        detail: None,
    }
}
