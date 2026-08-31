use crate::adapters::{AdapterResult, warning};
use crate::input::{MAX_JOBS, valid_environment_key};
use crate::model::{
    EnabledState, Evidence, JobScope, Provenance, ScheduleKind, ScheduleSpec, ScheduledJob,
    SchedulerKind, TimezoneBasis, Trigger,
};

pub fn parse_crontab(
    input: &str,
    source: &str,
    scope: JobScope,
    default_owner: Option<&str>,
    has_owner_column: bool,
) -> AdapterResult {
    let mut result = AdapterResult::default();
    let mut environment_keys = Vec::new();
    let mut definitions_seen = 0_usize;

    for (index, raw_line) in input.lines().enumerate() {
        let line_number = index + 1;
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        definitions_seen += 1;
        if definitions_seen > MAX_JOBS {
            result.warnings.push(warning(
                "cron.definitionLimit",
                format!("definition limit of {MAX_JOBS} reached"),
                source,
            ));
            break;
        }
        if let Some(key) = environment_assignment_key(line) {
            if !environment_keys.iter().any(|existing| existing == key) {
                environment_keys.push(key.to_owned());
                environment_keys.sort();
            }
            continue;
        }
        if result.jobs.len() >= MAX_JOBS {
            result.warnings.push(warning(
                "cron.jobLimit",
                format!("job limit of {MAX_JOBS} reached"),
                source,
            ));
            break;
        }

        match parse_job_line(line, has_owner_column) {
            Some((schedule, owner, command)) => {
                let source_reference = format!("{source}:{line_number}");
                let identifier = format!("{source}:{line_number}:{schedule}");
                let provenance = provenance(&source_reference);
                let Some((inline_environment_keys, executable, arguments)) =
                    split_command(&command)
                else {
                    result.warnings.push(warning(
                        "cron.malformedCommand",
                        format!("line {line_number} has no valid command"),
                        &source_reference,
                    ));
                    continue;
                };
                let display_name = executable.as_deref().unwrap_or("cron entry");
                let mut job = ScheduledJob::new(
                    SchedulerKind::Cron,
                    identifier,
                    display_name,
                    scope,
                    &source_reference,
                );
                job.enabled = Evidence::available(EnabledState::Enabled, provenance.clone());
                if let Some(owner) = owner.or_else(|| default_owner.map(str::to_owned)) {
                    job.owner = Evidence::available(owner, provenance.clone());
                }
                job.schedule = Evidence::available(
                    ScheduleSpec {
                        kind: ScheduleKind::Calendar,
                        native_expression: schedule.clone(),
                    },
                    provenance.clone(),
                );
                job.schedule_explanation =
                    Evidence::available(explain_schedule(&schedule), provenance.clone());
                job.timezone_basis = Evidence::available(
                    TimezoneBasis {
                        name: "local".into(),
                        source: if environment_keys.iter().any(|key| key == "CRON_TZ") {
                            "CRON_TZ is configured; its value is privacy-redacted".into()
                        } else {
                            "cron daemon default".into()
                        },
                    },
                    provenance.clone(),
                );
                if let Some(executable) = executable {
                    job.executable = Evidence::available(executable, provenance.clone());
                    job.arguments = Evidence::available(arguments, provenance.clone());
                }
                let mut job_environment_keys = environment_keys.clone();
                job_environment_keys.extend(inline_environment_keys);
                job_environment_keys.sort();
                job_environment_keys.dedup();
                job.environment_keys =
                    Evidence::available(job_environment_keys, provenance.clone());
                job.triggers = Evidence::available(
                    vec![Trigger {
                        kind: "calendar".into(),
                        expression: schedule,
                        explanation: "Cron evaluates this calendar expression".into(),
                    }],
                    provenance,
                );
                result.jobs.push(job);
            }
            None => result.warnings.push(warning(
                "cron.malformedLine",
                format!("line {line_number} is not a recognised cron entry"),
                &format!("{source}:{line_number}"),
            )),
        }
    }
    result
}

pub fn periodic_job(path: &str, period: &str, scope: JobScope) -> ScheduledJob {
    let provenance = provenance(path);
    let schedule = format!("@{}", period.trim_start_matches('@'));
    let mut job = ScheduledJob::new(SchedulerKind::Cron, path, path, scope, path);
    job.enabled = Evidence::available(EnabledState::Enabled, provenance.clone());
    job.schedule = Evidence::available(
        ScheduleSpec {
            kind: ScheduleKind::Calendar,
            native_expression: schedule.clone(),
        },
        provenance.clone(),
    );
    job.schedule_explanation = Evidence::available(explain_schedule(&schedule), provenance.clone());
    job.executable = Evidence::available(path.into(), provenance.clone());
    job.arguments = Evidence::available(Vec::new(), provenance.clone());
    job.triggers = Evidence::available(
        vec![Trigger {
            kind: "periodicDirectory".into(),
            expression: schedule,
            explanation: "Executed by the system periodic cron directory".into(),
        }],
        provenance,
    );
    job
}

fn parse_job_line(line: &str, has_owner_column: bool) -> Option<(String, Option<String>, String)> {
    let fields = line.split_whitespace().collect::<Vec<_>>();
    let schedule_fields = if line.starts_with('@') { 1 } else { 5 };
    let owner_fields = usize::from(has_owner_column);
    if fields.len() <= schedule_fields + owner_fields {
        return None;
    }
    if schedule_fields == 1 && !valid_nickname(fields[0]) {
        return None;
    }
    if schedule_fields == 5 && !valid_calendar_fields(&fields[..5]) {
        return None;
    }
    let schedule = fields[..schedule_fields].join(" ");
    let owner = has_owner_column.then(|| fields[schedule_fields].to_owned());
    let command = fields[schedule_fields + owner_fields..].join(" ");
    Some((schedule, owner, command))
}

fn valid_nickname(value: &str) -> bool {
    matches!(
        value.to_ascii_lowercase().as_str(),
        "@reboot"
            | "@yearly"
            | "@annually"
            | "@monthly"
            | "@weekly"
            | "@daily"
            | "@midnight"
            | "@hourly"
    )
}

fn valid_calendar_fields(fields: &[&str]) -> bool {
    let specifications = [
        (0, 59, &[][..]),
        (0, 23, &[][..]),
        (1, 31, &[][..]),
        (
            1,
            12,
            &[
                "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
            ][..],
        ),
        (0, 7, &["sun", "mon", "tue", "wed", "thu", "fri", "sat"][..]),
    ];
    fields
        .iter()
        .zip(specifications)
        .all(|(field, (minimum, maximum, names))| {
            valid_calendar_field(field, minimum, maximum, names)
        })
}

fn valid_calendar_field(field: &str, minimum: u8, maximum: u8, names: &[&str]) -> bool {
    !field.is_empty()
        && field.split(',').all(|item| {
            let (base, step) = item
                .split_once('/')
                .map_or((item, None), |(base, step)| (base, Some(step)));
            if step.is_some_and(|value| value.parse::<u16>().ok().is_none_or(|step| step == 0)) {
                return false;
            }
            if base == "*" {
                return true;
            }
            if let Some((start, end)) = base.split_once('-') {
                let Some(start) = calendar_value(start, minimum, maximum, names) else {
                    return false;
                };
                let Some(end) = calendar_value(end, minimum, maximum, names) else {
                    return false;
                };
                return start <= end;
            }
            calendar_value(base, minimum, maximum, names).is_some()
        })
}

fn calendar_value(value: &str, minimum: u8, maximum: u8, names: &[&str]) -> Option<u8> {
    if let Ok(number) = value.parse::<u8>() {
        return (minimum..=maximum).contains(&number).then_some(number);
    }
    names
        .iter()
        .position(|name| name.eq_ignore_ascii_case(value))
        .map(|index| minimum + index as u8)
}

fn environment_assignment_key(line: &str) -> Option<&str> {
    let (key, _) = line.split_once('=')?;
    valid_environment_key(key).then_some(key)
}

fn split_command(command: &str) -> Option<(Vec<String>, Option<String>, Vec<String>)> {
    let mut parts = shlex::split(command)?.into_iter().peekable();
    let mut environment_keys = Vec::new();
    while let Some(key) = parts
        .peek()
        .and_then(|part| environment_assignment_key(part))
    {
        environment_keys.push(key.to_owned());
        parts.next();
    }
    let executable = parts.next();
    executable.as_ref()?;
    Some((environment_keys, executable, parts.collect()))
}

fn explain_schedule(schedule: &str) -> String {
    match schedule {
        "@daily" | "@midnight" => "Once each day according to cron".into(),
        "@hourly" => "Once each hour according to cron".into(),
        "@weekly" => "Once each week according to cron".into(),
        "@monthly" => "Once each month according to cron".into(),
        "@reboot" => "When the cron daemon starts".into(),
        _ => format!("Cron calendar expression: {schedule}"),
    }
}

fn provenance(source: &str) -> Provenance {
    Provenance {
        adapter: SchedulerKind::Cron,
        source_reference: source.into(),
        detail: None,
    }
}
