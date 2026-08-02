// src/components/shred/ShredButton.test.tsx
import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ShredButton } from "./ShredButton";

function renderButton(overrides: {
  fileCount?: number;
  folderCount?: number;
  profileCount?: number;
} = {}) {
  const { fileCount = 0, folderCount = 0, profileCount = 0 } = overrides;
  return render(
    <ShredButton
      fileCount={fileCount}
      folderCount={folderCount}
      profileCount={profileCount}
      isShredding={false}
      onClick={vi.fn()}
    />
  );
}

describe("ShredButton counts", () => {
  it("labels files and folders together", () => {
    renderButton({ fileCount: 2, folderCount: 1 });
    expect(
      screen.getByRole("button", {
        name: "Shred Selected (2 files + 1 folder)",
      })
    ).toBeInTheDocument();
  });

  it("labels a folder-only selection", () => {
    renderButton({ folderCount: 1 });
    expect(
      screen.getByRole("button", { name: "Shred Selected (1 folder)" })
    ).toBeInTheDocument();
    expect(
      screen.getByText("this action is irreversible")
    ).toBeInTheDocument();
  });

  it("labels folders with profiles", () => {
    renderButton({ folderCount: 1, profileCount: 3 });
    expect(
      screen.getByRole("button", {
        name: "Shred Selected (1 folder + 3 profiles)",
      })
    ).toBeInTheDocument();
  });

  it("labels files, folders, and profiles together", () => {
    renderButton({ fileCount: 2, folderCount: 1, profileCount: 3 });
    expect(
      screen.getByRole("button", {
        name: "Shred Selected (2 files + 1 folder + 3 profiles)",
      })
    ).toBeInTheDocument();
  });

  it("keeps the existing files-and-profiles wording", () => {
    renderButton({ fileCount: 2, profileCount: 3 });
    expect(
      screen.getByRole("button", {
        name: "Shred Selected (2 files + 3 profiles)",
      })
    ).toBeInTheDocument();
  });

  it("shows a disabled Nothing to shred state when every count is zero", () => {
    renderButton();
    const button = screen.getByRole("button", { name: "Nothing to shred" });
    expect(button).toBeDisabled();
  });
});
