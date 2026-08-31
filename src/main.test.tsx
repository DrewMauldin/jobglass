import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { App } from "./App";
import { demoBundle, largeDemoBundle } from "./data/demo";

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
    await user.click(screen.getByRole("tab", { name: "Timeline" }));
    expect(screen.getByText("Next run unknown")).toBeVisible();

    await user.click(screen.getByRole("tab", { name: "List" }));
    await user.click(screen.getByRole("button", { name: /nightly backup/i }));
    expect(
      screen.getByRole("heading", { name: "Evidence inspector" }),
    ).toBeVisible();
    expect(screen.getByText("Native source")).toBeVisible();
    expect(
      screen.getByText("backup.timer", { selector: ".native-id" }),
    ).toBeVisible();
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

  it("navigates findings, resets empty filters, and selects a theme", async () => {
    const user = userEvent.setup();
    render(<App loader={loadDemo} />);
    await screen.findByRole("heading", { name: "Scheduled jobs" });

    await user.click(screen.getByRole("button", { name: /findings 3/i }));
    expect(
      screen.getByRole("heading", { name: "Last run failed" }),
    ).toBeVisible();
    expect(screen.getByText("Visibility finding")).toBeVisible();

    await user.click(screen.getByRole("button", { name: "Overview" }));
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
      screen.getByRole("button", { name: /Show 100 more jobs/ }),
    ).toHaveTextContent("Showing 100 of 101");
    await user.click(
      screen.getByRole("button", { name: /Show 100 more jobs/ }),
    );
    expect(
      screen.queryByRole("button", { name: /Show 100 more jobs/ }),
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
