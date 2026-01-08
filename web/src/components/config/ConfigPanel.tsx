import { useState, useEffect } from "react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Textarea } from "@/components/ui/textarea";
import { Slider } from "@/components/ui/slider";
import { Switch } from "@/components/ui/switch";
import { Button } from "@/components/ui/button";
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible";
import { Input } from "@/components/ui/input";
import { ChevronDown, ChevronUp, Lock, AlertCircle } from "lucide-react";

// Types from backend
interface ChatConfig {
  preset: string;
  system_prompt?: string;
  tools_enabled: boolean;
  intent: {
    creativity: number;
    verbosity: string;
    rounds: number;
  };
  overrides?: {
    model?: string;
    top_p?: number;
    frequency_penalty?: number;
    presence_penalty?: number;
  };
}

interface ResolvedConfig {
  provider: {
    model: string;
    temperature: number;
    max_tokens: number;
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

interface ConfigPanelProps {
  sessionId?: string;
  existingConfig?: ResolvedConfig;
  onSubmit?: (config: ChatConfig) => void;
  onReset?: () => void;
}

const PRESETS = {
  general: {
    name: "General Assistant",
    icon: "⚡",
    description: "Everyday conversation, Q&A, general tasks",
    systemPrompt: "You are a helpful, friendly assistant.",
  },
  coding: {
    name: "Software Engineer",
    icon: "💻",
    description: "Code generation, debugging, refactoring",
    systemPrompt: `You are an expert software engineer with deep knowledge of multiple programming languages.
- Write clean, well-documented code following best practices
- Consider performance, security, and maintainability
- Use tools to read/write files when needed
- Explain complex concepts clearly with examples`,
  },
  research: {
    name: "Research Assistant",
    icon: "🔬",
    description: "Research tasks, data analysis, complex problems",
    systemPrompt: `You are a thorough research assistant specializing in systematic analysis.
- Break down complex problems into clear components
- Use tools to search and analyze information
- Provide evidence-based reasoning with citations when possible
- Consider multiple perspectives and trade-offs`,
  },
  quick: {
    name: "Quick & Efficient",
    icon: "⏱️",
    description: "Fast answers, simple tasks, cost-sensitive",
    systemPrompt:
      "You are a concise, efficient assistant focused on quick answers.",
  },
};

export function ConfigPanel({
  sessionId,
  existingConfig,
  onSubmit,
  onReset,
}: ConfigPanelProps) {
  const isNewSession = !sessionId;
  const [preset, setPreset] = useState<string>("general");
  const [systemPrompt, setSystemPrompt] = useState<string>("");
  const [toolsEnabled, setToolsEnabled] = useState<boolean>(true);
  const [creativity, setCreativity] = useState<number>(0.5);
  const [verbosity, setVerbosity] = useState<string>("normal");
  const [rounds, setRounds] = useState<number>(30);
  const [showAdvanced, setShowAdvanced] = useState<boolean>(false);

  // Advanced overrides
  const [modelOverride, setModelOverride] = useState<string>("auto");
  const [topP, setTopP] = useState<string>("0.9");
  const [frequencyPenalty, setFrequencyPenalty] = useState<string>("0.0");
  const [presencePenalty, setPresencePenalty] = useState<string>("0.0");

  // Load preset defaults when preset changes
  useEffect(() => {
    const presetData = PRESETS[preset as keyof typeof PRESETS];
    if (presetData && isNewSession) {
      setSystemPrompt(presetData.systemPrompt);
    }
  }, [preset, isNewSession]);

  // Load existing config for existing sessions
  useEffect(() => {
    if (existingConfig) {
      setSystemPrompt(existingConfig.session.system_prompt);
      setToolsEnabled(existingConfig.agent.tools_enabled);
      setRounds(existingConfig.agent.max_rounds);
    }
  }, [existingConfig]);

  const handleSubmit = () => {
    const config: ChatConfig = {
      preset,
      system_prompt: isNewSession ? systemPrompt : undefined,
      tools_enabled: toolsEnabled,
      intent: {
        creativity,
        verbosity,
        rounds,
      },
    };

    // Add overrides if advanced panel is open and values are set
    if (showAdvanced) {
      config.overrides = {};
      if (modelOverride && modelOverride !== "auto") {
        config.overrides.model = modelOverride;
      }
      if (topP) config.overrides.top_p = parseFloat(topP);
      if (frequencyPenalty)
        config.overrides.frequency_penalty = parseFloat(frequencyPenalty);
      if (presencePenalty)
        config.overrides.presence_penalty = parseFloat(presencePenalty);
    }

    onSubmit?.(config);
  };

  const handleResetClick = () => {
    setPreset("general");
    setSystemPrompt(PRESETS.general.systemPrompt);
    setToolsEnabled(true);
    setCreativity(0.5);
    setVerbosity("normal");
    setRounds(30);
    setShowAdvanced(false);
    setModelOverride("auto");
    setTopP("0.9");
    setFrequencyPenalty("0.0");
    setPresencePenalty("0.0");
    onReset?.();
  };

  const getCreativityLabel = (value: number) => {
    if (value < 0.3) return "Deterministic";
    if (value < 0.7) return "Balanced";
    return "Creative";
  };

  const getRoundsLabel = (value: number) => {
    if (value <= 15) return "Quick tasks only";
    if (value <= 40) return "Standard conversations";
    return "Complex multi-step tasks";
  };

  const charCount = systemPrompt.length;
  const isOverLimit = charCount > 10000;

  return (
    <Card className="w-full max-w-2xl">
      <CardHeader>
        <CardTitle>Configuration</CardTitle>
      </CardHeader>
      <CardContent className="space-y-6">
        {/* Preset Selector */}
        <div className="space-y-2">
          <Label htmlFor="preset">Preset</Label>
          <Select value={preset} onValueChange={setPreset}>
            <SelectTrigger id="preset">
              <SelectValue>
                <span>
                  {PRESETS[preset as keyof typeof PRESETS]?.icon}{" "}
                  {PRESETS[preset as keyof typeof PRESETS]?.name}
                </span>
              </SelectValue>
            </SelectTrigger>
            <SelectContent>
              {Object.entries(PRESETS).map(([key, data]) => (
                <SelectItem key={key} value={key}>
                  <div className="flex items-center gap-2">
                    <span>{data.icon}</span>
                    <div>
                      <div className="font-medium">{data.name}</div>
                      <div className="text-xs text-muted-foreground">
                        {data.description}
                      </div>
                    </div>
                  </div>
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>

        {/* System Prompt */}
        <div className="space-y-2">
          <div className="flex items-center justify-between">
            <Label htmlFor="system-prompt">
              System Prompt{" "}
              {!isNewSession && <Lock className="inline w-4 h-4 ml-1" />}
            </Label>
            <span
              className={`text-xs ${isOverLimit ? "text-red-500" : "text-muted-foreground"}`}
            >
              {charCount} / 10,000
            </span>
          </div>
          <Textarea
            id="system-prompt"
            value={systemPrompt}
            onChange={(e: React.ChangeEvent<HTMLTextAreaElement>) =>
              setSystemPrompt(e.target.value)
            }
            disabled={!isNewSession}
            className={`min-h-[120px]  ${!isNewSession ? "opacity-50 cursor-not-allowed" : ""} ${isOverLimit ? "border-red-500" : ""}`}
            placeholder="System prompt..."
          />
          {!isNewSession && (
            <div className="flex items-center gap-2 text-xs text-muted-foreground">
              <AlertCircle className="w-4 h-4" />
              <span>
                Cannot be changed after session creation. Create a new session
                to use a different prompt.
              </span>
            </div>
          )}
        </div>

        {/* Tools Toggle */}
        <div className="flex items-center justify-between space-x-2 p-4 border rounded-lg">
          <div className="space-y-0.5">
            <Label htmlFor="tools-enabled">Enable Tools</Label>
            <p className="text-xs text-muted-foreground">
              When enabled, agent can use tools for file operations, web search,
              calculations, etc.
            </p>
          </div>
          <Switch
            id="tools-enabled"
            checked={toolsEnabled}
            onCheckedChange={setToolsEnabled}
          />
        </div>

        {/* Intent Controls */}
        <div className="space-y-4 p-4 border rounded-lg">
          <h3 className="text-sm font-medium ">Intent</h3>

          {/* Creativity Slider */}
          <div className="space-y-2">
            <div className="flex items-center justify-between">
              <Label htmlFor="creativity">Creativity</Label>
              <span className="text-sm text-muted-foreground">
                {creativity.toFixed(1)} - {getCreativityLabel(creativity)}
              </span>
            </div>
            <Slider
              id="creativity"
              min={0}
              max={1}
              step={0.1}
              value={[creativity]}
              onValueChange={(vals: number[]) => setCreativity(vals[0])}
            />
            <div className="flex justify-between text-xs text-muted-foreground">
              <span>Deterministic</span>
              <span>Creative</span>
            </div>
          </div>

          {/* Verbosity Select */}
          <div className="space-y-2">
            <Label htmlFor="verbosity">Verbosity</Label>
            <Select value={verbosity} onValueChange={setVerbosity}>
              <SelectTrigger id="verbosity">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="short">Short (8K tokens)</SelectItem>
                <SelectItem value="normal">Normal (16K tokens)</SelectItem>
                <SelectItem value="long">Long (32K tokens)</SelectItem>
              </SelectContent>
            </Select>
          </div>

          {/* Rounds Input */}
          <div className="space-y-2">
            <div className="flex items-center justify-between">
              <Label htmlFor="rounds">Max Rounds</Label>
              <span className="text-sm text-muted-foreground">{rounds}</span>
            </div>
            <Input
              id="rounds"
              type="number"
              min={1}
              max={100}
              value={rounds}
              onChange={(e: React.ChangeEvent<HTMLInputElement>) =>
                setRounds(parseInt(e.target.value) || 30)
              }
            />
            <p className="text-xs text-muted-foreground">
              {getRoundsLabel(rounds)}
            </p>
          </div>
        </div>

        {/* Advanced Overrides */}
        <Collapsible open={showAdvanced} onOpenChange={setShowAdvanced}>
          <CollapsibleTrigger asChild>
            <Button variant="outline" className="w-full   hover:/10">
              <span className="flex-1">Advanced Overrides</span>
              {showAdvanced ? (
                <ChevronUp className="w-4 h-4" />
              ) : (
                <ChevronDown className="w-4 h-4" />
              )}
            </Button>
          </CollapsibleTrigger>
          <CollapsibleContent className="mt-4 space-y-4 p-4 border rounded-lg">
            <div className="flex items-center gap-2 text-xs text-muted-foreground mb-4">
              <AlertCircle className="w-4 h-4" />
              <span>
                Only adjust if you understand nucleus sampling and penalties
              </span>
            </div>

            {/* Model Override */}
            <div className="space-y-2">
              <Label htmlFor="model-override">Model Override</Label>
              <Select value={modelOverride} onValueChange={setModelOverride}>
                <SelectTrigger id="model-override">
                  <SelectValue placeholder="Auto (from preset)" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="auto">Auto (from preset)</SelectItem>
                  <SelectItem value="gpt-5">gpt-5</SelectItem>
                  <SelectItem value="gpt-5-mini">gpt-5-mini</SelectItem>
                  <SelectItem value="gpt-5-nano">gpt-5-nano</SelectItem>
                  <SelectItem value="gpt-5.2">gpt-5.2</SelectItem>
                  <SelectItem value="claude-3-5-sonnet-20241022">
                    claude-3-5-sonnet
                  </SelectItem>
                  <SelectItem value="gemini-3-flash-preview">
                    gemini-3-flash
                  </SelectItem>
                </SelectContent>
              </Select>
            </div>

            {/* Top P */}
            <div className="space-y-2">
              <Label htmlFor="top-p">Top P (0.0 - 1.0)</Label>
              <Input
                id="top-p"
                type="number"
                min={0}
                max={1}
                step={0.1}
                value={topP}
                onChange={(e: React.ChangeEvent<HTMLInputElement>) =>
                  setTopP(e.target.value)
                }
              />
            </div>

            {/* Frequency Penalty */}
            <div className="space-y-2">
              <Label htmlFor="frequency-penalty">
                Frequency Penalty (-2.0 to 2.0)
              </Label>
              <Input
                id="frequency-penalty"
                type="number"
                min={-2}
                max={2}
                step={0.1}
                value={frequencyPenalty}
                onChange={(e: React.ChangeEvent<HTMLInputElement>) =>
                  setFrequencyPenalty(e.target.value)
                }
              />
            </div>

            {/* Presence Penalty */}
            <div className="space-y-2">
              <Label htmlFor="presence-penalty">
                Presence Penalty (-2.0 to 2.0)
              </Label>
              <Input
                id="presence-penalty"
                type="number"
                min={-2}
                max={2}
                step={0.1}
                value={presencePenalty}
                onChange={(e: React.ChangeEvent<HTMLInputElement>) =>
                  setPresencePenalty(e.target.value)
                }
              />
            </div>
          </CollapsibleContent>
        </Collapsible>

        {/* Action Buttons */}
        <div className="flex gap-4">
          <Button
            onClick={handleSubmit}
            className="flex-1  text-primary-foreground "
          >
            Apply Config
          </Button>
          <Button
            onClick={handleResetClick}
            variant="outline"
            className="  hover:/10"
          >
            Reset
          </Button>
        </div>
      </CardContent>
    </Card>
  );
}
