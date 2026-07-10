// src/components/settings/PinSetup.tsx
//
// Minimal stub restored to unblock pnpm lint. The full PIN setup UX lives
// in the PIN protection task (Task 5) and will be wired in separately.

import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
} from "@/components/ui/dialog";

interface PinSetupProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onPinSet: () => void;
}

export function PinSetup({ open, onOpenChange, onPinSet: _onPinSet }: PinSetupProps) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Setup PIN</DialogTitle>
          <DialogDescription>
            PIN setup UI is provided by the PIN protection task.
          </DialogDescription>
        </DialogHeader>
      </DialogContent>
    </Dialog>
  );
}