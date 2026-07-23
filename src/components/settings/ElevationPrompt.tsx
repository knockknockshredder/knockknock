// src/components/settings/ElevationPrompt.tsx
//
// Dialog shown when a shred operation fails with PermissionDenied. Offers
// to relaunch the app with administrator privileges via the
// `request_elevation` Tauri command, which on Windows triggers a UAC
// elevation prompt.

import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { ShieldWarning } from "@phosphor-icons/react";

interface ElevationPromptProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  errorMessage?: string;
}

export function ElevationPrompt({
  open,
  onOpenChange,
  errorMessage,
}: ElevationPromptProps) {
  const [elevating, setElevating] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleElevate = async () => {
    setError(null);
    setElevating(true);
    try {
      await invoke<void>("request_elevation");
      // On a successful elevation request the backend calls
      // std::process::exit(0) and the elevated instance takes over. We
      // only reach this line if the command returned without exiting
      // (i.e. an Err came back as a rejection rather than a process exit).
    } catch (err) {
      setError(String(err));
      setElevating(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <ShieldWarning size={16} className="text-amber-500" />
            Administrator privileges required
          </DialogTitle>
          <DialogDescription>
            KnockKnock could not shred this file because it is in a
            protected location or is locked by the system. Restarting with
            administrator privileges usually fixes this.
          </DialogDescription>
        </DialogHeader>

        {errorMessage && (
          <div className="border border-border bg-elevated p-3 font-mono text-xs text-muted-foreground">
            {errorMessage}
          </div>
        )}

        <p className="text-xs text-muted-foreground">
          The app will close and reopen as administrator. You may be
          prompted by Windows to confirm. If you do not have administrator
          access on this machine, you will need to ask your administrator
          for help or remove the file from the list.
        </p>

        {error && (
          <p className="font-mono text-xs text-red-500">{error}</p>
        )}

        <DialogFooter>
          <button
            type="button"
            onClick={() => onOpenChange(false)}
            disabled={elevating}
            className="px-4 py-2 font-mono text-xs uppercase tracking-wider border border-border text-foreground transition-colors hover:bg-elevated disabled:opacity-50"
          >
            Cancel
          </button>
          <button
            type="button"
            onClick={handleElevate}
            disabled={elevating}
            className="px-4 py-2 font-mono text-xs uppercase tracking-wider bg-amber-500 text-background transition-colors hover:bg-amber-400 disabled:opacity-50"
          >
            {elevating ? "Requesting..." : "Restart as administrator"}
          </button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}