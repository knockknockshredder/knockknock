// src/components/browser/BrowserCard.test.tsx
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { BrowserCard } from "./BrowserCard";
import type { BrowserRunningStatus, DetectedBrowser } from "@/types";

function browser(runningState: BrowserRunningStatus): DetectedBrowser {
  return {
    id: "chrome",
    name: "Chrome",
    icon: "",
    runningState,
    profiles: [],
  };
}

describe("BrowserCard running indicator", () => {
  it("shows an accessible running warning icon with the exact tooltip copy when running", async () => {
    const user = userEvent.setup();
    render(<BrowserCard browser={browser("running")} />);

    const trigger = screen.getByRole("button", {
      name: "Chrome is currently running",
    });
    expect(trigger).toBeInTheDocument();

    await user.hover(trigger);
    await waitFor(() =>
      expect(
        screen.getByText(
          "Chrome is currently running. Close it before deleting browser data."
        )
      ).toBeInTheDocument()
    );
  });

  it("renders no running warning icon when the browser is closed", () => {
    render(<BrowserCard browser={browser("closed")} />);
    expect(
      screen.queryByRole("button", { name: "Chrome is currently running" })
    ).not.toBeInTheDocument();
  });

  it("shows an unavailable warning when the browser state is unknown", async () => {
    const user = userEvent.setup();
    render(<BrowserCard browser={browser("unknown")} />);

    const trigger = screen.getByRole("button", {
      name: "Could not confirm that Chrome is closed",
    });
    await user.hover(trigger);
    await waitFor(() =>
      expect(
        screen.getByText(
          "KnockKnock could not confirm that Chrome is closed. Browser data deletion is unavailable."
        )
      ).toBeInTheDocument()
    );
  });
});
