// src/components/browser/BrowserCard.tsx
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { ProfileItem } from "./ProfileItem";
import { useBrowser } from "@/contexts/BrowserContext";
import type { DetectedBrowser } from "@/types";

export function BrowserCard({ browser }: { browser: DetectedBrowser }) {
  const { selectAllProfiles, deselectAllProfiles } = useBrowser();

  return (
    <Card>
      <CardHeader className="pb-2">
        <div className="flex items-center justify-between">
          <CardTitle className="font-mono text-sm">{browser.name}</CardTitle>
          <div className="flex gap-2">
            <button
              onClick={() => selectAllProfiles(browser.id)}
              className="text-xs text-accent hover:underline"
            >
              Select all
            </button>
            <button
              onClick={() => deselectAllProfiles(browser.id)}
              className="text-xs text-muted-foreground hover:underline"
            >
              Deselect all
            </button>
          </div>
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
