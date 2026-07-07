// src/components/browser/BrowserCard.tsx
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { ProfileItem } from "./ProfileItem";
import type { DetectedBrowser } from "@/types";

export function BrowserCard({ browser }: { browser: DetectedBrowser }) {
  return (
    <Card>
      <CardHeader className="pb-2">
        <CardTitle className="font-mono text-sm">{browser.name}</CardTitle>
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
