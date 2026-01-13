// SessionConfigPanel - Comprehensive persistent session-level configuration

import { useState } from "react";
import { Settings2, Save, RotateCcw } from "lucide-react";
import { PresetSelector } from "./PresetSelector";
import { type ChatOverrides } from "./OverrideSettings";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { Input } from "@/components/ui/input";
import { Slider } from "@/components/ui/slider";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible";
import { updateConfig } from "@/services/api";
import { SUPPORTED_MODELS, CUSTOM_MODEL_VALUE } from "@/lib/constants";

export interface SessionConfig {
  preset: string;
  toolsEnabled: boolean;
  creativity: number;
  verbosity: string;
  maxRounds: number;
  overrides: ChatOverrides;
}

export interface SessionConfigPanelProps {
  sessionId: string | null;
  config: SessionConfig;
  onConfigChanged: (config: SessionConfig) => void;
  disabled?: boolean;
}

export function SessionConfigPanel({
  sessionId,
  config,
  onConfigChanged,
  disabled,
}: SessionConfigPanelProps) {
  const [isOpen, setIsOpen] = useState(false);
  const [localConfig, setLocalConfig] = useState(config);
  const [isSaving, setIsSaving] = useState(false);
  const [hasChanges, setHasChanges] = useState(false);

  const hasOverrides = Object.keys(localConfig.overrides).length > 0;

  const handleConfigChange = (updates: Partial<SessionConfig>) => {
    const newConfig = { ...localConfig, ...updates };
    setLocalConfig(newConfig);
    setHasChanges(JSON.stringify(newConfig) !== JSON.stringify(config));
  };

  const handleOverrideChange = (key: keyof ChatOverrides, value: any) => {
    const newOverrides = { ...localConfig.overrides };
    if (value === undefined || value === "") {
      delete newOverrides[key];
    } else {
      newOverrides[key] = value;
    }
    handleConfigChange({ overrides: newOverrides });
  };

  const handleSave = async () => {
    if (!sessionId) return;

    try {
      setIsSaving(true);

      const configToSave = {
        preset: localConfig.preset,
        tools_enabled: localConfig.toolsEnabled,
        intent: {
          creativity: localConfig.creativity,
          verbosity: localConfig.verbosity as "short" | "normal" | "long",
          rounds: localConfig.maxRounds,
        },
        overrides: localConfig.overrides,
      };

      console.log("Saving config:", configToSave);

      const result = await updateConfig(sessionId, configToSave);

      console.log("Config saved successfully:", result);

      onConfigChanged(localConfig);
      setHasChanges(false);
      setIsOpen(false); // Close the settings panel after successful save
    } catch (err) {
      console.error("Failed to save config:", err);
      const errorMessage = err instanceof Error ? err.message : String(err);
      alert(`Failed to save configuration: ${errorMessage}`);
    } finally {
      setIsSaving(false);
    }
  };

  const handleReset = () => {
    setLocalConfig(config);
    setHasChanges(false);
  };

  // Determine if current model is in the supported list
  const isCustomModel =
    localConfig.overrides.model &&
    !SUPPORTED_MODELS.some((m) => m.value === localConfig.overrides.model);
  const selectValue = isCustomModel
    ? CUSTOM_MODEL_VALUE
    : localConfig.overrides.model;

  const handleModelSelectChange = (value: string) => {
    if (value === CUSTOM_MODEL_VALUE) {
      // Switch to custom input, keep current value if it's custom
      if (!isCustomModel) {
        handleOverrideChange("model", "");
      }
    } else {
      // Use selected model from dropdown
      handleOverrideChange("model", value);
    }
  };

  return (
    <div className="border-b border-border bg-muted/30 py-2">
      <div className="max-w-4xl mx-auto px-4">
        <Collapsible open={isOpen} onOpenChange={setIsOpen}>
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-3">
              <CollapsibleTrigger asChild>
                <Button
                  variant="ghost"
                  size="sm"
                  className="flex items-center gap-2"
                  disabled={disabled || isSaving}
                >
                  <Settings2 className="w-4 h-4" />
                  <span className="text-sm font-medium">
                    Session Settings{" "}
                    {hasOverrides &&
                      `(${Object.keys(localConfig.overrides).length} overrides)`}
                  </span>
                </Button>
              </CollapsibleTrigger>
              <span className="text-xs text-muted-foreground">
                (applies to all messages in this session)
              </span>
            </div>

            {hasChanges && (
              <div className="flex items-center gap-2">
                <span className="text-xs text-amber-600 dark:text-amber-400">
                  Unsaved changes
                </span>
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={handleReset}
                  disabled={isSaving}
                  className="text-xs"
                >
                  <RotateCcw className="w-3 h-3 mr-1" />
                  Reset
                </Button>
                <Button
                  variant="default"
                  size="sm"
                  onClick={handleSave}
                  disabled={isSaving}
                  className="text-xs"
                >
                  <Save className="w-3 h-3 mr-1" />
                  {isSaving ? "Saving..." : "Save to Session"}
                </Button>
              </div>
            )}
          </div>

          <CollapsibleContent className="pt-4">
            <div className="grid grid-cols-1 md:grid-cols-2 gap-6 p-4 rounded-lg border border-border bg-background">
              {/* Left Column - Basic Settings */}
              <div className="space-y-4">
                <div>
                  <h3 className="text-sm font-semibold mb-3">Basic Settings</h3>
                  <div className="space-y-4">
                    {/* Preset */}
                    <PresetSelector
                      value={localConfig.preset}
                      onChange={(preset) => handleConfigChange({ preset })}
                      disabled={disabled || isSaving}
                    />

                    {/* Tools Enabled */}
                    <div className="flex items-center justify-between">
                      <Label htmlFor="tools-enabled" className="text-sm">
                        Enable Tools
                      </Label>
                      <Switch
                        id="tools-enabled"
                        checked={localConfig.toolsEnabled}
                        onCheckedChange={(toolsEnabled) =>
                          handleConfigChange({ toolsEnabled })
                        }
                        disabled={disabled || isSaving}
                      />
                    </div>

                    {/* Creativity */}
                    <div className="space-y-2">
                      <div className="flex items-center justify-between">
                        <Label htmlFor="creativity" className="text-sm">
                          Creativity
                        </Label>
                        <span className="text-xs text-muted-foreground">
                          {localConfig.creativity.toFixed(2)}
                        </span>
                      </div>
                      <Slider
                        id="creativity"
                        min={0}
                        max={1}
                        step={0.05}
                        value={[localConfig.creativity]}
                        onValueChange={([creativity]) =>
                          handleConfigChange({ creativity })
                        }
                        disabled={disabled || isSaving}
                      />
                      <p className="text-xs text-muted-foreground">
                        Lower values are more focused, higher values are more
                        creative
                      </p>
                    </div>

                    {/* Verbosity */}
                    <div className="space-y-2">
                      <Label htmlFor="verbosity" className="text-sm">
                        Verbosity
                      </Label>
                      <Select
                        value={localConfig.verbosity}
                        onValueChange={(verbosity) =>
                          handleConfigChange({ verbosity })
                        }
                        disabled={disabled || isSaving}
                      >
                        <SelectTrigger id="verbosity" className="h-9">
                          <SelectValue />
                        </SelectTrigger>
                        <SelectContent>
                          <SelectItem value="short">
                            Short (concise responses)
                          </SelectItem>
                          <SelectItem value="normal">
                            Normal (balanced)
                          </SelectItem>
                          <SelectItem value="long">
                            Long (detailed responses)
                          </SelectItem>
                        </SelectContent>
                      </Select>
                    </div>

                    {/* Max Rounds */}
                    <div className="space-y-2">
                      <Label htmlFor="max-rounds" className="text-sm">
                        Max Tool Rounds
                      </Label>
                      <Input
                        id="max-rounds"
                        type="number"
                        min={1}
                        max={100}
                        value={localConfig.maxRounds}
                        onChange={(e) =>
                          handleConfigChange({
                            maxRounds: parseInt(e.target.value) || 30,
                          })
                        }
                        disabled={disabled || isSaving}
                        className="h-9"
                      />
                      <p className="text-xs text-muted-foreground">
                        Maximum number of tool calling rounds (1-100)
                      </p>
                    </div>
                  </div>
                </div>
              </div>

              {/* Right Column - Advanced Overrides */}
              <div className="space-y-4">
                <div>
                  <h3 className="text-sm font-semibold mb-3">
                    Advanced Overrides
                  </h3>
                  <div className="space-y-4">
                    {/* Model Override */}
                    <div className="space-y-2">
                      <div className="flex items-center justify-between">
                        <Label htmlFor="model-override" className="text-sm">
                          Model Override
                        </Label>
                        {localConfig.overrides.model && (
                          <Button
                            variant="ghost"
                            size="sm"
                            onClick={() =>
                              handleOverrideChange("model", undefined)
                            }
                            disabled={disabled || isSaving}
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
                        disabled={disabled || isSaving}
                      >
                        <SelectTrigger className="h-9">
                          <SelectValue placeholder="Select a model..." />
                        </SelectTrigger>
                        <SelectContent>
                          {SUPPORTED_MODELS.map((model) => (
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
                      {(isCustomModel ||
                        selectValue === CUSTOM_MODEL_VALUE) && (
                        <Input
                          id="model-override"
                          type="text"
                          placeholder="e.g., gpt-4, claude-3-opus-20240229"
                          value={localConfig.overrides.model || ""}
                          onChange={(e) =>
                            handleOverrideChange(
                              "model",
                              e.target.value || undefined,
                            )
                          }
                          disabled={disabled || isSaving}
                          className="h-9"
                        />
                      )}
                    </div>

                    {/* Top P */}
                    <div className="space-y-2">
                      <div className="flex items-center justify-between">
                        <Label htmlFor="top-p" className="text-sm">
                          Top P
                          {localConfig.overrides.top_p !== undefined && (
                            <span className="ml-2 text-muted-foreground font-normal">
                              {localConfig.overrides.top_p.toFixed(2)}
                            </span>
                          )}
                        </Label>
                        {localConfig.overrides.top_p !== undefined && (
                          <Button
                            variant="ghost"
                            size="sm"
                            onClick={() =>
                              handleOverrideChange("top_p", undefined)
                            }
                            disabled={disabled || isSaving}
                            className="h-6 px-2 text-xs"
                          >
                            Clear
                          </Button>
                        )}
                      </div>
                      <Slider
                        id="top-p"
                        min={0}
                        max={1}
                        step={0.05}
                        value={[localConfig.overrides.top_p ?? 0.9]}
                        onValueChange={([value]) =>
                          handleOverrideChange("top_p", value)
                        }
                        disabled={disabled || isSaving}
                      />
                      <p className="text-xs text-muted-foreground">
                        Nucleus sampling (0.0-1.0)
                      </p>
                    </div>

                    {/* Frequency Penalty */}
                    <div className="space-y-2">
                      <div className="flex items-center justify-between">
                        <Label htmlFor="freq-penalty" className="text-sm">
                          Frequency Penalty
                          {localConfig.overrides.frequency_penalty !==
                            undefined && (
                            <span className="ml-2 text-muted-foreground font-normal">
                              {localConfig.overrides.frequency_penalty.toFixed(
                                2,
                              )}
                            </span>
                          )}
                        </Label>
                        {localConfig.overrides.frequency_penalty !==
                          undefined && (
                          <Button
                            variant="ghost"
                            size="sm"
                            onClick={() =>
                              handleOverrideChange(
                                "frequency_penalty",
                                undefined,
                              )
                            }
                            disabled={disabled || isSaving}
                            className="h-6 px-2 text-xs"
                          >
                            Clear
                          </Button>
                        )}
                      </div>
                      <Slider
                        id="freq-penalty"
                        min={-2}
                        max={2}
                        step={0.1}
                        value={[localConfig.overrides.frequency_penalty ?? 0]}
                        onValueChange={([value]) =>
                          handleOverrideChange("frequency_penalty", value)
                        }
                        disabled={disabled || isSaving}
                      />
                      <p className="text-xs text-muted-foreground">
                        Reduce repetition (-2.0 to 2.0)
                      </p>
                    </div>

                    {/* Presence Penalty */}
                    <div className="space-y-2">
                      <div className="flex items-center justify-between">
                        <Label htmlFor="presence-penalty" className="text-sm">
                          Presence Penalty
                          {localConfig.overrides.presence_penalty !==
                            undefined && (
                            <span className="ml-2 text-muted-foreground font-normal">
                              {localConfig.overrides.presence_penalty.toFixed(
                                2,
                              )}
                            </span>
                          )}
                        </Label>
                        {localConfig.overrides.presence_penalty !==
                          undefined && (
                          <Button
                            variant="ghost"
                            size="sm"
                            onClick={() =>
                              handleOverrideChange(
                                "presence_penalty",
                                undefined,
                              )
                            }
                            disabled={disabled || isSaving}
                            className="h-6 px-2 text-xs"
                          >
                            Clear
                          </Button>
                        )}
                      </div>
                      <Slider
                        id="presence-penalty"
                        min={-2}
                        max={2}
                        step={0.1}
                        value={[localConfig.overrides.presence_penalty ?? 0]}
                        onValueChange={([value]) =>
                          handleOverrideChange("presence_penalty", value)
                        }
                        disabled={disabled || isSaving}
                      />
                      <p className="text-xs text-muted-foreground">
                        Encourage new topics (-2.0 to 2.0)
                      </p>
                    </div>

                    {/* Clear All Overrides */}
                    {hasOverrides && (
                      <div className="pt-2 border-t border-border">
                        <Button
                          variant="outline"
                          size="sm"
                          onClick={() => handleConfigChange({ overrides: {} })}
                          disabled={disabled || isSaving}
                          className="w-full"
                        >
                          Clear All Overrides
                        </Button>
                      </div>
                    )}
                  </div>
                </div>
              </div>
            </div>
          </CollapsibleContent>
        </Collapsible>
      </div>
    </div>
  );
}
