// src/sections/SettingsSection.tsx
import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ToggleSetting } from "@/components/settings/ToggleSetting";
import { PinSetup } from "@/components/settings/PinSetup";
import { useSettings } from "@/contexts/SettingsContext";
import { useShred } from "@/contexts/ShredContext";
import { cn } from "@/lib/utils";
import type { LogObfuscation } from "@/types";

const ALGO_HINTS: Record<number, string> = {
  0: "Best for SSDs, fast, single-pass",
  1: "Military-grade, 3-pass fixed pattern",
  2: "Simple random overwrite",
};

const LOG_OBFUSCATION_MODES: ReadonlyArray<{
  value: LogObfuscation;
  label: string;
}> = [
  { value: "none", label: "Full Paths" },
  { value: "numbered", label: "Numbered" },
  { value: "partial_mask", label: "Partial Mask" },
];

export function SettingsSection() {
  const {
    autoClearLog,
    setAutoClearLog,
    defaultAlgorithmIndex,
    setDefaultAlgorithmIndex,
    logObfuscation,
    setLogObfuscation,
  } = useSettings();
  const { algorithms } = useShred();
  const [pinEnabled, setPinEnabled] = useState(false);
  const [pinSetupOpen, setPinSetupOpen] = useState(false);

  useEffect(() => {
    invoke<boolean>("is_pin_enabled").then(setPinEnabled);
  }, []);

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
          PIN Protection
        </h2>
        <div className="flex flex-col gap-3">
          <ToggleSetting
            label="Enable PIN"
            description="Require PIN to open app and shred files"
            checked={pinEnabled}
            onCheckedChange={(enabled) => {
              if (enabled) {
                setPinSetupOpen(true);
              } else {
                invoke("disable_pin").then(() => setPinEnabled(false));
              }
            }}
          />
          {pinEnabled && (
            <button
              type="button"
              onClick={() => setPinSetupOpen(true)}
              className="px-4 py-2 text-sm font-mono border border-border hover:border-muted-foreground transition-colors"
            >
              Change PIN
            </button>
          )}
        </div>
        <PinSetup
          open={pinSetupOpen}
          onOpenChange={setPinSetupOpen}
          onPinSet={() => setPinEnabled(true)}
        />
      </section>

      <section>
        <h2 className="mb-2 font-mono text-xs uppercase tracking-wider text-muted-foreground">
          Log Path Display
        </h2>
        <div className="flex flex-col gap-2">
          <p className="text-xs text-muted-foreground">
            Control how file paths appear in the operation log
          </p>
          <div className="flex gap-2">
            {LOG_OBFUSCATION_MODES.map((mode) => (
              <button
                key={mode.value}
                type="button"
                onClick={() => setLogObfuscation(mode.value)}
                className={cn(
                  "px-3 py-1 text-xs font-mono border transition-colors",
                  logObfuscation === mode.value
                    ? "border-accent bg-accent/10 text-accent"
                    : "border-border hover:border-muted-foreground"
                )}
              >
                {mode.label}
              </button>
            ))}
          </div>
        </div>
      </section>

      <section>
          <h2 className="mb-2 font-mono text-xs uppercase tracking-wider text-muted-foreground">
          Select Default Algorithm
        </h2>
        <div className="flex flex-col gap-3">
          {algorithms.map((algo) => (
            <div
              key={algo.index}
              className={cn(
                "border p-4 transition-colors cursor-pointer",
                defaultAlgorithmIndex === algo.index
                  ? "border-accent bg-accent/5"
                  : "border-border bg-surface hover:border-muted-foreground"
              )}
              onClick={() => setDefaultAlgorithmIndex(algo.index)}
            >
              <div className="flex items-center gap-3">
                <button
                  type="button"
                  onClick={(e) => {
                    e.stopPropagation();
                    setDefaultAlgorithmIndex(algo.index);
                  }}
                  className={cn(
                    "h-4 w-4 rounded-full border-2 transition-colors flex-shrink-0",
                    defaultAlgorithmIndex === algo.index
                      ? "border-accent bg-accent"
                      : "border-muted-foreground hover:border-foreground"
                  )}
                  aria-label={`Set ${algo.name} as default`}
                />
                <div className="flex-1">
                  <div className="flex items-center gap-2">
                    <h3 className="font-mono text-sm font-semibold text-foreground">
                      {algo.name}
                    </h3>
                    <span className="font-mono text-xs text-muted-foreground">
                      {algo.default_passes} pass{algo.default_passes !== 1 ? "es" : ""}
                    </span>
                  </div>
                  <p className="mt-1 text-xs text-muted-foreground">{algo.description}</p>
                  <p className="mt-1 text-xs text-muted-foreground">
                    <span className="text-foreground/70">Best for:</span>{" "}
                    {ALGO_HINTS[algo.index] ?? "General use"}
                  </p>
                  <p className="mt-2 font-mono text-xs text-muted-foreground">
                    Max passes: {algo.max_passes} · Patterns: {algo.accepted_patterns.join(", ")}
                  </p>
                </div>
              </div>
            </div>
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
        <div className="border border-border bg-surface p-4">
          <p className="font-mono text-sm font-semibold text-foreground">
            KnockKnock v0.3.0
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
