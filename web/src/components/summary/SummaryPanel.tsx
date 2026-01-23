import { useState, useCallback } from "react";
import { FileText, Loader2, Copy, Check, RefreshCw } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible";
import { cn } from "@/lib/utils";
import { useContextString } from "@/hooks/useContextString";
import type { ContextStringStrategy } from "@/types/backend";

interface SummaryPanelProps {
  sessionId: string;
  leafId?: string;
  className?: string;
}

export function SummaryPanel({ sessionId, leafId, className }: SummaryPanelProps) {
  const [isOpen, setIsOpen] = useState(false);
  const [strategy, setStrategy] = useState<ContextStringStrategy>("default");
  const [copied, setCopied] = useState(false);

  const { content, isLoading, error, fetch } = useContextString({
    sessionId,
    leafId,
    strategy,
  });

  const handleFetch = useCallback(async () => {
    await fetch({ strategy });
  }, [fetch, strategy]);

  const handleCopy = useCallback(async () => {
    if (!content) return;
    try {
      await navigator.clipboard.writeText(content);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (err) {
      console.error("Failed to copy:", err);
    }
  }, [content]);

  const handleStrategyChange = useCallback(
    async (newStrategy: ContextStringStrategy) => {
      setStrategy(newStrategy);
      // Refetch with new strategy if panel is open and we have content
      if (isOpen && content) {
        await fetch({ strategy: newStrategy });
      }
    },
    [isOpen, content, fetch]
  );

  return (
    <Collapsible
      open={isOpen}
      onOpenChange={(open) => {
        setIsOpen(open);
        if (open && !content) {
          handleFetch();
        }
      }}
      className={cn("border rounded-lg", className)}
    >
      <CollapsibleTrigger asChild>
        <Button
          variant="ghost"
          className="w-full justify-between px-4 py-3 h-auto"
        >
          <div className="flex items-center gap-2">
            <FileText className="h-4 w-4" />
            <span className="font-medium">Conversation Summary</span>
          </div>
          <span className="text-xs text-muted-foreground">
            {isOpen ? "Click to collapse" : "Click to view"}
          </span>
        </Button>
      </CollapsibleTrigger>

      <CollapsibleContent>
        <div className="px-4 pb-4 space-y-3">
          {/* Strategy selector */}
          <div className="flex items-center gap-2">
            <span className="text-xs text-muted-foreground">Strategy:</span>
            <div className="flex gap-1">
              {(["full", "default", "aggressive"] as const).map((s) => (
                <Button
                  key={s}
                  variant={strategy === s ? "default" : "outline"}
                  size="sm"
                  className="h-6 px-2 text-xs"
                  onClick={() => handleStrategyChange(s)}
                >
                  {s.charAt(0).toUpperCase() + s.slice(1)}
                </Button>
              ))}
            </div>
            <Button
              variant="ghost"
              size="sm"
              className="h-6 w-6 p-0 ml-auto"
              onClick={handleFetch}
              disabled={isLoading}
            >
              <RefreshCw className={cn("h-3 w-3", isLoading && "animate-spin")} />
            </Button>
          </div>

          {/* Content area */}
          {isLoading && (
            <div className="flex items-center justify-center py-8">
              <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
            </div>
          )}

          {error && (
            <div className="p-3 rounded-md bg-red-50 text-red-600 text-sm">
              {error}
            </div>
          )}

          {content && !isLoading && (
            <div className="relative">
              <div
                className={cn(
                  "p-3 rounded-md text-sm font-mono whitespace-pre-wrap",
                  "bg-muted/50 max-h-[400px] overflow-y-auto"
                )}
              >
                {content}
              </div>
              <Button
                variant="ghost"
                size="sm"
                className="absolute top-2 right-2 h-6 w-6 p-0"
                onClick={handleCopy}
                title={copied ? "Copied!" : "Copy to clipboard"}
              >
                {copied ? (
                  <Check className="h-3 w-3 text-green-600" />
                ) : (
                  <Copy className="h-3 w-3" />
                )}
              </Button>
            </div>
          )}

          {/* Strategy explanation */}
          <div className="text-xs text-muted-foreground">
            {strategy === "full" && "Shows all messages from current position to root."}
            {strategy === "default" && "Shows messages to nearest checkpoint or root."}
            {strategy === "aggressive" && "Skips tool-related messages for a cleaner summary."}
          </div>
        </div>
      </CollapsibleContent>
    </Collapsible>
  );
}

export default SummaryPanel;
