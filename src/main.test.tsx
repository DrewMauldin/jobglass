import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { App } from "./App";

describe("foundation empty state", () => {
  it("explains that absent scheduler evidence is not invented", () => {
    render(<App />);
    expect(
      screen.getByRole("heading", { name: "No scheduler evidence loaded" }),
    ).toBeVisible();
    expect(screen.getByText(/reports only evidence visible/i)).toBeVisible();
  });
});
