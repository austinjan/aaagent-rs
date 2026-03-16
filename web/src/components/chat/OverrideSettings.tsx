// OverrideSettings component for advanced power user configurations

import { Label } from "@/components/ui/label";
import { Input } from "@/components/ui/input";
import { Slider } from "@/components/ui/slider";
import { Button } from "@/components/ui/button";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { CUSTOM_MODEL_VALUE } from "@/lib/constants";
import { useModels } from "@/hooks/useModels";

export interface ChatOverrides extends Record<string, unknown> {
  model?: string;
  top_p?: number;
  frequency_penalty?: number;
  presence_penalty?: number;
}

export interface OverrideSettingsProps {
  overrides: ChatOverrides;
  onChange: (overrides: ChatOverrides) => void;
  disabled?: boolean;
}

export function OverrideSettings({
  overrides,
  onChange,
  disabled,
}: OverrideSettingsProps) {
  const { availableModels, models } = useModels();

  const updateOverride = <K extends keyof ChatOverrides>(
    key: K,
    value: ChatOverrides[K],
  ) => {
    onChange({ ...overrides, [key]: value });
  };

  const clearOverride = (key: keyof ChatOverrides) => {
    const updated = { ...overrides };
    delete updated[key];
    onChange(updated);
  };

  // Determine if current model is in the supported list
  const isCustomModel =
    overrides.model &&
    !models.some((m) => m.value === overrides.model);
  const selectValue = isCustomModel ? CUSTOM_MODEL_VALUE : overrides.model;

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

  const hasOverrides = Object.keys(overrides).length > 0;

  return (
    <div className="space-y-4">
      {/* Model Override */}
      <div className="space-y-2">
        <div className="flex items-center justify-between">
          <Label htmlFor="model-override" className="text-sm font-medium">
            Model Override
          </Label>
          {overrides.model && (
            <Button
              variant="ghost"
              size="sm"
              onClick={() => clearOverride("model")}
              disabled={disabled}
              className="h-6 px-2 text-xs"
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
          <SelectTrigger className="h-9">
            <SelectValue placeholder="Select a model..." />
          </SelectTrigger>
          <SelectContent>
            {availableModels.map((model) => (
              <SelectItem key={model.value} value={model.value}>
                {model.label}
              </SelectItem>
            ))}
            <SelectItem value={CUSTOM_MODEL_VALUE}>Custom Model...</SelectItem>
          </SelectContent>
        </Select>

        {/* Custom Model Input */}
        {(isCustomModel || selectValue === CUSTOM_MODEL_VALUE) && (
          <Input
            id="model-override"
            type="text"
            placeholder="e.g., gpt-4, claude-3-opus-20240229"
            value={overrides.model || ""}
            onChange={(e) =>
              updateOverride("model", e.target.value || undefined)
            }
            disabled={disabled}
            className="h-9"
          />
        )}

        <p className="text-xs text-muted-foreground">
          {isCustomModel || selectValue === CUSTOM_MODEL_VALUE
            ? "Enter a custom model name"
            : "Select a model or choose custom"}
        </p>
      </div>

      {/* Top P */}
      <div className="space-y-2">
        <div className="flex items-center justify-between">
          <Label htmlFor="top-p-override" className="text-sm font-medium">
            Top P
            {overrides.top_p !== undefined && (
              <span className="ml-2 text-muted-foreground font-normal">
                {overrides.top_p.toFixed(2)}
              </span>
            )}
          </Label>
          {overrides.top_p !== undefined && (
            <Button
              variant="ghost"
              size="sm"
              onClick={() => clearOverride("top_p")}
              disabled={disabled}
              className="h-6 px-2 text-xs"
            >
              Clear
            </Button>
          )}
        </div>
        <Slider
          id="top-p-override"
          min={0}
          max={1}
          step={0.05}
          value={[overrides.top_p ?? 0.9]}
          onValueChange={([value]) => updateOverride("top_p", value)}
          disabled={disabled}
          className="py-2"
        />
        <p className="text-xs text-muted-foreground">
          Nucleus sampling: lower values are more focused (0.0-1.0)
        </p>
      </div>

      {/* Frequency Penalty */}
      <div className="space-y-2">
        <div className="flex items-center justify-between">
          <Label
            htmlFor="freq-penalty-override"
            className="text-sm font-medium"
          >
            Frequency Penalty
            {overrides.frequency_penalty !== undefined && (
              <span className="ml-2 text-muted-foreground font-normal">
                {overrides.frequency_penalty.toFixed(2)}
              </span>
            )}
          </Label>
          {overrides.frequency_penalty !== undefined && (
            <Button
              variant="ghost"
              size="sm"
              onClick={() => clearOverride("frequency_penalty")}
              disabled={disabled}
              className="h-6 px-2 text-xs"
            >
              Clear
            </Button>
          )}
        </div>
        <Slider
          id="freq-penalty-override"
          min={-2}
          max={2}
          step={0.1}
          value={[overrides.frequency_penalty ?? 0]}
          onValueChange={([value]) =>
            updateOverride("frequency_penalty", value)
          }
          disabled={disabled}
          className="py-2"
        />
        <p className="text-xs text-muted-foreground">
          Reduce repetition of tokens (-2.0 to 2.0)
        </p>
      </div>

      {/* Presence Penalty */}
      <div className="space-y-2">
        <div className="flex items-center justify-between">
          <Label
            htmlFor="presence-penalty-override"
            className="text-sm font-medium"
          >
            Presence Penalty
            {overrides.presence_penalty !== undefined && (
              <span className="ml-2 text-muted-foreground font-normal">
                {overrides.presence_penalty.toFixed(2)}
              </span>
            )}
          </Label>
          {overrides.presence_penalty !== undefined && (
            <Button
              variant="ghost"
              size="sm"
              onClick={() => clearOverride("presence_penalty")}
              disabled={disabled}
              className="h-6 px-2 text-xs"
            >
              Clear
            </Button>
          )}
        </div>
        <Slider
          id="presence-penalty-override"
          min={-2}
          max={2}
          step={0.1}
          value={[overrides.presence_penalty ?? 0]}
          onValueChange={([value]) => updateOverride("presence_penalty", value)}
          disabled={disabled}
          className="py-2"
        />
        <p className="text-xs text-muted-foreground">
          Encourage new topics (-2.0 to 2.0)
        </p>
      </div>

      {hasOverrides && (
        <div className="pt-2 border-t border-border">
          <Button
            variant="outline"
            size="sm"
            onClick={() => onChange({})}
            disabled={disabled}
            className="w-full"
          >
            Clear All Overrides
          </Button>
        </div>
      )}
    </div>
  );
}
