// src/components/browser/BrowserWarning.tsx
import { useState } from "react";
import { Warning } from "@phosphor-icons/react";
import { Checkbox } from "@/components/ui/checkbox";

interface BrowserWarningProps {
  browserName: string;
  onAcknowledge: () => void;
  acknowledged?: boolean;
}

export function BrowserWarning({ browserName, onAcknowledge, acknowledged = false }: BrowserWarningProps) {
  const [locallyAcknowledged, setLocallyAcknowledged] = useState(false);

  if (acknowledged) return null;

  return (
    <div className="flex items-start gap-3 border border-amber-500/30 bg-amber-500/10 px-4 py-3">
      <Warning size={20} className="mt-0.5 shrink-0 text-amber-500" />
      <div className="flex flex-col gap-2">
        <p className="text-sm text-foreground">
          <strong>{browserName}</strong> is currently running. Shredding browser
          data while the browser is open may cause errors. Close the browser
          before continuing.
        </p>
        <label className="flex items-center gap-2">
          <Checkbox
            checked={locallyAcknowledged}
            onCheckedChange={(checked) => {
              setLocallyAcknowledged(!!checked);
              if (checked) onAcknowledge();
            }}
          />
          <span className="text-xs text-muted-foreground">
            I understand the risk, continue anyway
          </span>
        </label>
      </div>
    </div>
  );
}
