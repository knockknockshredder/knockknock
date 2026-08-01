import type { ReactNode } from "react";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App";

const { invokeMock, listenMock, shredContext } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  listenMock: vi.fn(),
  shredContext: {
    loadVault: vi.fn().mockResolvedValue(undefined),
    addLogEntry: vi.fn(),
    clearFiles: vi.fn(),
    setVaultPin: vi.fn(),
  },
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: invokeMock,
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: listenMock,
}));

vi.mock("@/contexts/NavigationContext", () => ({
  NavigationProvider: ({ children }: { children: ReactNode }) => <>{children}</>,
  useNavigation: () => ({ activeSection: "home", setActiveSection: vi.fn() }),
}));

vi.mock("@/contexts/ShredContext", () => ({
  ShredProvider: ({ children }: { children: ReactNode }) => <>{children}</>,
  useShred: () => shredContext,
}));

vi.mock("@/contexts/SettingsContext", () => ({
  SettingsProvider: ({ children }: { children: ReactNode }) => <>{children}</>,
}));

vi.mock("@/contexts/BrowserContext", () => ({
  BrowserProvider: ({ children }: { children: ReactNode }) => <>{children}</>,
}));

vi.mock("@/components/settings/PinSetup", () => ({
  PinSetup: ({ open }: { open: boolean }) => (open ? <div>Set PIN</div> : null),
}));

vi.mock("@/components/layout/AppShell", () => ({
  AppShell: ({ children }: { children: ReactNode }) => <>{children}</>,
}));

vi.mock("@/components/layout/OperationLog", () => ({
  OperationLog: () => null,
}));

vi.mock("@/sections/ShredSection", () => ({
  ShredSection: () => null,
}));

vi.mock("@/sections/SettingsSection", () => ({
  SettingsSection: () => null,
}));

vi.mock("@/hooks/useBrowserDetection", () => ({
  useBrowserDetection: vi.fn(),
}));

describe("AppGate PIN recovery", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    listenMock.mockReset();
    shredContext.loadVault.mockClear();
    shredContext.addLogEntry.mockClear();
    shredContext.clearFiles.mockReset();
    shredContext.setVaultPin.mockReset();
    invokeMock.mockImplementation((command: string) => {
      if (command === "has_pin") return Promise.resolve(true);
      if (command === "is_pin_enabled") return Promise.resolve(true);
      if (command === "get_lockout_remaining") return Promise.resolve(0);
      return Promise.resolve(undefined);
    });
  });

  it("clears the shred state in order and transitions to Set PIN after reset", async () => {
    const user = userEvent.setup();
    const events: string[] = [];
    shredContext.clearFiles.mockImplementation(() => {
      events.push("clearFiles");
    });
    shredContext.setVaultPin.mockImplementation(() => {
      events.push("setVaultPin");
    });
    invokeMock.mockImplementation((command: string) => {
      if (command === "has_pin") return Promise.resolve(true);
      if (command === "is_pin_enabled") return Promise.resolve(true);
      if (command === "get_lockout_remaining") return Promise.resolve(0);
      if (command === "reset_app_without_pin") {
        events.push("reset_app_without_pin");
        return Promise.resolve(undefined);
      }
      return Promise.resolve(undefined);
    });

    render(<App />);

    await user.click(await screen.findByRole("button", { name: "Forgot PIN?" }));
    await user.type(screen.getByLabelText("Reset confirmation"), "RESET");
    await user.click(screen.getByRole("button", { name: "Reset app protection" }));

    await waitFor(() => expect(screen.getByText("Set PIN")).toBeInTheDocument());
    expect(events).toEqual(["reset_app_without_pin", "clearFiles", "setVaultPin"]);
    expect(shredContext.setVaultPin).toHaveBeenCalledWith(null);
  });

  it("keeps shred state intact and displays a rejected reset error", async () => {
    const user = userEvent.setup();
    invokeMock.mockImplementation((command: string) => {
      if (command === "has_pin") return Promise.resolve(true);
      if (command === "is_pin_enabled") return Promise.resolve(true);
      if (command === "get_lockout_remaining") return Promise.resolve(0);
      if (command === "reset_app_without_pin") {
        return Promise.reject(new Error("reset unavailable"));
      }
      return Promise.resolve(undefined);
    });

    render(<App />);

    await user.click(await screen.findByRole("button", { name: "Forgot PIN?" }));
    await user.type(screen.getByLabelText("Reset confirmation"), "RESET");
    await user.click(screen.getByRole("button", { name: "Reset app protection" }));

    expect(await screen.findByText("Error: reset unavailable")).toBeInTheDocument();
    expect(shredContext.clearFiles).not.toHaveBeenCalled();
    expect(shredContext.setVaultPin).not.toHaveBeenCalled();
  });
});
