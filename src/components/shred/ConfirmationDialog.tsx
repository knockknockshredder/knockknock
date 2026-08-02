// src/components/shred/ConfirmationDialog.tsx
import type { ReactNode } from "react";
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
  runningBrowsers: string[];
  onConfirm: () => void;
}

export function ConfirmationDialog({
  open,
  onOpenChange,
  fileCount,
  folderCount,
  profileCount,
  runningBrowsers,
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
        This will permanently shred {shredPhrase}
        {hasProfiles ? <> and {profilePart}</> : null}. This cannot be undone.
        Data will be overwritten, verified, renamed, truncated, and deleted.
      </>
    );
  } else if (hasProfiles) {
    description = (
      <>
        This will permanently clean {profilePart}. This cannot be undone.
      </>
    );
  } else {
    description = "Nothing to destroy.";
  }

  return (
    <AlertDialog open={open} onOpenChange={onOpenChange}>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle className="font-mono">
            Confirm Destruction
          </AlertDialogTitle>
          <AlertDialogDescription>{description}</AlertDialogDescription>
          {runningBrowsers.length > 0 && (
            <p className="mt-2 text-amber-500 font-mono text-xs">
              {runningBrowsers.join(", ")} {runningBrowsers.length === 1 ? "is" : "are"} currently running. Close {runningBrowsers.length === 1 ? "it" : "them"} first or data may be corrupted.
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
            className="bg-red-600 text-white hover:bg-red-700"
          >
            DESTROY
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
