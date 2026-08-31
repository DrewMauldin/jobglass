import { defineConfig, devices } from "@playwright/test";

export default defineConfig({
  testDir: "./tests/e2e",
  fullyParallel: true,
  forbidOnly: Boolean(process.env.CI),
  retries: process.env.CI ? 1 : 0,
  reporter: "list",
  use: {
    baseURL: "http://127.0.0.1:43177",
    trace: "retain-on-failure",
  },
  projects: [
    {
      name: "desktop",
      use: {
        ...devices["Desktop Chrome"],
        viewport: { width: 1180, height: 780 },
      },
    },
    {
      name: "minimum-window",
      use: {
        ...devices["Desktop Chrome"],
        viewport: { width: 720, height: 520 },
      },
    },
    {
      name: "intermediate-window",
      use: {
        ...devices["Desktop Chrome"],
        viewport: { width: 770, height: 700 },
      },
    },
  ],
  webServer: {
    command: "npm run preview -- --host 127.0.0.1 --port 43177 --strictPort",
    port: 43177,
    reuseExistingServer: false,
  },
});
