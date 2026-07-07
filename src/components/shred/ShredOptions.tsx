// src/components/shred/ShredOptions.tsx
import {
  Tooltip,
  TooltipTrigger,
  TooltipContent,
  TooltipProvider,
} from "@/components/ui/tooltip";
import { Question } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";

interface ShredOptionsProps {
  passes: number;
  onPassesChange: (v: number) => void;
  pattern: "random" | "zeros" | "ones";
  onPatternChange: (v: "random" | "zeros" | "ones") => void;
  verificationLevel: "none" | "sample" | "full";
  onVerificationLevelChange: (v: "none" | "sample" | "full") => void;
  maxPasses: number;
  currentAlgorithm?: {
    name: string;
    default_passes: number;
    has_fixed_pattern_sequence: boolean;
    accepted_patterns: string[];
  };
}

function HintTooltip({ text }: { text: string }) {
  return (
    <TooltipProvider>
      <Tooltip>
        <TooltipTrigger render={<span className="inline-flex cursor-help" />}>
          <Question size={14} className="text-muted-foreground" />
        </TooltipTrigger>
        <TooltipContent>{text}</TooltipContent>
      </Tooltip>
    </TooltipProvider>
  );
}

export function ShredOptions({
  passes,
  onPassesChange,
  pattern,
  onPatternChange,
  verificationLevel,
  onVerificationLevelChange,
  maxPasses,
  currentAlgorithm,
}: ShredOptionsProps) {
  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-col gap-1.5">
        <div className="flex items-center gap-1.5">
          <span className="font-mono text-xs text-muted-foreground">
            Pattern
          </span>
          <HintTooltip text="Byte pattern used for each overwrite pass. Random is most secure. Zeros/Ones are deterministic patterns used by some standards." />
        </div>
        <div className="flex w-full">
          {(["random", "zeros", "ones"] as const).map((p, i) => (
            <button
              key={p}
              type="button"
              onClick={() =>
                !currentAlgorithm?.has_fixed_pattern_sequence &&
                onPatternChange(p)
              }
              disabled={currentAlgorithm?.has_fixed_pattern_sequence}
              className={cn(
                "flex-1 px-3 py-1.5 font-mono text-xs border transition-colors",
                i === 0 && "rounded-l",
                i === 2 && "rounded-r",
                pattern === p
                  ? "bg-accent text-accent-foreground border-accent"
                  : "bg-transparent text-muted-foreground border-border hover:bg-elevated hover:text-foreground",
                currentAlgorithm?.has_fixed_pattern_sequence &&
                  "opacity-50 cursor-not-allowed"
              )}
            >
              {p.charAt(0).toUpperCase() + p.slice(1)}
            </button>
          ))}
        </div>
        {currentAlgorithm?.has_fixed_pattern_sequence && (
          <span className="font-mono text-xs text-muted-foreground">
            Fixed pattern for {currentAlgorithm.name ?? "this algorithm"}:{" "}
            {currentAlgorithm.accepted_patterns.join(", ")}
          </span>
        )}
      </div>

      <div className="flex flex-col gap-1.5">
        <div className="flex items-center gap-1.5">
          <span className="font-mono text-xs text-muted-foreground">
            Verification
          </span>
          <HintTooltip text="How thoroughly to verify that data was actually overwritten. None skips verification. Sample checks random blocks. Full reads back every block." />
        </div>
        <div className="flex w-full">
          {(["none", "sample", "full"] as const).map((v, i) => (
            <button
              key={v}
              type="button"
              onClick={() => onVerificationLevelChange(v)}
              className={cn(
                "flex-1 px-3 py-1.5 font-mono text-xs border transition-colors",
                i === 0 && "rounded-l",
                i === 2 && "rounded-r",
                verificationLevel === v
                  ? "bg-accent text-accent-foreground border-accent"
                  : "bg-transparent text-muted-foreground border-border hover:bg-elevated hover:text-foreground"
              )}
            >
              {v.charAt(0).toUpperCase() + v.slice(1)}
            </button>
          ))}
        </div>
      </div>

      <div className="flex flex-col gap-1.5">
        <div className="flex items-center gap-1.5">
          <span className="font-mono text-xs text-muted-foreground">
            Pass Repeats
          </span>
          <HintTooltip text="Number of overwrite passes per file. Higher values are more thorough but slower. For DoD (3-pass), setting this to 2 means 3 × 2 = 6 total overwrites." />
        </div>
        <input
          type="number"
          min={1}
          max={maxPasses}
          value={passes}
          onChange={(e) => {
            const v = parseInt(e.target.value, 10);
            if (!isNaN(v) && v >= 1 && v <= maxPasses) onPassesChange(v);
          }}
          className="w-full rounded border border-border bg-transparent px-2 py-1.5 font-mono text-xs text-foreground focus:border-ring focus:outline-none"
        />
        {currentAlgorithm?.has_fixed_pattern_sequence && (
          <span className="font-mono text-xs text-muted-foreground">
            {currentAlgorithm.default_passes} passes × {passes} repeats ={" "}
            {currentAlgorithm.default_passes * passes} total
          </span>
        )}
      </div>
    </div>
  );
}