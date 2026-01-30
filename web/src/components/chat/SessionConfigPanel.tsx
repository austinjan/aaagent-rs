// SessionConfigPanel - persistent session configuration

import { useEffect, useState } from "react";
import { Settings2, Save, RotateCcw } from "lucide-react";
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
import { Textarea } from "@/components/ui/textarea";
import { updateConfig } from "@/services/api";
import { SUPPORTED_MODELS, CUSTOM_MODEL_VALUE } from "@/lib/constants";

export interface SessionConfig {
  provider: {
    model: string;
    temperature: number;
    max_tokens: number;
    top_p?: number;
    frequency_penalty?: number;
    presence_penalty?: number;
    enable_grounding?: boolean;
  };
  agent: {
    max_rounds: number;
    tools_enabled: boolean;
  };
  session: {
    system_prompt: string;
    max_context_tokens: number;
  };
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

  useEffect(() => {
    setLocalConfig(config);
    setHasChanges(false);
  }, [config]);

  const updateConfigState = (nextConfig: SessionConfig) => {
    setLocalConfig(nextConfig);
    setHasChanges(JSON.stringify(nextConfig) !== JSON.stringify(config));
  };

  const updateProvider = (updates: Partial<SessionConfig["provider"]>) => {
    updateConfigState({
      ...localConfig,
      provider: { ...localConfig.provider, ...updates },
    });
  };

  const updateAgent = (updates: Partial<SessionConfig["agent"]>) => {
    updateConfigState({
      ...localConfig,
      agent: { ...localConfig.agent, ...updates },
    });
  };

  const updateSession = (updates: Partial<SessionConfig["session"]>) => {
    updateConfigState({
      ...localConfig,
      session: { ...localConfig.session, ...updates },
    });
  };

  const handleSave = async () => {
    if (!sessionId) return;

    try {
      setIsSaving(true);
      const result = await updateConfig(sessionId, localConfig);
      onConfigChanged(result);
      setHasChanges(false);
      setIsOpen(false);
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

  const isCustomModel =
    localConfig.provider.model &&
    !SUPPORTED_MODELS.some((m) => m.value === localConfig.provider.model);
  const selectValue = isCustomModel
    ? CUSTOM_MODEL_VALUE
    : localConfig.provider.model;

  const handleModelSelectChange = (value: string) => {
    if (value === CUSTOM_MODEL_VALUE) {
      if (!isCustomModel) {
        updateProvider({ model: "" });
      }
    } else {
      updateProvider({ model: value });
    }
  };

  return (
    <div className="border-b border-border bg-muted/30 py-2">
      <div className="max-w-4xl mx-auto px-4">
        <Collapsible open={isOpen} onOpenChange={setIsOpen}>
          <div className="flex flex-col gap-3 md:flex-row md:items-center md:justify-between">
            <div className="flex items-center gap-3">
              <CollapsibleTrigger asChild>
                <Button
                  variant="ghost"
                  size="sm"
                  className="flex items-center gap-2"
                  disabled={disabled || isSaving}
                >
                  <Settings2 className="w-4 h-4" />
                  <span className="text-sm font-medium">Session Settings</span>
                </Button>
              </CollapsibleTrigger>
              <span className="text-xs text-muted-foreground">
                (applies to all messages in this session)
              </span>
            </div>

            <div className="flex items-center gap-2">
              <Label htmlFor="quick-model" className="text-xs">
                Model
              </Label>
              <Select
                value={selectValue}
                onValueChange={handleModelSelectChange}
                disabled={disabled || isSaving}
              >
                <SelectTrigger id="quick-model" className="h-8 min-w-[12rem]">
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
              {(isCustomModel || selectValue === CUSTOM_MODEL_VALUE) && (
                <Input
                  id="quick-model-custom"
                  type="text"
                  placeholder="custom-model"
                  value={localConfig.provider.model || ""}
                  onChange={(e) => updateProvider({ model: e.target.value })}
                  disabled={disabled || isSaving}
                  className="h-8"
                />
              )}
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
              <div className="space-y-4">
                <div>
                  <h3 className="text-sm font-semibold mb-3">Provider</h3>
                  <div className="space-y-4">
                    <div className="space-y-2">
                      <div className="flex items-center justify-between">
                        <Label htmlFor="temperature" className="text-sm">
                          Temperature
                        </Label>
                        <span className="text-xs text-muted-foreground">
                          {localConfig.provider.temperature.toFixed(2)}
                        </span>
                      </div>
                      <Slider
                        id="temperature"
                        min={0}
                        max={2}
                        step={0.05}
                        value={[localConfig.provider.temperature]}
                        onValueChange={([temperature]) =>
                          updateProvider({ temperature })
                        }
                        disabled={disabled || isSaving}
                      />
                    </div>

                    <div className="space-y-2">
                      <Label htmlFor="max-tokens" className="text-sm">
                        Max Tokens
                      </Label>
                      <Input
                        id="max-tokens"
                        type="number"
                        min={1}
                        value={localConfig.provider.max_tokens}
                        onChange={(e) =>
                          updateProvider({
                            max_tokens: parseInt(e.target.value) || 1,
                          })
                        }
                        disabled={disabled || isSaving}
                        className="h-9"
                      />
                    </div>

                    <div className="space-y-2">
                      <Label htmlFor="top-p" className="text-sm">
                        Top P
                      </Label>
                      <Input
                        id="top-p"
                        type="number"
                        step={0.05}
                        min={0}
                        max={1}
                        value={localConfig.provider.top_p ?? ""}
                        onChange={(e) =>
                          updateProvider({
                            top_p:
                              e.target.value === ""
                                ? undefined
                                : parseFloat(e.target.value),
                          })
                        }
                        disabled={disabled || isSaving}
                        className="h-9"
                      />
                    </div>

                    <div className="space-y-2">
                      <Label htmlFor="freq-penalty" className="text-sm">
                        Frequency Penalty
                      </Label>
                      <Input
                        id="freq-penalty"
                        type="number"
                        step={0.1}
                        min={-2}
                        max={2}
                        value={localConfig.provider.frequency_penalty ?? ""}
                        onChange={(e) =>
                          updateProvider({
                            frequency_penalty:
                              e.target.value === ""
                                ? undefined
                                : parseFloat(e.target.value),
                          })
                        }
                        disabled={disabled || isSaving}
                        className="h-9"
                      />
                    </div>

                    <div className="space-y-2">
                      <Label htmlFor="presence-penalty" className="text-sm">
                        Presence Penalty
                      </Label>
                      <Input
                        id="presence-penalty"
                        type="number"
                        step={0.1}
                        min={-2}
                        max={2}
                        value={localConfig.provider.presence_penalty ?? ""}
                        onChange={(e) =>
                          updateProvider({
                            presence_penalty:
                              e.target.value === ""
                                ? undefined
                                : parseFloat(e.target.value),
                          })
                        }
                        disabled={disabled || isSaving}
                        className="h-9"
                      />
                    </div>
                  </div>
                </div>
              </div>

              <div className="space-y-4">
                <div>
                  <h3 className="text-sm font-semibold mb-3">Agent</h3>
                  <div className="space-y-4">
                    <div className="flex items-center justify-between">
                      <Label htmlFor="tools-enabled" className="text-sm">
                        Enable Tools
                      </Label>
                      <Switch
                        id="tools-enabled"
                        checked={localConfig.agent.tools_enabled}
                        onCheckedChange={(tools_enabled) =>
                          updateAgent({ tools_enabled })
                        }
                        disabled={disabled || isSaving}
                      />
                    </div>

                    <div className="space-y-2">
                      <Label htmlFor="max-rounds" className="text-sm">
                        Max Tool Rounds
                      </Label>
                      <Input
                        id="max-rounds"
                        type="number"
                        min={1}
                        max={100}
                        value={localConfig.agent.max_rounds}
                        onChange={(e) =>
                          updateAgent({
                            max_rounds: parseInt(e.target.value) || 1,
                          })
                        }
                        disabled={disabled || isSaving}
                        className="h-9"
                      />
                    </div>
                  </div>
                </div>

                <div>
                  <h3 className="text-sm font-semibold mb-3">Session</h3>
                  <div className="space-y-4">
                    <div className="space-y-2">
                      <Label htmlFor="system-prompt" className="text-sm">
                        System Prompt
                      </Label>
                      <Textarea
                        id="system-prompt"
                        value={localConfig.session.system_prompt}
                        onChange={(e) =>
                          updateSession({ system_prompt: e.target.value })
                        }
                        disabled={disabled || isSaving}
                        className="min-h-[120px]"
                      />
                    </div>

                    <div className="space-y-2">
                      <Label htmlFor="max-context" className="text-sm">
                        Max Context Tokens
                      </Label>
                      <Input
                        id="max-context"
                        type="number"
                        min={1}
                        value={localConfig.session.max_context_tokens}
                        onChange={(e) =>
                          updateSession({
                            max_context_tokens: parseInt(e.target.value) || 1,
                          })
                        }
                        disabled={disabled || isSaving}
                        className="h-9"
                      />
                    </div>
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
