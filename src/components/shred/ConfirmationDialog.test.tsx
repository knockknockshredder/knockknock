// src/components/shred/ConfirmationDialog.test.tsx
import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ConfirmationDialog } from "./ConfirmationDialog";

function renderDialog(overrides: {
  fileCount?: number;
  folderCount?: number;
  profileCount?: number;
  blockedSelectedBrowsers?: Array<{
    name: string;
    state: "closed" | "running" | "unknown";
  }>;
} = {}) {
  const {
    fileCount = 0,
    folderCount = 0,
    profileCount = 0,
    blockedSelectedBrowsers = [],
  } = overrides;
  return render(
    <ConfirmationDialog
      open
      onOpenChange={vi.fn()}
      fileCount={fileCount}
      folderCount={folderCount}
      profileCount={profileCount}
      blockedSelectedBrowsers={blockedSelectedBrowsers}
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
      "This will overwrite and delete 2 files and 1 folder. KnockKnock has no Undo. File and folder targets will be processed using the currently selected deletion method and write-check settings."
    );
  });

  it("states files, folders, and browser profiles together", () => {
    renderDialog({ fileCount: 2, folderCount: 1, profileCount: 3 });
    expect(descriptionText()).toBe(
      "This will overwrite and delete 2 files and 1 folder and selected local data from 3 browser profiles. KnockKnock has no Undo. File and folder targets will be processed using the currently selected deletion method and write-check settings."
    );
  });

  it("states a folder-only selection", () => {
    renderDialog({ folderCount: 1 });
    expect(descriptionText()).toBe(
      "This will overwrite and delete 1 folder. KnockKnock has no Undo. File and folder targets will be processed using the currently selected deletion method and write-check settings."
    );
  });

  it("keeps the profiles-only wording", () => {
    renderDialog({ profileCount: 2 });
    expect(descriptionText()).toBe(
      "This will delete selected local data from 2 browser profiles. KnockKnock has no Undo. Browser account data, synchronized copies, and copies stored on other devices are not affected."
    );
  });

  it("falls back to Nothing selected when every count is zero", () => {
    renderDialog();
    expect(descriptionText()).toBe("Nothing selected.");
  });

  it("warns about a selected running browser and disables DELETE", () => {
    renderDialog({
      fileCount: 1,
      blockedSelectedBrowsers: [{ name: "Chrome", state: "running" }],
    });
    expect(
      screen.getByText(
        "Chrome is currently running. Close it before deleting browser data."
      )
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "DELETE" })
    ).toBeDisabled();
  });

  it("keeps DELETE enabled when no selected browser is running", () => {
    renderDialog({ fileCount: 1, blockedSelectedBrowsers: [] });
    expect(
      screen.getByRole("button", { name: "DELETE" })
    ).toBeEnabled();
  });

  it("blocks DELETE when a selected browser state is unknown", () => {
    renderDialog({
      fileCount: 1,
      blockedSelectedBrowsers: [{ name: "Safari", state: "unknown" }],
    });
    expect(
      screen.getByText(
        "KnockKnock could not confirm that Safari is closed. Browser data deletion is unavailable."
      )
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "DELETE" })).toBeDisabled();
  });
});
