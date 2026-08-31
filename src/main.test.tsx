import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { App } from "./App";
import { demoBundle, largeDemoBundle } from "./data/demo";
import { renderBrowserExport } from "./export";

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));

const loadDemo = () => Promise.resolve(demoBundle);

describe("JobGlass desktop experience", () => {
  it("builds a bounded 5,000-job browser fixture with unique evidence", () => {
    const bundle = largeDemoBundle(5_000);

    expect(bundle.jobs).toHaveLength(5_000);
    expect(new Set(bundle.jobs.map((job) => job.id)).size).toBe(5_000);
    expect(bundle.jobs.at(-1)?.displayName).toMatchObject({
      availability: "available",
      value: "Fixture job 05000",
    });
  });

  it("moves from loading to a populated, visibility-aware overview", async () => {
    render(<App loader={loadDemo} />);

    expect(screen.getByText("Reading native schedulers…")).toBeVisible();
    expect(
      await screen.findByRole("heading", { name: "Scheduled jobs" }),
    ).toBeVisible();
    expect(screen.getByText("Partial visibility")).toBeVisible();
    expect(screen.getByText("4 jobs")).toBeVisible();
  });

  it("filters jobs, switches to timeline, and opens evidence details", async () => {
    const user = userEvent.setup();
    render(<App loader={loadDemo} />);
    await screen.findByRole("heading", { name: "Scheduled jobs" });

    await user.type(
      screen.getByRole("searchbox", { name: "Search jobs" }),
      "cache",
    );
    expect(
      screen.getByRole("button", { name: /refresh cache/i }),
    ).toBeVisible();
    expect(
      screen.queryByRole("button", { name: /nightly backup/i }),
    ).toBeNull();

    await user.clear(screen.getByRole("searchbox", { name: "Search jobs" }));
    await user.type(
      screen.getByRole("searchbox", { name: "Search jobs" }),
      "*-*-* 03:30:00",
    );
    expect(
      screen.getByRole("button", { name: /nightly backup/i }),
    ).toBeVisible();

    await user.clear(screen.getByRole("searchbox", { name: "Search jobs" }));
    const viewControl = screen.getByRole("group", { name: "Job view" });
    const timelineButton = within(viewControl).getByRole("button", {
      name: "Timeline",
    });
    expect(timelineButton).toHaveAttribute("aria-pressed", "false");
    await user.click(timelineButton);
    expect(timelineButton).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByText("Next run unknown")).toBeVisible();

    await user.click(within(viewControl).getByRole("button", { name: "List" }));
    await user.click(screen.getByRole("button", { name: /nightly backup/i }));
    expect(
      screen.getByRole("heading", { name: "Evidence inspector" }),
    ).toBeVisible();
    expect(screen.getByText("Native source")).toBeVisible();
    expect(
      screen.getByText("backup.timer", { selector: ".native-id" }),
    ).toBeVisible();
    expect(screen.getByText("Owner")).toBeVisible();
    expect(screen.getByText("Privilege")).toBeVisible();
    expect(screen.getByText("Schedule expression")).toBeVisible();
    expect(screen.getByText("Scheduler timezone")).toBeVisible();
    expect(screen.getByText("Last outcome")).toBeVisible();
    expect(screen.getByText("Target service")).toBeVisible();
    expect(screen.getByText("Triggers")).toBeVisible();
    expect(screen.getByText("Dependencies")).toBeVisible();
    expect(screen.getByText("Parse warnings")).toBeVisible();
  });

  it("sorts jobs by the next observable run and exposes selection semantics", async () => {
    const user = userEvent.setup();
    render(<App loader={loadDemo} />);
    await screen.findByRole("heading", { name: "Scheduled jobs" });

    const jobList = document.querySelector(".job-list");
    expect(jobList).not.toBeNull();
    const rows = within(jobList as HTMLElement).getAllByRole("button");
    expect(rows.map((row) => row.getAttribute("aria-label"))).toEqual([
      "Refresh cache, cron",
      "Nightly backup, systemd",
      "Documents sync, launchd",
      "Fixture cleanup, Task Scheduler",
    ]);

    const backup = screen.getByRole("button", { name: /nightly backup/i });
    expect(backup).toHaveAttribute("aria-current", "true");
    const sync = screen.getByRole("button", { name: /documents sync/i });
    await user.click(sync);
    expect(sync).toHaveAttribute("aria-current", "true");
    expect(backup).not.toHaveAttribute("aria-current");
    expect(screen.getByText("exit code 1")).toBeVisible();
    expect(screen.getAllByText(/Unavailable: not reported/i)[0]).toBeVisible();
  });

  it("renders each browser export in the requested format", () => {
    const policy = { reviewed: true, includeArguments: false } as const;
    const json = renderBrowserExport(demoBundle, "json", policy);
    const csv = renderBrowserExport(demoBundle, "csv", policy);
    const html = renderBrowserExport(demoBundle, "html", policy);

    const parsed = JSON.parse(json) as {
      jobs: { arguments: { value: string[] } }[];
    };
    expect(parsed.jobs[0]?.arguments.value).toEqual(["<redacted>"]);
    expect(csv).toMatch(/^id,scheduler,name,/);
    expect(csv).toContain("Nightly backup");
    expect(csv).toContain("<redacted>");
    expect(csv).not.toContain("--incremental");
    expect(html).toMatch(/^<!doctype html>/i);
    expect(html).toContain("<table>");
    expect(html).toContain("Nightly backup");
    expect(html).toContain("&lt;redacted&gt;");
    expect(html).not.toContain("--incremental");
  });

  it("redacts argument-derived finding evidence and neutralises CSV formulas", () => {
    const secret = '=HYPERLINK("https://example.invalid")';
    const overlappingSecret = "private-marker";
    const escapedSecret = 'private-marker"secret\\path';
    const firstJob = demoBundle.jobs[0];
    if (!firstJob) throw new Error("demo fixture must contain a job");
    const bundle = {
      ...demoBundle,
      jobs: [
        {
          ...firstJob,
          displayName: {
            ...firstJob.displayName,
            availability: "available" as const,
            value: "=sensitive name",
          },
          arguments: {
            ...firstJob.arguments,
            availability: "available" as const,
            value: [secret, overlappingSecret, escapedSecret],
          },
        },
      ],
      findings: [
        {
          id: "finding_secret",
          code: "duplicateCommand",
          severity: "warning" as const,
          title: "Duplicate command",
          explanation: "Same executable and arguments",
          jobIds: [firstJob.id],
          evidence: [
            `command: /usr/local/sbin/backup ${secret}`,
            `debug arguments: [${JSON.stringify(escapedSecret)}, ${JSON.stringify(overlappingSecret)}]`,
          ],
        },
      ],
    };

    const json = renderBrowserExport(bundle, "json", {
      reviewed: true,
      includeArguments: false,
    });
    const csv = renderBrowserExport(bundle, "csv", {
      reviewed: true,
      includeArguments: false,
    });
    const parsed = JSON.parse(json) as {
      findings: { evidence: string[] }[];
    };

    expect(parsed.findings[0]?.evidence).toEqual([
      "command: /usr/local/sbin/backup <redacted>",
      "debug arguments: [<redacted>, <redacted>]",
    ]);
    expect(json).not.toContain(overlappingSecret);
    expect(json).not.toContain(JSON.stringify(escapedSecret));
    expect(csv).toContain("'=sensitive name");
    expect(csv).not.toContain('"=sensitive name"');
  });

  it("blocks export until privacy review is acknowledged", async () => {
    const user = userEvent.setup();
    const exporter = vi.fn(() => Promise.resolve("report"));
    render(<App loader={loadDemo} exporter={exporter} />);
    await screen.findByRole("heading", { name: "Scheduled jobs" });

    await user.click(screen.getByRole("button", { name: "Export report" }));
    const prepare = screen.getByRole("button", { name: "Prepare JSON" });
    expect(prepare).toBeDisabled();
    expect(
      screen.getByText(/arguments are redacted by default/i),
    ).toBeVisible();

    await user.click(
      screen.getByRole("checkbox", { name: /reviewed the privacy summary/i }),
    );
    await user.click(prepare);
    expect(exporter).toHaveBeenCalledWith(
      demoBundle,
      "json",
      expect.objectContaining({ reviewed: true, includeArguments: false }),
    );
    expect(
      await screen.findByText("JSON report prepared locally."),
    ).toBeVisible();
  });

  it("moves focus into the export dialog and restores it on Escape", async () => {
    const user = userEvent.setup();
    render(<App loader={loadDemo} />);
    await screen.findByRole("heading", { name: "Scheduled jobs" });

    const exportButton = screen.getByRole("button", { name: "Export report" });
    await user.click(exportButton);
    expect(
      screen.getByRole("button", { name: "Close export review" }),
    ).toHaveFocus();
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(exportButton).toHaveFocus();
  });

  it("keeps keyboard focus inside the export review", async () => {
    const user = userEvent.setup();
    render(<App loader={loadDemo} />);
    await screen.findByRole("heading", { name: "Scheduled jobs" });

    await user.click(screen.getByRole("button", { name: "Export report" }));
    const close = screen.getByRole("button", { name: "Close export review" });
    const review = screen.getByRole("checkbox", {
      name: /reviewed the privacy summary/i,
    });

    expect(close).toHaveFocus();
    await user.keyboard("{Shift>}{Tab}{/Shift}");
    expect(review).toHaveFocus();
    await user.keyboard("{Tab}");
    expect(close).toHaveFocus();
  });

  it("shows explicit empty and error states", async () => {
    const { rerender } = render(
      <App
        loader={() =>
          Promise.resolve({ ...demoBundle, jobs: [], findings: [] })
        }
      />,
    );
    expect(
      await screen.findByRole("heading", { name: "No scheduled jobs found" }),
    ).toBeVisible();

    rerender(<App loader={() => Promise.reject(new Error("scan failed"))} />);
    expect(
      await screen.findByRole("heading", { name: "Scheduler scan failed" }),
    ).toBeVisible();
  });

  it("keeps findings reachable when no jobs were parsed", async () => {
    const user = userEvent.setup();
    const finding = demoBundle.findings[0];
    if (!finding) throw new Error("demo fixture must contain a finding");
    render(
      <App
        loader={() =>
          Promise.resolve({ ...demoBundle, jobs: [], findings: [finding] })
        }
      />,
    );

    await screen.findByRole("heading", { name: "No scheduled jobs found" });
    await user.click(screen.getByRole("button", { name: /findings 1/i }));
    expect(screen.getByRole("heading", { name: "Findings" })).toBeVisible();
    expect(screen.getByRole("heading", { name: finding.title })).toBeVisible();
    expect(screen.getByText("1 referenced job unavailable")).toBeVisible();
    expect(screen.getByText(finding.evidence[0] ?? "")).toBeVisible();
  });

  it("navigates findings, resets empty filters, and selects a theme", async () => {
    const user = userEvent.setup();
    render(<App loader={loadDemo} />);
    await screen.findByRole("heading", { name: "Scheduled jobs" });

    const overview = screen.getByRole("button", { name: "Overview" });
    const findings = screen.getByRole("button", { name: /findings 3/i });
    expect(overview).toHaveAttribute("aria-current", "page");
    expect(findings).not.toHaveAttribute("aria-current");
    await user.click(findings);
    expect(findings).toHaveAttribute("aria-current", "page");
    expect(overview).not.toHaveAttribute("aria-current");
    expect(
      screen.getByRole("heading", { name: "Last run failed" }),
    ).toBeVisible();
    expect(screen.getByText("Visibility finding")).toBeVisible();
    expect(screen.getByText("/etc/cron.d/private")).toBeVisible();

    await user.click(overview);
    expect(overview).toHaveAttribute("aria-current", "page");
    await user.selectOptions(
      screen.getByRole("combobox", { name: "Filter by scheduler" }),
      "launchd",
    );
    await user.type(
      screen.getByRole("searchbox", { name: "Search jobs" }),
      "no such job",
    );
    expect(
      screen.getByRole("heading", { name: "No jobs match these filters" }),
    ).toBeVisible();
    await user.click(screen.getByRole("button", { name: "Clear filters" }));
    expect(screen.getByText("4 jobs")).toBeVisible();

    await user.selectOptions(
      screen.getByRole("combobox", { name: "Theme" }),
      "dark",
    );
    expect(document.documentElement.dataset.theme).toBe("dark");
  });

  it("shows a clean findings state and complete visibility", async () => {
    render(
      <App
        loader={() =>
          Promise.resolve({
            ...demoBundle,
            findings: [],
            visibility: demoBundle.visibility.map((item) => ({
              ...item,
              status: "complete" as const,
            })),
          })
        }
      />,
    );
    await screen.findByRole("heading", { name: "Scheduled jobs" });
    expect(screen.getByText("Full queried visibility")).toBeVisible();
    await userEvent.click(screen.getByRole("button", { name: /findings 0/i }));
    expect(screen.getByRole("heading", { name: "No findings" })).toBeVisible();
  });

  it("retries a failed scan", async () => {
    const user = userEvent.setup();
    const loader = vi
      .fn()
      .mockRejectedValueOnce(new Error("temporary failure"))
      .mockResolvedValueOnce(demoBundle);
    render(<App loader={loader} />);

    expect(
      await screen.findByRole("heading", { name: "Scheduler scan failed" }),
    ).toBeVisible();
    await user.click(screen.getByRole("button", { name: "Try again" }));
    expect(
      await screen.findByRole("heading", { name: "Scheduled jobs" }),
    ).toBeVisible();
    expect(loader).toHaveBeenCalledTimes(2);
  });

  it("reports non-Error scan and export failures safely", async () => {
    const user = userEvent.setup();
    const loader = vi.fn().mockRejectedValue("private scan failure");
    const { unmount } = render(<App loader={loader} />);
    expect(
      await screen.findByRole("heading", { name: "Scheduler scan failed" }),
    ).toBeVisible();
    expect(screen.getByText("Unknown scan error")).toBeVisible();
    unmount();

    const exporter = vi.fn().mockRejectedValue("private export failure");
    render(<App loader={loadDemo} exporter={exporter} />);
    await screen.findByRole("heading", { name: "Scheduled jobs" });
    await user.click(screen.getByRole("button", { name: "Export report" }));
    await user.click(
      screen.getByRole("checkbox", { name: /reviewed the privacy summary/i }),
    );
    await user.click(screen.getByRole("button", { name: "Prepare HTML" }));
    expect(await screen.findByText("Report preparation failed.")).toBeVisible();
  });

  it("paginates a large bundle without changing canonical job IDs", async () => {
    const jobs = largeDemoBundle(101).jobs;
    const user = userEvent.setup();
    render(<App loader={() => Promise.resolve({ ...demoBundle, jobs })} />);

    await screen.findByRole("heading", { name: "Scheduled jobs" });
    expect(
      screen.getByRole("button", { name: /Show 25 more jobs/ }),
    ).toHaveTextContent("Showing 25 of 101");
    for (let page = 0; page < 4; page += 1) {
      await user.click(
        screen.getByRole("button", { name: /Show 25 more jobs/ }),
      );
    }
    expect(
      screen.queryByRole("button", { name: /Show 25 more jobs/ }),
    ).toBeNull();
  });

  it("includes arguments only after review and reports export failures", async () => {
    const user = userEvent.setup();
    const exporter = vi.fn(() => Promise.reject(new Error("export failed")));
    render(<App loader={loadDemo} exporter={exporter} />);
    await screen.findByRole("heading", { name: "Scheduled jobs" });

    await user.click(screen.getByRole("button", { name: "Export report" }));
    await user.click(
      screen.getByRole("checkbox", { name: /include command arguments/i }),
    );
    await user.click(
      screen.getByRole("checkbox", { name: /reviewed the privacy summary/i }),
    );
    await user.click(screen.getByRole("button", { name: "Prepare CSV" }));
    expect(exporter).toHaveBeenCalledWith(
      demoBundle,
      "csv",
      expect.objectContaining({ reviewed: true, includeArguments: true }),
    );
    expect(await screen.findByText("export failed")).toBeVisible();
    await user.click(
      screen.getByRole("button", { name: "Close export review" }),
    );
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("uses the two-command native IPC boundary when Tauri is present", async () => {
    invokeMock.mockImplementation((command: string) =>
      Promise.resolve(command === "scan_jobs" ? demoBundle : "native report"),
    );
    Object.defineProperty(window, "__TAURI_INTERNALS__", {
      configurable: true,
      value: {},
    });
    const user = userEvent.setup();
    render(<App />);

    await screen.findByRole("heading", { name: "Scheduled jobs" });
    expect(invokeMock).toHaveBeenCalledWith("scan_jobs");
    await user.click(screen.getByRole("button", { name: "Export report" }));
    await user.click(
      screen.getByRole("checkbox", { name: /reviewed the privacy summary/i }),
    );
    await user.click(screen.getByRole("button", { name: "Prepare HTML" }));
    expect(invokeMock).toHaveBeenCalledWith(
      "render_export",
      expect.objectContaining({ format: "html", reviewed: true }),
    );
    Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
  });
});
