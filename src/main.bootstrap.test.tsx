import { screen } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";

vi.mock("./App", () => ({
  App: () => <main>Booted JobGlass</main>,
}));

beforeEach(() => {
  vi.resetModules();
  document.body.innerHTML = "";
});

it("boots JobGlass into the required root element", async () => {
  document.body.innerHTML = '<div id="root"></div>';

  await import("./main");

  expect(await screen.findByText("Booted JobGlass")).toBeVisible();
});

it("fails closed when the application root is missing", async () => {
  await expect(import("./main")).rejects.toThrow(
    "JobGlass root element is missing",
  );
});
