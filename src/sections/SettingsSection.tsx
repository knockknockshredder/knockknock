// src/sections/SettingsSection.tsx
import { ToggleSetting } from "@/components/settings/ToggleSetting";
import { AlgorithmInfo } from "@/components/settings/AlgorithmInfo";
import { useSettings } from "@/contexts/SettingsContext";
import { useShred } from "@/contexts/ShredContext";

export function SettingsSection() {
  const { autoClearLog, setAutoClearLog } = useSettings();
  const { algorithms } = useShred();

  return (
    <div className="flex flex-col gap-6">
      <h1 className="font-sans text-xl font-semibold">Settings</h1>

      <section>
        <h2 className="mb-2 font-mono text-xs uppercase tracking-wider text-muted-foreground">
          Log
        </h2>
        <ToggleSetting
          label="Auto-clear log"
          description="Clear the operation log after each shredding session"
          checked={autoClearLog}
          onCheckedChange={setAutoClearLog}
        />
      </section>

      <section>
        <h2 className="mb-2 font-mono text-xs uppercase tracking-wider text-muted-foreground">
          Algorithms
        </h2>
        <div className="flex flex-col gap-3">
          {algorithms.map((algo) => (
            <AlgorithmInfo key={algo.index} algo={algo} />
          ))}
          {algorithms.length === 0 && (
            <p className="text-xs text-muted-foreground">Loading algorithms...</p>
          )}
        </div>
      </section>

      <section>
        <h2 className="mb-2 font-mono text-xs uppercase tracking-wider text-muted-foreground">
          About
        </h2>
        <div className="rounded border border-border bg-surface p-4">
          <p className="font-mono text-sm font-semibold text-foreground">
            KnockKnock v0.1.0
          </p>
          <p className="mt-1 text-xs text-muted-foreground">
            Emergency file shredder for Windows, macOS, and Linux. Implements
            NIST 800-88 Clear, DoD 5220.22-M, and random overwrite algorithms.
          </p>
          <p className="mt-2 text-xs text-muted-foreground">
            This tool is for legitimate privacy/security purposes only. The user
            is responsible for how they use it.
          </p>
        </div>
      </section>
    </div>
  );
}
