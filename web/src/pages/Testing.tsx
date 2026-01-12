import { useState } from "react";
import { ConfigPanel } from "@/components/config/ConfigPanel";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Separator } from "@/components/ui/separator";

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

export function Testing() {
  const [submittedConfig, setSubmittedConfig] = useState<ChatConfig | null>(
    null,
  );

  const handleConfigSubmit = (config: ChatConfig) => {
    console.log("Config submitted:", config);
    setSubmittedConfig(config);
  };

  const handleConfigReset = () => {
    console.log("Config reset");
    setSubmittedConfig(null);
  };

  return (
    <div className="min-h-screen bg-background">
      <header className="border-b border">
        <div className="container mx-auto px-4 py-3">
          <h1 className="text-xl font-bold text-foreground">
            Component Testing
          </h1>
          <p className="text-sm text-muted-foreground">
            Test UI components in isolation
          </p>
        </div>
      </header>

      <main className="container mx-auto p-4 space-y-8">
        {/* ConfigPanel Test */}
        <section className="space-y-4">
          <div>
            <h2 className="text-2xl font-bold text-foreground">
              ConfigPanel Component
            </h2>
            <p className="text-muted-foreground">
              Test the configuration panel with all controls
            </p>
          </div>

          <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
            {/* Component */}
            <div>
              <h3 className="text-lg font-semibold text-foreground mb-4">
                New Session Mode
              </h3>
              <ConfigPanel
                onSubmit={handleConfigSubmit}
                onReset={handleConfigReset}
              />
            </div>

            {/* Output Display */}
            <div>
              <h3 className="text-lg font-semibold text-foreground mb-4">
                Submitted Config
              </h3>
              <Card className="bg-background border">
                <CardHeader>
                  <CardTitle className="text-foreground">
                    Config Output
                  </CardTitle>
                </CardHeader>
                <CardContent>
                  {submittedConfig ? (
                    <pre className="text-xs text-white overflow-auto max-h-[600px] bg-gray-900 p-4 rounded">
                      {JSON.stringify(submittedConfig, null, 2)}
                    </pre>
                  ) : (
                    <p className="text-muted-foreground italic">
                      No config submitted yet. Fill out the form and click
                      "Apply Config".
                    </p>
                  )}
                </CardContent>
              </Card>
            </div>
          </div>
        </section>

        <Separator className="bg-yellow-500/30" />

        {/* Existing Session Mode Test */}
        <section className="space-y-4">
          <div>
            <h2 className="text-2xl font-bold text-foreground">
              Existing Session Mode
            </h2>
            <p className="text-muted-foreground">
              System prompt is locked (read-only)
            </p>
          </div>

          <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
            <div>
              <h3 className="text-lg font-semibold text-foreground mb-4">
                With Locked System Prompt
              </h3>
              <ConfigPanel
                sessionId="test-session-123"
                existingConfig={{
                  provider: {
                    model: "gpt-5-mini",
                    temperature: 1.0,
                    max_tokens: 16384,
                  },
                  agent: {
                    max_rounds: 30,
                    tools_enabled: true,
                  },
                  session: {
                    system_prompt:
                      "You are a test assistant. This prompt is locked.",
                    max_context_tokens: 200000,
                  },
                }}
                onSubmit={(config: ChatConfig) =>
                  console.log("Existing session config:", config)
                }
                onReset={() => console.log("Existing session reset")}
              />
            </div>

            <div className="space-y-4">
              <Card className="bg-background border">
                <CardHeader>
                  <CardTitle className="text-foreground">
                    Test Instructions
                  </CardTitle>
                </CardHeader>
                <CardContent className="space-y-2 text-sm text-muted-foreground">
                  <div>
                    <h4 className="font-semibold text-foreground mb-2">
                      New Session Mode (Left Panel)
                    </h4>
                    <ul className="list-disc list-inside space-y-1 text-muted-foreground">
                      <li>System prompt should be editable</li>
                      <li>All presets change system prompt when selected</li>
                      <li>Character counter updates (max 10,000)</li>
                      <li>Creativity slider works (0.0 - 1.0)</li>
                      <li>Verbosity dropdown (short/normal/long)</li>
                      <li>Rounds input accepts numbers (1-100)</li>
                      <li>Tools toggle switches on/off</li>
                      <li>Advanced panel collapses/expands</li>
                      <li>Submit shows JSON output</li>
                    </ul>
                  </div>
                  <Separator className="bg-yellow-500/30" />
                  <div>
                    <h4 className="font-semibold text-foreground mb-2">
                      Existing Session Mode (Right Panel)
                    </h4>
                    <ul className="list-disc list-inside space-y-1 text-muted-foreground">
                      <li>System prompt should be grayed out (read-only)</li>
                      <li>Lock icon visible next to "System Prompt" label</li>
                      <li>Warning message below system prompt textarea</li>
                      <li>All other controls should work normally</li>
                      <li>
                        Config submission should NOT include system_prompt field
                      </li>
                    </ul>
                  </div>
                  <Separator className="bg-yellow-500/30" />
                  <div>
                    <h4 className="font-semibold text-foreground mb-2">
                      Visual Tests
                    </h4>
                    <ul className="list-disc list-inside space-y-1 text-muted-foreground">
                      <li>
                        BlackBear theme: Yellow (#E8C236) on Black (#000000)
                      </li>
                      <li>All borders should be yellow</li>
                      <li>Buttons: Yellow background with black text</li>
                      <li>Text: White/gray on black background</li>
                      <li>Hover states work correctly</li>
                    </ul>
                  </div>
                </CardContent>
              </Card>
            </div>
          </div>
        </section>

        <Separator className="bg-yellow-500/30" />

        {/* Browser Console */}
        <section className="space-y-4">
          <div>
            <h2 className="text-2xl font-bold text-foreground">
              Browser Console
            </h2>
            <p className="text-muted-foreground">
              Check the browser console for config submit/reset logs
            </p>
          </div>
          <Card className="bg-background border">
            <CardContent className="pt-6">
              <p className="text-sm text-muted-foreground">
                Open DevTools (F12) and check the Console tab. You should see:
              </p>
              <ul className="list-disc list-inside mt-2 space-y-1 text-sm text-muted-foreground">
                <li>
                  <code className="text-foreground">
                    Config submitted: &#123;...&#125;
                  </code>{" "}
                  - When clicking "Apply Config"
                </li>
                <li>
                  <code className="text-foreground">Config reset</code> - When
                  clicking "Reset"
                </li>
                <li>
                  <code className="text-foreground">
                    Existing session config: &#123;...&#125;
                  </code>{" "}
                  - From existing session panel
                </li>
              </ul>
            </CardContent>
          </Card>
        </section>
      </main>
    </div>
  );
}
