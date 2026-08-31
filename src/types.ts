export type SchedulerKind =
  "launchd" | "cron" | "systemd" | "windowsTaskScheduler";

export type JobScope = "user" | "system";
export type PrivilegeLevel = "standardUser" | "elevated" | "system" | "unknown";
export type EnabledState = "enabled" | "disabled" | "unknown";
export type UnavailableReason =
  | "notReported"
  | "permissionDenied"
  | "unsupported"
  | "notApplicable"
  | "parseFailure"
  | "sourceMissing";

export interface Provenance {
  readonly adapter: SchedulerKind;
  readonly sourceReference: string;
  readonly detail?: string;
}

export type Evidence<T> =
  | {
      readonly availability: "available";
      readonly value: T;
      readonly provenance: Provenance;
    }
  | {
      readonly availability: "unavailable";
      readonly reason: UnavailableReason;
      readonly provenance: Provenance;
    };

export interface ScheduleSpec {
  readonly kind:
    "calendar" | "interval" | "event" | "boot" | "manual" | "composite";
  readonly nativeExpression: string;
}

export interface TimezoneBasis {
  readonly name: string;
  readonly source: string;
}

export interface RunTime {
  readonly iso8601: string;
  readonly timezoneBasis: string;
}

export interface LastOutcome {
  readonly state: "success" | "failed" | "running" | "unknown";
  readonly nativeCode: number | null;
  readonly explanation: string;
}

export interface NativeSource {
  readonly sourceType: string;
  readonly reference: string;
}

export interface Trigger {
  readonly kind: string;
  readonly expression: string;
  readonly explanation: string;
}

export interface ParseWarning {
  readonly code: string;
  readonly message: string;
  readonly sourceReference: string;
}

export interface ScheduledJob {
  readonly schemaVersion: "1.0";
  readonly id: string;
  readonly scheduler: Evidence<SchedulerKind>;
  readonly nativeIdentifier: Evidence<string>;
  readonly displayName: Evidence<string>;
  readonly owner: Evidence<string>;
  readonly scope: Evidence<JobScope>;
  readonly privilegeLevel: Evidence<PrivilegeLevel>;
  readonly enabled: Evidence<EnabledState>;
  readonly schedule: Evidence<ScheduleSpec>;
  readonly scheduleExplanation: Evidence<string>;
  readonly timezoneBasis: Evidence<TimezoneBasis>;
  readonly nextRun: Evidence<RunTime>;
  readonly lastRun: Evidence<RunTime>;
  readonly lastOutcome: Evidence<LastOutcome>;
  readonly executable: Evidence<string>;
  readonly arguments: Evidence<readonly string[]>;
  readonly workingDirectory: Evidence<string>;
  readonly environmentKeys: Evidence<readonly string[]>;
  readonly nativeSource: Evidence<NativeSource>;
  readonly triggers: Evidence<readonly Trigger[]>;
  readonly dependencies: Evidence<readonly string[]>;
  readonly targetService: Evidence<string>;
  readonly parseWarnings: readonly ParseWarning[];
}
