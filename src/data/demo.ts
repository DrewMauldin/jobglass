import type {
  EnabledState,
  Evidence,
  Finding,
  JobScope,
  Provenance,
  ScanBundle,
  ScheduledJob,
  SchedulerKind,
} from "../types";

function provenance(
  adapter: SchedulerKind,
  sourceReference: string,
): Provenance {
  return { adapter, sourceReference };
}

function available<T>(value: T, source: Provenance): Evidence<T> {
  return { availability: "available", value, provenance: source };
}

function unavailable<T>(source: Provenance): Evidence<T> {
  return {
    availability: "unavailable",
    reason: "notReported",
    provenance: source,
  };
}

interface DemoJobInput {
  id: string;
  scheduler: SchedulerKind;
  nativeIdentifier: string;
  name: string;
  scope: JobScope;
  enabled?: EnabledState;
  source: string;
  executable: string;
  arguments: readonly string[];
  schedule: string;
  scheduleExplanation: string;
  nextRun?: string;
  lastRun?: string;
  outcome?: "success" | "failed" | "unknown";
  environmentKeys?: readonly string[];
  targetService?: string;
}

function demoJob(input: DemoJobInput): ScheduledJob {
  const source = provenance(input.scheduler, input.source);
  return {
    schemaVersion: "1.0",
    id: input.id,
    scheduler: available(input.scheduler, source),
    nativeIdentifier: available(input.nativeIdentifier, source),
    displayName: available(input.name, source),
    owner: available(
      input.scope === "user" ? "current user" : "system",
      source,
    ),
    scope: available(input.scope, source),
    privilegeLevel: available(
      input.scope === "user" ? "standardUser" : "system",
      source,
    ),
    enabled: available(input.enabled ?? "enabled", source),
    schedule: available(
      { kind: "calendar", nativeExpression: input.schedule },
      source,
    ),
    scheduleExplanation: available(input.scheduleExplanation, source),
    timezoneBasis: available(
      { name: "Australia/Brisbane", source: "native scheduler" },
      source,
    ),
    nextRun: input.nextRun
      ? available(
          { iso8601: input.nextRun, timezoneBasis: "Australia/Brisbane" },
          source,
        )
      : unavailable(source),
    lastRun: input.lastRun
      ? available(
          { iso8601: input.lastRun, timezoneBasis: "Australia/Brisbane" },
          source,
        )
      : unavailable(source),
    lastOutcome: input.outcome
      ? available(
          {
            state: input.outcome,
            nativeCode: input.outcome === "success" ? 0 : null,
            explanation: `Native scheduler reports ${input.outcome}`,
          },
          source,
        )
      : unavailable(source),
    executable: available(input.executable, source),
    arguments: available(input.arguments, source),
    workingDirectory: available(
      input.scope === "user" ? "/Users/example" : "/",
      source,
    ),
    environmentKeys: available(input.environmentKeys ?? ["PATH"], source),
    nativeSource: available(
      { sourceType: "nativeDefinition", reference: input.source },
      source,
    ),
    triggers: available(
      [
        {
          kind: "calendar",
          expression: input.schedule,
          explanation: input.scheduleExplanation,
        },
      ],
      source,
    ),
    dependencies: available([], source),
    targetService: input.targetService
      ? available(input.targetService, source)
      : unavailable(source),
    parseWarnings: [],
  };
}

const jobs: readonly ScheduledJob[] = [
  demoJob({
    id: "job_backup",
    scheduler: "systemd",
    nativeIdentifier: "backup.timer",
    name: "Nightly backup",
    scope: "system",
    source: "systemctl show backup.timer",
    executable: "/usr/local/sbin/backup",
    arguments: ["--incremental"],
    schedule: "*-*-* 03:30:00",
    scheduleExplanation: "Every day at 3:30 am",
    nextRun: "2026-09-01T03:30:00+10:00",
    lastRun: "2026-08-31T03:30:00+10:00",
    outcome: "success",
    targetService: "backup.service",
  }),
  demoJob({
    id: "job_cache",
    scheduler: "cron",
    nativeIdentifier: "user-crontab:12",
    name: "Refresh cache",
    scope: "user",
    source: "user crontab:12",
    executable: "/usr/local/bin/refresh-cache",
    arguments: ["--quiet"],
    schedule: "*/15 * * * *",
    scheduleExplanation: "Every 15 minutes",
    nextRun: "2026-08-31T12:15:00+10:00",
    lastRun: "2026-08-31T12:00:00+10:00",
    outcome: "success",
  }),
  demoJob({
    id: "job_sync",
    scheduler: "launchd",
    nativeIdentifier: "com.example.documents-sync",
    name: "Documents sync",
    scope: "user",
    source: "~/Library/LaunchAgents/com.example.documents-sync.plist",
    executable: "/usr/local/bin/sync-documents",
    arguments: ["--local-only"],
    schedule: "StartInterval=3600",
    scheduleExplanation: "Every hour while launchd is available",
    lastRun: "2026-08-31T11:04:00+10:00",
    outcome: "failed",
    environmentKeys: ["HOME", "PATH"],
  }),
  demoJob({
    id: "job_cleanup",
    scheduler: "windowsTaskScheduler",
    nativeIdentifier: "\\Maintenance\\Fixture cleanup",
    name: "Fixture cleanup",
    scope: "user",
    enabled: "disabled",
    source: "Task Scheduler XML",
    executable: "C:\\Tools\\cleanup.exe",
    arguments: ["--older-than", "30d"],
    schedule: "2026-09-01T01:00:00+10:00",
    scheduleExplanation: "Every day at 1:00 am",
  }),
];

const findings: readonly Finding[] = [
  {
    id: "finding_failure",
    code: "lastRunFailed",
    severity: "error",
    title: "Last run failed",
    explanation: "launchctl reported a non-zero last exit code.",
    jobIds: ["job_sync"],
    evidence: ["exit code 1"],
  },
  {
    id: "finding_disabled",
    code: "disabledJob",
    severity: "info",
    title: "Job is disabled",
    explanation: "Task Scheduler reports this job as disabled.",
    jobIds: ["job_cleanup"],
    evidence: [],
  },
  {
    id: "finding_visibility",
    code: "permissionLimited",
    severity: "warning",
    title: "System cron visibility is limited",
    explanation:
      "One protected cron directory was not readable without elevation.",
    jobIds: [],
    evidence: ["/etc/cron.d/private"],
  },
];

export const demoBundle: ScanBundle = {
  schemaVersion: "1.0",
  scanId: "demo-2026-08-31",
  generatedAt: "2026-08-31T12:08:00+10:00",
  platform: "Sample evidence",
  jobs,
  findings,
  warnings: [],
  visibility: [
    {
      scheduler: "launchd",
      scope: "user",
      status: "complete",
      explanation: "User launch agents were readable.",
    },
    {
      scheduler: "cron",
      scope: "system",
      status: "permissionLimited",
      explanation: "One protected cron directory was not readable.",
    },
  ],
  sampleData: true,
};

export function largeDemoBundle(count: number): ScanBundle {
  if (!Number.isInteger(count) || count < 1 || count > 10_000) {
    throw new RangeError(
      "fixture job count must be an integer from 1 to 10,000",
    );
  }

  const largeJobs = Array.from({ length: count }, (_, index) => {
    const template = jobs[index % jobs.length];
    if (!template) throw new Error("browser fixture templates are unavailable");
    const suffix = String(index + 1).padStart(5, "0");
    const sourceReference = `browser fixture:${suffix}`;
    const source = { ...template.nativeSource.provenance, sourceReference };
    return {
      ...template,
      id: `fixture_job_${suffix}`,
      nativeIdentifier: available(`fixture.job.${suffix}`, source),
      displayName: available(`Fixture job ${suffix}`, source),
      nativeSource: available(
        { sourceType: "nativeDefinition" as const, reference: sourceReference },
        source,
      ),
    };
  });

  return {
    ...demoBundle,
    scanId: `browser-fixture-${String(count)}`,
    jobs: largeJobs,
    findings: [],
  };
}
