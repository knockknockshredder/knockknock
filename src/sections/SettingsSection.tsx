// src/sections/SettingsSection.tsx
import { ToggleSetting } from "@/components/settings/ToggleSetting";
import { useSettings } from "@/contexts/SettingsContext";
import { useShred } from "@/contexts/ShredContext";
import { cn } from "@/lib/utils";

const ALGO_HINTS: Record<number, string> = {
  0: "Best for SSDs, fast, single-pass",
  1: "Military-grade, 3-pass fixed pattern",
  2: "Simple random overwrite",
};

export function SettingsSection() {
  const { autoClearLog, setAutoClearLog, defaultAlgorithmIndex, setDefaultAlgorithmIndex } =
    useSettings();
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
        <div className="overflow-x-auto rounded border border-border">
          <table className="w-full font-mono text-xs">
            <thead>
              <tr className="border-b border-border bg-surface text-left text-muted-foreground">
                <th className="px-3 py-2 font-medium">Algorithm</th>
                <th className="px-3 py-2 text-center font-medium">Default</th>
                <th className="px-3 py-2 font-medium">Passes</th>
                <th className="px-3 py-2 font-medium">Max</th>
                <th className="px-3 py-2 font-medium">Patterns</th>
              </tr>
            </thead>
            <tbody>
              {algorithms.map((algo) => (
                <tr
                  key={algo.index}
                  className="border-b border-border last:border-b-0"
                >
                  <td className="px-3 py-2">
                    <div className="font-semibold text-foreground">{algo.name}</div>
                    <div className="text-muted-foreground">
                      {ALGO_HINTS[algo.index] ?? algo.description}
                    </div>
                  </td>
                  <td className="px-3 py-2 text-center">
                    <button
                      type="button"
                      onClick={() => setDefaultAlgorithmIndex(algo.index)}
                      className={cn(
                        "mx-auto h-4 w-4 rounded-full border-2 transition-colors",
                        defaultAlgorithmIndex === algo.index
                          ? "border-accent bg-accent"
                          : "border-muted-foreground hover:border-foreground"
                      )}
                      aria-label={`Set ${algo.name} as default`}
                    />
                  </td>
                  <td className="px-3 py-2">{algo.default_passes}</td>
                  <td className="px-3 py-2">{algo.max_passes}</td>
                  <td className="px-3 py-2 text-muted-foreground">
                    {algo.accepted_patterns.length}
                  </td>
                </tr>
              ))}
              {algorithms.length === 0 && (
                <tr>
                  <td
                    colSpan={5}
                    className="px-3 py-2 text-center text-muted-foreground"
                  >
                    Loading algorithms...
                  </td>
                </tr>
              )}
            </tbody>
          </table>
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
