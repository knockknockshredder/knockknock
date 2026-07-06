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
  profileCount: number;
  onConfirm: () => void;
}

export function ConfirmationDialog({
  open,
  onOpenChange,
  fileCount,
  profileCount,
  onConfirm,
}: ConfirmationDialogProps) {
  const hasFiles = fileCount > 0;
  const hasProfiles = profileCount > 0;

  const filePart = hasFiles ? (
    <>
      <strong>
        {fileCount} file{fileCount !== 1 ? "s" : ""}
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

  let description: ReactNode;
  if (hasFiles && hasProfiles) {
    description = (
      <>
        This will permanently shred {filePart} and {profilePart}. This cannot
        be undone. Data will be overwritten, verified, renamed, truncated, and
        deleted.
      </>
    );
  } else if (hasFiles) {
    description = (
      <>
        This will permanently shred {filePart}. This cannot be undone. Data
        will be overwritten, verified, renamed, truncated, and deleted.
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
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel>Cancel</AlertDialogCancel>
          <AlertDialogAction
            onClick={onConfirm}
            className="bg-red-600 text-white hover:bg-red-700"
          >
            DESTROY
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
