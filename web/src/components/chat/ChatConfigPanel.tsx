// ChatConfigPanel - combines preset selector and override settings

import { useState } from "react";
import { Settings2 } from "lucide-react";
import { PresetSelector } from "./PresetSelector";
import { OverrideSettings, type ChatOverrides } from "./OverrideSettings";
import { Button } from "@/components/ui/button";
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible";

export interface ChatConfig {
  preset: string;
  overrides: ChatOverrides;
}

export interface ChatConfigPanelProps {
  config: ChatConfig;
  onChange: (config: ChatConfig) => void;
  disabled?: boolean;
}

export function ChatConfigPanel({
  config,
  onChange,
  disabled,
}: ChatConfigPanelProps) {
  const [isOpen, setIsOpen] = useState(false);
  const hasOverrides = Object.keys(config.overrides).length > 0;

  return (
    <Collapsible open={isOpen} onOpenChange={setIsOpen}>
      <div className="flex items-center gap-3">
        <PresetSelector
          value={config.preset}
          onChange={(preset) => onChange({ ...config, preset })}
          disabled={disabled}
        />
        <CollapsibleTrigger asChild>
          <Button
            variant="ghost"
            size="sm"
            className="flex items-center gap-2 text-muted-foreground hover:text-foreground"
            disabled={disabled}
          >
            <Settings2 className="w-4 h-4" />
            <span className="text-sm">
              Advanced Settings{" "}
              {hasOverrides && `(${Object.keys(config.overrides).length})`}
            </span>
          </Button>
        </CollapsibleTrigger>
      </div>

      <CollapsibleContent className="pt-4">
        <div className="rounded-lg border border-border bg-muted/30 p-4 space-y-4">
          <OverrideSettings
            overrides={config.overrides}
            onChange={(overrides) => onChange({ ...config, overrides })}
            disabled={disabled}
          />
        </div>
      </CollapsibleContent>
    </Collapsible>
  );
}
