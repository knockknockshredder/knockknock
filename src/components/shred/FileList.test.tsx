// src/components/shred/FileList.test.tsx
import { render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { FileList } from "./FileList";
import type { ShredFile } from "@/types";

const { invokeMock, filesMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  filesMock: [] as ShredFile[],
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: invokeMock,
}));

vi.mock("@/contexts/ShredContext", () => ({
  useShred: () => ({ files: filesMock }),
}));

function makeFile(id: string, path: string, kind: ShredFile["kind"]): ShredFile {
  return {
    id,
    path,
    name: path.split("\\").pop() ?? path,
    size: 1,
    status: "pending",
    kind,
    is_shortcut: false,
    shortcut_target: null,
  };
}

describe("FileList group counts", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    filesMock.length = 0;
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_all_drive_info") return Promise.resolve([]);
      return Promise.resolve(undefined);
    });
  });

  it("counts files and folders separately in the group label", async () => {
    filesMock.push(
      makeFile("1", "C:\\a.txt", "file"),
      makeFile("2", "C:\\docs", "directory")
    );

    render(<FileList />);

    await waitFor(() =>
      expect(screen.getByText("1 file + 1 folder")).toBeInTheDocument()
    );
  });

  it("labels a folder-only group with the singular folder count", async () => {
    filesMock.push(makeFile("1", "C:\\docs", "directory"));

    render(<FileList />);

    await waitFor(() =>
      expect(screen.getByText("1 folder")).toBeInTheDocument()
    );
    expect(screen.queryByText(/file/)).not.toBeInTheDocument();
  });

  it("keeps the plain file count for file-only groups", async () => {
    filesMock.push(makeFile("1", "C:\\a.txt", "file"));

    render(<FileList />);

    await waitFor(() =>
      expect(screen.getByText("1 file")).toBeInTheDocument()
    );
  });
});
