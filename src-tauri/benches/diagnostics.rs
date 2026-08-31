use chrono::Utc;
use jobglass_lib::adapters::cron;
use jobglass_lib::diagnostics::diagnose;
use jobglass_lib::model::JobScope;
use std::time::{Duration, Instant};

fn main() {
    let fixture = (0..5_000)
        .map(|index| {
            format!(
                "{} * * * * FIXTURE_KEY=value /usr/bin/jobglass-fixture-{index} --quiet",
                index % 60
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let started = Instant::now();
    let normalised = cron::parse_crontab(
        &fixture,
        "5,000-job benchmark fixture",
        JobScope::User,
        Some("fixture-user"),
        false,
    );
    assert!(normalised.warnings.is_empty(), "{:?}", normalised.warnings);
    assert_eq!(normalised.jobs.len(), 5_000);

    let findings = diagnose(
        &normalised.jobs,
        &normalised.warnings,
        &[],
        Utc::now(),
        |_| true,
        |_| true,
    );
    let elapsed = started.elapsed();
    println!(
        "parsed, normalised, and diagnosed {} fixture jobs with {} findings in {elapsed:?}",
        normalised.jobs.len(),
        findings.len()
    );
    assert!(
        elapsed < Duration::from_millis(500),
        "5,000-job diagnostic budget exceeded: {elapsed:?}"
    );
}
