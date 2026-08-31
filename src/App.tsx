import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { demoBundle, largeDemoBundle } from "./data/demo";
import { renderBrowserExport } from "./export";
import type {
  Evidence,
  ExportFormat,
  ExportPolicy,
  Finding,
  RunTime,
  ScanBundle,
  ScheduledJob,
  SchedulerKind,
} from "./types";

type Loader = () => Promise<ScanBundle>;
type Exporter = (
  bundle: ScanBundle,
  format: ExportFormat,
  policy: ExportPolicy,
) => Promise<string>;
interface AppProps {
  loader?: Loader;
  exporter?: Exporter;
}
type ViewMode = "list" | "timeline";
type ThemeMode = "system" | "light" | "dark";
const PAGE_SIZE = 25;

export function App({
  loader = loadBundle,
  exporter = renderExport,
}: AppProps) {
  const [bundle, setBundle] = useState<ScanBundle | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [retry, setRetry] = useState(0);
  const [query, setQuery] = useState("");
  const [scheduler, setScheduler] = useState<"all" | SchedulerKind>("all");
  const [view, setView] = useState<ViewMode>("list");
  const [visibleCount, setVisibleCount] = useState(PAGE_SIZE);
  const [selectedKey, setSelectedKey] = useState<string | null>(null);
  const [theme, setTheme] = useState<ThemeMode>("system");
  const [showFindings, setShowFindings] = useState(false);
  const [exportOpen, setExportOpen] = useState(false);
  const [reviewed, setReviewed] = useState(false);
  const [includeArguments, setIncludeArguments] = useState(false);
  const [exportStatus, setExportStatus] = useState<string | null>(null);
  const exportButtonRef = useRef<HTMLButtonElement>(null);

  const closeExport = useCallback(() => {
    setExportOpen(false);
    queueMicrotask(() => {
      exportButtonRef.current?.focus();
    });
  }, []);

  useEffect(() => {
    let active = true;
    void loader()
      .then((nextBundle) => {
        if (!active) return;
        setBundle(nextBundle);
        setSelectedKey(
          nextBundle.jobs[0] ? jobUiKey(nextBundle.jobs[0]) : null,
        );
        setLoading(false);
      })
      .catch((reason: unknown) => {
        if (!active) return;
        setBundle(null);
        setError(
          reason instanceof Error ? reason.message : "Unknown scan error",
        );
        setLoading(false);
      });
    return () => {
      active = false;
    };
  }, [loader, retry]);

  useEffect(() => {
    document.documentElement.dataset.theme = theme;
  }, [theme]);

  const filteredJobs = useMemo(() => {
    if (!bundle) return [];
    const normalisedQuery = query.trim().toLocaleLowerCase();
    return bundle.jobs
      .filter((job) => {
        if (scheduler !== "all" && evidence(job.scheduler) !== scheduler)
          return false;
        if (!normalisedQuery) return true;
        return [
          evidence(job.displayName),
          evidence(job.nativeIdentifier),
          evidence(job.executable),
          evidence(job.schedule)?.nativeExpression,
          evidence(job.scheduleExplanation),
        ].some((value) =>
          (value ?? "").toLocaleLowerCase().includes(normalisedQuery),
        );
      })
      .sort(compareJobsByNextRun);
  }, [bundle, query, scheduler]);

  const selectedJob =
    filteredJobs.find((job) => jobUiKey(job) === selectedKey) ??
    filteredJobs[0] ??
    null;
  const selectedFindings = bundle
    ? bundle.findings.filter(
        (finding) => selectedJob && finding.jobIds.includes(selectedJob.id),
      )
    : [];
  const visibleJobs = filteredJobs.slice(0, visibleCount);

  async function prepareExport(format: ExportFormat) {
    if (!bundle || !reviewed) return;
    setExportStatus(null);
    try {
      const content = await exporter(bundle, format, {
        reviewed: true,
        includeArguments,
      });
      download(content, format);
      setExportStatus(`${format.toUpperCase()} report prepared locally.`);
    } catch (reason) {
      setExportStatus(
        reason instanceof Error ? reason.message : "Report preparation failed.",
      );
    }
  }

  return (
    <div className="app-frame">
      <a
        className="skip-link"
        href="#main-content"
        onClick={() => {
          setTimeout(() => {
            document.getElementById("main-content")?.focus();
          }, 0);
        }}
      >
        Skip to scheduled jobs
      </a>
      <header className="topbar">
        <div className="brand-lockup" aria-label="JobGlass">
          <Logo />
          <div>
            <strong>JobGlass</strong>
            <span>See what runs next.</span>
          </div>
        </div>
        <div className="topbar-actions">
          <div className="local-status" title="No data leaves this device">
            <span aria-hidden="true" /> Local only
          </div>
          <label className="theme-control">
            <span className="sr-only">Theme</span>
            <select
              value={theme}
              onChange={(event) => {
                setTheme(event.target.value as ThemeMode);
              }}
            >
              <option value="system">System theme</option>
              <option value="light">Light theme</option>
              <option value="dark">Dark theme</option>
            </select>
          </label>
          <button
            ref={exportButtonRef}
            className="button button-primary"
            type="button"
            onClick={() => {
              setReviewed(false);
              setIncludeArguments(false);
              setExportStatus(null);
              setExportOpen(true);
            }}
            disabled={!bundle || bundle.jobs.length === 0}
          >
            <ExportIcon /> Export report
          </button>
        </div>
      </header>

      <div className="workspace">
        <aside className="sidebar" aria-label="JobGlass sections">
          <nav>
            <p className="nav-label">Workspace</p>
            <button
              className={!showFindings ? "nav-item active" : "nav-item"}
              aria-current={!showFindings ? "page" : undefined}
              type="button"
              onClick={() => {
                setShowFindings(false);
              }}
            >
              <GridIcon /> Overview
            </button>
            <button
              className={showFindings ? "nav-item active" : "nav-item"}
              aria-current={showFindings ? "page" : undefined}
              type="button"
              onClick={() => {
                setShowFindings(true);
              }}
            >
              <FindingIcon /> Findings{" "}
              {bundle && (
                <span className="nav-count">{bundle.findings.length}</span>
              )}
            </button>
          </nav>
          <div className="sidebar-note">
            <ShieldIcon />
            <div>
              <strong>Read-only by design</strong>
              <p>JobGlass never runs, edits, or disables a scheduled job.</p>
            </div>
          </div>
        </aside>

        <main id="main-content" className="main-panel" tabIndex={-1}>
          {loading && <LoadingState />}
          {!loading && error && (
            <ErrorState
              error={error}
              onRetry={() => {
                setLoading(true);
                setError(null);
                setBundle(null);
                setRetry((value) => value + 1);
              }}
            />
          )}
          {!loading && bundle?.jobs.length === 0 && !showFindings && (
            <EmptyState bundle={bundle} />
          )}
          {!loading && bundle && (bundle.jobs.length > 0 || showFindings) && (
            <>
              <section
                className="overview-heading"
                aria-labelledby="overview-title"
              >
                <div>
                  <div className="heading-kicker">
                    {bundle.sampleData ? "Sample evidence" : bundle.platform}
                  </div>
                  <h1 id="overview-title">
                    {showFindings ? "Findings" : "Scheduled jobs"}
                  </h1>
                  <p>
                    Scanned {formatDate(bundle.generatedAt)}. Values marked
                    unavailable are not inferred.
                  </p>
                </div>
                <VisibilityPill bundle={bundle} />
              </section>
              <SummaryCards bundle={bundle} />
              {showFindings ? (
                <FindingsView findings={bundle.findings} jobs={bundle.jobs} />
              ) : (
                <section
                  className="jobs-workbench"
                  aria-label="Scheduled job browser"
                >
                  <div className="jobs-column">
                    <div className="toolbar">
                      <label className="search-field">
                        <SearchIcon />
                        <span className="sr-only">Search jobs</span>
                        <input
                          type="search"
                          aria-label="Search jobs"
                          placeholder="Search jobs, commands, schedules…"
                          value={query}
                          onChange={(event) => {
                            setQuery(event.target.value);
                            setVisibleCount(PAGE_SIZE);
                          }}
                        />
                      </label>
                      <label className="filter-field">
                        <span className="sr-only">Filter by scheduler</span>
                        <select
                          value={scheduler}
                          onChange={(event) => {
                            setScheduler(
                              event.target.value as "all" | SchedulerKind,
                            );
                            setVisibleCount(PAGE_SIZE);
                          }}
                        >
                          <option value="all">All schedulers</option>
                          <option value="launchd">launchd</option>
                          <option value="cron">cron</option>
                          <option value="systemd">systemd</option>
                          <option value="windowsTaskScheduler">
                            Task Scheduler
                          </option>
                        </select>
                      </label>
                      <div
                        className="view-tabs"
                        role="group"
                        aria-label="Job view"
                      >
                        <button
                          type="button"
                          aria-pressed={view === "list"}
                          className={view === "list" ? "selected" : ""}
                          onClick={() => {
                            setView("list");
                          }}
                        >
                          List
                        </button>
                        <button
                          type="button"
                          aria-pressed={view === "timeline"}
                          className={view === "timeline" ? "selected" : ""}
                          onClick={() => {
                            setView("timeline");
                          }}
                        >
                          Timeline
                        </button>
                      </div>
                    </div>
                    <div className="result-meta" aria-live="polite">
                      <strong>
                        {filteredJobs.length}{" "}
                        {plural(filteredJobs.length, "job")}
                      </strong>
                      <span>Sorted by next observable run</span>
                    </div>
                    {filteredJobs.length === 0 ? (
                      <div className="no-results">
                        <h2>No jobs match these filters</h2>
                        <button
                          type="button"
                          className="text-button"
                          onClick={() => {
                            setQuery("");
                            setScheduler("all");
                          }}
                        >
                          Clear filters
                        </button>
                      </div>
                    ) : view === "list" ? (
                      <JobList
                        jobs={visibleJobs}
                        findings={bundle.findings}
                        selectedKey={selectedJob ? jobUiKey(selectedJob) : null}
                        onSelect={setSelectedKey}
                      />
                    ) : (
                      <Timeline
                        jobs={visibleJobs}
                        selectedKey={selectedJob ? jobUiKey(selectedJob) : null}
                        onSelect={setSelectedKey}
                      />
                    )}
                    {visibleCount < filteredJobs.length && (
                      <button
                        className="load-more"
                        type="button"
                        onClick={() => {
                          setVisibleCount((count) => count + PAGE_SIZE);
                        }}
                      >
                        Show {PAGE_SIZE} more jobs
                        <span>
                          Showing {visibleJobs.length} of {filteredJobs.length}
                        </span>
                      </button>
                    )}
                  </div>
                  <Inspector job={selectedJob} findings={selectedFindings} />
                </section>
              )}
            </>
          )}
        </main>
      </div>

      {exportOpen && (
        <ExportReview
          reviewed={reviewed}
          includeArguments={includeArguments}
          status={exportStatus}
          onReviewed={setReviewed}
          onIncludeArguments={setIncludeArguments}
          onPrepare={(format) => void prepareExport(format)}
          onClose={closeExport}
        />
      )}
    </div>
  );
}

function SummaryCards({ bundle }: { bundle: ScanBundle }) {
  const errors = bundle.findings.filter(
    (finding) => finding.severity === "error",
  ).length;
  const nextDay = bundle.jobs.filter((job) => evidence(job.nextRun)).length;
  const limited = bundle.visibility.filter(
    (item) => item.status !== "complete",
  ).length;
  return (
    <section className="summary-grid" aria-label="Scheduler summary">
      <article>
        <span>Scheduled jobs</span>
        <strong>{bundle.jobs.length}</strong>
        <small>
          Across{" "}
          {new Set(bundle.jobs.map((job) => evidence(job.scheduler))).size}{" "}
          {plural(
            new Set(bundle.jobs.map((job) => evidence(job.scheduler))).size,
            "scheduler",
          )}
        </small>
      </article>
      <article>
        <span>Needs attention</span>
        <strong>{errors}</strong>
        <small>
          {bundle.findings.length} total{" "}
          {plural(bundle.findings.length, "finding")}
        </small>
      </article>
      <article>
        <span>Observable next run</span>
        <strong>{nextDay}</strong>
        <small>{bundle.jobs.length - nextDay} unknown</small>
      </article>
      <article>
        <span>Visibility limits</span>
        <strong>{limited}</strong>
        <small>
          {limited === 0
            ? "All queried scopes visible"
            : "No elevation requested"}
        </small>
      </article>
    </section>
  );
}

function VisibilityPill({ bundle }: { bundle: ScanBundle }) {
  const limited = bundle.visibility.some((item) => item.status !== "complete");
  return (
    <div
      className={
        limited ? "visibility-pill warning" : "visibility-pill complete"
      }
    >
      <span aria-hidden="true" />{" "}
      {limited ? "Partial visibility" : "Full queried visibility"}
    </div>
  );
}

function JobList({
  jobs,
  findings,
  selectedKey,
  onSelect,
}: {
  jobs: readonly ScheduledJob[];
  findings: readonly Finding[];
  selectedKey: string | null;
  onSelect: (id: string) => void;
}) {
  return (
    <div className="job-list" aria-label="Scheduled jobs">
      {jobs.map((job) => {
        const jobFindings = findings.filter((finding) =>
          finding.jobIds.includes(job.id),
        );
        const name = evidence(job.displayName) ?? "Unnamed job";
        return (
          <button
            type="button"
            className={
              jobUiKey(job) === selectedKey ? "job-row selected" : "job-row"
            }
            key={jobUiKey(job)}
            onClick={() => {
              onSelect(jobUiKey(job));
            }}
            aria-current={jobUiKey(job) === selectedKey ? "true" : undefined}
            aria-label={`${name}, ${schedulerLabel(evidence(job.scheduler))}`}
          >
            <span
              className={`scheduler-mark ${evidence(job.scheduler) ?? "unknown"}`}
            >
              {schedulerMonogram(evidence(job.scheduler))}
            </span>
            <span className="job-primary">
              <strong>{name}</strong>
              <small>
                {evidence(job.scheduleExplanation) ?? "Schedule unavailable"}
              </small>
              <code>
                {evidence(job.executable) ?? "Executable unavailable"}
              </code>
            </span>
            <span className="job-timing">
              <small>Next run</small>
              <strong>{formatRun(job.nextRun)}</strong>
            </span>
            <span className="job-status">
              <Outcome job={job} />
              {jobFindings.length > 0 && (
                <small>
                  {jobFindings.length} {plural(jobFindings.length, "finding")}
                </small>
              )}
            </span>
          </button>
        );
      })}
    </div>
  );
}

function Timeline({
  jobs,
  selectedKey,
  onSelect,
}: {
  jobs: readonly ScheduledJob[];
  selectedKey: string | null;
  onSelect: (id: string) => void;
}) {
  const known = jobs.filter((job) => evidence(job.nextRun));
  const unknown = jobs.filter((job) => !evidence(job.nextRun));
  return (
    <div className="timeline">
      <div className="timeline-track" aria-label="Observable next runs">
        {known.map((job, index) => (
          <button
            type="button"
            key={jobUiKey(job)}
            onClick={() => {
              onSelect(jobUiKey(job));
            }}
            aria-current={jobUiKey(job) === selectedKey ? "true" : undefined}
          >
            <span className="timeline-time">{formatRun(job.nextRun)}</span>
            <span className="timeline-dot" aria-hidden="true" />
            <strong>{evidence(job.displayName)}</strong>
            <small>{schedulerLabel(evidence(job.scheduler))}</small>
            {index < known.length - 1 && (
              <span className="timeline-line" aria-hidden="true" />
            )}
          </button>
        ))}
      </div>
      {unknown.length > 0 && (
        <section className="unknown-runs" aria-labelledby="unknown-title">
          <h2 id="unknown-title">Next run unknown</h2>
          <p>The native scheduler did not expose a portable next-run value.</p>
          {unknown.map((job) => (
            <button
              type="button"
              key={jobUiKey(job)}
              onClick={() => {
                onSelect(jobUiKey(job));
              }}
              aria-current={jobUiKey(job) === selectedKey ? "true" : undefined}
            >
              {evidence(job.displayName)}{" "}
              <span>{schedulerLabel(evidence(job.scheduler))}</span>
            </button>
          ))}
        </section>
      )}
    </div>
  );
}

function Inspector({
  job,
  findings,
}: {
  job: ScheduledJob | null;
  findings: readonly Finding[];
}) {
  if (!job)
    return <aside className="inspector" aria-label="Evidence inspector" />;
  return (
    <aside className="inspector" aria-labelledby="inspector-title">
      <div className="inspector-heading">
        <span
          className={`scheduler-mark ${evidence(job.scheduler) ?? "unknown"}`}
        >
          {schedulerMonogram(evidence(job.scheduler))}
        </span>
        <div>
          <p>{schedulerLabel(evidence(job.scheduler))}</p>
          <h2 id="inspector-title">Evidence inspector</h2>
        </div>
      </div>
      <h3>{evidence(job.displayName) ?? "Unnamed job"}</h3>
      <p className="native-id">{evidenceSummary(job.nativeIdentifier)}</p>
      <dl className="evidence-list">
        <EvidenceRow
          label="Native identifier"
          field={job.nativeIdentifier}
          code
        />
        <EvidenceRow label="Owner" field={job.owner} />
        <EvidenceRow label="Scope" field={job.scope} />
        <EvidenceRow label="Privilege" field={job.privilegeLevel} />
        <EvidenceRow label="Enabled" field={job.enabled} />
        <EvidenceRow
          label="Schedule expression"
          field={job.schedule}
          format={(value) => `${value.kind}: ${value.nativeExpression}`}
          code
        />
        <EvidenceRow
          label="Schedule explanation"
          field={job.scheduleExplanation}
        />
        <EvidenceRow
          label="Scheduler timezone"
          field={job.timezoneBasis}
          format={(value) => `${value.name} (${value.source})`}
        />
        <EvidenceRow
          label="Next run"
          field={job.nextRun}
          format={formatRunValue}
        />
        <EvidenceRow
          label="Last run"
          field={job.lastRun}
          format={formatRunValue}
        />
        <EvidenceRow
          label="Last outcome"
          field={job.lastOutcome}
          format={(value) =>
            `${value.state}; native code ${value.nativeCode === null ? "not reported" : String(value.nativeCode)}; ${value.explanation}`
          }
        />
        <EvidenceRow label="Executable" field={job.executable} code />
        <EvidenceRow
          label="Arguments"
          field={job.arguments}
          format={(value) => value.join(" ") || "None"}
          code
        />
        <EvidenceRow
          label="Working directory"
          field={job.workingDirectory}
          code
        />
        <EvidenceRow
          label="Environment keys"
          field={job.environmentKeys}
          format={(value) => value.join(", ") || "None"}
          code
        />
        <EvidenceRow
          label="Native source"
          field={job.nativeSource}
          format={(value) => `${value.sourceType}: ${value.reference}`}
          code
        />
        <EvidenceRow
          label="Triggers"
          field={job.triggers}
          format={(value) =>
            value
              .map(
                (trigger) =>
                  `${trigger.kind}: ${trigger.expression} — ${trigger.explanation}`,
              )
              .join("; ") || "None"
          }
        />
        <EvidenceRow
          label="Dependencies"
          field={job.dependencies}
          format={(value) => value.join(", ") || "None"}
          code
        />
        <EvidenceRow label="Target service" field={job.targetService} code />
        <div>
          <dt>Parse warnings</dt>
          <dd className={job.parseWarnings.length === 0 ? "unavailable" : ""}>
            {job.parseWarnings.length === 0 ? (
              "None"
            ) : (
              <ul className="parse-warnings">
                {job.parseWarnings.map((warning) => (
                  <li key={`${warning.code}:${warning.sourceReference}`}>
                    <code>{warning.code}</code>: {warning.message}
                    <small>Source: {warning.sourceReference}</small>
                  </li>
                ))}
              </ul>
            )}
          </dd>
        </div>
      </dl>
      <section
        className="inspector-findings"
        aria-labelledby="job-findings-title"
      >
        <h3 id="job-findings-title">Relevant findings</h3>
        {findings.length === 0 ? (
          <p>No findings for this job.</p>
        ) : (
          findings.map((finding) => (
            <article key={finding.id}>
              <Severity severity={finding.severity} />
              <strong>{finding.title}</strong>
              <p>{finding.explanation}</p>
              <FindingEvidence evidence={finding.evidence} />
            </article>
          ))
        )}
      </section>
    </aside>
  );
}

function EvidenceRow<T>({
  label,
  field,
  format = String,
  code = false,
}: {
  label: string;
  field: Evidence<T>;
  format?: (value: T) => string;
  code?: boolean;
}) {
  const available = field.availability === "available";
  const value = available
    ? format(field.value)
    : `Unavailable: ${unavailableReasonLabel(field.reason)}`;
  return (
    <div>
      <dt>{label}</dt>
      <dd className={!available ? "unavailable" : ""}>
        {code && available ? <code>{value}</code> : value}
        <small className="evidence-provenance">
          Source: {field.provenance.sourceReference}
          {field.provenance.detail ? ` — ${field.provenance.detail}` : ""}
        </small>
      </dd>
    </div>
  );
}

function FindingEvidence({ evidence: items }: { evidence: readonly string[] }) {
  if (items.length === 0) return null;
  return (
    <ul className="finding-evidence" aria-label="Supporting evidence">
      {items.map((item) => (
        <li key={item}>
          <code>{item}</code>
        </li>
      ))}
    </ul>
  );
}

function FindingsView({
  findings,
  jobs,
}: {
  findings: readonly Finding[];
  jobs: readonly ScheduledJob[];
}) {
  return (
    <section className="findings-view" aria-label="Diagnostic findings">
      {findings.length === 0 ? (
        <div className="no-results">
          <h2>No findings</h2>
          <p>No deterministic diagnostic rules fired.</p>
        </div>
      ) : (
        findings.map((finding) => (
          <article key={finding.id}>
            <Severity severity={finding.severity} />
            <div>
              <h2>{finding.title}</h2>
              <p>{finding.explanation}</p>
              <small>{findingJobNames(finding, jobs)}</small>
              <FindingEvidence evidence={finding.evidence} />
            </div>
            <code>{finding.code}</code>
          </article>
        ))
      )}
    </section>
  );
}

function ExportReview({
  reviewed,
  includeArguments,
  status,
  onReviewed,
  onIncludeArguments,
  onPrepare,
  onClose,
}: {
  reviewed: boolean;
  includeArguments: boolean;
  status: string | null;
  onReviewed: (value: boolean) => void;
  onIncludeArguments: (value: boolean) => void;
  onPrepare: (format: ExportFormat) => void;
  onClose: () => void;
}) {
  const dialogRef = useRef<HTMLElement>(null);

  useEffect(() => {
    const dialog = dialogRef.current;
    const focusable = () =>
      Array.from(
        dialog?.querySelectorAll<HTMLElement>(
          'button:not([disabled]), input:not([disabled]), select:not([disabled]), [href], [tabindex]:not([tabindex="-1"])',
        ) ?? [],
      );
    focusable()[0]?.focus();

    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") {
        event.preventDefault();
        onClose();
        return;
      }
      if (event.key !== "Tab") return;
      const elements = focusable();
      const first = elements[0];
      const last = elements.at(-1);
      if (!first || !last) return;
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first.focus();
      }
    }

    document.addEventListener("keydown", handleKeyDown);
    return () => {
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [onClose]);

  return (
    <div className="modal-backdrop" role="presentation" onMouseDown={onClose}>
      <section
        ref={dialogRef}
        className="export-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="export-title"
        onMouseDown={(event) => {
          event.stopPropagation();
        }}
      >
        <button
          className="dialog-close"
          type="button"
          onClick={onClose}
          aria-label="Close export review"
        >
          ×
        </button>
        <p className="heading-kicker">Privacy checkpoint</p>
        <h2 id="export-title">Review before exporting</h2>
        <p>
          Reports are generated locally. Environment values never enter the
          JobGlass model.
        </p>
        <div className="privacy-summary">
          <ShieldIcon />
          <div>
            <strong>Arguments are redacted by default</strong>
            <p>
              Paths, owners, commands, schedules, and source references can
              still identify this machine.
            </p>
          </div>
        </div>
        <label className="check-row">
          <input
            type="checkbox"
            checked={includeArguments}
            onChange={(event) => {
              onIncludeArguments(event.target.checked);
            }}
          />
          Include command arguments after review
        </label>
        <label className="check-row acknowledgement">
          <input
            type="checkbox"
            checked={reviewed}
            onChange={(event) => {
              onReviewed(event.target.checked);
            }}
          />
          I reviewed the privacy summary and intended destination
        </label>
        <div className="export-actions">
          {(["json", "csv", "html"] as const).map((format) => (
            <button
              className={format === "json" ? "button button-primary" : "button"}
              type="button"
              key={format}
              disabled={!reviewed}
              onClick={() => {
                onPrepare(format);
              }}
            >
              Prepare {format.toUpperCase()}
            </button>
          ))}
        </div>
        {status && (
          <p className="export-status" role="status">
            {status}
          </p>
        )}
      </section>
    </div>
  );
}

function LoadingState() {
  return (
    <div className="state-panel" role="status">
      <div className="scan-loader" aria-hidden="true">
        <span />
        <span />
        <span />
      </div>
      <h1>Reading native schedulers…</h1>
      <p>Definitions are read locally without elevation or mutation.</p>
    </div>
  );
}
function ErrorState({
  error,
  onRetry,
}: {
  error: string;
  onRetry: () => void;
}) {
  return (
    <div className="state-panel error-state" role="alert">
      <FindingIcon />
      <h1>Scheduler scan failed</h1>
      <p>{error}</p>
      <button className="button button-primary" type="button" onClick={onRetry}>
        Try again
      </button>
    </div>
  );
}
function EmptyState({ bundle }: { bundle: ScanBundle }) {
  return (
    <div className="state-panel">
      <GridIcon />
      <h1>No scheduled jobs found</h1>
      <p>
        JobGlass checked the scheduler scopes visible to the current user on{" "}
        {bundle.platform}.
      </p>
      <VisibilityPill bundle={bundle} />
    </div>
  );
}
function Outcome({ job }: { job: ScheduledJob }) {
  const outcome = evidence(job.lastOutcome)?.state;
  const enabled = evidence(job.enabled);
  if (enabled === "disabled")
    return <span className="outcome disabled">Disabled</span>;
  if (!outcome) return <span className="outcome unknown">Unknown</span>;
  return <span className={`outcome ${outcome}`}>{outcome}</span>;
}
function Severity({ severity }: { severity: Finding["severity"] }) {
  return <span className={`severity ${severity}`}>{severity}</span>;
}

function evidence<T>(field: Evidence<T>): T | null {
  return field.availability === "available" ? field.value : null;
}
function formatRun(field: ScheduledJob["nextRun"]): string {
  const run = evidence(field);
  if (!run) return "Unknown";
  return formatRunValue(run);
}
function formatRunValue(run: RunTime): string {
  return `${run.iso8601.replace("T", " ")} · ${run.timezoneBasis}`;
}
function formatDate(value: string): string {
  const date = new Date(value);
  return Number.isNaN(date.valueOf())
    ? value
    : new Intl.DateTimeFormat(undefined, {
        dateStyle: "medium",
        timeStyle: "short",
      }).format(date);
}
function schedulerLabel(scheduler: SchedulerKind | null): string {
  return scheduler === "windowsTaskScheduler"
    ? "Task Scheduler"
    : (scheduler ?? "Unknown");
}
function schedulerMonogram(scheduler: SchedulerKind | null): string {
  return scheduler === "windowsTaskScheduler"
    ? "TS"
    : (scheduler?.slice(0, 2).toUpperCase() ?? "?");
}

function jobUiKey(job: ScheduledJob): string {
  return `${job.id}:${job.nativeSource.provenance.sourceReference}`;
}

function compareJobsByNextRun(left: ScheduledJob, right: ScheduledJob): number {
  const leftRun = evidence(left.nextRun);
  const rightRun = evidence(right.nextRun);
  const leftTime = leftRun
    ? Date.parse(leftRun.iso8601)
    : Number.POSITIVE_INFINITY;
  const rightTime = rightRun
    ? Date.parse(rightRun.iso8601)
    : Number.POSITIVE_INFINITY;
  const safeLeft = Number.isNaN(leftTime) ? Number.POSITIVE_INFINITY : leftTime;
  const safeRight = Number.isNaN(rightTime)
    ? Number.POSITIVE_INFINITY
    : rightTime;
  return safeLeft === safeRight ? 0 : safeLeft - safeRight;
}

function unavailableReasonLabel(reason: string): string {
  return reason.replace(/([a-z])([A-Z])/g, "$1 $2").toLocaleLowerCase();
}

function evidenceSummary<T>(field: Evidence<T>): string {
  return field.availability === "available"
    ? String(field.value)
    : `Unavailable: ${unavailableReasonLabel(field.reason)}`;
}

function plural(count: number, singular: string): string {
  return count === 1 ? singular : `${singular}s`;
}

function findingJobNames(
  finding: Finding,
  jobs: readonly ScheduledJob[],
): string {
  const names = finding.jobIds.flatMap((id) => {
    const job = jobs.find((candidate) => candidate.id === id);
    const name = job ? evidence(job.displayName) : null;
    return name ? [name] : [];
  });
  if (names.length > 0) return names.join(", ");
  return finding.jobIds.length > 0
    ? `${String(finding.jobIds.length)} referenced ${plural(finding.jobIds.length, "job")} unavailable`
    : "Visibility finding";
}

async function loadBundle(): Promise<ScanBundle> {
  if (isTauri()) return invoke<ScanBundle>("scan_jobs");
  await Promise.resolve();
  if (new URLSearchParams(window.location.search).get("fixtureJobs") === "5000")
    return largeDemoBundle(5_000);
  return demoBundle;
}
async function renderExport(
  bundle: ScanBundle,
  format: ExportFormat,
  policy: ExportPolicy,
): Promise<string> {
  if (isTauri())
    return invoke<string>("render_export", {
      jobs: bundle.jobs,
      findings: bundle.findings,
      format,
      reviewed: policy.reviewed,
      includeArguments: policy.includeArguments,
    });
  return renderBrowserExport(bundle, format, policy);
}

function isTauri(): boolean {
  return "__TAURI_INTERNALS__" in window;
}
function download(content: string, format: ExportFormat) {
  if (
    typeof URL.createObjectURL !== "function" ||
    navigator.userAgent.includes("jsdom")
  )
    return;
  const type =
    format === "html"
      ? "text/html"
      : format === "csv"
        ? "text/csv"
        : "application/json";
  const url = URL.createObjectURL(new Blob([content], { type }));
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = `jobglass-report.${format}`;
  anchor.click();
  URL.revokeObjectURL(url);
}

function Logo() {
  return (
    <svg className="logo" viewBox="0 0 32 32" aria-hidden="true">
      <rect x="3" y="4" width="22" height="19" rx="5" />
      <path d="M9 10h10M9 14h7M9 18h4" />
      <path d="m17 22 3 5 9-13" />
    </svg>
  );
}
function GridIcon() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <rect x="3" y="3" width="7" height="7" rx="1" />
      <rect x="14" y="3" width="7" height="7" rx="1" />
      <rect x="3" y="14" width="7" height="7" rx="1" />
      <rect x="14" y="14" width="7" height="7" rx="1" />
    </svg>
  );
}
function FindingIcon() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <path d="M12 3 2.8 19h18.4L12 3Z" />
      <path d="M12 9v4m0 3h.01" />
    </svg>
  );
}
function ShieldIcon() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <path d="M12 3 4.5 6v5.5c0 4.6 3.2 7.9 7.5 9.5 4.3-1.6 7.5-4.9 7.5-9.5V6L12 3Z" />
      <path d="m8.5 12 2.2 2.2 4.8-5" />
    </svg>
  );
}
function SearchIcon() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <circle cx="11" cy="11" r="6" />
      <path d="m16 16 4 4" />
    </svg>
  );
}
function ExportIcon() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <path d="M12 3v12m0 0 4-4m-4 4-4-4" />
      <path d="M5 17v3h14v-3" />
    </svg>
  );
}
