// src/components/browser/ProfileItem.tsx
import { Checkbox } from "@/components/ui/checkbox";
import { useBrowser } from "@/contexts/BrowserContext";
import type { BrowserProfile } from "@/types";

interface ProfileItemProps {
  browserId: string;
  profile: BrowserProfile;
}

export function ProfileItem({ browserId, profile }: ProfileItemProps) {
  const { toggleProfile } = useBrowser();

  return (
    <div className="flex items-center gap-3 py-2">
      <Checkbox
        checked={profile.selected}
        onCheckedChange={() => toggleProfile(browserId, profile.id)}
      />
      <div className="min-w-0 flex-1">
        <p className="truncate text-sm text-foreground">{profile.name}</p>
        <p className="truncate font-mono text-xs text-muted-foreground">
          {profile.path}
        </p>
      </div>
      <span className="font-mono text-xs text-muted-foreground">
        {(profile.size / 1048576).toFixed(0)} MB
      </span>
    </div>
  );
}
