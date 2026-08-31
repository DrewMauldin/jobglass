use std::collections::HashMap;
use std::io::Cursor;

use plist::{Dictionary, Value};

use crate::adapters::warning;
use crate::input::{valid_environment_key, validate_bounded_bytes};
use crate::model::{
    EnabledState, Evidence, JobScope, LastOutcome, OutcomeState, ParseWarning, Provenance,
    ScheduleKind, ScheduleSpec, ScheduledJob, SchedulerKind, TimezoneBasis, Trigger,
};

pub fn parse_plist(
    input: &[u8],
    source: &str,
    scope: JobScope,
) -> Result<ScheduledJob, ParseWarning> {
    validate_bounded_bytes(input)
        .map_err(|error| warning("launchd.input", error.to_string(), source))?;
    let value = Value::from_reader(Cursor::new(input))
        .map_err(|error| warning("launchd.xml", error.to_string(), source))?;
    let dictionary = value.as_dictionary().ok_or_else(|| {
        warning(
            "launchd.root",
            "property list root is not a dictionary",
            source,
        )
    })?;
    let label = string(dictionary, "Label")
        .ok_or_else(|| warning("launchd.label", "Label is missing or invalid", source))?;
    let provenance = provenance(source);
    let mut job = ScheduledJob::new(SchedulerKind::Launchd, label, label, scope, source);

    let enabled = match dictionary.get("Disabled") {
        None => EnabledState::Enabled,
        Some(value) if value.as_boolean() == Some(true) => EnabledState::Disabled,
        Some(value) if value.as_boolean() == Some(false) => EnabledState::Enabled,
        Some(_) => {
            job.parse_warnings.push(warning(
                "launchd.disabled",
                "Disabled has an invalid value",
                source,
            ));
            EnabledState::Unknown
        }
    };
    job.enabled = Evidence::available(enabled, provenance.clone());

    let program = string(dictionary, "Program").map(str::to_owned);
    let program_arguments = strings(dictionary.get("ProgramArguments"));
    let executable = program
        .clone()
        .or_else(|| program_arguments.first().cloned());
    if let Some(executable) = executable {
        job.executable = Evidence::available(executable.clone(), provenance.clone());
        let arguments = if program.is_none() || program_arguments.first() == Some(&executable) {
            program_arguments.into_iter().skip(1).collect()
        } else {
            program_arguments
        };
        job.arguments = Evidence::available(arguments, provenance.clone());
    }
    if let Some(directory) = string(dictionary, "WorkingDirectory") {
        job.working_directory = Evidence::available(directory.into(), provenance.clone());
    }

    let mut environment_keys = Vec::new();
    if let Some(environment) = dictionary
        .get("EnvironmentVariables")
        .and_then(Value::as_dictionary)
    {
        for key in environment.keys() {
            if valid_environment_key(key) {
                environment_keys.push(key.clone());
            } else {
                job.parse_warnings.push(warning(
                    "launchd.environmentKey",
                    "an invalid environment key was omitted",
                    source,
                ));
            }
        }
    }
    environment_keys.sort();
    job.environment_keys = Evidence::available(environment_keys, provenance.clone());

    let triggers = launchd_triggers(dictionary);
    if !triggers.is_empty() {
        let kind = if triggers.len() > 1 {
            ScheduleKind::Composite
        } else {
            match triggers[0].kind.as_str() {
                "interval" => ScheduleKind::Interval,
                "calendar" => ScheduleKind::Calendar,
                _ => ScheduleKind::Event,
            }
        };
        let expression = triggers
            .iter()
            .map(|trigger| trigger.expression.as_str())
            .collect::<Vec<_>>()
            .join("; ");
        let explanation = triggers
            .iter()
            .map(|trigger| trigger.explanation.as_str())
            .collect::<Vec<_>>()
            .join("; ");
        job.schedule = Evidence::available(
            ScheduleSpec {
                kind,
                native_expression: expression,
            },
            provenance.clone(),
        );
        job.schedule_explanation = Evidence::available(explanation, provenance.clone());
        job.triggers = Evidence::available(triggers, provenance.clone());
        job.timezone_basis = Evidence::available(
            TimezoneBasis {
                name: "local".into(),
                source: "launchd calendar interpretation".into(),
            },
            provenance,
        );
    }
    Ok(job)
}

pub fn enrich_launchctl(job: &mut ScheduledJob, output: &str) {
    let provenance = match &job.native_identifier {
        Evidence::Available { provenance, .. } => Provenance {
            detail: Some("launchctl print".into()),
            ..provenance.clone()
        },
        Evidence::Unavailable { .. } => return,
    };
    let state = output.lines().find_map(|line| {
        line.trim()
            .strip_prefix("state = ")
            .map(str::trim)
            .map(str::to_owned)
    });
    let exit_code = output.lines().find_map(|line| {
        line.trim()
            .strip_prefix("last exit code = ")
            .and_then(|value| value.trim().parse::<i64>().ok())
    });
    if let Some(exit_code) = exit_code {
        job.last_outcome = Evidence::available(
            LastOutcome {
                state: if exit_code == 0 {
                    OutcomeState::Success
                } else {
                    OutcomeState::Failed
                },
                native_code: Some(exit_code),
                explanation: format!("launchctl reported exit code {exit_code}"),
            },
            provenance,
        );
    } else if state.as_deref() == Some("running") {
        job.last_outcome = Evidence::available(
            LastOutcome {
                state: OutcomeState::Running,
                native_code: None,
                explanation: "launchctl reports that the job is running".into(),
            },
            provenance,
        );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LaunchctlDomainState {
    pid: Option<u32>,
    native_code: Option<i64>,
}

pub type LaunchctlDomain = HashMap<String, LaunchctlDomainState>;

pub fn parse_launchctl_domain(output: &str) -> LaunchctlDomain {
    output
        .lines()
        .filter_map(|line| {
            let fields = line.split_whitespace().collect::<Vec<_>>();
            if fields.len() < 3 {
                return None;
            }
            let pid = fields[0].parse::<u32>().ok();
            let native_code = fields[1].parse::<i64>().ok();
            if pid.is_none() && native_code.is_none() {
                return None;
            }
            Some((
                fields.last()?.to_string(),
                LaunchctlDomainState { pid, native_code },
            ))
        })
        .collect()
}

pub fn enrich_launchctl_domain(job: &mut ScheduledJob, domain: &LaunchctlDomain) {
    let (label, provenance) = match &job.native_identifier {
        Evidence::Available { value, provenance } => (
            value,
            Provenance {
                detail: Some("launchctl domain service table".into()),
                ..provenance.clone()
            },
        ),
        Evidence::Unavailable { .. } => return,
    };
    let Some(domain_state) = domain.get(label) else {
        return;
    };
    let (state, explanation) = if domain_state.pid.is_some_and(|value| value > 0) {
        (
            OutcomeState::Running,
            "launchctl domain table reports that the job is running".into(),
        )
    } else if let Some(code) = domain_state.native_code {
        (
            if code == 0 {
                OutcomeState::Success
            } else {
                OutcomeState::Failed
            },
            format!("launchctl domain table reports exit code {code}"),
        )
    } else {
        return;
    };
    job.last_outcome = Evidence::available(
        LastOutcome {
            state,
            native_code: domain_state.native_code,
            explanation,
        },
        provenance,
    );
}

fn launchd_triggers(dictionary: &Dictionary) -> Vec<Trigger> {
    let mut triggers = Vec::new();
    if let Some(seconds) = dictionary
        .get("StartInterval")
        .and_then(Value::as_unsigned_integer)
        .or_else(|| {
            dictionary
                .get("StartInterval")
                .and_then(Value::as_signed_integer)
                .and_then(|value| u64::try_from(value).ok())
        })
    {
        triggers.push(Trigger {
            kind: "interval".into(),
            expression: format!("StartInterval={seconds}"),
            explanation: format!("Every {seconds} seconds while launchd is available"),
        });
    }
    if let Some(calendar) = dictionary.get("StartCalendarInterval") {
        triggers.push(Trigger {
            kind: "calendar".into(),
            expression: format!("StartCalendarInterval={calendar:?}"),
            explanation: "At the configured local calendar interval".into(),
        });
    }
    for (key, explanation) in [
        ("WatchPaths", "When a watched path changes"),
        ("QueueDirectories", "When a queued directory is not empty"),
    ] {
        if dictionary.contains_key(key) {
            triggers.push(Trigger {
                kind: "path".into(),
                expression: key.into(),
                explanation: explanation.into(),
            });
        }
    }
    if dictionary.contains_key("KeepAlive") {
        triggers.push(Trigger {
            kind: "keepAlive".into(),
            expression: "KeepAlive".into(),
            explanation: "launchd may keep or restart the job according to KeepAlive".into(),
        });
    }
    triggers
}

fn string<'a>(dictionary: &'a Dictionary, key: &str) -> Option<&'a str> {
    dictionary.get(key).and_then(Value::as_string)
}

fn strings(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_string)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn provenance(source: &str) -> Provenance {
    Provenance {
        adapter: SchedulerKind::Launchd,
        source_reference: source.into(),
        detail: None,
    }
}
