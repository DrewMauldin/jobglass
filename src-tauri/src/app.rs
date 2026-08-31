use std::path::Path;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::diagnostics::{Finding, Visibility, diagnose};
use crate::export::{ExportPolicy, export_csv, export_html, export_json};
use crate::model::{ParseWarning, ScheduledJob};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanBundle {
    pub schema_version: &'static str,
    pub scan_id: String,
    pub generated_at: String,
    pub platform: String,
    pub jobs: Vec<ScheduledJob>,
    pub findings: Vec<Finding>,
    pub warnings: Vec<ParseWarning>,
    pub visibility: Vec<Visibility>,
    pub sample_data: bool,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ExportFormat {
    Json,
    Csv,
    Html,
}

#[tauri::command]
pub async fn scan_jobs() -> Result<ScanBundle, String> {
    tauri::async_runtime::spawn_blocking(scan_current_platform)
        .await
        .map_err(|error| format!("scheduler scan worker failed: {error}"))
}

#[tauri::command]
pub fn render_export(
    jobs: Vec<ScheduledJob>,
    findings: Vec<Finding>,
    format: ExportFormat,
    reviewed: bool,
    include_arguments: bool,
) -> Result<String, String> {
    let policy = ExportPolicy {
        reviewed,
        include_arguments,
    };
    match format {
        ExportFormat::Json => export_json(&jobs, &findings, policy),
        ExportFormat::Csv => export_csv(&jobs, &findings, policy),
        ExportFormat::Html => export_html(&jobs, &findings, policy),
    }
    .map_err(|error| error.to_string())
}

fn bundle(
    platform: &str,
    jobs: Vec<ScheduledJob>,
    warnings: Vec<ParseWarning>,
    visibility: Vec<Visibility>,
) -> ScanBundle {
    let generated_at = Utc::now();
    let findings = diagnose(&jobs, &warnings, &visibility, generated_at, |path| {
        Path::new(path).exists()
    });
    eprintln!(
        "{{\"event\":\"scan.complete\",\"platform\":\"{platform}\",\"jobs\":{},\"warnings\":{},\"findings\":{}}}",
        jobs.len(),
        warnings.len(),
        findings.len()
    );
    ScanBundle {
        schema_version: "1.0",
        scan_id: Uuid::new_v4().to_string(),
        generated_at: generated_at.to_rfc3339(),
        platform: platform.into(),
        jobs,
        findings,
        warnings,
        visibility,
        sample_data: false,
    }
}

#[cfg(target_os = "macos")]
fn scan_current_platform() -> ScanBundle {
    macos::scan()
}

#[cfg(target_os = "linux")]
fn scan_current_platform() -> ScanBundle {
    linux::scan()
}

#[cfg(target_os = "windows")]
fn scan_current_platform() -> ScanBundle {
    windows::scan()
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn scan_current_platform() -> ScanBundle {
    bundle("Unsupported platform", Vec::new(), Vec::new(), Vec::new())
}

#[cfg(target_os = "macos")]
mod macos {
    use std::path::{Path, PathBuf};
    use std::time::Duration;

    use crate::adapters::launchd;
    use crate::adapters::warning;
    use crate::app::{ScanBundle, bundle};
    use crate::diagnostics::{Visibility, VisibilityStatus};
    use crate::input::{MAX_JOBS, read_bounded_file_bytes};
    use crate::model::{JobScope, ParseWarning, ScheduledJob, SchedulerKind};
    use crate::process::{NativeTool, run_native_tool};

    pub(super) fn scan() -> ScanBundle {
        let mut jobs = Vec::new();
        let mut warnings = Vec::new();
        let mut visibility = Vec::new();
        let uid = unsafe { libc::getuid() };
        let gui_target = format!("gui/{uid}");
        let gui_output = domain_output(&gui_target);
        let system_output = domain_output("system");

        match std::env::var_os("HOME") {
            Some(home) => {
                let root = PathBuf::from(home).join("Library/LaunchAgents");
                let limited = scan_directory(
                    &root,
                    JobScope::User,
                    gui_output.as_deref(),
                    &mut jobs,
                    &mut warnings,
                );
                visibility.push(Visibility {
                    scheduler: SchedulerKind::Launchd,
                    scope: JobScope::User,
                    status: if limited {
                        VisibilityStatus::PermissionLimited
                    } else {
                        VisibilityStatus::Complete
                    },
                    explanation: if limited {
                        "One or more user launch agent definitions were not readable.".into()
                    } else {
                        "Readable user launch agent definitions were scanned.".into()
                    },
                });
            }
            None => visibility.push(Visibility {
                scheduler: SchedulerKind::Launchd,
                scope: JobScope::User,
                status: VisibilityStatus::Unavailable,
                explanation: "The user LaunchAgents directory could not be located.".into(),
            }),
        }

        let mut system_limited = false;
        for (root, domain) in [
            ("/Library/LaunchAgents", gui_output.as_deref()),
            ("/Library/LaunchDaemons", system_output.as_deref()),
            ("/System/Library/LaunchAgents", gui_output.as_deref()),
            ("/System/Library/LaunchDaemons", system_output.as_deref()),
        ] {
            system_limited |= scan_directory(
                Path::new(root),
                JobScope::System,
                domain,
                &mut jobs,
                &mut warnings,
            );
        }
        visibility.push(Visibility {
            scheduler: SchedulerKind::Launchd,
            scope: JobScope::System,
            status: if system_limited {
                VisibilityStatus::PermissionLimited
            } else {
                VisibilityStatus::Complete
            },
            explanation: if system_limited {
                "One or more system launchd definitions were not readable without elevation.".into()
            } else {
                "Readable system launchd definitions were scanned without elevation.".into()
            },
        });
        jobs.sort_by(|left, right| left.id.cmp(&right.id));
        bundle("macOS launchd", jobs, warnings, visibility)
    }

    fn scan_directory(
        root: &Path,
        scope: JobScope,
        domain_output: Option<&str>,
        jobs: &mut Vec<ScheduledJob>,
        warnings: &mut Vec<ParseWarning>,
    ) -> bool {
        if !root.exists() {
            return false;
        }
        let entries = match std::fs::read_dir(root) {
            Ok(entries) => entries,
            Err(error) => {
                warnings.push(warning(
                    "launchd.directory",
                    error.to_string(),
                    &root.display().to_string(),
                ));
                return true;
            }
        };
        let mut limited = false;
        for entry in entries {
            if jobs.len() >= MAX_JOBS {
                warnings.push(warning(
                    "launchd.jobLimit",
                    format!("job limit of {MAX_JOBS} reached"),
                    &root.display().to_string(),
                ));
                break;
            }
            let path = match entry {
                Ok(entry) => entry.path(),
                Err(error) => {
                    warnings.push(warning(
                        "launchd.entry",
                        error.to_string(),
                        &root.display().to_string(),
                    ));
                    limited = true;
                    continue;
                }
            };
            if path.extension().and_then(|value| value.to_str()) != Some("plist") {
                continue;
            }
            let source = path.display().to_string();
            let contents = match read_bounded_file_bytes(&path, &[root]) {
                Ok(contents) => contents,
                Err(error) => {
                    warnings.push(warning("launchd.read", error.to_string(), &source));
                    limited = true;
                    continue;
                }
            };
            match launchd::parse_plist(&contents, &source, scope) {
                Ok(mut job) => {
                    if let Some(domain_output) = domain_output {
                        launchd::enrich_launchctl_domain(&mut job, domain_output);
                    }
                    jobs.push(job);
                }
                Err(parse_warning) => warnings.push(parse_warning),
            }
        }
        limited
    }

    fn domain_output(target: &str) -> Option<String> {
        run_native_tool(
            NativeTool::Launchctl,
            &["print", target],
            Duration::from_secs(2),
        )
        .ok()
        .filter(|output| output.exit_code == Some(0))
        .map(|output| output.stdout)
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use std::path::Path;
    use std::time::Duration;

    use crate::adapters::{cron, systemd, warning};
    use crate::app::{ScanBundle, bundle};
    use crate::diagnostics::{Visibility, VisibilityStatus};
    use crate::input::read_bounded_file;
    use crate::model::{JobScope, SchedulerKind};
    use crate::process::{NativeTool, run_native_tool};

    pub(super) fn scan() -> ScanBundle {
        let mut jobs = Vec::new();
        let mut warnings = Vec::new();
        let mut visibility = Vec::new();
        match run_native_tool(NativeTool::Crontab, &["-l"], Duration::from_secs(2)) {
            Ok(output) if output.exit_code == Some(0) => {
                let result = cron::parse_crontab(
                    &output.stdout,
                    "user crontab",
                    JobScope::User,
                    None,
                    false,
                );
                jobs.extend(result.jobs);
                warnings.extend(result.warnings);
                visibility.push(visible(SchedulerKind::Cron, JobScope::User));
            }
            Ok(output) => visibility.push(limited(
                SchedulerKind::Cron,
                JobScope::User,
                if output.stderr.is_empty() {
                    "The user crontab was unavailable."
                } else {
                    "The user crontab could not be read."
                },
            )),
            Err(error) => {
                warnings.push(warning("cron.command", error.to_string(), "crontab -l"));
                visibility.push(limited(
                    SchedulerKind::Cron,
                    JobScope::User,
                    "The crontab utility was unavailable.",
                ));
            }
        }
        let root = Path::new("/etc");
        if let Ok(contents) = read_bounded_file(Path::new("/etc/crontab"), &[root]) {
            let result =
                cron::parse_crontab(&contents, "/etc/crontab", JobScope::System, None, true);
            jobs.extend(result.jobs);
            warnings.extend(result.warnings);
            visibility.push(visible(SchedulerKind::Cron, JobScope::System));
        } else {
            visibility.push(limited(
                SchedulerKind::Cron,
                JobScope::System,
                "The system crontab was missing or unreadable without elevation.",
            ));
        }
        for user_scope in [false, true] {
            let mut arguments = vec!["list-unit-files", "--type=timer", "--no-legend", "--plain"];
            if user_scope {
                arguments.insert(0, "--user");
            }
            if let Ok(list) =
                run_native_tool(NativeTool::Systemctl, &arguments, Duration::from_secs(3))
            {
                for identifier in list
                    .stdout
                    .lines()
                    .filter_map(|line| line.split_whitespace().next())
                {
                    let mut show_arguments = vec!["show", identifier, "--no-pager"];
                    if user_scope {
                        show_arguments.insert(0, "--user");
                    }
                    if let Ok(show) = run_native_tool(
                        NativeTool::Systemctl,
                        &show_arguments,
                        Duration::from_secs(2),
                    ) && show.exit_code == Some(0)
                    {
                        match systemd::parse_timer_show(
                            &show.stdout,
                            if user_scope {
                                JobScope::User
                            } else {
                                JobScope::System
                            },
                        ) {
                            Ok(job) => jobs.push(job),
                            Err(parse_warning) => warnings.push(parse_warning),
                        }
                    }
                }
                visibility.push(visible(
                    SchedulerKind::Systemd,
                    if user_scope {
                        JobScope::User
                    } else {
                        JobScope::System
                    },
                ));
            } else {
                visibility.push(limited(
                    SchedulerKind::Systemd,
                    if user_scope {
                        JobScope::User
                    } else {
                        JobScope::System
                    },
                    "The systemd manager was unavailable or permission-limited.",
                ));
            }
        }
        jobs.sort_by(|left, right| left.id.cmp(&right.id));
        bundle("Linux cron and systemd", jobs, warnings, visibility)
    }

    fn visible(scheduler: SchedulerKind, scope: JobScope) -> Visibility {
        Visibility {
            scheduler,
            scope,
            status: VisibilityStatus::Complete,
            explanation: "Readable scheduler evidence was scanned.".into(),
        }
    }

    fn limited(scheduler: SchedulerKind, scope: JobScope, explanation: &str) -> Visibility {
        Visibility {
            scheduler,
            scope,
            status: VisibilityStatus::PermissionLimited,
            explanation: explanation.into(),
        }
    }
}

#[cfg(target_os = "windows")]
mod windows {
    use std::time::Duration;

    use crate::adapters::{warning, windows};
    use crate::app::{ScanBundle, bundle};
    use crate::diagnostics::{Visibility, VisibilityStatus};
    use crate::model::{JobScope, SchedulerKind};
    use crate::process::{NativeTool, run_native_tool};

    pub(super) fn scan() -> ScanBundle {
        let mut jobs = Vec::new();
        let mut warnings = Vec::new();
        let (status, explanation) = match run_native_tool(
            NativeTool::Schtasks,
            &["/query", "/xml", "ONE"],
            Duration::from_secs(10),
        ) {
            Ok(output) if output.exit_code == Some(0) => {
                match windows::parse_task_xml(output.stdout.as_bytes(), "schtasks /query /xml") {
                    Ok(job) => jobs.push(job),
                    Err(parse_warning) => warnings.push(parse_warning),
                }
                (
                    VisibilityStatus::Complete,
                    "Task Scheduler returned readable local XML.",
                )
            }
            Ok(_) => (
                VisibilityStatus::PermissionLimited,
                "Task Scheduler query was denied or unavailable for the current token.",
            ),
            Err(error) => {
                warnings.push(warning("windows.query", error.to_string(), "schtasks"));
                (
                    VisibilityStatus::Unavailable,
                    "The schtasks utility was unavailable.",
                )
            }
        };
        bundle(
            "Windows Task Scheduler",
            jobs,
            warnings,
            vec![Visibility {
                scheduler: SchedulerKind::WindowsTaskScheduler,
                scope: JobScope::User,
                status,
                explanation: explanation.into(),
            }],
        )
    }
}
