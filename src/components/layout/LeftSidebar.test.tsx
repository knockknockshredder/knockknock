// src/components/layout/LeftSidebar.test.tsx
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { BrowserProvider } from "@/contexts/BrowserContext";
import { LeftSidebar } from "./LeftSidebar";

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: invokeMock,
}));

vi.mock("@/contexts/ShredContext", () => ({
  useShred: () => ({ addLogEntry: vi.fn() }),
}));

describe("LeftSidebar browser refresh", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockResolvedValue([]);
  });

  it("scans only when the user explicitly requests a refresh", async () => {
    const user = userEvent.setup();
    render(
      <BrowserProvider>
        <LeftSidebar />
      </BrowserProvider>
    );

    // No automatic scan from rendering the sidebar.
    expect(invokeMock).not.toHaveBeenCalled();

    await user.click(screen.getByRole("button", { name: "Rescan browsers" }));
    await waitFor(() => expect(invokeMock).toHaveBeenCalledTimes(1));
    expect(invokeMock).toHaveBeenCalledWith("detect_browsers");

    // Each explicit click yields exactly one additional scan.
    await user.click(screen.getByRole("button", { name: "Rescan browsers" }));
    await waitFor(() => expect(invokeMock).toHaveBeenCalledTimes(2));
  });
});
