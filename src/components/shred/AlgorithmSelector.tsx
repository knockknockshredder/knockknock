// src/components/shred/AlgorithmSelector.tsx
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { useShred } from "@/contexts/ShredContext";

export function AlgorithmSelector() {
  const { algorithms, algorithmIndex, setAlgorithmIndex } = useShred();

  if (algorithms.length === 0) {
    return (
      <p className="text-xs text-muted-foreground">Loading algorithms...</p>
    );
  }

  return (
    <div className="flex flex-col gap-1.5">
      <label className="font-mono text-xs text-muted-foreground">
        Algorithm
      </label>
      <Select
        value={String(algorithmIndex)}
        onValueChange={(v) => setAlgorithmIndex(Number(v))}
      >
        <SelectTrigger className="font-mono text-sm">
          <SelectValue />
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
