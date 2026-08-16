// src/components/shred/WriteCheckSelector.test.tsx
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { WriteCheckSelector } from "./WriteCheckSelector";
import type { WriteCheck } from "@/types";

const { contextMock } = vi.hoisted(() => ({
  contextMock: {
    writeCheck: "spot" as WriteCheck,
    setWriteCheck: vi.fn(),
  },
}));

vi.mock("@/contexts/ShredContext", () => ({
  useShred: () => contextMock,
}));

describe("WriteCheckSelector", () => {
  beforeEach(() => {
    contextMock.writeCheck = "spot";
    contextMock.setWriteCheck.mockReset();
  });

  it("defaults to Spot with the Spot description", () => {
    render(<WriteCheckSelector />);

    expect(
      screen.getByRole("button", { name: "Spot" })
    ).toHaveAttribute("aria-pressed", "true");
    expect(
      screen.getByText(
        "Checks the final overwrite at distributed locations. Small files are checked in full."
      )
    ).toBeInTheDocument();
  });

  it("shows the exact copy for every write-check option", () => {
    const { rerender } = render(<WriteCheckSelector />);

    expect(screen.getByRole("button", { name: "Off" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Full" })).toBeInTheDocument();

    contextMock.writeCheck = "off";
    rerender(<WriteCheckSelector />);
    expect(
      screen.getByText("Skips read-back after the overwrite.")
    ).toBeInTheDocument();

    contextMock.writeCheck = "full";
    rerender(<WriteCheckSelector />);
    expect(
      screen.getByText(
        "Reads back the entire final logical file range. This checks the write result, not physical-media erasure."
      )
    ).toBeInTheDocument();
  });

  it("reports the selected write check", async () => {
    const user = userEvent.setup();
    render(<WriteCheckSelector />);

    await user.click(screen.getByRole("button", { name: "Full" }));
    expect(contextMock.setWriteCheck).toHaveBeenCalledWith("full");

    await user.click(screen.getByRole("button", { name: "Off" }));
    expect(contextMock.setWriteCheck).toHaveBeenCalledWith("off");
  });
});
