// src/components/shred/ConfirmationDialog.test.tsx
import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ConfirmationDialog } from "./ConfirmationDialog";

function renderDialog(overrides: {
  fileCount?: number;
  folderCount?: number;
  profileCount?: number;
  runningBrowsers?: string[];
} = {}) {
  const {
    fileCount = 0,
    folderCount = 0,
    profileCount = 0,
    runningBrowsers = [],
  } = overrides;
  return render(
    <ConfirmationDialog
      open
      onOpenChange={vi.fn()}
      fileCount={fileCount}
      folderCount={folderCount}
      profileCount={profileCount}
      runningBrowsers={runningBrowsers}
      onConfirm={vi.fn()}
    />
  );
}

function descriptionText(): string | null | undefined {
  return document.querySelector(
    '[data-slot="alert-dialog-description"]'
  )?.textContent;
}

describe("ConfirmationDialog counts", () => {
  it("states files and folders together", () => {
    renderDialog({ fileCount: 2, folderCount: 1 });
    expect(descriptionText()).toBe(
      "This will permanently shred 2 files and 1 folder. This cannot be undone. Data will be overwritten, verified, renamed, truncated, and deleted."
    );
  });

  it("states files, folders, and browser profiles together", () => {
    renderDialog({ fileCount: 2, folderCount: 1, profileCount: 3 });
    expect(descriptionText()).toBe(
      "This will permanently shred 2 files and 1 folder and 3 browser profiles. This cannot be undone. Data will be overwritten, verified, renamed, truncated, and deleted."
    );
  });

  it("states a folder-only selection", () => {
    renderDialog({ folderCount: 1 });
    expect(descriptionText()).toBe(
      "This will permanently shred 1 folder. This cannot be undone. Data will be overwritten, verified, renamed, truncated, and deleted."
    );
  });

  it("keeps the profiles-only wording", () => {
    renderDialog({ profileCount: 2 });
    expect(descriptionText()).toBe(
      "This will permanently clean 2 browser profiles. This cannot be undone."
    );
  });

  it("falls back to Nothing to destroy when every count is zero", () => {
    renderDialog();
    expect(descriptionText()).toBe("Nothing to destroy.");
  });

  it("keeps the running-browser warning and DESTROY action", () => {
    renderDialog({ fileCount: 1, runningBrowsers: ["Chrome"] });
    expect(
      screen.getByText(
        "Chrome is currently running. Close it first or data may be corrupted."
      )
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "DESTROY" })
    ).toBeInTheDocument();
  });
});
