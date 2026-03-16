// TemporaryConfigPanel - Simple per-message override (one-time use)

import { useState } from "react";
import { Zap, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { type ChatOverrides } from "./OverrideSettings";
import { SUPPORTED_MODELS, CUSTOM_MODEL_VALUE } from "@/lib/constants";
import { useProvidersStatus } from "@/hooks/useProvidersStatus";

export interface TemporaryConfig {
  overrides: ChatOverrides;
}

export interface TemporaryConfigPanelProps {
  config: TemporaryConfig;
  onChange: (config: TemporaryConfig) => void;
  disabled?: boolean;
}

export function TemporaryConfigPanel({
  config,
  onChange,
  disabled,
}: TemporaryConfigPanelProps) {
  const [isOpen, setIsOpen] = useState(false);
  const hasOverrides = Object.keys(config.overrides).length > 0;
  const { status: providersStatus, loaded: providersLoaded } = useProvidersStatus();

  const updateOverride = (key: keyof ChatOverrides, value: any) => {
    const newOverrides = { ...config.overrides };
    if (value === undefined || value === "") {
      delete newOverrides[key];
    } else {
      newOverrides[key] = value;
    }
    onChange({ overrides: newOverrides });
  };

  const clearAll = () => {
    onChange({ overrides: {} });
  };

  // Determine if current model is in the supported list
  const isCustomModel =
    config.overrides.model &&
    !SUPPORTED_MODELS.some((m) => m.value === config.overrides.model);
  const selectValue = isCustomModel
    ? CUSTOM_MODEL_VALUE
    : config.overrides.model ?? "";

  const handleModelSelectChange = (value: string) => {
    if (value === CUSTOM_MODEL_VALUE) {
      // Switch to custom input, keep current value if it's custom
      if (!isCustomModel) {
        updateOverride("model", "");
      }
    } else {
      // Use selected model from dropdown
      updateOverride("model", value);
    }
  };

  return (
    <Collapsible open={isOpen} onOpenChange={setIsOpen}>
      <div className="flex items-center gap-2">
        <CollapsibleTrigger asChild>
          <Button
            variant={hasOverrides ? "secondary" : "ghost"}
            size="sm"
            className="flex items-center gap-2"
            disabled={disabled}
          >
            <Zap className="w-3 h-3" />
            <span className="text-xs">
              {hasOverrides
                ? `One-time Override (${Object.keys(config.overrides).length})`
                : "One-time Override"}
            </span>
          </Button>
        </CollapsibleTrigger>
        {hasOverrides && (
          <Button
            variant="ghost"
            size="sm"
            onClick={(e) => {
              e.stopPropagation();
              clearAll();
            }}
            disabled={disabled}
            className="h-7 px-2"
          >
            <X className="w-3 h-3" />
          </Button>
        )}
      </div>

      <CollapsibleContent className="pt-3">
        <div className="rounded-lg border border-border bg-muted/20 p-3 space-y-3">
          <div className="flex items-center gap-2 text-xs text-muted-foreground">
            <Zap className="w-3 h-3" />
            <span>This override applies only to the next message</span>
          </div>

          {/* Model Override */}
          <div className="space-y-1.5">
            <div className="flex items-center justify-between">
              <Label htmlFor="temp-model" className="text-xs">
                Model
              </Label>
              {config.overrides.model && (
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={() => updateOverride("model", undefined)}
                  disabled={disabled}
                  className="h-5 px-1.5 text-xs"
                >
                  Clear
                </Button>
              )}
            </div>

            {/* Model Selector */}
            <Select
              value={selectValue}
              onValueChange={handleModelSelectChange}
              disabled={disabled}
            >
              <SelectTrigger className="h-8 text-xs">
                <SelectValue placeholder="Select a model..." />
              </SelectTrigger>
              <SelectContent>
                {SUPPORTED_MODELS.filter(
                  (model) => !providersLoaded || providersStatus[model.provider],
                ).map((model) => (
                  <SelectItem key={model.value} value={model.value}>
                    {model.label}
                  </SelectItem>
                ))}
                <SelectItem value={CUSTOM_MODEL_VALUE}>
                  Custom Model...
                </SelectItem>
              </SelectContent>
            </Select>

            {/* Custom Model Input */}
            {(isCustomModel || selectValue === CUSTOM_MODEL_VALUE) && (
              <Input
                id="temp-model"
                type="text"
                placeholder="e.g., gpt-4, claude-3-opus-20240229"
                value={config.overrides.model || ""}
                onChange={(e) =>
                  updateOverride("model", e.target.value || undefined)
                }
                disabled={disabled}
                className="h-8 text-xs"
              />
            )}
          </div>

          {hasOverrides && (
            <Button
              variant="outline"
              size="sm"
              onClick={clearAll}
              disabled={disabled}
              className="w-full h-7 text-xs"
            >
              Clear All Overrides
            </Button>
          )}
        </div>
      </CollapsibleContent>
    </Collapsible>
  );
}
