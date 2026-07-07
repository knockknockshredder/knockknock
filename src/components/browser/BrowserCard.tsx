// src/components/browser/BrowserCard.tsx
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { ProfileItem } from "./ProfileItem";
import type { DetectedBrowser } from "@/types";

const BROWSER_COLORS: Record<string, string> = {
  Chrome: "#4285F4",
  Firefox: "#FF7139",
  Edge: "#0078D7",
  Brave: "#FB542B",
  Opera: "#FF1B2D",
  Vivaldi: "#EF3939",
  Safari: "#006CFF",
};

function BrowserIcon({ name }: { name: string }) {
  const color = BROWSER_COLORS[name] ?? "#6B7280";
  const letter = name.charAt(0).toUpperCase();
  return (
    <div
      className="flex h-5 w-5 shrink-0 items-center justify-center rounded-full font-mono text-[10px] font-bold text-white"
      style={{ backgroundColor: color }}
    >
      {letter}
    </div>
  );
}

export function BrowserCard({ browser }: { browser: DetectedBrowser }) {
  return (
    <Card>
      <CardHeader className="pb-2">
        <div className="flex items-center gap-2">
          <BrowserIcon name={browser.name} />
          <CardTitle className="font-mono text-sm">{browser.name}</CardTitle>
        </div>
        {browser.isRunning && (
          <p className="text-xs text-amber-500">Browser is currently running</p>
        )}
      </CardHeader>
      <CardContent>
        {browser.profiles.map((profile) => (
          <ProfileItem
            key={profile.id}
            browserId={browser.id}
            profile={profile}
          />
        ))}
      </CardContent>
    </Card>
  );
}
