// src/components/shred/DeletionMethodSelector.test.tsx
import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { DeletionMethodSelector } from "./DeletionMethodSelector";
import type { DeletionMethod, DetectedBrowser, DriveInfo, ShredFile } from "@/types";

const { invokeMock, contextMock, browserState } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  contextMock: {
    files: [] as ShredFile[],
    deletionMethod: "automatic" as DeletionMethod,
    setDeletionMethod: vi.fn(),
  },
  browserState: {
    browsers: [] as DetectedBrowser[],
  },
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: invokeMock,
}));

vi.mock("@/contexts/ShredContext", () => ({
  useShred: () => contextMock,
}));

vi.mock("@/contexts/BrowserContext", () => ({
  useBrowser: () => browserState,
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

function pendingFile(path: string, id = "1"): ShredFile {
  return {
    id,
    path,
    name: "a.txt",
    size: 1,
    status: "pending",
    kind: "file",
    is_shortcut: false,
    shortcut_target: null,
  };
}

function selectedProfile(path: string): DetectedBrowser {
  return {
    id: "chrome",
    name: "Chrome",
    icon: "",
    isRunning: false,
    profiles: [
      {
        id: "default",
        name: "Default",
        path,
        size: 1,
        selected: true,
      },
    ],
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

describe("DeletionMethodSelector", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    contextMock.files = [];
    contextMock.deletionMethod = "automatic";
    contextMock.setDeletionMethod.mockReset();
    browserState.browsers = [];
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
    expect(screen.getByRole("button", { name: /Legacy 3-pass/ })).toBeDisabled();
  });

  it("disables Legacy 3-pass when drive information is incomplete", async () => {
    contextMock.files = [pendingFile("C:\\a.txt")];

    render(<DeletionMethodSelector />);

    await waitFor(() =>
      expect(screen.getByRole("button", { name: /Legacy 3-pass/ })).toBeDisabled()
    );
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

  it("includes selected browser profile paths when classifying Legacy availability", async () => {
    browserState.browsers = [selectedProfile("C:\\chrome\\default")];
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_all_drive_info") return Promise.resolve([drive("hdd")]);
      return Promise.resolve(undefined);
    });

    render(<DeletionMethodSelector />);

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("get_all_drive_info", {
        paths: ["C:\\chrome\\default"],
      })
    );
    expect(screen.getByRole("button", { name: /Legacy 3-pass/ })).toBeEnabled();
  });

  it("disables Legacy 3-pass when a selected browser profile cannot be classified", async () => {
    browserState.browsers = [selectedProfile("relative-profile")];

    render(<DeletionMethodSelector />);

    await waitFor(() =>
      expect(screen.getByRole("button", { name: /Legacy 3-pass/ })).toBeDisabled()
    );
  });

  it("classifies every selected Unix path before disabling Legacy for an SSD", async () => {
    contextMock.files = [
      pendingFile("/home/alice/report.txt", "1"),
      pendingFile("/home/bob/archive.txt", "2"),
    ];
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_all_drive_info") {
        return Promise.resolve([drive("hdd"), drive("ssd")]);
      }
      return Promise.resolve(undefined);
    });

    render(<DeletionMethodSelector />);

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("get_all_drive_info", {
        paths: ["/home/alice/report.txt", "/home/bob/archive.txt"],
      })
    );
    expect(screen.getByRole("button", { name: /Legacy 3-pass/ })).toBeDisabled();
  });

  it("clears stale drive classification while an updated path request is pending", async () => {
    const updatedClassification = deferred<DriveInfo[]>();
    let classificationCalls = 0;
    contextMock.files = [pendingFile("C:\\first.txt")];
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_all_drive_info") {
        classificationCalls += 1;
        return classificationCalls === 1
          ? Promise.resolve([drive("hdd")])
          : updatedClassification.promise;
      }
      return Promise.resolve(undefined);
    });

    const { rerender } = render(<DeletionMethodSelector />);
    const legacy = screen.getByRole("button", { name: /Legacy 3-pass/ });
    await waitFor(() => expect(legacy).toBeEnabled());

    contextMock.files = [pendingFile("D:\\second.txt")];
    rerender(<DeletionMethodSelector />);

    await waitFor(() => expect(legacy).toBeDisabled());
    await act(async () => {
      updatedClassification.resolve([drive("hdd")]);
    });
    await waitFor(() => expect(legacy).toBeEnabled());
  });

  it("resets a selected Legacy method when its storage classification is unavailable", async () => {
    contextMock.files = [pendingFile("C:\\a.txt")];
    contextMock.deletionMethod = "legacy_three_pass";
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_all_drive_info") return Promise.resolve([drive("ssd")]);
      return Promise.resolve(undefined);
    });

    render(<DeletionMethodSelector />);

    await waitFor(() =>
      expect(contextMock.setDeletionMethod).toHaveBeenCalledWith("automatic")
    );
  });

  it("reports the Automatic selection when clicked", async () => {
    const user = userEvent.setup();
    render(<DeletionMethodSelector />);

    await user.click(screen.getByRole("button", { name: /Automatic/ }));
    expect(contextMock.setDeletionMethod).toHaveBeenCalledWith("automatic");
  });
});
