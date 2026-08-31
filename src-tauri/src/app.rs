use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::diagnostics::{Finding, Visibility, diagnose};
use crate::export::{ExportPolicy, export_csv, export_html, export_json};
use crate::input::{local_directory_exists, local_executable_exists};
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
    let diagnostics_started = std::time::Instant::now();
    let findings = diagnose(
        &jobs,
        &warnings,
        &visibility,
        generated_at,
        local_executable_exists,
        local_directory_exists,
    );
    let diagnostics_ms = diagnostics_started.elapsed().as_secs_f64() * 1_000.0;
    eprintln!(
        "{{\"event\":\"scan.complete\",\"platform\":\"{platform}\",\"jobs\":{},\"warnings\":{},\"findings\":{},\"diagnosticsMs\":{diagnostics_ms:.1}}}",
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
    use std::path::Path;
    use std::time::Duration;

    use crate::adapters::launchd;
    use crate::adapters::warning;
    use crate::app::{ScanBundle, bundle};
    use crate::diagnostics::{Visibility, VisibilityStatus};
    use crate::input::{
        BoundaryError, MAX_JOBS, current_user_home, read_bounded_file_bytes, read_directory,
    };
    use crate::model::{JobScope, ParseWarning, ScheduledJob, SchedulerKind};
    use crate::process::{NativeTool, run_native_tool};

    pub(super) fn scan() -> ScanBundle {
        let scan_started = std::time::Instant::now();
        let mut jobs = Vec::new();
        let mut warnings = Vec::new();
        let mut visibility = Vec::new();
        let uid = unsafe { libc::getuid() };
        let gui_target = format!("gui/{uid}");
        let domains_started = std::time::Instant::now();
        let gui_output = domain_output(&gui_target);
        let system_output = domain_output("system");
        let domains_ms = domains_started.elapsed().as_secs_f64() * 1_000.0;

        match current_user_home() {
            Some(home) => {
                let root = home.join("Library/LaunchAgents");
                let limited = scan_directory(
                    &root,
                    JobScope::User,
                    gui_output.as_ref(),
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
            ("/Library/LaunchAgents", gui_output.as_ref()),
            ("/Library/LaunchDaemons", system_output.as_ref()),
            ("/System/Library/LaunchAgents", gui_output.as_ref()),
            ("/System/Library/LaunchDaemons", system_output.as_ref()),
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
        let collect_ms = scan_started.elapsed().as_secs_f64() * 1_000.0;
        eprintln!(
            "{{\"event\":\"scan.collect\",\"platform\":\"macOS launchd\",\"jobs\":{},\"warnings\":{},\"domainsMs\":{domains_ms:.1},\"collectMs\":{collect_ms:.1}}}",
            jobs.len(),
            warnings.len()
        );
        bundle("macOS launchd", jobs, warnings, visibility)
    }

    fn scan_directory(
        root: &Path,
        scope: JobScope,
        domain_output: Option<&launchd::LaunchctlDomain>,
        jobs: &mut Vec<ScheduledJob>,
        warnings: &mut Vec<ParseWarning>,
    ) -> bool {
        let entries = match read_directory(root) {
            Ok(entries) => entries,
            Err(BoundaryError::PathMissing) => return false,
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
        let scan_started = std::time::Instant::now();
        let mut definitions_seen = 0_usize;
        for entry in entries {
            if scan_started.elapsed() >= Duration::from_secs(5) {
                warnings.push(warning(
                    "launchd.directoryTimeout",
                    "directory scan exceeded its time limit",
                    &root.display().to_string(),
                ));
                limited = true;
                break;
            }
            if jobs.len() >= MAX_JOBS {
                warnings.push(warning(
                    "launchd.jobLimit",
                    format!("job limit of {MAX_JOBS} reached"),
                    &root.display().to_string(),
                ));
                limited = true;
                break;
            }
            let path = match entry {
                Ok(path) => path,
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
            definitions_seen += 1;
            if definitions_seen > MAX_JOBS {
                warnings.push(warning(
                    "launchd.definitionLimit",
                    format!("definition limit of {MAX_JOBS} reached"),
                    &root.display().to_string(),
                ));
                limited = true;
                break;
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
                    if let Some(domain) = domain_output {
                        launchd::enrich_launchctl_domain(&mut job, domain);
                    }
                    jobs.push(job);
                }
                Err(parse_warning) => warnings.push(parse_warning),
            }
        }
        limited
    }

    fn domain_output(target: &str) -> Option<launchd::LaunchctlDomain> {
        run_native_tool(
            NativeTool::Launchctl,
            &["print", target],
            Duration::from_secs(2),
        )
        .ok()
        .filter(|output| output.exit_code == Some(0))
        .map(|output| launchd::parse_launchctl_domain(&output.stdout))
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn aggregate_job_limit_marks_launchd_visibility_as_limited() {
            let root_guard = tempfile::tempdir().expect("launchd root");
            let root = root_guard
                .path()
                .canonicalize()
                .expect("canonical launchd root");
            std::fs::write(root.join("pending.plist"), b"fixture").expect("launchd fixture");
            let fixture = ScheduledJob::new(
                SchedulerKind::Launchd,
                "existing",
                "Existing fixture",
                JobScope::System,
                "fixture",
            );
            let mut jobs = vec![fixture; MAX_JOBS];
            let mut warnings = Vec::new();

            let limited = scan_directory(&root, JobScope::System, None, &mut jobs, &mut warnings);

            assert!(limited);
            assert_eq!(jobs.len(), MAX_JOBS);
            assert!(
                warnings
                    .iter()
                    .any(|warning| warning.code == "launchd.jobLimit")
            );
        }

        #[test]
        fn broken_symlinked_launchd_root_is_permission_limited() {
            use std::os::unix::fs::symlink;

            let parent_guard = tempfile::tempdir().expect("launchd parent");
            let parent = parent_guard
                .path()
                .canonicalize()
                .expect("canonical launchd parent");
            let root = parent.join("LaunchAgents");
            symlink(parent.join("missing"), &root).expect("broken launchd root symlink");
            let mut jobs = Vec::new();
            let mut warnings = Vec::new();

            let limited = scan_directory(&root, JobScope::User, None, &mut jobs, &mut warnings);

            assert!(limited);
            assert!(jobs.is_empty());
            assert!(
                warnings
                    .iter()
                    .any(|warning| warning.code == "launchd.directory")
            );
        }
    }
}

#[cfg(any(target_os = "linux", all(test, unix)))]
mod linux {
    use std::io::ErrorKind;
    use std::path::Path;
    use std::time::Duration;

    use crate::adapters::{AdapterResult, cron, systemd, warning};
    use crate::app::{ScanBundle, bundle};
    use crate::diagnostics::{Visibility, VisibilityStatus};
    use crate::input::{
        BoundaryError, MAX_JOBS, current_user_name, read_bounded_file, read_directory,
    };
    use crate::model::{JobScope, ParseWarning, ScheduledJob, SchedulerKind};
    use crate::process::{NativeTool, run_native_tool};

    pub(super) fn scan() -> ScanBundle {
        let mut jobs = Vec::new();
        let mut warnings = Vec::new();
        let mut visibility = Vec::new();
        visibility.push(match current_user_name() {
            Some(username) => scan_user_cron_roots(
                &[
                    Path::new("/var/spool/cron/crontabs"),
                    Path::new("/var/spool/cron"),
                ],
                &username,
                &mut jobs,
                &mut warnings,
            ),
            None => unavailable(
                SchedulerKind::Cron,
                JobScope::User,
                "The current user identity was unavailable; the crontab helper was not invoked.",
            ),
        });
        let (system_cron_seen, system_cron_limited) =
            scan_system_cron(Path::new("/etc"), &mut jobs, &mut warnings);
        if system_cron_seen && !system_cron_limited {
            visibility.push(visible(SchedulerKind::Cron, JobScope::System));
        } else if system_cron_seen {
            visibility.push(limited(
                SchedulerKind::Cron,
                JobScope::System,
                "One or more system cron sources were unreadable without elevation.",
            ));
        } else {
            visibility.push(unavailable(
                SchedulerKind::Cron,
                JobScope::System,
                "No supported system cron source was present.",
            ));
        }
        for user_scope in [false, true] {
            visibility.push(scan_systemd_scope(user_scope, &mut jobs, &mut warnings));
        }
        jobs.sort_by(|left, right| left.id.cmp(&right.id));
        bundle("Linux cron and systemd", jobs, warnings, visibility)
    }

    fn scan_user_cron_roots(
        roots: &[&Path],
        username: &str,
        jobs: &mut Vec<ScheduledJob>,
        warnings: &mut Vec<ParseWarning>,
    ) -> Visibility {
        let mut read_any = false;
        let mut limited_any = false;
        for root in roots {
            match std::fs::symlink_metadata(root) {
                Ok(metadata) if metadata.file_type().is_symlink() => {
                    warnings.push(warning(
                        "cron.userDirectory",
                        BoundaryError::SymlinkRejected.to_string(),
                        &root.display().to_string(),
                    ));
                    limited_any = true;
                    continue;
                }
                Ok(metadata) if !metadata.is_dir() => {
                    warnings.push(warning(
                        "cron.userDirectory",
                        BoundaryError::NotADirectory.to_string(),
                        &root.display().to_string(),
                    ));
                    limited_any = true;
                    continue;
                }
                Ok(_) => {}
                Err(error) if error.kind() == ErrorKind::NotFound => continue,
                Err(error) => {
                    warnings.push(warning(
                        "cron.userDirectory",
                        error.to_string(),
                        &root.display().to_string(),
                    ));
                    limited_any = true;
                    continue;
                }
            }
            let path = root.join(username);
            match read_bounded_file(&path, &[*root]) {
                Ok(contents) => {
                    let source = path.display().to_string();
                    append_result(
                        jobs,
                        warnings,
                        cron::parse_crontab(
                            &contents,
                            &source,
                            JobScope::User,
                            Some(username),
                            false,
                        ),
                        &source,
                    );
                    read_any = true;
                }
                Err(BoundaryError::PathMissing) => {}
                Err(error) => {
                    warnings.push(warning(
                        "cron.userRead",
                        error.to_string(),
                        &path.display().to_string(),
                    ));
                    limited_any = true;
                }
            }
        }
        if limited_any {
            limited(
                SchedulerKind::Cron,
                JobScope::User,
                "The current user's direct cron spool file was permission-limited.",
            )
        } else if read_any {
            visible(SchedulerKind::Cron, JobScope::User)
        } else {
            unavailable(
                SchedulerKind::Cron,
                JobScope::User,
                "No readable current-user cron spool file was present; the crontab helper was not invoked.",
            )
        }
    }

    fn scan_system_cron(
        root: &Path,
        jobs: &mut Vec<ScheduledJob>,
        warnings: &mut Vec<ParseWarning>,
    ) -> (bool, bool) {
        let mut seen = false;
        let mut limited = false;
        let crontab = root.join("crontab");
        match read_optional_cron_file(&crontab, root, true, jobs, warnings) {
            SourceState::Read => seen = true,
            SourceState::Missing => {}
            SourceState::Limited => {
                seen = true;
                limited = true;
            }
        }
        let cron_d = root.join("cron.d");
        match scan_cron_directory(&cron_d, jobs, warnings) {
            SourceState::Read => seen = true,
            SourceState::Missing => {}
            SourceState::Limited => {
                seen = true;
                limited = true;
            }
        }
        for period in ["hourly", "daily", "weekly", "monthly"] {
            let directory = root.join(format!("cron.{period}"));
            match scan_periodic_directory(&directory, period, jobs, warnings) {
                SourceState::Read => seen = true,
                SourceState::Missing => {}
                SourceState::Limited => {
                    seen = true;
                    limited = true;
                }
            }
        }
        (seen, limited)
    }

    #[derive(Clone, Copy)]
    enum SourceState {
        Read,
        Missing,
        Limited,
    }

    fn read_optional_cron_file(
        path: &Path,
        allowed_root: &Path,
        has_owner_column: bool,
        jobs: &mut Vec<ScheduledJob>,
        warnings: &mut Vec<ParseWarning>,
    ) -> SourceState {
        match read_bounded_file(path, &[allowed_root]) {
            Ok(contents) => {
                let source = path.display().to_string();
                append_result(
                    jobs,
                    warnings,
                    cron::parse_crontab(
                        &contents,
                        &source,
                        JobScope::System,
                        None,
                        has_owner_column,
                    ),
                    &source,
                );
                SourceState::Read
            }
            Err(BoundaryError::PathMissing) => SourceState::Missing,
            Err(error) => {
                warnings.push(warning(
                    "cron.read",
                    error.to_string(),
                    &path.display().to_string(),
                ));
                SourceState::Limited
            }
        }
    }

    fn scan_cron_directory(
        root: &Path,
        jobs: &mut Vec<ScheduledJob>,
        warnings: &mut Vec<ParseWarning>,
    ) -> SourceState {
        let entries = match directory_entries(root, warnings) {
            Ok(Some(entries)) => entries,
            Ok(None) => return SourceState::Missing,
            Err(()) => return SourceState::Limited,
        };
        let mut limited = false;
        let scan_started = std::time::Instant::now();
        let mut definitions_seen = 0_usize;
        for entry in entries {
            if scan_started.elapsed() >= Duration::from_secs(5) {
                warnings.push(warning(
                    "cron.directoryTimeout",
                    "directory scan exceeded its time limit",
                    &root.display().to_string(),
                ));
                limited = true;
                break;
            }
            definitions_seen += 1;
            if definitions_seen > MAX_JOBS {
                push_job_limit_warning(warnings, &root.display().to_string());
                limited = true;
                break;
            }
            let path = match entry {
                Ok(path) => path,
                Err(error) => {
                    warnings.push(warning(
                        "cron.entry",
                        error.to_string(),
                        &root.display().to_string(),
                    ));
                    limited = true;
                    continue;
                }
            };
            match read_optional_cron_file(&path, root, true, jobs, warnings) {
                SourceState::Read | SourceState::Missing => {}
                SourceState::Limited => limited = true,
            }
            if jobs.len() >= MAX_JOBS {
                limited = true;
                break;
            }
        }
        if limited {
            SourceState::Limited
        } else {
            SourceState::Read
        }
    }

    fn scan_periodic_directory(
        root: &Path,
        period: &str,
        jobs: &mut Vec<ScheduledJob>,
        warnings: &mut Vec<ParseWarning>,
    ) -> SourceState {
        use std::os::unix::fs::PermissionsExt;

        let entries = match directory_entries(root, warnings) {
            Ok(Some(entries)) => entries,
            Ok(None) => return SourceState::Missing,
            Err(()) => return SourceState::Limited,
        };
        let mut limited = false;
        let scan_started = std::time::Instant::now();
        let mut definitions_seen = 0_usize;
        for entry in entries {
            if scan_started.elapsed() >= Duration::from_secs(5) {
                warnings.push(warning(
                    "cron.periodicDirectoryTimeout",
                    "directory scan exceeded its time limit",
                    &root.display().to_string(),
                ));
                return SourceState::Limited;
            }
            definitions_seen += 1;
            if definitions_seen > MAX_JOBS {
                push_job_limit_warning(warnings, &root.display().to_string());
                return SourceState::Limited;
            }
            if jobs.len() >= MAX_JOBS {
                push_job_limit_warning(warnings, &root.display().to_string());
                return SourceState::Limited;
            }
            let path = match entry {
                Ok(path) => path,
                Err(error) => {
                    warnings.push(warning(
                        "cron.periodicEntry",
                        error.to_string(),
                        &root.display().to_string(),
                    ));
                    limited = true;
                    continue;
                }
            };
            let Ok(metadata) = std::fs::symlink_metadata(&path) else {
                limited = true;
                continue;
            };
            if metadata.file_type().is_symlink() {
                warnings.push(warning(
                    "cron.periodicSymlink",
                    "symbolic link input was rejected",
                    &path.display().to_string(),
                ));
                limited = true;
                continue;
            }
            if metadata.is_file() && metadata.permissions().mode() & 0o111 != 0 {
                jobs.push(cron::periodic_job(
                    &path.display().to_string(),
                    period,
                    JobScope::System,
                ));
            }
        }
        if limited {
            SourceState::Limited
        } else {
            SourceState::Read
        }
    }

    fn directory_entries(
        root: &Path,
        warnings: &mut Vec<ParseWarning>,
    ) -> Result<Option<Vec<Result<std::path::PathBuf, BoundaryError>>>, ()> {
        match read_directory(root) {
            Ok(entries) => Ok(Some(entries)),
            Err(BoundaryError::PathMissing) => Ok(None),
            Err(error) => {
                warnings.push(warning(
                    "cron.directory",
                    error.to_string(),
                    &root.display().to_string(),
                ));
                Err(())
            }
        }
    }

    fn append_result(
        jobs: &mut Vec<ScheduledJob>,
        warnings: &mut Vec<ParseWarning>,
        result: AdapterResult,
        source: &str,
    ) {
        warnings.extend(result.warnings);
        let remaining = MAX_JOBS.saturating_sub(jobs.len());
        let truncated = result.jobs.len() > remaining;
        jobs.extend(result.jobs.into_iter().take(remaining));
        if truncated {
            push_job_limit_warning(warnings, source);
        }
    }

    fn push_job_limit_warning(warnings: &mut Vec<ParseWarning>, source: &str) {
        if !warnings
            .iter()
            .any(|warning| warning.code == "scan.jobLimit")
        {
            warnings.push(warning(
                "scan.jobLimit",
                format!("aggregate job limit of {MAX_JOBS} reached"),
                source,
            ));
        }
    }

    fn scan_systemd_scope(
        user_scope: bool,
        jobs: &mut Vec<ScheduledJob>,
        warnings: &mut Vec<ParseWarning>,
    ) -> Visibility {
        let scope = if user_scope {
            JobScope::User
        } else {
            JobScope::System
        };
        let mut arguments = vec!["list-unit-files", "--type=timer", "--no-legend", "--plain"];
        if user_scope {
            arguments.insert(0, "--user");
        }
        let list = match run_native_tool(NativeTool::Systemctl, &arguments, Duration::from_secs(3))
        {
            Ok(list) if list.exit_code == Some(0) => list,
            Ok(list) => {
                warnings.push(warning(
                    "systemd.list",
                    format!(
                        "systemctl exited with {:?}: {}",
                        list.exit_code, list.stderr
                    ),
                    if user_scope {
                        "systemctl --user list-unit-files"
                    } else {
                        "systemctl list-unit-files"
                    },
                ));
                return limited(
                    SchedulerKind::Systemd,
                    scope,
                    "The systemd manager was unavailable or permission-limited.",
                );
            }
            Err(error) => {
                warnings.push(warning(
                    "systemd.list",
                    error.to_string(),
                    "systemctl list-unit-files",
                ));
                return limited(
                    SchedulerKind::Systemd,
                    scope,
                    "The systemd manager was unavailable or permission-limited.",
                );
            }
        };
        let mut scan_limited = false;
        let scan_started = std::time::Instant::now();
        let mut definitions_seen = 0_usize;
        for identifier in list
            .stdout
            .lines()
            .filter_map(|line| line.split_whitespace().next())
        {
            if scan_started.elapsed() >= Duration::from_secs(15) {
                warnings.push(warning(
                    "systemd.scopeTimeout",
                    "timer scan exceeded its aggregate time limit",
                    "systemctl list-unit-files",
                ));
                scan_limited = true;
                break;
            }
            definitions_seen += 1;
            if definitions_seen > MAX_JOBS {
                push_job_limit_warning(warnings, "systemctl list-unit-files");
                scan_limited = true;
                break;
            }
            if jobs.len() >= MAX_JOBS {
                push_job_limit_warning(warnings, "systemctl list-unit-files");
                scan_limited = true;
                break;
            }
            if !systemd::valid_timer_identifier(identifier) {
                warnings.push(warning(
                    "systemd.identifier",
                    "systemctl returned an invalid timer identifier",
                    "systemctl list-unit-files",
                ));
                scan_limited = true;
                continue;
            }
            let mut show_arguments = vec!["show", "--no-pager", "--", identifier];
            if user_scope {
                show_arguments.insert(0, "--user");
            }
            match run_native_tool(
                NativeTool::Systemctl,
                &show_arguments,
                Duration::from_secs(2),
            ) {
                Ok(show) if show.exit_code == Some(0) => {
                    match systemd::parse_timer_show(&show.stdout, scope) {
                        Ok(job) => jobs.push(job),
                        Err(parse_warning) => {
                            warnings.push(parse_warning);
                            scan_limited = true;
                        }
                    }
                }
                Ok(show) => {
                    warnings.push(warning(
                        "systemd.show",
                        format!(
                            "systemctl exited with {:?}: {}",
                            show.exit_code, show.stderr
                        ),
                        identifier,
                    ));
                    scan_limited = true;
                }
                Err(error) => {
                    warnings.push(warning("systemd.show", error.to_string(), identifier));
                    scan_limited = true;
                }
            }
        }
        if scan_limited {
            limited(
                SchedulerKind::Systemd,
                scope,
                "One or more systemd timer definitions were unavailable or invalid.",
            )
        } else {
            visible(SchedulerKind::Systemd, scope)
        }
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

    fn unavailable(scheduler: SchedulerKind, scope: JobScope, explanation: &str) -> Visibility {
        Visibility {
            scheduler,
            scope,
            status: VisibilityStatus::Unavailable,
            explanation: explanation.into(),
        }
    }

    #[cfg(test)]
    mod tests {
        use std::os::unix::fs::PermissionsExt;

        use super::*;
        use crate::model::{Evidence, SchedulerKind};

        #[test]
        fn system_cron_collector_reads_cron_d_and_periodic_directories() {
            let _compile_complete_linux_scanner: fn() -> ScanBundle = scan;
            let root_guard = tempfile::tempdir().expect("cron root");
            let root = root_guard
                .path()
                .canonicalize()
                .expect("canonical cron root");
            std::fs::write(root.join("crontab"), "0 2 * * * root /usr/bin/backup\n")
                .expect("system crontab");
            let cron_d = root.join("cron.d");
            std::fs::create_dir(&cron_d).expect("cron.d");
            std::fs::write(cron_d.join("cleanup"), "0 3 * * * root /usr/bin/cleanup\n")
                .expect("cron.d file");
            let daily = root.join("cron.daily");
            std::fs::create_dir(&daily).expect("cron.daily");
            let periodic = daily.join("rotate");
            std::fs::write(&periodic, "#!/bin/sh\n").expect("periodic fixture");
            let mut permissions = std::fs::metadata(&periodic)
                .expect("periodic metadata")
                .permissions();
            permissions.set_mode(0o700);
            std::fs::set_permissions(&periodic, permissions).expect("periodic permissions");

            let mut jobs = Vec::new();
            let mut warnings = Vec::new();
            let (seen, limited) = scan_system_cron(&root, &mut jobs, &mut warnings);

            assert!(seen);
            assert!(!limited, "{warnings:?}");
            assert_eq!(jobs.len(), 3);
            assert!(jobs.iter().all(|job| {
                matches!(
                    job.scheduler,
                    Evidence::Available {
                        value: SchedulerKind::Cron,
                        ..
                    }
                )
            }));
        }

        #[test]
        fn broken_system_cron_entry_is_permission_limited() {
            use std::os::unix::fs::symlink;

            let root_guard = tempfile::tempdir().expect("cron root");
            let root = root_guard
                .path()
                .canonicalize()
                .expect("canonical cron root");
            let cron_d = root.join("cron.d");
            std::fs::create_dir(&cron_d).expect("cron.d");
            symlink(root.join("missing"), cron_d.join("broken"))
                .expect("broken cron entry symlink");
            let mut jobs = Vec::new();
            let mut warnings = Vec::new();

            let (seen, limited) = scan_system_cron(&root, &mut jobs, &mut warnings);

            assert!(seen);
            assert!(limited);
            assert!(jobs.is_empty());
            assert!(warnings.iter().any(|warning| warning.code == "cron.read"));
        }

        #[test]
        fn user_cron_collector_reads_a_direct_accessible_spool_file() {
            let root_guard = tempfile::tempdir().expect("user cron root");
            let root = root_guard
                .path()
                .canonicalize()
                .expect("canonical user cron root");
            std::fs::write(
                root.join("fixture-user"),
                "0 4 * * * /usr/bin/refresh --quiet\n",
            )
            .expect("user crontab");
            let mut jobs = Vec::new();
            let mut warnings = Vec::new();

            let visibility =
                scan_user_cron_roots(&[&root], "fixture-user", &mut jobs, &mut warnings);

            assert_eq!(visibility.status, VisibilityStatus::Complete);
            assert!(warnings.is_empty(), "{warnings:?}");
            assert_eq!(jobs.len(), 1);
            assert!(matches!(
                jobs[0].scope,
                Evidence::Available {
                    value: JobScope::User,
                    ..
                }
            ));
        }

        #[test]
        fn broken_user_cron_root_is_permission_limited() {
            use std::os::unix::fs::symlink;

            let parent_guard = tempfile::tempdir().expect("user cron parent");
            let parent = parent_guard
                .path()
                .canonicalize()
                .expect("canonical user cron parent");
            let root = parent.join("cron");
            symlink(parent.join("missing"), &root).expect("broken user cron root symlink");
            let mut jobs = Vec::new();
            let mut warnings = Vec::new();

            let visibility =
                scan_user_cron_roots(&[&root], "fixture-user", &mut jobs, &mut warnings);

            assert_eq!(visibility.status, VisibilityStatus::PermissionLimited);
            assert!(jobs.is_empty());
            assert!(
                warnings
                    .iter()
                    .any(|warning| warning.code == "cron.userDirectory")
            );
        }

        #[test]
        fn complete_linux_scanner_returns_bounded_visibility_evidence() {
            let bundle = scan();

            assert_eq!(bundle.platform, "Linux cron and systemd");
            assert!(bundle.jobs.len() <= MAX_JOBS);
            assert_eq!(bundle.visibility.len(), 4);
        }

        #[test]
        fn aggregate_job_append_never_exceeds_the_global_limit() {
            let mut jobs = (0..MAX_JOBS - 1)
                .map(|index| {
                    ScheduledJob::new(
                        SchedulerKind::Cron,
                        format!("existing-{index}"),
                        "Existing fixture",
                        JobScope::User,
                        "fixture",
                    )
                })
                .collect::<Vec<_>>();
            let incoming = (0..2)
                .map(|index| {
                    ScheduledJob::new(
                        SchedulerKind::Systemd,
                        format!("incoming-{index}.timer"),
                        "Incoming fixture",
                        JobScope::System,
                        "fixture",
                    )
                })
                .collect();
            let mut warnings = Vec::new();

            append_result(
                &mut jobs,
                &mut warnings,
                AdapterResult {
                    jobs: incoming,
                    warnings: Vec::new(),
                },
                "fixture",
            );

            assert_eq!(jobs.len(), MAX_JOBS);
            assert!(
                warnings
                    .iter()
                    .any(|warning| warning.code == "scan.jobLimit")
            );
        }
    }
}

#[cfg(any(target_os = "windows", all(test, unix)))]
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
        let (mut status, mut explanation) = match run_native_tool(
            NativeTool::Schtasks,
            &["/query", "/xml"],
            Duration::from_secs(10),
        ) {
            Ok(output) if output.exit_code == Some(0) => {
                let result = windows::parse_task_xml_collection(
                    output.stdout.as_bytes(),
                    "schtasks /query /xml",
                );
                let complete = result.warnings.is_empty();
                jobs = result.jobs;
                warnings.extend(result.warnings);
                if complete {
                    (
                        VisibilityStatus::Complete,
                        "Task Scheduler returned readable local XML.",
                    )
                } else {
                    (
                        VisibilityStatus::PermissionLimited,
                        "One or more Task Scheduler definitions were invalid or unavailable.",
                    )
                }
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
        if !jobs.is_empty() {
            match run_native_tool(
                NativeTool::PowerShell,
                &[
                    "-NoLogo",
                    "-NoProfile",
                    "-NonInteractive",
                    "-Command",
                    RUNTIME_QUERY,
                ],
                Duration::from_secs(15),
            ) {
                Ok(output) if output.exit_code == Some(0) => {
                    if let Err(parse_warning) = windows::enrich_runtime_json(
                        &mut jobs,
                        &output.stdout,
                        "Get-ScheduledTaskInfo local runtime query",
                    ) {
                        warnings.push(parse_warning);
                        status = VisibilityStatus::PermissionLimited;
                        explanation =
                            "Task definitions were readable, but runtime evidence was invalid.";
                    }
                }
                Ok(output) => {
                    warnings.push(warning(
                        "windows.runtime",
                        format!(
                            "runtime query exited with {:?}: {}",
                            output.exit_code, output.stderr
                        ),
                        "Get-ScheduledTaskInfo",
                    ));
                    status = VisibilityStatus::PermissionLimited;
                    explanation =
                        "Task definitions were readable, but runtime evidence was unavailable.";
                }
                Err(error) => {
                    warnings.push(warning(
                        "windows.runtime",
                        error.to_string(),
                        "Get-ScheduledTaskInfo",
                    ));
                    status = VisibilityStatus::PermissionLimited;
                    explanation =
                        "Task definitions were readable, but runtime evidence was unavailable.";
                }
            }
        }
        jobs.sort_by(|left, right| left.id.cmp(&right.id));
        bundle(
            "Windows Task Scheduler",
            jobs,
            warnings,
            [JobScope::User, JobScope::System]
                .into_iter()
                .map(|scope| Visibility {
                    scheduler: SchedulerKind::WindowsTaskScheduler,
                    scope,
                    status,
                    explanation: explanation.into(),
                })
                .collect(),
        )
    }

    const RUNTIME_QUERY: &str = r#"$OutputEncoding=[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false); $module=[IO.Path]::Combine([Environment]::SystemDirectory,'WindowsPowerShell','v1.0','Modules','ScheduledTasks','ScheduledTasks.psd1'); Import-Module -Name $module -ErrorAction Stop; function Convert-Iso($value) { if ($null -eq $value -or $value -le [DateTime]::MinValue) { return $null }; return $value.ToUniversalTime().ToString('o',[Globalization.CultureInfo]::InvariantCulture) }; $records=@(Get-ScheduledTask -ErrorAction Stop | ForEach-Object { $task=$_; $info=$task | Get-ScheduledTaskInfo -ErrorAction Stop; [PSCustomObject]@{ identifier=($task.TaskPath+$task.TaskName); nextRunTime=(Convert-Iso $info.NextRunTime); lastRunTime=(Convert-Iso $info.LastRunTime); lastTaskResult=[Int64]$info.LastTaskResult; state=[String]$task.State } }); ConvertTo-Json -InputObject $records -Compress"#;

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn windows_scanner_and_invariant_runtime_query_compile_together() {
            let _compile_complete_windows_scanner: fn() -> ScanBundle = scan;
            assert!(RUNTIME_QUERY.contains("ToUniversalTime().ToString('o'"));
            assert!(RUNTIME_QUERY.contains("[Environment]::SystemDirectory"));
            assert!(!RUNTIME_QUERY.contains("/S "));
            assert!(!RUNTIME_QUERY.contains("ExecutionPolicy"));
        }

        #[cfg(unix)]
        #[test]
        fn unavailable_windows_tools_produce_explicit_scope_visibility() {
            let bundle = scan();

            assert_eq!(bundle.platform, "Windows Task Scheduler");
            assert!(bundle.jobs.is_empty());
            assert_eq!(bundle.visibility.len(), 2);
            assert!(bundle.visibility.iter().all(|item| {
                item.status == VisibilityStatus::Unavailable
                    && item.scheduler == SchedulerKind::WindowsTaskScheduler
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ExportFormat, render_export, scan_current_platform};

    #[test]
    fn live_read_only_scan_and_export_respect_the_boundary_contract() {
        let bundle = scan_current_platform();

        assert_eq!(bundle.schema_version, "1.0");
        assert!(!bundle.scan_id.is_empty());
        assert!(!bundle.generated_at.is_empty());
        assert!(!bundle.platform.is_empty());
        assert!(!bundle.sample_data);
        assert!(bundle.jobs.len() <= crate::input::MAX_JOBS);

        for format in [ExportFormat::Json, ExportFormat::Csv, ExportFormat::Html] {
            let report = render_export(
                bundle.jobs.clone(),
                bundle.findings.clone(),
                format,
                true,
                false,
            )
            .expect("reviewed native scan should export");
            assert!(!report.is_empty());
        }
        assert!(
            render_export(
                bundle.jobs,
                bundle.findings,
                ExportFormat::Json,
                false,
                false,
            )
            .is_err()
        );
    }
}
