// src/components/shred/ConfirmationDialog.tsx
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
  onConfirm: () => void;
}

export function ConfirmationDialog({
  open,
  onOpenChange,
  fileCount,
  onConfirm,
}: ConfirmationDialogProps) {
  return (
    <AlertDialog open={open} onOpenChange={onOpenChange}>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle className="font-mono">
            Confirm Destruction
          </AlertDialogTitle>
          <AlertDialogDescription>
            You are about to permanently destroy{" "}
            <strong>
              {fileCount} file{fileCount !== 1 ? "s" : ""}
            </strong>
            . This action cannot be undone. Data will be overwritten, verified,
            renamed, truncated, and deleted.
          </AlertDialogDescription>
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
