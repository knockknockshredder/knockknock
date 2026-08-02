// src/sections/ShredSection.test.tsx
import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ShredProvider, useShred } from "@/contexts/ShredContext";
import { ShredSection } from "./ShredSection";

const { invokeMock, listenMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  listenMock: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: invokeMock,
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: listenMock,
}));

vi.mock("@/contexts/BrowserContext", () => ({
  useBrowser: () => ({ getSelectedCount: () => 0, browsers: [] }),
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
      if (command === "get_algorithms") return Promise.resolve([]);
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
      if (command === "get_algorithms") return Promise.resolve([]);
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
    await user.click(screen.getByRole("button", { name: "Shred Selected (2 files)" }));
    await user.click(await screen.findByRole("button", { name: "DESTROY" }));

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
