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

vi.mock("@/components/layout/AppShell", () => ({
  AppShell: ({ children }: { children: ReactNode }) => <>{children}</>,
}));

vi.mock("@/components/layout/OperationLog", () => ({
  OperationLog: () => null,
}));

vi.mock("@/sections/ShredSection", () => ({
  ShredSection: () => <div>Target UI</div>,
}));

vi.mock("@/sections/SettingsSection", () => ({
  SettingsSection: () => null,
}));

vi.mock("@/hooks/useBrowserDetection", () => ({
  useBrowserDetection: vi.fn(),
}));

describe("AppGate vault unlock", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    listenMock.mockReset();
    listenMock.mockResolvedValue(vi.fn());
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

  const unlock = async (user: ReturnType<typeof userEvent.setup>, pin = "123456") => {
    await user.type(screen.getByLabelText("PIN"), pin);
    await user.click(screen.getByRole("button", { name: "Unlock" }));
  };

  it("requires unlock before showing targets when a PIN-enabled vault exists", async () => {
    const user = userEvent.setup();
    invokeMock.mockImplementation((command: string) => {
      if (command === "has_pin") return Promise.resolve(true);
      if (command === "is_pin_enabled") return Promise.resolve(true);
      if (command === "vault_exists") return Promise.resolve(true);
      if (command === "get_lockout_remaining") return Promise.resolve(0);
      if (command === "verify_pin") return Promise.resolve(true);
      return Promise.resolve(undefined);
    });

    render(<App />);

    expect(await screen.findByRole("heading", { name: "Enter PIN" })).toBeInTheDocument();
    expect(screen.queryByText("Target UI")).not.toBeInTheDocument();

    await unlock(user);

    await waitFor(() => expect(screen.getByText("Target UI")).toBeInTheDocument());
    expect(shredContext.loadVault).toHaveBeenCalledWith("123456");
  });

  it("requires unlock when a PIN exists but PIN protection is disabled and the vault exists", async () => {
    const user = userEvent.setup();
    invokeMock.mockImplementation((command: string) => {
      if (command === "has_pin") return Promise.resolve(true);
      if (command === "is_pin_enabled") return Promise.resolve(false);
      if (command === "vault_exists") return Promise.resolve(true);
      if (command === "get_lockout_remaining") return Promise.resolve(0);
      if (command === "verify_pin") return Promise.resolve(true);
      return Promise.resolve(undefined);
    });

    render(<App />);

    expect(await screen.findByRole("heading", { name: "Enter PIN" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Skip" })).not.toBeInTheDocument();
    expect(invokeMock).not.toHaveBeenCalledWith("is_pin_enabled");
    expect(invokeMock).not.toHaveBeenCalledWith("vault_exists");

    await unlock(user);

    await waitFor(() => expect(screen.getByText("Target UI")).toBeInTheDocument());
  });

  it("requires unlock when a PIN exists but PIN protection is disabled and no vault exists", async () => {
    const user = userEvent.setup();
    invokeMock.mockImplementation((command: string) => {
      if (command === "has_pin") return Promise.resolve(true);
      if (command === "is_pin_enabled") return Promise.resolve(false);
      if (command === "vault_exists") return Promise.resolve(false);
      if (command === "get_lockout_remaining") return Promise.resolve(0);
      if (command === "verify_pin") return Promise.resolve(true);
      return Promise.resolve(undefined);
    });

    render(<App />);

    expect(await screen.findByRole("heading", { name: "Enter PIN" })).toBeInTheDocument();
    expect(screen.queryByText("Target UI")).not.toBeInTheDocument();

    await unlock(user);

    await waitFor(() => expect(screen.getByText("Target UI")).toBeInTheDocument());
  });

  it("keeps targets closed after an incorrect PIN or vault decryption failure", async () => {
    const user = userEvent.setup();
    invokeMock.mockImplementation((command: string) => {
      if (command === "has_pin") return Promise.resolve(true);
      if (command === "get_lockout_remaining") return Promise.resolve(0);
      if (command === "verify_pin") return Promise.resolve(false);
      return Promise.resolve(undefined);
    });

    render(<App />);

    await screen.findByRole("heading", { name: "Enter PIN" });
    await unlock(user, "000000");

    expect(await screen.findByText("Incorrect PIN")).toBeInTheDocument();
    expect(shredContext.loadVault).not.toHaveBeenCalled();
    expect(screen.queryByText("Target UI")).not.toBeInTheDocument();

    invokeMock.mockImplementation((command: string) => {
      if (command === "has_pin") return Promise.resolve(true);
      if (command === "get_lockout_remaining") return Promise.resolve(0);
      if (command === "verify_pin") return Promise.resolve(true);
      return Promise.resolve(undefined);
    });
    shredContext.loadVault.mockRejectedValueOnce(new Error("vault decrypt failed"));

    await unlock(user);

    await waitFor(() =>
      expect(shredContext.addLogEntry).toHaveBeenCalledWith("error", "Failed to unlock vault"),
    );
    expect(screen.getByRole("heading", { name: "Enter PIN" })).toBeInTheDocument();
    expect(screen.queryByText("Target UI")).not.toBeInTheDocument();
  });

  it("initializes the writable vault session after onboarding sets a PIN", async () => {
    const user = userEvent.setup();
    invokeMock.mockImplementation((command: string) => {
      if (command === "has_pin") return Promise.resolve(false);
      return Promise.resolve(undefined);
    });

    render(<App />);

    await user.type(await screen.findByLabelText("PIN"), "123456");
    await user.type(screen.getByLabelText("Confirm PIN"), "123456");
    await user.click(screen.getByRole("button", { name: "Save PIN" }));

    await waitFor(() => expect(screen.getByText("Target UI")).toBeInTheDocument());
    expect(invokeMock).toHaveBeenCalledWith("setup_pin", { newPin: "123456" });
    expect(shredContext.loadVault).toHaveBeenCalledWith("123456");
    expect(invokeMock).not.toHaveBeenCalledWith("set_pin_enabled", expect.anything());
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
