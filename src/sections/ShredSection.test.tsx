// src/sections/ShredSection.test.tsx
import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ShredProvider, useShred } from "@/contexts/ShredContext";
import { ShredSection, computeProgressPercent } from "./ShredSection";
import type { ProgressEvent } from "@/types";

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

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

function rootResult(status: "destroyed" | "failed" | "cancelled" | "skipped") {
  return {
    target_id: "root-1",
    requested_path: "C:\\a.txt",
    kind: "file" as const,
    status,
    root_removed: status === "destroyed",
    files_destroyed: status === "destroyed" ? 1 : 0,
    directories_removed: 0,
    bytes_shredded: status === "destroyed" ? 1 : 0,
    write_check: "passed" as const,
    errors: [],
  };
}

function lifecycleCommands() {
  return invokeMock.mock.calls
    .map(([command]) => command)
    .filter((command) =>
      [
        "begin_shred_operation",
        "execute_roots",
        "shred_browser_data",
        "send_notification",
      ].includes(command)
    );
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
      if (command === "is_shred_operation_cancelled") return Promise.resolve(false);
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
    expect(invokeMock).toHaveBeenCalledWith("begin_shred_operation");
    expect(latest.files.every((file) => file.status === "pending")).toBe(true);
    expect(latest.isShredding).toBe(false);
  });

  it("restores execution state without starting destructive commands when beginning fails", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "vault_exists") return Promise.resolve(true);
      if (command === "load_vault") {
        return Promise.resolve({
          source_schema: "v2",
          migration_required: false,
          targets: [target("C:\\a.txt")],
        });
      }
      if (command === "validate_targets") return Promise.resolve([metadata("C:\\a.txt")]);
      if (command === "is_pin_enabled") return Promise.resolve(false);
      if (command === "get_all_drive_info") return Promise.resolve([]);
      if (command === "begin_shred_operation") {
        return Promise.reject(new Error("session unavailable"));
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

    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "Delete Selected (2 files)" }));
    await user.click(await screen.findByRole("button", { name: "DELETE" }));

    await waitFor(() =>
      expect(
        latest.logEntries.some((entry) =>
          entry.message.includes("could not begin operation")
        )
      ).toBe(true)
    );
    expect(invokeMock).not.toHaveBeenCalledWith("execute_roots", expect.anything());
    expect(invokeMock).not.toHaveBeenCalledWith("shred_browser_data", expect.anything());
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
      if (command === "is_shred_operation_cancelled") return Promise.resolve(false);
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
      screen.getByRole("button", { name: /Delete Selected/ })
    );
    await user.click(await screen.findByRole("button", { name: "DELETE" }));
  }

  it("does not rescan browsers when opening the confirmation or deleting files", async () => {
    await renderWithOneFile();
    await confirmDeletion();
    await waitFor(() => expect(latest.isShredding).toBe(false));
    expect(browserState.rescanBrowsers).not.toHaveBeenCalled();
  });

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
      if (command === "is_shred_operation_cancelled") return Promise.resolve(false);
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

  it("begins once and defers the final notification until browser cleanup completes", async () => {
    const browserCleanup = deferred<{ roots: ReturnType<typeof rootResult>[] }>();
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
    invokeMock.mockImplementation((command: string) => {
      if (command === "vault_exists") return Promise.resolve(true);
      if (command === "load_vault") {
        return Promise.resolve({
          source_schema: "v2",
          migration_required: false,
          targets: [target("C:\\a.txt")],
        });
      }
      if (command === "validate_targets") return Promise.resolve([metadata("C:\\a.txt")]);
      if (command === "is_pin_enabled") return Promise.resolve(false);
      if (command === "get_all_drive_info") return Promise.resolve([]);
      if (command === "begin_shred_operation") return Promise.resolve(undefined);
      if (command === "execute_roots") return Promise.resolve({ roots: [] });
      if (command === "is_shred_operation_cancelled") return Promise.resolve(false);
      if (command === "shred_browser_data") return browserCleanup.promise;
      return Promise.resolve(undefined);
    });

    await renderWithOneFile();
    await confirmDeletion();

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("shred_browser_data", expect.anything())
    );
    expect(lifecycleCommands()).toEqual([
      "begin_shred_operation",
      "execute_roots",
      "shred_browser_data",
    ]);
    expect(invokeMock).not.toHaveBeenCalledWith("send_notification", expect.anything());

    browserCleanup.resolve({ roots: [] });

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("send_notification", expect.anything())
    );
    expect(lifecycleCommands()).toEqual([
      "begin_shred_operation",
      "execute_roots",
      "shred_browser_data",
      "send_notification",
    ]);
    expect(
      invokeMock.mock.calls.filter(([command]) => command === "begin_shred_operation")
    ).toHaveLength(1);
  });

  it("queries cancellation status after roots even when Stop was requested and skips browser cleanup", async () => {
    const roots = deferred<{ roots: ReturnType<typeof rootResult>[] }>();
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
    invokeMock.mockImplementation((command: string) => {
      if (command === "vault_exists") return Promise.resolve(true);
      if (command === "load_vault") {
        return Promise.resolve({
          source_schema: "v2",
          migration_required: false,
          targets: [target("C:\\a.txt")],
        });
      }
      if (command === "validate_targets") return Promise.resolve([metadata("C:\\a.txt")]);
      if (command === "is_pin_enabled") return Promise.resolve(false);
      if (command === "get_all_drive_info") return Promise.resolve([]);
      if (command === "execute_roots") return roots.promise;
      if (command === "is_shred_operation_cancelled") return Promise.resolve(false);
      return Promise.resolve(undefined);
    });

    await renderWithOneFile();
    await confirmDeletion();
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("execute_roots", expect.anything()));

    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "Stop Processing" }));
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("cancel_shred"));
    roots.resolve({ roots: [rootResult("destroyed")] });

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("is_shred_operation_cancelled")
    );
    expect(invokeMock).not.toHaveBeenCalledWith("shred_browser_data", expect.anything());
  });

  it("queries cancellation status after each browser profile and skips later profiles after Stop", async () => {
    const statusAfterFirstProfile = deferred<boolean>();
    let statusCalls = 0;
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
          {
            id: "p2",
            name: "Profile 2",
            path: "C:\\chrome\\profile-2",
            size: 1,
            selected: true,
          },
        ],
      },
    ];
    invokeMock.mockImplementation((command: string) => {
      if (command === "vault_exists") {
        return Promise.resolve({
          source_schema: "v2",
          migration_required: false,
          targets: [],
        });
      }
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
      if (command === "shred_browser_data") return Promise.resolve({ roots: [rootResult("destroyed")] });
      if (command === "is_shred_operation_cancelled") {
        statusCalls += 1;
        return statusCalls === 1 ? statusAfterFirstProfile.promise : Promise.resolve(false);
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
      screen.getByRole("button", { name: "Clean Selected Browser Data (2 profiles)" })
    );
    await user.click(await screen.findByRole("button", { name: "DELETE" }));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("is_shred_operation_cancelled")
    );

    await user.click(screen.getByRole("button", { name: "Stop Processing" }));
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("cancel_shred"));
    statusAfterFirstProfile.resolve(false);

    await waitFor(() => expect(latest.isShredding).toBe(false));
    expect(
      invokeMock.mock.calls.filter(([command]) => command === "shred_browser_data")
    ).toHaveLength(1);
  });

  it("does not start browser cleanup when the root cancellation status query fails", async () => {
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
    invokeMock.mockImplementation((command: string) => {
      if (command === "vault_exists") return Promise.resolve(true);
      if (command === "load_vault") {
        return Promise.resolve({
          source_schema: "v2",
          migration_required: false,
          targets: [target("C:\\a.txt")],
        });
      }
      if (command === "validate_targets") return Promise.resolve([metadata("C:\\a.txt")]);
      if (command === "is_pin_enabled") return Promise.resolve(false);
      if (command === "get_all_drive_info") return Promise.resolve([]);
      if (command === "execute_roots") return Promise.resolve({ roots: [rootResult("destroyed")] });
      if (command === "is_shred_operation_cancelled") {
        return Promise.reject(new Error("status unavailable"));
      }
      return Promise.resolve(undefined);
    });

    await renderWithOneFile();
    await confirmDeletion();

    await waitFor(() => expect(latest.isShredding).toBe(false));
    expect(invokeMock).not.toHaveBeenCalledWith("shred_browser_data", expect.anything());
  });

  it("applies completed roots before reporting a rejected root cancellation status query", async () => {
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
    invokeMock.mockImplementation((command: string, args?: unknown) => {
      if (command === "vault_exists") return Promise.resolve(true);
      if (command === "load_vault") {
        return Promise.resolve({
          source_schema: "v2",
          migration_required: false,
          targets: [target("C:\\a.txt")],
        });
      }
      if (command === "validate_targets") return Promise.resolve([metadata("C:\\a.txt")]);
      if (command === "is_pin_enabled") return Promise.resolve(false);
      if (command === "get_all_drive_info") return Promise.resolve([]);
      if (command === "execute_roots") {
        const root = (args as { request: { roots: Array<{ target_id: string; path: string; kind: "file" }> } })
          .request.roots[0];
        return Promise.resolve({
          roots: [
            {
              ...rootResult("destroyed"),
              target_id: root.target_id,
              requested_path: root.path,
              kind: root.kind,
            },
          ],
        });
      }
      if (command === "is_shred_operation_cancelled") {
        return Promise.reject(new Error("status unavailable"));
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
    await user.click(screen.getByRole("button", { name: "Delete Selected (1 file + 1 profile)" }));
    await user.click(await screen.findByRole("button", { name: "DELETE" }));

    await waitFor(() => expect(latest.isShredding).toBe(false));
    expect(latest.files).toHaveLength(0);
    expect(
      latest.logEntries.some((entry) => entry.message.includes("status unavailable"))
    ).toBe(true);
    expect(invokeMock).not.toHaveBeenCalledWith("shred_browser_data", expect.anything());
  });

  it("rechecks cancellation after applying roots before starting browser cleanup", async () => {
    const rootSave = deferred<void>();
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
    invokeMock.mockImplementation((command: string, args?: unknown) => {
      if (command === "vault_exists") return Promise.resolve(true);
      if (command === "load_vault") {
        return Promise.resolve({
          source_schema: "v2",
          migration_required: false,
          targets: [target("C:\\a.txt")],
        });
      }
      if (command === "validate_targets") return Promise.resolve([metadata("C:\\a.txt")]);
      if (command === "is_pin_enabled") return Promise.resolve(false);
      if (command === "get_all_drive_info") return Promise.resolve([]);
      if (command === "execute_roots") {
        const root = (args as { request: { roots: Array<{ target_id: string; path: string; kind: "file" }> } })
          .request.roots[0];
        return Promise.resolve({
          roots: [
            {
              ...rootResult("destroyed"),
              target_id: root.target_id,
              requested_path: root.path,
              kind: root.kind,
            },
          ],
        });
      }
      if (command === "save_vault") return rootSave.promise;
      if (command === "is_shred_operation_cancelled") return Promise.resolve(false);
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
    await user.click(screen.getByRole("button", { name: "Delete Selected (1 file + 1 profile)" }));
    await user.click(await screen.findByRole("button", { name: "DELETE" }));
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("save_vault", expect.anything()));

    await user.click(screen.getByRole("button", { name: "Stop Processing" }));
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("cancel_shred"));
    rootSave.resolve();

    await waitFor(() => expect(latest.isShredding).toBe(false));
    expect(invokeMock).not.toHaveBeenCalledWith("shred_browser_data", expect.anything());
  });

  it("keeps Stop effective while the vault flush is pending because the shared operation starts first", async () => {
    const vaultSave = deferred<void>();
    invokeMock.mockImplementation((command: string) => {
      if (command === "vault_exists") return Promise.resolve(true);
      if (command === "load_vault") {
        return Promise.resolve({
          source_schema: "v2",
          migration_required: false,
          targets: [target("C:\\a.txt")],
        });
      }
      if (command === "validate_targets") return Promise.resolve([metadata("C:\\a.txt")]);
      if (command === "is_pin_enabled") return Promise.resolve(false);
      if (command === "get_all_drive_info") return Promise.resolve([]);
      if (command === "save_vault") return vaultSave.promise;
      if (command === "is_shred_operation_cancelled") return Promise.resolve(true);
      return Promise.resolve(undefined);
    });

    await renderWithOneFile();
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("save_vault", expect.anything()));
    await confirmDeletion();
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("begin_shred_operation"));

    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "Stop Processing" }));
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("cancel_shred"));
    vaultSave.resolve();

    await waitFor(() => expect(latest.isShredding).toBe(false));
    const beginIndex = invokeMock.mock.calls.findIndex(
      ([command]) => command === "begin_shred_operation"
    );
    const cancelIndex = invokeMock.mock.calls.findIndex(([command]) => command === "cancel_shred");
    expect(beginIndex).toBeLessThan(cancelIndex);
    expect(invokeMock).not.toHaveBeenCalledWith("execute_roots", expect.anything());
  });

  it("runs browser-only cleanup without execute_roots and sends one final notification", async () => {
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
    invokeMock.mockImplementation((command: string) => {
      if (command === "vault_exists") return Promise.resolve(true);
      if (command === "load_vault") {
        return Promise.resolve({ source_schema: "v2", migration_required: false, targets: [] });
      }
      if (command === "validate_targets") return Promise.resolve([]);
      if (command === "is_pin_enabled") return Promise.resolve(false);
      if (command === "get_all_drive_info") return Promise.resolve([]);
      if (command === "shred_browser_data") return Promise.resolve({ roots: [rootResult("destroyed")] });
      if (command === "is_shred_operation_cancelled") return Promise.resolve(false);
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

    await waitFor(() => expect(latest.isShredding).toBe(false));
    expect(invokeMock).not.toHaveBeenCalledWith("execute_roots", expect.anything());
    expect(
      invokeMock.mock.calls.filter(([command]) => command === "send_notification")
    ).toHaveLength(1);
  });

  it("sends one final notification for file-only cleanup", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "vault_exists") return Promise.resolve(true);
      if (command === "load_vault") {
        return Promise.resolve({
          source_schema: "v2",
          migration_required: false,
          targets: [target("C:\\a.txt")],
        });
      }
      if (command === "validate_targets") return Promise.resolve([metadata("C:\\a.txt")]);
      if (command === "is_pin_enabled") return Promise.resolve(false);
      if (command === "get_all_drive_info") return Promise.resolve([]);
      if (command === "execute_roots") return Promise.resolve({ roots: [rootResult("destroyed")] });
      if (command === "is_shred_operation_cancelled") return Promise.resolve(false);
      return Promise.resolve(undefined);
    });

    await renderWithOneFile();
    await confirmDeletion();

    await waitFor(() => expect(latest.isShredding).toBe(false));
    expect(
      invokeMock.mock.calls.filter(([command]) => command === "send_notification")
    ).toHaveLength(1);
  });

  it("logs lifecycle progress without synthetic pass numbers", async () => {
    const roots = deferred<{ roots: ReturnType<typeof rootResult>[] }>();
    let progressListener: ((event: { payload: ProgressEvent }) => void) | undefined;
    listenMock.mockImplementation(
      async (
        eventName: string,
        callback: (event: { payload: ProgressEvent }) => void
      ) => {
        if (eventName === "shred-progress") progressListener = callback;
        return () => {};
      }
    );
    invokeMock.mockImplementation((command: string) => {
      if (command === "vault_exists") return Promise.resolve(true);
      if (command === "load_vault") {
        return Promise.resolve({
          source_schema: "v2",
          migration_required: false,
          targets: [target("C:\\a.txt")],
        });
      }
      if (command === "validate_targets") return Promise.resolve([metadata("C:\\a.txt")]);
      if (command === "is_pin_enabled") return Promise.resolve(false);
      if (command === "get_all_drive_info") return Promise.resolve([]);
      if (command === "execute_roots") return roots.promise;
      return Promise.resolve(undefined);
    });

    await renderWithOneFile();
    await confirmDeletion();
    await waitFor(() => expect(progressListener).toBeDefined());

    const progressEvent = (
      status: ProgressEvent["status"],
      current_pass: number,
      total_passes: number
    ): { payload: ProgressEvent } => ({
      payload: {
        file_path: "C:\\a.txt",
        file_size: 1,
        bytes_written: 0,
        current_pass,
        total_passes,
        speed_bytes_per_sec: 0,
        estimated_time_remaining_secs: 0,
        status,
      },
    });

    act(() => {
      progressListener?.(progressEvent({ type: "Shredding" }, 0, 0));
      progressListener?.(progressEvent({ type: "Shredding" }, 1, 1));
      progressListener?.(progressEvent({ type: "Shredding" }, 2, 3));
      progressListener?.(progressEvent({ type: "Complete" }, 0, 0));
      progressListener?.(progressEvent({ type: "Complete" }, 1, 1));
      progressListener?.(progressEvent({ type: "Complete" }, 3, 3));
      progressListener?.(progressEvent({ type: "Warning", message: "w1" }, 2, 3));
      progressListener?.(progressEvent({ type: "Error", message: "e1" }, 2, 3));
    });

    const messages = latest.logEntries.map((entry) => entry.message);
    expect(messages).toContain("[C:\\a.txt] Shredding");
    expect(messages).toContain("[C:\\a.txt] Shredding (pass 1/1)");
    expect(messages).toContain("[C:\\a.txt] Shredding (pass 2/3)");
    expect(messages).toContain("[C:\\a.txt] Complete");
    expect(messages).toContain("[C:\\a.txt] warning: w1");
    expect(messages).toContain("[C:\\a.txt] error: e1");
    expect(messages).not.toContain(expect.stringContaining("pass 0/0"));
    // Realistic backend completion events (Automatic 1/1, Legacy 3/3) must
    // not render a pass suffix.
    expect(messages).not.toContain("[C:\\a.txt] Complete (pass 1/1)");
    expect(messages).not.toContain("[C:\\a.txt] Complete (pass 3/3)");
    // Error/Warning entries never receive synthetic pass suffixes.
    expect(messages).not.toContain(expect.stringContaining("warning: w1 (pass"));
    expect(messages).not.toContain(expect.stringContaining("error: e1 (pass"));

    await act(async () => {
      roots.resolve({ roots: [] });
      await Promise.resolve();
    });
  });

  it("records a rejected root command as an operation failure without starting browser cleanup", async () => {
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
    invokeMock.mockImplementation((command: string) => {
      if (command === "vault_exists") return Promise.resolve(true);
      if (command === "load_vault") {
        return Promise.resolve({
          source_schema: "v2",
          migration_required: false,
          targets: [target("C:\\a.txt")],
        });
      }
      if (command === "validate_targets") return Promise.resolve([metadata("C:\\a.txt")]);
      if (command === "is_pin_enabled") return Promise.resolve(false);
      if (command === "get_all_drive_info") return Promise.resolve([]);
      if (command === "execute_roots") return Promise.reject(new Error("backend unavailable"));
      return Promise.resolve(undefined);
    });

    await renderWithOneFile();
    await confirmDeletion();

    await waitFor(() => expect(latest.isShredding).toBe(false));
    const errors = latest.logEntries.filter((entry) => entry.level === "error");
    expect(errors).toHaveLength(1);
    expect(errors[0].message).toContain("Deletion terminated unexpectedly");
    expect(errors[0].message).toContain("backend unavailable");
    expect(invokeMock).not.toHaveBeenCalledWith("shred_browser_data", expect.anything());
    expect(latest.files.every((file) => file.status === "pending")).toBe(true);
    expect(invokeMock).toHaveBeenCalledWith("send_notification", {
      title: "Deletion Failed",
      body: expect.stringContaining("backend unavailable"),
    });
    expect(invokeMock).not.toHaveBeenCalledWith("send_notification", {
      title: "Deletion Complete",
      body: expect.anything(),
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
