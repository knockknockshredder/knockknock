// src/components/shred/AlgorithmSelector.tsx
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  Tooltip,
  TooltipTrigger,
  TooltipContent,
  TooltipProvider,
} from "@/components/ui/tooltip";
import { Question } from "@phosphor-icons/react";
import { useShred } from "@/contexts/ShredContext";
import type { AlgorithmOption } from "@/types";

// Short hint per algorithm, shown below the name in the dropdown.
// Falls back to the description when no hint is mapped.
const ALGO_HINTS: Record<string, string> = {
  "NIST 800-88 Clear": "Best for SSDs, fast, single-pass",
  "DoD 5220.22-M": "Military-grade, 3-pass fixed pattern",
  "RandomOnly": "Simple random overwrite",
};

function hintFor(algo: AlgorithmOption): string {
  return ALGO_HINTS[algo.name] ?? algo.description;
}

export function AlgorithmSelector() {
  const { algorithms, algorithmIndex, setAlgorithmIndex } = useShred();

  if (algorithms.length === 0) {
    return (
      <p className="text-xs text-muted-foreground">Loading algorithms...</p>
    );
  }

  const current = algorithms[algorithmIndex];

  return (
    <div className="flex flex-col gap-1.5">
      <div className="flex items-center gap-1.5">
        <label
          htmlFor="algorithm-select"
          className="font-mono text-xs text-muted-foreground"
        >
          Algorithm
        </label>
        <TooltipProvider>
          <Tooltip>
            <TooltipTrigger
              render={<span className="inline-flex cursor-help" />}
            >
              <Question size={14} className="text-muted-foreground" />
            </TooltipTrigger>
            <TooltipContent>
              The overwrite algorithm used to destroy file data. NIST 800-88
              Clear is recommended for most use cases. DoD 5220.22-M uses
              a fixed 3-pass pattern. RandomOnly overwrites with random bytes.
            </TooltipContent>
          </Tooltip>
        </TooltipProvider>
      </div>
      <Select
        value={String(algorithmIndex)}
        onValueChange={(v) => setAlgorithmIndex(Number(v))}
      >
        <TooltipProvider>
          <Tooltip>
            <TooltipTrigger
              render={
                <SelectTrigger
                  id="algorithm-select"
                  className="w-full font-mono text-sm"
                />
              }
            >
              <SelectValue
                placeholder={current ? current.name : "Select algorithm"}
              />
            </TooltipTrigger>
            <TooltipContent>{current?.description}</TooltipContent>
          </Tooltip>
        </TooltipProvider>
        <SelectContent>
          {algorithms.map((algo) => (
            <SelectItem key={algo.index} value={String(algo.index)}>
              <div className="flex flex-col">
                <span>{algo.name}</span>
                <span className="text-xs text-muted-foreground">
                  {hintFor(algo)}
                </span>
              </div>
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    </div>
  );
}