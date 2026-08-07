// src/sections/ShredSection.test.tsx
import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ShredProvider, useShred } from "@/contexts/ShredContext";
import { ShredSection, computeProgressPercent } from "./ShredSection";

const { invokeMock, listenMock, browserState } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  listenMock: vi.fn(),
  browserState: {
    browsers: [] as Array<{
      id: string;
      name: string;
      icon: string;
      isRunning: boolean;
      profiles: Array<{
        id: string;
        name: string;
        path: string;
        size: number;
        selected: boolean;
      }>;
    }>,
    rescanBrowsers: vi.fn(),
  },
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: invokeMock,
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: listenMock,
}));

vi.mock("@/contexts/BrowserContext", () => ({
  useBrowser: () => ({
    getSelectedCount: () =>
      browserState.browsers.reduce(
        (sum, b) => sum + b.profiles.filter((p) => p.selected).length,
        0
      ),
    browsers: browserState.browsers,
    rescanBrowsers: browserState.rescanBrowsers,
  }),
}));

vi.mock("@/contexts/SettingsContext", () => ({
  useSettings: () => ({ logObfuscation: "none", autoClearLog: false }),
}));

let latest: ReturnType<typeof useShred>;

function Probe() {
  latest = useShred();
  return null;
}

function target(path: string) {
  return { path, kind: "file" as const };
}

function metadata(path: string) {
  return {
    path,
    kind: "file" as const,
    availability: "ready" as const,
    reason: null,
    name: path,
    size: 1,
  };
}

function readyFile(path: string) {
  return {
    path,
    name: path,
    size: 1,
    kind: "file" as const,
    is_shortcut: false,
    shortcut_target: null,
  };
}

describe("ShredSection executeShred", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    listenMock.mockReset();
    listenMock.mockResolvedValue(() => {});
    browserState.browsers = [];
    browserState.rescanBrowsers.mockReset();
    browserState.rescanBrowsers.mockResolvedValue(undefined);
    invokeMock.mockImplementation((command: string) => {
      if (command === "vault_exists") return Promise.resolve(true);
      if (command === "load_vault") {
        return Promise.resolve({
          source_schema: "v2",
          migration_required: false,
          targets: [target("C:\\a.txt")],
        });
      }
      if (command === "validate_targets") {
        return Promise.resolve([metadata("C:\\a.txt")]);
      }
      if (command === "is_pin_enabled") return Promise.resolve(false);
      if (command === "get_all_drive_info") return Promise.resolve([]);
      return Promise.resolve(undefined);
    });
  });

  it("aborts before execution when the pre-execution vault flush fails", async () => {
    let saveAttempts = 0;
    invokeMock.mockImplementation((command: string) => {
      if (command === "vault_exists") return Promise.resolve(true);
      if (command === "load_vault") {
        return Promise.resolve({
          source_schema: "v2",
          migration_required: false,
          targets: [target("C:\\a.txt")],
        });
      }
      if (command === "validate_targets") {
        return Promise.resolve([metadata("C:\\a.txt")]);
      }
      if (command === "is_pin_enabled") return Promise.resolve(false);
      if (command === "get_all_drive_info") return Promise.resolve([]);
      if (command === "save_vault") {
        saveAttempts += 1;
        return Promise.reject(new Error("vault write failed"));
      }
      return Promise.resolve(undefined);
    });

    render(
      <ShredProvider>
        <ShredSection />
        <Probe />
      </ShredProvider>
    );

    await act(async () => {
      await latest.loadVault("pin");
    });
    await act(async () => {
      latest.addFiles([readyFile("C:\\b.txt")]);
    });
    // The auto-save attempt for the new file fails, so the writer is in an
    // error state and the pre-execution flush cannot succeed.
    await waitFor(() => expect(saveAttempts).toBeGreaterThanOrEqual(1));
    await waitFor(() => expect(latest.vaultState).toBe("error"));

    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "Delete Selected (2 files)" }));
    await user.click(await screen.findByRole("button", { name: "DELETE" }));

    await waitFor(() =>
      expect(
        latest.logEntries.some((entry) =>
          entry.message.includes("Refusing to shred")
        )
      ).toBe(true)
    );
    expect(invokeMock).not.toHaveBeenCalledWith("execute_roots", expect.anything());
    expect(latest.files.every((file) => file.status === "pending")).toBe(true);
    expect(latest.isShredding).toBe(false);
  });
});

describe("ShredSection policy wiring", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    listenMock.mockReset();
    listenMock.mockResolvedValue(() => {});
    browserState.browsers = [];
    browserState.rescanBrowsers.mockReset();
    browserState.rescanBrowsers.mockResolvedValue(undefined);
    invokeMock.mockImplementation((command: string) => {
      if (command === "vault_exists") return Promise.resolve(true);
      if (command === "load_vault") {
        return Promise.resolve({
          source_schema: "v2",
          migration_required: false,
          targets: [target("C:\\a.txt")],
        });
      }
      if (command === "validate_targets") {
        return Promise.resolve([metadata("C:\\a.txt")]);
      }
      if (command === "is_pin_enabled") return Promise.resolve(false);
      if (command === "get_all_drive_info") return Promise.resolve([]);
      if (command === "execute_roots") {
        return Promise.resolve({ roots: [] });
      }
      if (command === "shred_browser_data") {
        return Promise.resolve({ roots: [] });
      }
      return Promise.resolve(undefined);
    });
  });

  async function renderWithOneFile() {
    render(
      <ShredProvider>
        <ShredSection />
        <Probe />
      </ShredProvider>
    );
    await act(async () => {
      await latest.loadVault("pin");
    });
    await act(async () => {
      latest.addFiles([readyFile("C:\\b.txt")]);
    });
  }

  async function confirmDeletion() {
    const user = userEvent.setup();
    await user.click(
      screen.getByRole("button", { name: "Delete Selected (2 files)" })
    );
    await user.click(await screen.findByRole("button", { name: "DELETE" }));
  }

  it("sends the selected deletion method and write check to execute_roots", async () => {
    await renderWithOneFile();
    await confirmDeletion();

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("execute_roots", expect.anything())
    );
    const [, args] = invokeMock.mock.calls.find(
      ([command]) => command === "execute_roots"
    ) as [string, unknown];
    expect(args).toMatchObject({
      method: "automatic",
      writeCheck: "spot",
      logObfuscation: "none",
    });
  });

  it("sends the policy and the dialog-confirmed consent flag to the browser flow", async () => {
    browserState.browsers = [
      {
        id: "chrome",
        name: "Chrome",
        icon: "",
        isRunning: false,
        profiles: [
          {
            id: "p1",
            name: "Default",
            path: "C:\\chrome\\default",
            size: 1,
            selected: true,
          },
        ],
      },
    ];
    // Browser-only selection: the vault holds no file targets.
    invokeMock.mockImplementation((command: string) => {
      if (command === "vault_exists") return Promise.resolve(true);
      if (command === "load_vault") {
        return Promise.resolve({
          source_schema: "v2",
          migration_required: false,
          targets: [],
        });
      }
      if (command === "validate_targets") return Promise.resolve([]);
      if (command === "is_pin_enabled") return Promise.resolve(false);
      if (command === "get_all_drive_info") return Promise.resolve([]);
      if (command === "execute_roots") return Promise.resolve({ roots: [] });
      if (command === "shred_browser_data") {
        return Promise.resolve({ roots: [] });
      }
      return Promise.resolve(undefined);
    });

    render(
      <ShredProvider>
        <ShredSection />
        <Probe />
      </ShredProvider>
    );
    await act(async () => {
      await latest.loadVault("pin");
    });

    const user = userEvent.setup();
    await user.click(
      screen.getByRole("button", { name: "Clean Selected Browser Data (1 profile)" })
    );
    await user.click(await screen.findByRole("button", { name: "DELETE" }));

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith(
        "shred_browser_data",
        expect.anything()
      )
    );
    const [, args] = invokeMock.mock.calls.find(
      ([command]) => command === "shred_browser_data"
    ) as [string, unknown];
    expect(args).toEqual({
      request: {
        browser_name: "Chrome",
        profile_path: "C:\\chrome\\default",
        data_types: ["cache", "cookies", "history", "passwords"],
        method: "automatic",
        write_check: "spot",
        explicit_consent: true,
      },
    });
  });
});

describe("computeProgressPercent", () => {
  it("combines completed passes with the pass-local fraction (M5)", () => {
    expect(computeProgressPercent(1, 3, 0, 100)).toBe(0);
    expect(computeProgressPercent(2, 3, 50, 100)).toBeCloseTo(50);
    expect(computeProgressPercent(3, 3, 100, 100)).toBe(100);
    expect(computeProgressPercent(2, 2, 100, 100)).toBe(100);
  });

  it("guards invalid pass counts and zero-size files without NaN", () => {
    expect(computeProgressPercent(1, 0, 10, 10)).toBe(0);
    expect(computeProgressPercent(0, 3, 0, 0)).toBe(0);
    expect(computeProgressPercent(1, 3, 0, 0)).toBe(0);
  });

  it("clamps overshoot into 0..=100", () => {
    expect(computeProgressPercent(1, 3, 500, 100)).toBeLessThanOrEqual(100);
    expect(computeProgressPercent(2, 1, 100, 100)).toBe(100);
  });
});
