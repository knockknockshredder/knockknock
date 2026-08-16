// src/contexts/BrowserContext.test.tsx
import { act, render } from "@testing-library/react";
import { useEffect } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { BrowserProvider, useBrowser } from "./BrowserContext";
import type { DetectedBrowser } from "@/types";

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: invokeMock,
}));

vi.mock("@/contexts/ShredContext", () => ({
  useShred: () => ({ addLogEntry: vi.fn() }),
}));

const discovered: DetectedBrowser[] = [
  {
    id: "chrome",
    name: "Chrome",
    icon: "",
    runningState: "closed",
    profiles: [
      {
        id: "p1",
        name: "Default",
        path: "C:\\chrome\\default",
        size: 1,
        selected: false,
      },
    ],
  },
  {
    id: "firefox",
    name: "Firefox",
    icon: "",
    runningState: "closed",
    profiles: [
      {
        id: "p2",
        name: "default",
        path: "C:\\firefox",
        size: 1,
        selected: false,
      },
    ],
  },
];

let latest: ReturnType<typeof useBrowser> | null = null;

function Probe() {
  latest = useBrowser();
  return null;
}

/** Populates the context exactly like the one-time initial discovery. */
function DiscoverOnMount() {
  const { setBrowsers } = useBrowser();
  useEffect(() => {
    setBrowsers(discovered);
  }, [setBrowsers]);
  return <Probe />;
}

async function renderWithDiscoveredBrowsers() {
  const view = render(
    <BrowserProvider>
      <DiscoverOnMount />
    </BrowserProvider>
  );
  // Flush the discovery effect and the provider's watcher setup.
  await act(async () => {});
  return view;
}

function lightweightCalls() {
  return invokeMock.mock.calls.filter(
    ([command]) => command === "check_browser_running_states"
  );
}

describe("BrowserContext lightweight running-state watcher", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockResolvedValue([]);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("does not poll while no browsers are known", async () => {
    vi.useFakeTimers();
    render(
      <BrowserProvider>
        <Probe />
      </BrowserProvider>
    );
    await act(async () => {
      vi.advanceTimersByTime(15000);
    });
    expect(lightweightCalls()).toHaveLength(0);
    expect(invokeMock).not.toHaveBeenCalledWith("detect_browsers");
  });

  it("polls only the lightweight command after discovery — never full discovery again", async () => {
    vi.useFakeTimers();
    await renderWithDiscoveredBrowsers();

    await act(async () => {
      vi.advanceTimersByTime(5000);
    });
    expect(lightweightCalls()).toHaveLength(1);
    expect(invokeMock).not.toHaveBeenCalledWith("detect_browsers");

    // Repeated ticks keep using the lightweight command only. Each advance
    // is a separate act so in-flight responses drain between ticks (as they
    // do in real use, where invoke round-trips complete between intervals).
    await act(async () => {
      vi.advanceTimersByTime(5000);
    });
    await act(async () => {
      vi.advanceTimersByTime(5000);
    });
    await act(async () => {
      vi.advanceTimersByTime(5000);
    });
    expect(lightweightCalls().length).toBeGreaterThanOrEqual(3);
    expect(invokeMock).not.toHaveBeenCalledWith("detect_browsers");
  });

  it("transitions running state closed → running → closed across refreshes", async () => {
    vi.useFakeTimers();
    let runningNow = false;
    invokeMock.mockImplementation((command: string) => {
      if (command === "check_browser_running_states") {
        return Promise.resolve([
          { browserId: "chrome", state: runningNow ? "running" : "closed" },
          { browserId: "firefox", state: "closed" },
        ]);
      }
      return Promise.resolve(undefined);
    });

    await renderWithDiscoveredBrowsers();
    expect(latest!.browsers[0].runningState).toBe("closed");

    runningNow = true;
    await act(async () => {
      vi.advanceTimersByTime(5000);
    });
    expect(latest!.browsers[0].runningState).toBe("running");
    // Identities, profiles, and selection are preserved — only state flips.
    expect(latest!.browsers[0].id).toBe("chrome");
    expect(latest!.browsers[0].profiles).toHaveLength(1);
    expect(latest!.browsers[0].profiles[0].selected).toBe(false);

    runningNow = false;
    await act(async () => {
      vi.advanceTimersByTime(5000);
    });
    expect(latest!.browsers[0].runningState).toBe("closed");
  });

  it("never lets running-state requests overlap", async () => {
    vi.useFakeTimers();
    let resolveCheck!: (value: unknown) => void;
    invokeMock.mockImplementation((command: string) => {
      if (command === "check_browser_running_states") {
        return new Promise((resolve) => {
          resolveCheck = resolve;
        });
      }
      return Promise.resolve(undefined);
    });

    await renderWithDiscoveredBrowsers();

    // First tick starts and stays pending; later ticks must be skipped.
    await act(async () => {
      vi.advanceTimersByTime(5000);
    });
    await act(async () => {
      vi.advanceTimersByTime(10000);
    });
    expect(lightweightCalls()).toHaveLength(1);

    await act(async () => {
      resolveCheck([{ browserId: "chrome", state: "closed" }]);
    });
    await act(async () => {
      vi.advanceTimersByTime(5000);
    });
    expect(lightweightCalls()).toHaveLength(2);
  });

  it("keeps the previous displayed state when a refresh fails, and stops on unmount", async () => {
    vi.useFakeTimers();
    let failNext = false;
    invokeMock.mockImplementation((command: string) => {
      if (command === "check_browser_running_states") {
        return failNext
          ? Promise.reject(new Error("inspection failed"))
          : Promise.resolve([{ browserId: "chrome", state: "running" }]);
      }
      return Promise.resolve(undefined);
    });

    const view = await renderWithDiscoveredBrowsers();
    await act(async () => {
      vi.advanceTimersByTime(5000);
    });
    expect(latest!.browsers[0].runningState).toBe("running");

    // A failing refresh must not clear the displayed running state.
    failNext = true;
    await act(async () => {
      vi.advanceTimersByTime(5000);
    });
    expect(latest!.browsers[0].runningState).toBe("running");

    // Unmount stops the polling entirely.
    const callsBeforeUnmount = lightweightCalls().length;
    view.unmount();
    await act(async () => {
      vi.advanceTimersByTime(20000);
    });
    expect(lightweightCalls()).toHaveLength(callsBeforeUnmount);
  });
});
