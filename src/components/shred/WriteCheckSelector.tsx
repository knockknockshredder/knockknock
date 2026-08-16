// src/components/shred/WriteCheckSelector.tsx
import {
  Tooltip,
  TooltipTrigger,
  TooltipContent,
  TooltipProvider,
} from "@/components/ui/tooltip";
import { Question } from "@phosphor-icons/react";
import { useShred } from "@/contexts/ShredContext";
import { cn } from "@/lib/utils";
import type { WriteCheck } from "@/types";

const WRITE_CHECK_OPTIONS: ReadonlyArray<{
  value: WriteCheck;
  label: string;
  description: string;
}> = [
  {
    value: "off",
    label: "Off",
    description: "Skips read-back after the overwrite.",
  },
  {
    value: "spot",
    label: "Spot",
    description:
      "Checks the final overwrite at distributed locations. Small files are checked in full.",
  },
  {
    value: "full",
    label: "Full",
    description:
      "Reads back the entire final logical file range. This checks the write result, not physical-media erasure.",
  },
];

export function WriteCheckSelector() {
  const { writeCheck, setWriteCheck } = useShred();
  const selected =
    WRITE_CHECK_OPTIONS.find((option) => option.value === writeCheck) ??
    WRITE_CHECK_OPTIONS[1];

  return (
    <div className="flex flex-col gap-1.5 w-full">
      <div className="flex items-center gap-1.5">
        <span className="font-mono text-xs text-muted-foreground">
          Write Check
        </span>
        <TooltipProvider>
          <Tooltip>
            <TooltipTrigger
              render={<span className="inline-flex cursor-help" />}
            >
              <Question size={14} className="text-muted-foreground" />
            </TooltipTrigger>
            <TooltipContent>
              Final-state read-back of the overwritten logical file range,
              performed once after the last pass. The check verifies the
              write result; it cannot prove physical-media erasure.
            </TooltipContent>
          </Tooltip>
        </TooltipProvider>
      </div>
      <div className="flex w-full">
        {WRITE_CHECK_OPTIONS.map((option) => (
          <button
            key={option.value}
            type="button"
            onClick={() => setWriteCheck(option.value)}
            aria-pressed={writeCheck === option.value}
            className={cn(
              "flex-1 px-3 py-1.5 font-mono text-xs border transition-colors",
              writeCheck === option.value
                ? "bg-accent text-accent-foreground border-accent"
                : "bg-transparent text-muted-foreground border-border hover:bg-elevated hover:text-foreground"
            )}
          >
            {option.label}
          </button>
        ))}
      </div>
      <p className="font-mono text-xs text-muted-foreground">
        {selected.description}
      </p>
    </div>
  );
}
