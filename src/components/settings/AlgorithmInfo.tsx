// src/components/settings/AlgorithmInfo.tsx
import { Badge } from "@/components/ui/badge";
import type { AlgorithmOption } from "@/types";

export function AlgorithmInfo({ algo }: { algo: AlgorithmOption }) {
  return (
    <div className="rounded border border-border bg-surface p-4">
      <div className="flex items-center gap-2">
        <h3 className="font-mono text-sm font-semibold text-foreground">
          {algo.name}
        </h3>
        <Badge variant="outline" className="font-mono text-xs">
          {algo.default_passes} pass{algo.default_passes !== 1 ? "es" : ""}
        </Badge>
      </div>
      <p className="mt-1 text-xs text-muted-foreground">{algo.description}</p>
      <p className="mt-2 font-mono text-xs text-muted-foreground">
        Max passes: {algo.max_passes} · Patterns: {algo.accepted_patterns.join(", ")}
      </p>
    </div>
  );
}
