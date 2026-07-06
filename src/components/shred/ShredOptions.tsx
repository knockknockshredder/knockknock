// src/components/shred/ShredOptions.tsx
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

interface ShredOptionsProps {
  passes: number;
  onPassesChange: (v: number) => void;
  pattern: "random" | "zeros" | "ones";
  onPatternChange: (v: "random" | "zeros" | "ones") => void;
  verificationLevel: "none" | "sample" | "full";
  onVerificationLevelChange: (v: "none" | "sample" | "full") => void;
  maxPasses: number;
  currentAlgorithm?: {
    default_passes: number;
    has_fixed_pattern_sequence: boolean;
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
    <div className="flex flex-wrap gap-4">
      <div className="flex flex-col gap-1.5">
        <div className="flex items-center gap-1.5">
          <span className="font-mono text-xs text-muted-foreground">
            Pass repeats
          </span>
          <HintTooltip text="How many times to repeat the algorithm's full pass sequence. DoD (3 passes) × 7 repeats = 21 total overwrites." />
        </div>
        <Select
          value={String(passes)}
          onValueChange={(v) => onPassesChange(Number(v))}
        >
          <SelectTrigger className="w-[100px] font-mono text-sm">
            <SelectValue placeholder={String(passes)} />
          </SelectTrigger>
          <SelectContent>
            {Array.from({ length: maxPasses }, (_, i) => i + 1).map((n) => (
              <SelectItem key={n} value={String(n)}>
                {n}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
        {currentAlgorithm?.has_fixed_pattern_sequence && (
          <span className="font-mono text-xs text-muted-foreground">
            {currentAlgorithm.default_passes} passes × {passes} repeats ={" "}
            {currentAlgorithm.default_passes * passes} total
          </span>
        )}
      </div>

      <div className="flex flex-col gap-1.5">
        <div className="flex items-center gap-1.5">
          <span className="font-mono text-xs text-muted-foreground">
            Pattern
          </span>
          <HintTooltip text="Byte pattern used for each overwrite pass. Random is most secure. Zeros/Ones are deterministic patterns used by some standards." />
        </div>
        <Select
          value={pattern}
          onValueChange={(v) => v && onPatternChange(v)}
        >
          <SelectTrigger className="w-[120px] font-mono text-sm">
            <SelectValue placeholder="Pattern" />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="random">Random</SelectItem>
            <SelectItem value="zeros">Zeros</SelectItem>
            <SelectItem value="ones">Ones</SelectItem>
          </SelectContent>
        </Select>
      </div>

      <div className="flex flex-col gap-1.5">
        <div className="flex items-center gap-1.5">
          <span className="font-mono text-xs text-muted-foreground">
            Verification
          </span>
          <HintTooltip text="How thoroughly to verify that data was actually overwritten. None skips verification. Sample checks random blocks. Full reads back every block." />
        </div>
        <Select
          value={verificationLevel}
          onValueChange={(v) => v && onVerificationLevelChange(v)}
        >
          <SelectTrigger className="w-[120px] font-mono text-sm">
            <SelectValue placeholder="Verification" />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="none">None</SelectItem>
            <SelectItem value="sample">Sample</SelectItem>
            <SelectItem value="full">Full</SelectItem>
          </SelectContent>
        </Select>
      </div>
    </div>
  );
}