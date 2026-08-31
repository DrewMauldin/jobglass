use std::collections::BTreeMap;

use crate::adapters::warning;
use crate::model::{
    EnabledState, Evidence, JobScope, LastOutcome, OutcomeState, ParseWarning, Provenance, RunTime,
    ScheduleKind, ScheduleSpec, ScheduledJob, SchedulerKind, TimezoneBasis, Trigger,
};
use chrono::NaiveDateTime;

pub fn parse_timer_show(input: &str, scope: JobScope) -> Result<ScheduledJob, ParseWarning> {
    let properties = input
        .lines()
        .filter_map(|line| line.split_once('='))
        .map(|(key, value)| (key.trim(), value.trim()))
        .collect::<BTreeMap<_, _>>();
    let identifier = properties
        .get("Id")
        .filter(|value| valid_timer_identifier(value))
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
            name: "UTC".into(),
            source: "systemctl with TZ=UTC".into(),
        },
        provenance.clone(),
    );
    if let Some(next) = properties
        .get("NextElapseUSecRealtime")
        .filter(|value| !value.is_empty())
        && let Some(run) = normalise_timestamp(next)
    {
        job.next_run = Evidence::available(run, provenance.clone());
    }
    if let Some(last) = properties
        .get("LastTriggerUSec")
        .filter(|value| !value.is_empty())
        && let Some(run) = normalise_timestamp(last)
    {
        job.last_run = Evidence::available(run, provenance.clone());
    }
    if let Some(result) = properties.get("Result").filter(|value| !value.is_empty()) {
        let state = match *result {
            "success" => OutcomeState::Success,
            "running" => OutcomeState::Running,
            "exit-code" | "signal" | "timeout" | "watchdog" | "resources" | "failed" => {
                OutcomeState::Failed
            }
            _ => OutcomeState::Unknown,
        };
        job.last_outcome = Evidence::available(
            LastOutcome {
                state,
                native_code: properties
                    .get("ExecMainStatus")
                    .and_then(|value| value.parse::<i64>().ok()),
                explanation: format!("systemd Result={result}"),
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

pub fn valid_timer_identifier(identifier: &str) -> bool {
    !identifier.is_empty()
        && identifier.len() <= 256
        && identifier.ends_with(".timer")
        && !identifier.starts_with('-')
        && identifier.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, ':' | '_' | '.' | '@' | '-')
        })
}

fn normalise_timestamp(value: &str) -> Option<RunTime> {
    if value.is_empty() || value.eq_ignore_ascii_case("n/a") {
        return None;
    }
    if let Ok(parsed) = chrono::DateTime::parse_from_rfc3339(value) {
        return Some(RunTime {
            iso8601: parsed.to_rfc3339(),
            timezone_basis: "native RFC3339 offset".into(),
        });
    }
    let mut fields = value.split_whitespace();
    let _weekday = fields.next()?;
    let date = fields.next()?;
    let time = fields.next()?;
    let timezone = fields.next()?;
    if !matches!(timezone, "UTC" | "UCT" | "GMT" | "Z") || fields.next().is_some() {
        return None;
    }
    let parsed =
        NaiveDateTime::parse_from_str(&format!("{date} {time}"), "%Y-%m-%d %H:%M:%S").ok()?;
    Some(RunTime {
        iso8601: parsed.and_utc().to_rfc3339(),
        timezone_basis: "UTC from systemctl TZ=UTC".into(),
    })
}

fn provenance(source: &str) -> Provenance {
    Provenance {
        adapter: SchedulerKind::Systemd,
        source_reference: source.into(),
        detail: None,
    }
}
