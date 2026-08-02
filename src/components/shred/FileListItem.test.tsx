// src/components/shred/FileListItem.test.tsx
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { FileListItem } from "./FileListItem";
import type { ChildErrorDto, ShredFile } from "@/types";

const { removeFileMock } = vi.hoisted(() => ({
  removeFileMock: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("@/contexts/ShredContext", () => ({
  useShred: () => ({ removeFile: removeFileMock }),
}));

function makeFile(overrides: Partial<ShredFile> = {}): ShredFile {
  return {
    id: "1",
    path: "C:\\a.txt",
    name: "a.txt",
    size: 1048576,
    status: "pending",
    kind: "file",
    is_shortcut: false,
    shortcut_target: null,
    ...overrides,
  };
}

describe("FileListItem", () => {
  beforeEach(() => {
    removeFileMock.mockReset();
  });

  it("renders a folder row with a folder affordance and no file-like size", () => {
    render(
      <FileListItem
        file={makeFile({
          name: "docs",
          path: "C:\\docs",
          kind: "directory",
          size: 0,
        })}
      />
    );

    expect(screen.getByText("docs")).toBeInTheDocument();
    expect(screen.getByText("folder")).toBeInTheDocument();
    expect(screen.queryByText(/MB|GB/)).not.toBeInTheDocument();
  });

  it("renders a file row with its size", () => {
    render(<FileListItem file={makeFile({ size: 2097152 })} />);

    expect(screen.getByText("2.0 MB")).toBeInTheDocument();
    expect(screen.queryByText("folder")).not.toBeInTheDocument();
  });

  it("renders a blocked target with the reason emphasized and no Retry as admin", () => {
    render(
      <FileListItem
        file={makeFile({
          status: "error",
          error: "Network roots are not safe execution roots",
        })}
      />
    );

    expect(
      screen.getByText("Network roots are not safe execution roots")
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Retry as admin" })
    ).not.toBeInTheDocument();
  });

  it("never offers Retry as admin for a blocked reason that mentions access denial", () => {
    render(
      <FileListItem
        file={makeFile({
          status: "error",
          error: "Cannot inspect target: Access is denied",
        })}
      />
    );

    expect(
      screen.getByText("Cannot inspect target: Access is denied")
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Retry as admin" })
    ).not.toBeInTheDocument();
  });

  it("renders a retained failed root with its status and first child error", () => {
    const childError: ChildErrorDto = {
      path: "C:\\locked\\a.txt",
      stage: "overwrite",
      error_type: "AccessDenied",
      message: "Access is denied",
      actionable: "Close the file in another app and retry.",
    };
    render(
      <FileListItem
        file={makeFile({
          status: "error",
          error: "failed: overwrite: Access is denied",
          root_status: "failed",
          child_errors: [childError],
        })}
      />
    );

    const statusLine = screen.getByText((_, node) => {
      const el = node as HTMLElement | null;
      return (
        el?.tagName === "P" && el.textContent === "failed: Access is denied"
      );
    });
    expect(statusLine).toBeInTheDocument();
    // The root status is emphasized visually via the uppercase utility.
    expect(
      statusLine.querySelector("span")?.className
    ).toContain("uppercase");
    expect(
      screen.getByText("Close the file in another app and retry.")
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Retry as admin" })
    ).toBeInTheDocument();
  });

  it("shows the generic error when a retained root has no child errors", () => {
    render(
      <FileListItem
        file={makeFile({
          status: "error",
          error: "Destroyed but vault save failed: boom",
          root_status: "destroyed",
          child_errors: [],
        })}
      />
    );

    expect(
      screen.getByText("Destroyed but vault save failed: boom")
    ).toBeInTheDocument();
  });

  it("allows removing pending and error rows", async () => {
    const user = userEvent.setup();
    const pending = makeFile({ name: "a.txt" });
    const blocked = makeFile({
      id: "2",
      name: "dead",
      status: "error",
      error: "Legacy target is missing",
    });

    const { rerender } = render(<FileListItem file={pending} />);
    const removePending = screen.getByRole("button", {
      name: "Remove a.txt",
    });
    expect(removePending).toHaveAttribute("type", "button");
    await user.click(removePending);
    expect(removeFileMock).toHaveBeenCalledWith("1");

    rerender(<FileListItem file={blocked} />);
    await user.click(screen.getByRole("button", { name: "Remove dead" }));
    expect(removeFileMock).toHaveBeenCalledWith("2");
  });
});
