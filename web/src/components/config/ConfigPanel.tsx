import { useEffect, useState } from "react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { Slider } from "@/components/ui/slider";
import { Switch } from "@/components/ui/switch";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Loader2 } from "lucide-react";
import { getSessionConfig, updateSessionConfig } from "@/lib/api";
import type { SessionConfig } from "@/lib/api";
import { SUPPORTED_MODELS, CUSTOM_MODEL_VALUE, getProviderForModel } from "@/lib/constants";
import { useProvidersStatus } from "@/hooks/useProvidersStatus";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

interface ConfigPanelProps {
  sessionId?: string;
  existingConfig?: SessionConfig;
  onSubmit?: (config: SessionConfig) => void;
  onReset?: () => void;
}

const DEFAULT_CONFIG: SessionConfig = {
  provider: {
    model: "gpt-5-mini",
    temperature: 1.0,
    max_tokens: 16384,
    top_p: undefined,
    frequency_penalty: undefined,
    presence_penalty: undefined,
  },
  agent: {
    max_rounds: 30,
    tools_enabled: true,
  },
  session: {
    system_prompt: "You are a helpful, friendly assistant.",
    max_context_tokens: 200000,
  },
};

export function ConfigPanel({
  sessionId,
  existingConfig,
  onSubmit,
  onReset,
}: ConfigPanelProps) {
  const [config, setConfig] = useState<SessionConfig>(
    existingConfig ?? DEFAULT_CONFIG,
  );
  const [isLoading, setIsLoading] = useState<boolean>(false);
  const [isSaving, setIsSaving] = useState<boolean>(false);
  const [error, setError] = useState<string | null>(null);
  const [successMessage, setSuccessMessage] = useState<string | null>(null);
  const { status: providersStatus, loaded: providersLoaded } = useProvidersStatus();

  // Check if the currently selected model's provider has an API key
  const selectedProvider = getProviderForModel(config.provider.model);
  const isSelectedProviderAvailable = providersLoaded ? providersStatus[selectedProvider] : true;

  useEffect(() => {
    if (!sessionId) {
      setConfig(existingConfig ?? DEFAULT_CONFIG);
      return;
    }

    const loadConfig = async () => {
      setIsLoading(true);
      setError(null);

      try {
        const response = await getSessionConfig(sessionId);
        setConfig(response.session_config);
      } catch (err) {
        setError(err instanceof Error ? err.message : "Failed to load config");
      } finally {
        setIsLoading(false);
      }
    };

    loadConfig();
  }, [sessionId, existingConfig]);

  const updateProvider = (updates: Partial<SessionConfig["provider"]>) => {
    setConfig((prev) => ({
      ...prev,
      provider: { ...prev.provider, ...updates },
    }));
  };

  const updateAgent = (updates: Partial<SessionConfig["agent"]>) => {
    setConfig((prev) => ({
      ...prev,
      agent: { ...prev.agent, ...updates },
    }));
  };

  const updateSession = (updates: Partial<SessionConfig["session"]>) => {
    setConfig((prev) => ({
      ...prev,
      session: { ...prev.session, ...updates },
    }));
  };

  const handleSubmit = async () => {
    onSubmit?.(config);

    if (!sessionId) return;

    setIsSaving(true);
    setError(null);
    setSuccessMessage(null);

    try {
      await updateSessionConfig(sessionId, config);
      setSuccessMessage("Configuration updated successfully!");
      setTimeout(() => setSuccessMessage(null), 3000);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to update config");
    } finally {
      setIsSaving(false);
    }
  };

  const handleResetClick = () => {
    setConfig(existingConfig ?? DEFAULT_CONFIG);
    setError(null);
    setSuccessMessage(null);
    onReset?.();
  };

  const isCustomModel =
    config.provider.model &&
    !SUPPORTED_MODELS.some((m) => m.value === config.provider.model);
  const selectValue = isCustomModel ? CUSTOM_MODEL_VALUE : config.provider.model;

  return (
    <Card className="bg-background border">
      <CardHeader>
        <CardTitle className="text-foreground">Session Config</CardTitle>
      </CardHeader>
      <CardContent className="space-y-6">
        {providersLoaded && !isSelectedProviderAvailable && (
          <div className="text-sm text-yellow-400 border border-yellow-400/30 rounded px-3 py-2">
            No API key configured for <strong>{selectedProvider}</strong>. Set{" "}
            <code className="text-xs">{selectedProvider.toUpperCase()}_API_KEY</code>{" "}
            environment variable or configure in config.yaml.
          </div>
        )}
        {error && (
          <div className="text-sm text-red-400">{error}</div>
        )}
        {successMessage && (
          <div className="text-sm text-green-400">{successMessage}</div>
        )}

        {isLoading ? (
          <div className="flex items-center gap-2 text-sm text-muted-foreground">
            <Loader2 className="h-4 w-4 animate-spin" />
            Loading config...
          </div>
        ) : (
          <div className="space-y-6">
            <div className="space-y-4">
              <h3 className="text-sm font-semibold text-foreground">Provider</h3>
              <div className="space-y-2">
                <Label htmlFor="provider-model">Model</Label>
                <Select
                  value={selectValue}
                  onValueChange={(value) => {
                    if (value === CUSTOM_MODEL_VALUE) {
                      if (!isCustomModel) {
                        updateProvider({ model: "" });
                      }
                    } else {
                      updateProvider({ model: value });
                    }
                  }}
                >
                  <SelectTrigger id="provider-model">
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
                    <SelectItem value={CUSTOM_MODEL_VALUE}>Custom Model...</SelectItem>
                  </SelectContent>
                </Select>
                {(isCustomModel || selectValue === CUSTOM_MODEL_VALUE) && (
                  <Input
                    id="provider-model-custom"
                    value={config.provider.model}
                    onChange={(e) => updateProvider({ model: e.target.value })}
                    placeholder="custom-model"
                  />
                )}
              </div>

              <div className="space-y-2">
                <div className="flex items-center justify-between">
                  <Label htmlFor="provider-temp">Temperature</Label>
                  <span className="text-xs text-muted-foreground">
                    {config.provider.temperature.toFixed(2)}
                  </span>
                </div>
                <Slider
                  id="provider-temp"
                  min={0}
                  max={2}
                  step={0.05}
                  value={[config.provider.temperature]}
                  onValueChange={([temperature]) => updateProvider({ temperature })}
                />
              </div>

              <div className="space-y-2">
                <Label htmlFor="provider-max-tokens">Max Tokens</Label>
                <Input
                  id="provider-max-tokens"
                  type="number"
                  min={1}
                  value={config.provider.max_tokens}
                  onChange={(e) =>
                    updateProvider({
                      max_tokens: parseInt(e.target.value) || 1,
                    })
                  }
                />
              </div>

              <div className="space-y-2">
                <Label htmlFor="provider-top-p">Top P</Label>
                <Input
                  id="provider-top-p"
                  type="number"
                  step={0.05}
                  min={0}
                  max={1}
                  value={config.provider.top_p ?? ""}
                  onChange={(e) =>
                    updateProvider({
                      top_p:
                        e.target.value === ""
                          ? undefined
                          : parseFloat(e.target.value),
                    })
                  }
                />
              </div>

              <div className="space-y-2">
                <Label htmlFor="provider-freq-penalty">Frequency Penalty</Label>
                <Input
                  id="provider-freq-penalty"
                  type="number"
                  step={0.1}
                  min={-2}
                  max={2}
                  value={config.provider.frequency_penalty ?? ""}
                  onChange={(e) =>
                    updateProvider({
                      frequency_penalty:
                        e.target.value === ""
                          ? undefined
                          : parseFloat(e.target.value),
                    })
                  }
                />
              </div>

              <div className="space-y-2">
                <Label htmlFor="provider-presence-penalty">Presence Penalty</Label>
                <Input
                  id="provider-presence-penalty"
                  type="number"
                  step={0.1}
                  min={-2}
                  max={2}
                  value={config.provider.presence_penalty ?? ""}
                  onChange={(e) =>
                    updateProvider({
                      presence_penalty:
                        e.target.value === ""
                          ? undefined
                          : parseFloat(e.target.value),
                    })
                  }
                />
              </div>
            </div>

            <div className="space-y-4">
              <h3 className="text-sm font-semibold text-foreground">Agent</h3>
              <div className="flex items-center justify-between">
                <Label htmlFor="agent-tools">Enable Tools</Label>
                <Switch
                  id="agent-tools"
                  checked={config.agent.tools_enabled}
                  onCheckedChange={(tools_enabled) => updateAgent({ tools_enabled })}
                />
              </div>
              <div className="space-y-2">
                <Label htmlFor="agent-max-rounds">Max Tool Rounds</Label>
                <Input
                  id="agent-max-rounds"
                  type="number"
                  min={1}
                  max={100}
                  value={config.agent.max_rounds}
                  onChange={(e) =>
                    updateAgent({
                      max_rounds: parseInt(e.target.value) || 1,
                    })
                  }
                />
              </div>
            </div>

            <div className="space-y-4">
              <h3 className="text-sm font-semibold text-foreground">Session</h3>
              <div className="space-y-2">
                <Label htmlFor="session-prompt">System Prompt</Label>
                <Textarea
                  id="session-prompt"
                  value={config.session.system_prompt}
                  onChange={(e) => updateSession({ system_prompt: e.target.value })}
                />
              </div>
              <div className="space-y-2">
                <Label htmlFor="session-max-context">Max Context Tokens</Label>
                <Input
                  id="session-max-context"
                  type="number"
                  min={1}
                  value={config.session.max_context_tokens}
                  onChange={(e) =>
                    updateSession({
                      max_context_tokens: parseInt(e.target.value) || 1,
                    })
                  }
                />
              </div>
            </div>

            <div className="flex items-center gap-2">
              <Button
                type="button"
                onClick={handleSubmit}
                disabled={isSaving}
              >
                {isSaving ? "Saving..." : "Apply Config"}
              </Button>
              <Button
                type="button"
                variant="ghost"
                onClick={handleResetClick}
                disabled={isSaving}
              >
                Reset
              </Button>
            </div>
          </div>
        )}
      </CardContent>
    </Card>
  );
}
