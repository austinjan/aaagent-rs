import { useState } from "react";
import { Wrench, CheckCircle, XCircle } from "lucide-react";

export interface ToolResult {
  tool_call_id: string;
  tool_name: string;
  result: string;
  is_error: boolean;
}

export interface ToolCallCardProps {
  id: string;
  name: string;
  input: Record<string, unknown> | undefined;
  result?: ToolResult;
  isExpanded?: boolean;
  onToggle?: () => void;
}

export function ToolCallCard({
  name,
  input,
  result,
  isExpanded: controlledExpanded,
  onToggle,
}: ToolCallCardProps) {
  const [internalExpanded, setInternalExpanded] = useState(true);
  const isExpanded = controlledExpanded ?? internalExpanded;

  const handleToggle = () => {
    if (onToggle) {
      onToggle();
    } else {
      setInternalExpanded(!internalExpanded);
    }
  };

  return (
    <div className="bg-[hsl(var(--role-tool)/0.08)] border border-[hsl(var(--role-tool)/0.25)] rounded-lg overflow-hidden shadow-sm">
      {/* Tool Call Header */}
      <div
        className="flex items-center justify-between p-3 cursor-pointer hover:bg-[hsl(var(--role-tool)/0.15)] transition-colors"
        onClick={handleToggle}
      >
        <div className="flex items-center gap-2">
          <Wrench className="w-3.5 h-3.5 text-[hsl(var(--role-tool))]" />
          <span className="text-sm font-semibold text-[hsl(var(--role-tool))]">
            {name}
          </span>
        </div>
        <span className="text-xs text-muted-foreground">
          {isExpanded ? "▼" : "▶"}
        </span>
      </div>

      {/* Expanded Content */}
      {isExpanded && (
        <div className="px-3 pb-3 space-y-2">
          {/* Input */}
          <div>
            <div className="text-xs font-semibold text-muted-foreground mb-1">
              Input:
            </div>
            <pre className="text-xs text-foreground bg-muted rounded p-2 overflow-x-auto">
              {JSON.stringify(input, null, 2)}
            </pre>
          </div>

          {/* Result */}
          {result && (
            <div
              className={`rounded p-2 border ${
                result.is_error
                  ? "bg-[hsl(var(--role-error)/0.08)] border-[hsl(var(--role-error)/0.25)]"
                  : "bg-[hsl(var(--role-tool)/0.12)] border-[hsl(var(--role-tool)/0.3)]"
              }`}
            >
              <div
                className={`text-xs font-semibold mb-1 flex items-center gap-1 ${
                  result.is_error
                    ? "text-[hsl(var(--role-error))]"
                    : "text-[hsl(var(--role-tool))]"
                }`}
              >
                {result.is_error ? (
                  <XCircle className="w-3.5 h-3.5" />
                ) : (
                  <CheckCircle className="w-3.5 h-3.5" />
                )}
                <span>Result</span>
              </div>
              <pre className="text-xs text-foreground overflow-x-auto max-h-40">
                {result.result}
              </pre>
            </div>
          )}
        </div>
      )}

      {/* Collapsed View */}
      {!isExpanded && result && (
        <div className="px-3 pb-2">
          <span
            className={`text-xs flex items-center gap-1 ${
              result.is_error
                ? "text-[hsl(var(--role-error))]"
                : "text-[hsl(var(--role-tool))]"
            }`}
          >
            {result.is_error ? (
              <>
                <XCircle className="w-3 h-3" />
                <span>Error</span>
              </>
            ) : (
              <>
                <CheckCircle className="w-3 h-3" />
                <span>Success</span>
              </>
            )}
          </span>
        </div>
      )}
    </div>
  );
}
