import { Info, CheckCircle, XCircle, Loader2 } from "lucide-react";
import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { Button } from "@/components/ui/button";

interface HealthResponse {
  status: string;
  message: string;
  version: string;
}

export function Home() {
  const [healthStatus, setHealthStatus] = useState<"loading" | "ok" | "error">(
    "loading",
  );
  const [healthData, setHealthData] = useState<HealthResponse | null>(null);
  const [errorMessage, setErrorMessage] = useState<string>("");

  useEffect(() => {
    const checkHealth = async () => {
      try {
        const response = await fetch("/api/health");
        if (!response.ok) {
          throw new Error(`HTTP ${response.status}: ${response.statusText}`);
        }
        const data: HealthResponse = await response.json();
        setHealthData(data);
        setHealthStatus("ok");
      } catch (error) {
        setHealthStatus("error");
        setErrorMessage(
          error instanceof Error ? error.message : "Unknown error",
        );
      }
    };

    checkHealth();
    // Poll every 5 seconds
    const interval = setInterval(checkHealth, 5000);
    return () => clearInterval(interval);
  }, []);

  return (
    <div className="min-h-screen bg-background">
      <header className="border-b">
        <div className="container mx-auto px-4 py-3 flex items-center justify-between">
          <h1 className="text-xl font-bold text-primary">aaagent-rs</h1>
          <nav className="flex gap-2 flex-wrap">
            <Link to="/">
              <Button variant="ghost" size="sm">Home</Button>
            </Link>
            <Link to="/chat">
              <Button variant="ghost" size="sm">Chat</Button>
            </Link>
            <Link to="/testing">
              <Button variant="ghost" size="sm">Testing</Button>
            </Link>
            <Link to="/message-demo">
              <Button variant="ghost" size="sm">Message Demo</Button>
            </Link>
            <Link to="/branch-demo">
              <Button variant="ghost" size="sm">Branch Demo</Button>
            </Link>
          </nav>
        </div>
      </header>

      <main className="container mx-auto p-4">
        <div className="bg-muted border border-border rounded-lg p-4 flex items-start gap-3">
          <Info className="w-5 h-5 text-primary mt-0.5 shrink-0" />
          <span className="text-foreground">
            Chat UI Foundation - Ready for implementation with shadcn/ui
          </span>
        </div>

        <div className="mt-8 grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          <div className="bg-card border border-border rounded-lg shadow-sm p-6">
            <h2 className="text-lg font-semibold text-primary mb-2">
              Frontend
            </h2>
            <p className="text-muted-foreground mb-3">
              React + TypeScript + Vite
            </p>
            <div className="inline-flex items-center rounded-full px-2.5 py-0.5 text-xs font-semibold bg-accent text-accent-foreground">
              Active
            </div>
          </div>

          <div className="bg-card border border-border rounded-lg shadow-sm p-6">
            <h2 className="text-lg font-semibold text-secondary mb-2">
              Backend
            </h2>
            <div className="flex items-center gap-2 mb-3">
              {healthStatus === "loading" && (
                <>
                  <Loader2 className="w-4 h-4 animate-spin text-muted-foreground" />
                  <p className="text-muted-foreground text-sm">Checking...</p>
                </>
              )}
              {healthStatus === "ok" && (
                <>
                  <CheckCircle className="w-4 h-4 text-accent" />
                  <p className="text-muted-foreground text-sm">
                    {healthData?.message || "Running"}
                  </p>
                </>
              )}
              {healthStatus === "error" && (
                <>
                  <XCircle className="w-4 h-4 text-destructive" />
                  <p className="text-destructive text-sm">{errorMessage}</p>
                </>
              )}
            </div>
            {healthStatus === "ok" && healthData && (
              <div className="text-xs text-muted-foreground mb-3">
                Version: {healthData.version}
              </div>
            )}
            <div
              className={`inline-flex items-center rounded-full px-2.5 py-0.5 text-xs font-semibold ${
                healthStatus === "ok"
                  ? "bg-accent text-accent-foreground"
                  : healthStatus === "error"
                    ? "bg-destructive text-destructive-foreground"
                    : "bg-yellow-100 text-yellow-800 border border-yellow-300"
              }`}
            >
              {healthStatus === "ok"
                ? "Running"
                : healthStatus === "error"
                  ? "Offline"
                  : "Checking..."}
            </div>
          </div>

          <div className="bg-card border border-border rounded-lg shadow-sm p-6">
            <h2 className="text-lg font-semibold text-foreground mb-2">
              Features
            </h2>
            <ul className="list-disc list-inside text-sm text-muted-foreground space-y-1">
              <li>SSE Streaming</li>
              <li>Tool Pairs</li>
              <li>Error Handling</li>
              <li>Config UI</li>
            </ul>
          </div>
        </div>

        {/* Pages Section */}
        <div className="mt-8">
          <h2 className="text-lg font-semibold text-foreground mb-4">Pages</h2>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
            <Link to="/chat" className="block">
              <div className="bg-card border border-border rounded-lg p-4 hover:border-primary/50 hover:shadow-sm transition-all">
                <h3 className="font-medium text-foreground">Chat</h3>
                <p className="text-sm text-muted-foreground">
                  Main chat interface with streaming support
                </p>
              </div>
            </Link>
            <Link to="/testing" className="block">
              <div className="bg-card border border-border rounded-lg p-4 hover:border-primary/50 hover:shadow-sm transition-all">
                <h3 className="font-medium text-foreground">Testing</h3>
                <p className="text-sm text-muted-foreground">
                  Component testing and debugging
                </p>
              </div>
            </Link>
            <Link to="/message-demo" className="block">
              <div className="bg-card border border-border rounded-lg p-4 hover:border-primary/50 hover:shadow-sm transition-all">
                <h3 className="font-medium text-foreground">Message Demo</h3>
                <p className="text-sm text-muted-foreground">
                  MessageCard component showcase
                </p>
              </div>
            </Link>
            <Link to="/branch-demo" className="block">
              <div className="bg-card border border-border rounded-lg p-4 hover:border-primary/50 hover:shadow-sm transition-all">
                <h3 className="font-medium text-foreground">Branch Demo</h3>
                <p className="text-sm text-muted-foreground">
                  Branch navigation and checkpoint components
                </p>
              </div>
            </Link>
          </div>
        </div>
      </main>
    </div>
  );
}
