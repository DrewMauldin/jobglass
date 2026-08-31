use chrono::Utc;
use jobglass_lib::diagnostics::diagnose;
use jobglass_lib::model::{
    Evidence, JobScope, ScheduleKind, ScheduleSpec, ScheduledJob, SchedulerKind,
};
use std::time::{Duration, Instant};

fn main() {
    let jobs = (0..5_000)
        .map(|index| {
            let source = format!("fixture:{index}");
            let mut job = ScheduledJob::new(
                SchedulerKind::Cron,
                format!("fixture-{index}"),
                format!("Fixture {index}"),
                JobScope::User,
                &source,
            );
            let provenance = match &job.schedule {
                Evidence::Available { provenance, .. }
                | Evidence::Unavailable { provenance, .. } => provenance.clone(),
            };
            job.schedule = Evidence::available(
                ScheduleSpec {
                    kind: ScheduleKind::Calendar,
                    native_expression: format!("{} * * * *", index % 60),
                },
                provenance.clone(),
            );
            job.executable = Evidence::available(format!("fixture-tool-{index}"), provenance);
            job
        })
        .collect::<Vec<_>>();

    let started = Instant::now();
    let findings = diagnose(&jobs, &[], &[], Utc::now(), |_| true);
    let elapsed = started.elapsed();
    println!(
        "normalised and diagnosed {} fixture jobs with {} findings in {elapsed:?}",
        jobs.len(),
        findings.len()
    );
    assert!(
        elapsed < Duration::from_millis(500),
        "5,000-job diagnostic budget exceeded: {elapsed:?}"
    );
}
