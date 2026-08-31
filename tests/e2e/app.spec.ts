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

async function contrastRatio(
  page: Page,
  foregroundSelector: string,
  backgroundSelector: string,
) {
  return page.evaluate(
    ([foregroundSelector, backgroundSelector]) => {
      const luminance = (color: string) => {
        const channels = color
          .match(/[\d.]+/gu)
          ?.slice(0, 3)
          .map(Number);
        if (channels?.length !== 3)
          throw new Error(`could not parse color ${color}`);
        const [red, green, blue] = channels.map((channel) => {
          const value = channel / 255;
          return value <= 0.04045
            ? value / 12.92
            : ((value + 0.055) / 1.055) ** 2.4;
        });
        return 0.2126 * red + 0.7152 * green + 0.0722 * blue;
      };
      const foreground = document.querySelector(foregroundSelector);
      const background = document.querySelector(backgroundSelector);
      if (!foreground || !background)
        throw new Error("contrast target missing");
      const foregroundLuminance = luminance(getComputedStyle(foreground).color);
      const backgroundLuminance = luminance(
        getComputedStyle(background).backgroundColor,
      );
      return (
        (Math.max(foregroundLuminance, backgroundLuminance) + 0.05) /
        (Math.min(foregroundLuminance, backgroundLuminance) + 0.05)
      );
    },
    [foregroundSelector, backgroundSelector] as const,
  );
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
  expect(
    await contrastRatio(page, ".summary-grid small", ".summary-grid article"),
  ).toBeGreaterThanOrEqual(4.5);

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
  expect(
    await contrastRatio(page, ".privacy-summary p", ".privacy-summary"),
  ).toBeGreaterThanOrEqual(4.5);
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await expect(
    page.getByRole("button", { name: "Export report" }),
  ).toBeFocused();

  await page.getByRole("combobox", { name: "Theme" }).selectOption("dark");
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");

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
  const timelineOverflows = await page
    .locator(".timeline-time")
    .evaluateAll((elements) =>
      elements.some((element) => element.scrollWidth > element.clientWidth),
    );
  expect(timelineOverflows).toBe(false);
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
