// src/components/settings/ToggleSetting.tsx
import { Switch } from "@/components/ui/switch";

interface ToggleSettingProps {
  label: string;
  description: string;
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
}

export function ToggleSetting({
  label,
  description,
  checked,
  onCheckedChange,
}: ToggleSettingProps) {
  return (
    <div className="flex items-center justify-between border-b border-border py-4">
      <div>
        <p className="text-sm text-foreground">{label}</p>
        <p className="text-xs text-muted-foreground">{description}</p>
      </div>
      <Switch checked={checked} onCheckedChange={onCheckedChange} />
    </div>
  );
}
