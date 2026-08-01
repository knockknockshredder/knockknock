import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { PinVerify } from "./PinVerify";

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: invokeMock,
}));

describe("PinVerify PIN recovery", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_lockout_remaining") {
        return Promise.resolve(0);
      }

      return Promise.resolve(undefined);
    });
  });

  it("guards app-open recovery until RESET is typed and awaits the reset callback", async () => {
    const user = userEvent.setup();
    let resolveReset!: () => void;
    const resetPromise = new Promise<void>((resolve) => {
      resolveReset = resolve;
    });
    const onReset = vi.fn(() => resetPromise);

    render(
      <PinVerify
        open
        onOpenChange={vi.fn()}
        onVerified={vi.fn()}
        onReset={onReset}
        purpose="app_open"
      />,
    );

    await user.click(screen.getByRole("button", { name: "Forgot PIN?" }));

    expect(screen.getByRole("heading", { name: "Reset app protection?" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Reset app protection" })).toBeDisabled();

    await user.type(screen.getByLabelText("Reset confirmation"), "RESET");
    await user.click(screen.getByRole("button", { name: "Reset app protection" }));

    await waitFor(() => expect(onReset).toHaveBeenCalledTimes(1));
    expect(screen.getByRole("heading", { name: "Reset app protection?" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Resetting..." })).toBeInTheDocument();

    resolveReset();

    await waitFor(() =>
      expect(screen.queryByRole("heading", { name: "Reset app protection?" })).not.toBeInTheDocument(),
    );
  });

  it("disables app-open recovery while verification and unlocking are pending", async () => {
    const user = userEvent.setup();
    let resolveVerify!: (verified: boolean) => void;
    let resolveUnlock!: () => void;
    const verifyPromise = new Promise<boolean>((resolve) => {
      resolveVerify = resolve;
    });
    const unlockPromise = new Promise<void>((resolve) => {
      resolveUnlock = resolve;
    });
    const onOpenChange = vi.fn();
    const onVerified = vi.fn(() => unlockPromise);

    invokeMock.mockImplementation((command: string) => {
      if (command === "get_lockout_remaining") {
        return Promise.resolve(0);
      }
      if (command === "verify_pin") {
        return verifyPromise;
      }
      return Promise.resolve(undefined);
    });

    render(
      <PinVerify
        open
        onOpenChange={onOpenChange}
        onVerified={onVerified}
        onReset={vi.fn()}
        purpose="app_open"
      />,
    );

    await user.type(screen.getByLabelText("PIN"), "123456");
    await user.click(screen.getByRole("button", { name: "Unlock" }));

    expect(screen.getByRole("button", { name: "Forgot PIN?" })).toBeDisabled();

    resolveVerify(true);
    await waitFor(() => expect(onVerified).toHaveBeenCalledWith("123456"));
    expect(screen.getByRole("button", { name: "Forgot PIN?" })).toBeDisabled();

    resolveUnlock();
    await waitFor(() => expect(onOpenChange).toHaveBeenCalledWith(false));
  });

  it("keeps PIN recovery available during lockout", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_lockout_remaining") {
        return Promise.resolve(30);
      }

      return Promise.resolve(undefined);
    });

    render(
      <PinVerify
        open
        onOpenChange={vi.fn()}
        onVerified={vi.fn()}
        onReset={vi.fn()}
        purpose="app_open"
      />,
    );

    expect(await screen.findByText("Too many incorrect attempts.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Forgot PIN?" })).toBeInTheDocument();
  });

  it("keeps the recovery confirmation open and displays reset errors", async () => {
    const user = userEvent.setup();
    const onReset = vi.fn().mockRejectedValue(new Error("Reset failed"));

    render(
      <PinVerify
        open
        onOpenChange={vi.fn()}
        onVerified={vi.fn()}
        onReset={onReset}
        purpose="app_open"
      />,
    );

    await user.click(screen.getByRole("button", { name: "Forgot PIN?" }));
    await user.type(screen.getByLabelText("Reset confirmation"), "RESET");
    await user.click(screen.getByRole("button", { name: "Reset app protection" }));

    expect(await screen.findByText("Error: Reset failed")).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Reset app protection?" })).toBeInTheDocument();
  });

  it("does not offer PIN recovery for non-gate prompts", () => {
    render(
      <PinVerify
        open
        onOpenChange={vi.fn()}
        onVerified={vi.fn()}
        onReset={vi.fn()}
        purpose="shred"
      />,
    );

    expect(screen.queryByRole("button", { name: "Forgot PIN?" })).not.toBeInTheDocument();
  });
});
