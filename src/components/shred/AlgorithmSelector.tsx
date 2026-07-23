// src/components/shred/AlgorithmSelector.tsx
import {
  Tooltip,
  TooltipTrigger,
  TooltipContent,
  TooltipProvider,
} from "@/components/ui/tooltip";
import { Question } from "@phosphor-icons/react";
import { useShred } from "@/contexts/ShredContext";
import { cn } from "@/lib/utils";

export function AlgorithmSelector() {
  const { algorithms, algorithmIndex, setAlgorithmIndex } = useShred();

  if (algorithms.length === 0) {
    return (
      <p className="text-xs text-muted-foreground">Loading algorithms...</p>
    );
  }

  return (
    <div className="flex flex-col gap-1.5 w-full">
      <div className="flex items-center gap-1.5">
        <span className="font-mono text-xs text-muted-foreground">
          Algorithm
        </span>
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
      <div className="flex w-full">
        {algorithms.map((algo) => (
          <button
            key={algo.index}
            type="button"
            onClick={() => setAlgorithmIndex(algo.index)}
            className={cn(
              "flex-1 px-3 py-1.5 font-mono text-xs border transition-colors",
              algorithmIndex === algo.index
                ? "bg-accent text-accent-foreground border-accent"
                : "bg-transparent text-muted-foreground border-border hover:bg-elevated hover:text-foreground"
            )}
          >
            <span className="inline-flex items-center gap-1">
              {algo.name}
              <TooltipProvider>
                <Tooltip>
                  <TooltipTrigger
                    render={<span className="inline-flex cursor-help" />}
                  >
                    <Question size={12} className="text-muted-foreground" />
                  </TooltipTrigger>
                  <TooltipContent>
                    <div className="max-w-xs space-y-1">
                      <p className="text-xs">{algo.description}</p>
                      <p className="text-xs opacity-80">
                        Passes: {algo.default_passes} default / {algo.max_passes} max
                      </p>
                      {algo.accepted_patterns.length > 0 && (
                        <p className="text-xs opacity-80">
                          Patterns: {algo.accepted_patterns.join(", ")}
                        </p>
                      )}
                    </div>
                  </TooltipContent>
                </Tooltip>
              </TooltipProvider>
            </span>
          </button>
        ))}
      </div>
    </div>
  );
}