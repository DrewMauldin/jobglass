import { expect, test, type Page } from "@playwright/test";
import axe from "axe-core";

interface AxeWindow extends Window {
  axe: {
    run(): Promise<{
      violations: { id: string; impact: string | null }[];
    }>;
  };
}

interface MetricsWindow extends Window {
  jobglassLongTasks: number[];
}

async function expectNoLongTasks(page: Page, operation: string) {
  await page.waitForTimeout(100);
  const longTasks = await page.evaluate(
    () => (window as unknown as MetricsWindow).jobglassLongTasks,
  );
  expect(longTasks, `${operation} exceeded the 50 ms long-task budget`).toEqual(
    [],
  );
  await page.evaluate(() => {
    (window as unknown as MetricsWindow).jobglassLongTasks = [];
  });
}

test("is keyboard-operable, responsive, and free of serious axe findings", async ({
  page,
}) => {
  await page.goto("/");
  await expect(
    page.getByRole("heading", { name: "Scheduled jobs" }),
  ).toBeVisible();
  const hasHorizontalOverflow = await page.evaluate(
    () => document.documentElement.scrollWidth > window.innerWidth,
  );
  expect(hasHorizontalOverflow).toBe(false);

  await page.keyboard.press("Tab");
  await expect(
    page.getByRole("link", { name: "Skip to scheduled jobs" }),
  ).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(page).toHaveURL(/#main-content$/);
  await expect(page.locator("#main-content")).toBeFocused();

  await page.addScriptTag({ content: axe.source });
  const lightViolations = await page.evaluate(async () => {
    const result = await (window as unknown as AxeWindow).axe.run();
    return result.violations.filter(
      ({ impact }) => impact === "serious" || impact === "critical",
    );
  });
  expect(lightViolations).toEqual([]);

  await page.getByRole("combobox", { name: "Theme" }).selectOption("dark");
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");

  await page.getByRole("button", { name: "Export report" }).click();
  await expect(
    page.getByRole("dialog", { name: "Review before exporting" }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Close export review" }),
  ).toBeFocused();
  const dialogViolations = await page.evaluate(async () => {
    const result = await (window as unknown as AxeWindow).axe.run();
    return result.violations.filter(
      ({ impact }) => impact === "serious" || impact === "critical",
    );
  });
  expect(dialogViolations).toEqual([]);
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await expect(
    page.getByRole("button", { name: "Export report" }),
  ).toBeFocused();

  const darkViolations = await page.evaluate(async () => {
    const result = await (window as unknown as AxeWindow).axe.run();
    return result.violations.filter(
      ({ impact }) => impact === "serious" || impact === "critical",
    );
  });
  expect(darkViolations).toEqual([]);
});

test("keeps 5,000-job interactions below the long-task budget", async ({
  page,
}) => {
  await page.addInitScript(() => {
    const metricsWindow = window as unknown as MetricsWindow;
    metricsWindow.jobglassLongTasks = [];
    const observer = new PerformanceObserver((list) => {
      metricsWindow.jobglassLongTasks.push(
        ...list.getEntries().map((entry) => entry.duration),
      );
    });
    observer.observe({ entryTypes: ["longtask"] });
  });
  await page.goto("/?fixtureJobs=5000");
  await expect(page.getByText("5000 jobs", { exact: true })).toBeVisible();
  await page.evaluate(() => {
    (window as unknown as MetricsWindow).jobglassLongTasks = [];
  });

  await page.getByRole("button", { name: "Timeline" }).click();
  await expect(page.getByRole("button", { name: "Timeline" })).toHaveAttribute(
    "aria-pressed",
    "true",
  );
  await expectNoLongTasks(page, "timeline view");
  await page.getByRole("button", { name: "List" }).click();
  await expectNoLongTasks(page, "list view");
  await page.getByRole("button", { name: "Export report" }).click();
  await expect(
    page.getByRole("dialog", { name: "Review before exporting" }),
  ).toBeVisible();
  await expectNoLongTasks(page, "export dialog open");
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await expectNoLongTasks(page, "export dialog close");

  await page
    .getByRole("searchbox", { name: "Search jobs" })
    .fill("Fixture job 05000");
  await expect(page.getByText("1 job", { exact: true })).toBeVisible();
  await expectNoLongTasks(page, "search");
});
