import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { App } from "./App";
import { demoBundle } from "./data/demo";

const loadDemo = () => Promise.resolve(demoBundle);

describe("JobGlass desktop experience", () => {
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
});
