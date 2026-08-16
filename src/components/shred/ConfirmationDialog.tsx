// src/components/shred/ConfirmationDialog.tsx
import type { ReactNode } from "react";
import type { BrowserRunningStatus } from "@/types";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";

interface ConfirmationDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  fileCount: number;
  folderCount: number;
  profileCount: number;
  /** Selected browsers whose cached state is not reliably closed. DELETE is
   * disabled while non-empty; the fresh backend pre-delete check remains the
   * final decision either way. */
  blockedSelectedBrowsers: Array<{ name: string; state: BrowserRunningStatus }>;
  onConfirm: () => void;
}

export function ConfirmationDialog({
  open,
  onOpenChange,
  fileCount,
  folderCount,
  profileCount,
  blockedSelectedBrowsers,
  onConfirm,
}: ConfirmationDialogProps) {
  const hasFiles = fileCount > 0;
  const hasFolders = folderCount > 0;
  const hasProfiles = profileCount > 0;

  const filePart = hasFiles ? (
    <>
      <strong>
        {fileCount} file{fileCount !== 1 ? "s" : ""}
      </strong>
    </>
  ) : null;
  const folderPart = hasFolders ? (
    <>
      <strong>
        {folderCount} folder{folderCount !== 1 ? "s" : ""}
      </strong>
    </>
  ) : null;
  const profilePart = hasProfiles ? (
    <>
      <strong>
        {profileCount} browser profile{profileCount !== 1 ? "s" : ""}
      </strong>
    </>
  ) : null;

  const hasShredTargets = hasFiles || hasFolders;

  let shredPhrase: ReactNode;
  if (hasFiles && hasFolders) {
    shredPhrase = (
      <>
        {filePart} and {folderPart}
      </>
    );
  } else if (hasFiles) {
    shredPhrase = filePart;
  } else {
    shredPhrase = folderPart;
  }

  let description: ReactNode;

  if (hasShredTargets) {
    description = (
      <>
        This will overwrite and delete {shredPhrase}
        {hasProfiles ? (
          <> and selected local data from {profilePart}</>
        ) : null}
        . KnockKnock has no Undo. File and folder targets will be processed
        using the currently selected deletion method and write-check settings.
      </>
    );
  } else if (hasProfiles) {
    description = (
      <>
        This will delete selected local data from {profilePart}. KnockKnock has
        no Undo. Browser account data, synchronized copies, and copies stored on
        other devices are not affected.
      </>
    );
  } else {
    description = "Nothing selected.";
  }

  return (
    <AlertDialog open={open} onOpenChange={onOpenChange}>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle className="font-mono">
            Confirm Deletion
          </AlertDialogTitle>
          <AlertDialogDescription>{description}</AlertDialogDescription>
          {blockedSelectedBrowsers.length > 0 && (
            <p className="mt-2 text-amber-500 font-mono text-xs" role="alert">
              {blockedSelectedBrowsers
                .map(({ name, state }) =>
                  state === "running"
                    ? `${name} is currently running. Close it before deleting browser data.`
                    : `KnockKnock could not confirm that ${name} is closed. Browser data deletion is unavailable.`
                )
                .join(" ")}
            </p>
          )}
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel>Cancel</AlertDialogCancel>
          <AlertDialogAction
            onClick={() => {
              onConfirm();
              onOpenChange(false);
            }}
            disabled={blockedSelectedBrowsers.length > 0}
            className="bg-red-600 text-white hover:bg-red-700"
          >
            DELETE
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
