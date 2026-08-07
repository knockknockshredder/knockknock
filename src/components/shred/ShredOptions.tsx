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
          <HintTooltip text="Byte pattern written during each overwrite pass. Random, Zeros, and Ones define what is written to the selected logical file range; the pattern itself does not provide a universal physical-erasure guarantee." />
        </div>
        <div className="flex w-full">
          {(["random", "zeros", "ones"] as const).map((p) => (
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
          <HintTooltip text="Read-back check of the overwritten logical file range. None skips verification. Sample checks the beginning, middle, and end. Full reads back the entire logical range." />
        </div>
        <div className="flex w-full">
          {(["none", "sample", "full"] as const).map((v) => (
            <button
              key={v}
              type="button"
              onClick={() => onVerificationLevelChange(v)}
              className={cn(
                "flex-1 px-3 py-1.5 font-mono text-xs border transition-colors",
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
            Passes / Repeats
          </span>
          <HintTooltip text="Controls how many overwrite passes are performed. Additional passes take longer and are mainly relevant to magnetic media; they do not overcome SSD wear-leveling or block remapping. For a fixed 3-pass mode, 2 repeats means 6 total overwrites." />
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
          className="w-full border border-border bg-transparent px-2 py-1.5 font-mono text-xs text-foreground focus:border-ring focus:outline-none"
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