// src/components/shred/ShredOptions.tsx
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

interface ShredOptionsProps {
  passes: number;
  onPassesChange: (v: number) => void;
  pattern: "random" | "zeros" | "ones";
  onPatternChange: (v: "random" | "zeros" | "ones") => void;
  verificationLevel: "none" | "sample" | "full";
  onVerificationLevelChange: (v: "none" | "sample" | "full") => void;
  maxPasses: number;
}

export function ShredOptions({
  passes,
  onPassesChange,
  pattern,
  onPatternChange,
  verificationLevel,
  onVerificationLevelChange,
  maxPasses,
}: ShredOptionsProps) {
  return (
    <div className="flex flex-wrap gap-4">
      <div className="flex flex-col gap-1.5">
        <label className="font-mono text-xs text-muted-foreground">Passes</label>
        <Select
          value={String(passes)}
          onValueChange={(v) => onPassesChange(Number(v))}
        >
          <SelectTrigger className="w-[100px] font-mono text-sm">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {Array.from({ length: maxPasses }, (_, i) => i + 1).map((n) => (
              <SelectItem key={n} value={String(n)}>
                {n}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>

      <div className="flex flex-col gap-1.5">
        <label className="font-mono text-xs text-muted-foreground">Pattern</label>
        <Select
          value={pattern}
          onValueChange={(v) => v && onPatternChange(v)}
        >
          <SelectTrigger className="w-[120px] font-mono text-sm">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="random">Random</SelectItem>
            <SelectItem value="zeros">Zeros</SelectItem>
            <SelectItem value="ones">Ones</SelectItem>
          </SelectContent>
        </Select>
      </div>

      <div className="flex flex-col gap-1.5">
        <label className="font-mono text-xs text-muted-foreground">
          Verification
        </label>
        <Select
          value={verificationLevel}
          onValueChange={(v) => v && onVerificationLevelChange(v)}
        >
          <SelectTrigger className="w-[120px] font-mono text-sm">
            <SelectValue />
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
