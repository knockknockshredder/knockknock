// src/components/browser/BrowserCard.tsx
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { ProfileItem } from "./ProfileItem";
import type { DetectedBrowser } from "@/types";
import {
  siGooglechrome,
  siFirefoxbrowser,
  siBrave,
  siOpera,
  siSafari,
  siVivaldi,
  siTorbrowser,
} from "simple-icons";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import { faEdge, faInternetExplorer } from "@fortawesome/free-brands-svg-icons";

// Simple Icons SVG path extraction
function siPath(icon: { svg: string }): string {
  return icon.svg.match(/d="([^"]+)"/)?.[1] ?? "";
}

// Browser → Simple Icons mapping (white fill)
const SI_BROWSERS: Record<string, string> = {
  Chrome: siPath(siGooglechrome),
  Chromium: siPath(siGooglechrome), // Chromium uses Chrome logo
  Firefox: siPath(siFirefoxbrowser),
  Brave: siPath(siBrave),
  Opera: siPath(siOpera),
  Safari: siPath(siSafari),
  Vivaldi: siPath(siVivaldi),
  "Tor Browser": siPath(siTorbrowser),
};

function BrowserIcon({ name }: { name: string }) {
  // FontAwesome icons for Edge and IE (slightly larger to match Simple Icons visual weight)
  if (name === "Edge") {
    return <FontAwesomeIcon icon={faEdge} className="h-[22px] w-[22px] shrink-0 text-white" />;
  }
  if (name === "Internet Explorer") {
    return <FontAwesomeIcon icon={faInternetExplorer} className="h-[22px] w-[22px] shrink-0 text-white" />;
  }

  // Simple Icons SVGs (white)
  const pathData = SI_BROWSERS[name];
  if (pathData) {
    return (
      <svg
        role="img"
        viewBox="0 0 24 24"
        className="h-5 w-5 shrink-0 fill-white"
      >
        <title>{name}</title>
        <path d={pathData} />
      </svg>
    );
  }

  // Fallback: gray circle with first letter
  return (
    <div className="flex h-5 w-5 shrink-0 items-center justify-center rounded-full bg-gray-500 font-mono text-[10px] font-bold text-white">
      {name.charAt(0).toUpperCase()}
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
