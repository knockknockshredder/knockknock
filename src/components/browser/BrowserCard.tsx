// src/components/browser/BrowserCard.tsx
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { ProfileItem } from "./ProfileItem";
import type { DetectedBrowser } from "@/types";
import {
  siGooglechrome,
  siFirefox,
  siBrave,
  siOpera,
  siSafari,
  siVivaldi,
  siTorbrowser,
} from "simple-icons";

// Simple Icons SVG data for browsers available in the package
const SIMPLE_ICONS: Record<string, { svg: string; hex: string }> = {
  Chrome: siGooglechrome,
  Firefox: siFirefox,
  Brave: siBrave,
  Opera: siOpera,
  Safari: siSafari,
  Vivaldi: siVivaldi,
  "Tor Browser": siTorbrowser,
};

// Custom SVG paths for browsers not in simple-icons
const CUSTOM_BROWSER_ICONS: Record<string, { path: string; viewBox?: string; hex: string }> = {
  Edge: {
    path: "M12 2C6.477 2 2 6.477 2 12c0 2.125.67 4.095 1.81 5.705C5.09 19.09 6.82 20 9 20c3.314 0 6-2.686 6-6 0-1.306-.425-2.51-1.14-3.49C15.09 8.42 17.36 7 20 7c.74 0 1.46.08 2.15.24C21.15 4.47 16.9 2 12 2zm-1.5 14c-1.933 0-3.5-1.567-3.5-3.5 0-1.57 1.04-2.88 2.44-3.36.28-.1.58-.14.88-.14 1.11 0 2.12.46 2.85 1.19.37.37.67.81.87 1.3C13.34 12.84 13 14.36 11.5 16z",
    hex: "#0078D7",
  },
  "Internet Explorer": {
    path: "M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-1 17.93c-3.95-.49-7-3.85-7-7.93 0-.62.08-1.21.21-1.79L9 15v1c0 1.1.9 2 2 2v1.93zm6.9-2.54c-.26-.81-1-1.39-1.9-1.39h-1v-3c0-.55-.45-1-1-1H8v-2h2c.55 0 1-.45 1-1V7h2c1.1 0 2-.9 2-2v-.41c2.93 1.19 5 4.06 5 7.41 0 2.08-.8 3.97-2.1 5.39z",
    hex: "#0076D7",
  },
  Chromium: {
    path: "M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-1 17.93c-3.95-.49-7-3.85-7-7.93 0-.62.08-1.21.21-1.79L9 15v1c0 1.1.9 2 2 2v1.93zm6.9-2.54c-.26-.81-1-1.39-1.9-1.39h-1v-3c0-.55-.45-1-1-1H8v-2h2c.55 0 1-.45 1-1V7h2c1.1 0 2-.9 2-2v-.41c2.93 1.19 5 4.06 5 7.41 0 2.08-.8 3.97-2.1 5.39z",
    hex: "#4285F4",
  },
};

function BrowserIcon({ name }: { name: string }) {
  // Try Simple Icons first
  const simpleIcon = SIMPLE_ICONS[name];
  if (simpleIcon) {
    return (
      <svg
        role="img"
        viewBox="0 0 24 24"
        className="h-5 w-5 shrink-0"
        fill={`#${simpleIcon.hex}`}
      >
        <title>{name}</title>
        <path d={simpleIcon.svg.match(/d="([^"]+)"/)?.[1] ?? ""} />
      </svg>
    );
  }

  // Try custom icons
  const customIcon = CUSTOM_BROWSER_ICONS[name];
  if (customIcon) {
    return (
      <svg
        viewBox="0 0 24 24"
        className="h-5 w-5 shrink-0"
        fill={customIcon.hex}
      >
        <title>{name}</title>
        <path d={customIcon.path} />
      </svg>
    );
  }

  // Fallback: colored circle with first letter
  const color = "#6B7280";
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
