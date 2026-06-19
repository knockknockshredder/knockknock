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
        <label className="font-mono text-xs text-muted-foreground">
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
        <SelectTrigger className="w-full font-mono text-sm">
          <SelectValue placeholder={current ? `${current.name} — ${current.description}` : "Select algorithm"} />
        </SelectTrigger>
        <SelectContent>
          {algorithms.map((algo) => (
            <SelectItem key={algo.index} value={String(algo.index)}>
              {algo.name} — {algo.description}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    </div>
  );
}
