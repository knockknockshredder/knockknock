// src/components/shred/FileDropZone.test.tsx
import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { FileDropZone } from "./FileDropZone";

const FILE_ACTION_LABEL = "Add files — opens the file picker";
const FOLDER_ACTION_LABEL = "Add folders — opens the folder picker";

const { invokeMock, openMock, dragDropMock, addFilesMock, addLogEntryMock } =
  vi.hoisted(() => ({
    invokeMock: vi.fn(),
    openMock: vi.fn(),
    dragDropMock: vi.fn(),
    addFilesMock: vi.fn(),
    addLogEntryMock: vi.fn(),
  }));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: invokeMock,
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: openMock,
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ onDragDropEvent: dragDropMock }),
}));

vi.mock("@/contexts/ShredContext", () => ({
  useShred: () => ({ addFiles: addFilesMock, addLogEntry: addLogEntryMock }),
}));

function setPlatform(ua: string) {
  Object.defineProperty(window.navigator, "userAgent", {
    value: ua,
    configurable: true,
  });
}

const WINDOWS_UA =
  "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko)";
const LINUX_UA =
  "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko)";

function metadata(path: string, kind: "file" | "directory") {
  return {
    path,
    name: path,
    size: 1,
    kind,
    is_shortcut: false,
    shortcut_target: null,
  };
}

describe("FileDropZone", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    openMock.mockReset();
    dragDropMock.mockReset();
    addFilesMock.mockReset();
    addLogEntryMock.mockReset();
    setPlatform(LINUX_UA);
    dragDropMock.mockResolvedValue(vi.fn());
    invokeMock.mockImplementation((command: string) => {
      if (command === "validate_paths") {
        return Promise.resolve([[], []]);
      }
      return Promise.resolve(undefined);
    });
  });

  it("routes the file action to the Windows picker command on Windows", async () => {
    setPlatform(WINDOWS_UA);
    invokeMock.mockImplementation((command: string) => {
      if (command === "open_files_windows") {
        return Promise.resolve(["C:\\a.txt", "C:\\b.txt"]);
      }
      if (command === "validate_paths") {
        return Promise.resolve([
          [metadata("C:\\a.txt", "file"), metadata("C:\\b.txt", "file")],
          [],
        ]);
      }
      return Promise.resolve(undefined);
    });

    const user = userEvent.setup();
    render(<FileDropZone />);
    await user.click(screen.getByRole("button", { name: FILE_ACTION_LABEL }));

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("open_files_windows")
    );
    expect(openMock).not.toHaveBeenCalled();
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("validate_paths", {
        paths: ["C:\\a.txt", "C:\\b.txt"],
      })
    );
    expect(addFilesMock).toHaveBeenCalledTimes(1);
  });

  it("routes the folder action to the Windows folder picker command on Windows", async () => {
    setPlatform(WINDOWS_UA);
    invokeMock.mockImplementation((command: string) => {
      if (command === "open_folders_windows") {
        return Promise.resolve(["C:\\docs"]);
      }
      if (command === "validate_paths") {
        return Promise.resolve([[metadata("C:\\docs", "directory")], []]);
      }
      return Promise.resolve(undefined);
    });

    const user = userEvent.setup();
    render(<FileDropZone />);
    await user.click(screen.getByRole("button", { name: FOLDER_ACTION_LABEL }));

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("open_folders_windows")
    );
    expect(openMock).not.toHaveBeenCalled();
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("validate_paths", {
        paths: ["C:\\docs"],
      })
    );
  });

  it("opens the plugin folder dialog with directory and multiple outside Windows", async () => {
    openMock.mockResolvedValue(["/home/u/docs"]);
    invokeMock.mockImplementation((command: string) => {
      if (command === "validate_paths") {
        return Promise.resolve([[metadata("/home/u/docs", "directory")], []]);
      }
      return Promise.resolve(undefined);
    });

    const user = userEvent.setup();
    render(<FileDropZone />);
    await user.click(screen.getByRole("button", { name: FOLDER_ACTION_LABEL }));

    await waitFor(() =>
      expect(openMock).toHaveBeenCalledWith({
        multiple: true,
        directory: true,
        title: "Select folders to shred",
      })
    );
    expect(invokeMock).not.toHaveBeenCalledWith("open_folders_windows");
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("validate_paths", {
        paths: ["/home/u/docs"],
      })
    );
  });

  it("opens the plugin file dialog with multiple and no directory outside Windows", async () => {
    openMock.mockResolvedValue(["/home/u/a.txt"]);

    const user = userEvent.setup();
    render(<FileDropZone />);
    await user.click(screen.getByRole("button", { name: FILE_ACTION_LABEL }));

    await waitFor(() =>
      expect(openMock).toHaveBeenCalledWith({
        multiple: true,
        directory: false,
        title: "Select files to shred",
      })
    );
    expect(invokeMock).not.toHaveBeenCalledWith("open_files_windows");
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("validate_paths", {
        paths: ["/home/u/a.txt"],
      })
    );
  });

  it("filters a dismissed Windows picker as a silent cancel", async () => {
    setPlatform(WINDOWS_UA);
    invokeMock.mockImplementation((command: string) => {
      if (command === "open_files_windows") {
        return Promise.reject(new Error("0x800704C7"));
      }
      if (command === "open_folders_windows") {
        return Promise.reject(new Error("HRESULT 0x800704C7 (cancel)"));
      }
      return Promise.resolve(undefined);
    });

    const user = userEvent.setup();
    render(<FileDropZone />);
    await user.click(screen.getByRole("button", { name: FILE_ACTION_LABEL }));
    await user.click(screen.getByRole("button", { name: FOLDER_ACTION_LABEL }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("open_folders_windows");
    });
    expect(addLogEntryMock).not.toHaveBeenCalled();
    expect(invokeMock).not.toHaveBeenCalledWith(
      "validate_paths",
      expect.anything()
    );
  });

  it("treats a null plugin dialog result as a cancel", async () => {
    openMock.mockResolvedValue(null);

    const user = userEvent.setup();
    render(<FileDropZone />);
    await user.click(screen.getByRole("button", { name: FOLDER_ACTION_LABEL }));

    await waitFor(() => expect(openMock).toHaveBeenCalled());
    expect(invokeMock).not.toHaveBeenCalledWith(
      "validate_paths",
      expect.anything()
    );
    expect(addLogEntryMock).not.toHaveBeenCalled();
  });

  it("logs a picker failure that is not a cancellation", async () => {
    openMock.mockRejectedValue(new Error("dialog exploded"));

    const user = userEvent.setup();
    render(<FileDropZone />);
    await user.click(screen.getByRole("button", { name: FOLDER_ACTION_LABEL }));

    await waitFor(() =>
      expect(addLogEntryMock).toHaveBeenCalledWith(
        "error",
        "Folder dialog failed: Error: dialog exploded"
      )
    );
  });

  it("renders compact controls with the exact accessible names and titles", () => {
    render(<FileDropZone compact />);

    const fileButton = screen.getByRole("button", { name: FILE_ACTION_LABEL });
    const folderButton = screen.getByRole("button", {
      name: FOLDER_ACTION_LABEL,
    });
    expect(fileButton.title).toBe(FILE_ACTION_LABEL);
    expect(folderButton.title).toBe(FOLDER_ACTION_LABEL);
    expect(screen.getAllByRole("button")).toHaveLength(2);
  });

  it("renders desktop controls with visible labels and the helper copy", () => {
    render(<FileDropZone />);

    const fileButton = screen.getByRole("button", { name: FILE_ACTION_LABEL });
    const folderButton = screen.getByRole("button", {
      name: FOLDER_ACTION_LABEL,
    });
    expect(fileButton).toHaveTextContent("Add Files");
    expect(folderButton).toHaveTextContent("Add Folder");
    expect(
      screen.getByText(
        "Items are added to the review list. Nothing is shredded until you confirm."
      )
    ).toBeInTheDocument();
  });

  it("registers a native drag-drop listener and validates dropped paths", async () => {
    render(<FileDropZone />);

    expect(dragDropMock).toHaveBeenCalledTimes(1);
    const handler = dragDropMock.mock.calls[0][0];
    await act(async () => {
      handler({ payload: { type: "drop", paths: ["C:\\drop.txt"] } });
    });

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("validate_paths", {
        paths: ["C:\\drop.txt"],
      })
    );
  });
});
