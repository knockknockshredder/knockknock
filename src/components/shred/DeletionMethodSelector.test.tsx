// src/components/shred/DeletionMethodSelector.test.tsx
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { DeletionMethodSelector } from "./DeletionMethodSelector";
import type { DeletionMethod, DriveInfo, ShredFile } from "@/types";

const { invokeMock, contextMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  contextMock: {
    files: [] as ShredFile[],
    deletionMethod: "automatic" as DeletionMethod,
    setDeletionMethod: vi.fn(),
  },
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: invokeMock,
}));

vi.mock("@/contexts/ShredContext", () => ({
  useShred: () => contextMock,
}));

function drive(drive_type: DriveInfo["drive_type"]): DriveInfo {
  return {
    drive_letter: "C:",
    drive_type,
    label: "Test",
    total_bytes: 0,
    free_bytes: 0,
  };
}

function pendingFile(path: string): ShredFile {
  return {
    id: "1",
    path,
    name: "a.txt",
    size: 1,
    status: "pending",
    kind: "file",
    is_shortcut: false,
    shortcut_target: null,
  };
}

describe("DeletionMethodSelector", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    contextMock.files = [];
    contextMock.deletionMethod = "automatic";
    contextMock.setDeletionMethod.mockReset();
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_all_drive_info") return Promise.resolve([]);
      return Promise.resolve(undefined);
    });
  });

  it("defaults to Automatic with the Recommended badge and exact copy", () => {
    render(<DeletionMethodSelector />);

    expect(screen.getByText("Automatic")).toBeInTheDocument();
    expect(screen.getByText("Recommended")).toBeInTheDocument();
    expect(screen.getByText("Legacy 3-pass")).toBeInTheDocument();
    expect(
      screen.getByText(
        "Storage-aware local deletion. Uses one logical overwrite pass before removal."
      )
    ).toBeInTheDocument();
    expect(
      screen.getByText(
        "Fixed zeros → ones → random sequence. Available only for confirmed magnetic HDD storage."
      )
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: /Automatic/ })
    ).toHaveAttribute("aria-pressed", "true");
  });

  it("disables Legacy 3-pass with an SSD limitation note when targets sit on SSDs", async () => {
    contextMock.files = [pendingFile("C:\\a.txt")];
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_all_drive_info") return Promise.resolve([drive("ssd")]);
      return Promise.resolve(undefined);
    });

    render(<DeletionMethodSelector />);

    const legacy = screen.getByRole("button", { name: /Legacy 3-pass/ });
    await waitFor(() => expect(legacy).toBeDisabled());
    expect(
      screen.getByText(
        "Selected targets include solid-state storage. Additional overwrite passes do not overcome SSD wear-leveling or block-remapping limitations."
      )
    ).toBeInTheDocument();
  });

  it("disables Legacy 3-pass with a note when storage is unknown", async () => {
    contextMock.files = [pendingFile("C:\\a.txt")];
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_all_drive_info") return Promise.resolve([drive("unknown")]);
      return Promise.resolve(undefined);
    });

    render(<DeletionMethodSelector />);

    const legacy = screen.getByRole("button", { name: /Legacy 3-pass/ });
    await waitFor(() => expect(legacy).toBeDisabled());
    expect(
      screen.getByText(
        "The storage type of some selected targets is unknown. The Legacy 3-pass method is unavailable unless every target is on confirmed magnetic HDD storage."
      )
    ).toBeInTheDocument();
  });

  it("enables Legacy 3-pass on magnetic HDD storage and reports the selection", async () => {
    contextMock.files = [pendingFile("C:\\a.txt")];
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_all_drive_info") return Promise.resolve([drive("hdd")]);
      return Promise.resolve(undefined);
    });

    render(<DeletionMethodSelector />);

    const legacy = screen.getByRole("button", { name: /Legacy 3-pass/ });
    await waitFor(() => expect(legacy).toBeEnabled());
    expect(
      screen.getByText(
        "All selected targets are on magnetic HDD storage. The Legacy 3-pass method is available for this batch."
      )
    ).toBeInTheDocument();

    const user = userEvent.setup();
    await user.click(legacy);
    expect(contextMock.setDeletionMethod).toHaveBeenCalledWith(
      "legacy_three_pass"
    );
  });

  it("reports the Automatic selection when clicked", async () => {
    const user = userEvent.setup();
    render(<DeletionMethodSelector />);

    await user.click(screen.getByRole("button", { name: /Automatic/ }));
    expect(contextMock.setDeletionMethod).toHaveBeenCalledWith("automatic");
  });
});
