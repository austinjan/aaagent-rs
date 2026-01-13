// PresetSelector component for quick chat configuration

import {
  MessageCircle,
  Code,
  FlaskConical,
  Zap,
  type LucideIcon,
} from "lucide-react";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Label } from "@/components/ui/label";

export interface PresetOption {
  value: string;
  label: string;
  Icon: LucideIcon;
}

export const PRESETS: PresetOption[] = [
  { value: "general", label: "General Assistant", Icon: MessageCircle },
  { value: "coding", label: "Software Engineer", Icon: Code },
  { value: "research", label: "Research Assistant", Icon: FlaskConical },
  { value: "quick", label: "Quick & Efficient", Icon: Zap },
];

export interface PresetSelectorProps {
  value: string;
  onChange: (value: string) => void;
  disabled?: boolean;
}

export function PresetSelector({
  value,
  onChange,
  disabled,
}: PresetSelectorProps) {
  return (
    <div className="flex items-center gap-2">
      <Label className="text-sm text-muted-foreground">Preset:</Label>
      <Select value={value} onValueChange={onChange} disabled={disabled}>
        <SelectTrigger className="w-[220px] h-9">
          <SelectValue placeholder="Select preset..." />
        </SelectTrigger>
        <SelectContent>
          {PRESETS.map((preset) => {
            const Icon = preset.Icon;
            return (
              <SelectItem key={preset.value} value={preset.value}>
                <div className="flex items-center gap-2">
                  <Icon className="w-4 h-4" />
                  <span>{preset.label}</span>
                </div>
              </SelectItem>
            );
          })}
        </SelectContent>
      </Select>
    </div>
  );
}
