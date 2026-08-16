// src/hooks/useBrowserDetection.test.tsx
import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { BrowserProvider } from "@/contexts/BrowserContext";
import { useBrowserDetection } from "./useBrowserDetection";
import type { DetectedBrowser } from "@/types";

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: invokeMock,
}));

vi.mock("@/contexts/ShredContext", () => ({
  useShred: () => ({ addLogEntry: vi.fn() }),
}));

const detectedBrowsers: DetectedBrowser[] = [
  {
    id: "chrome",
    name: "Chrome",
    icon: "",
    runningState: "closed",
    profiles: [
      { id: "p1", name: "Default", path: "C:\\chrome", size: 1, selected: false },
    ],
  },
];

function Probe() {
  useBrowserDetection();
  return null;
}

/** Re-renders the whole provider tree on demand, like unrelated app state churn. */
function Harness() {
  const [, setTick] = useState(0);
  return (
    <BrowserProvider>
      <Probe />
      <button type="button" onClick={() => setTick((t) => t + 1)}>
        rerender
      </button>
    </BrowserProvider>
  );
}

describe("useBrowserDetection lifecycle", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockResolvedValue(detectedBrowsers);
  });

  it("scans installed browsers exactly once when the app initializes", async () => {
    render(
      <BrowserProvider>
        <Probe />
      </BrowserProvider>
    );
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("detect_browsers"));
    expect(invokeMock).toHaveBeenCalledTimes(1);
  });

  it("does not rescan when the provider tree re-renders", async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await waitFor(() => expect(invokeMock).toHaveBeenCalledTimes(1));

    // The scan itself re-renders the provider when results arrive; a further
    // unrelated re-render must not trigger a second scan.
    await waitFor(() => expect(invokeMock).toHaveBeenCalledTimes(1));
    await act(async () => {
      await user.click(screen.getByRole("button", { name: "rerender" }));
    });
    expect(invokeMock).toHaveBeenCalledTimes(1);
  });
});
