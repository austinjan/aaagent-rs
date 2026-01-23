// CheckpointMessageCard - Displays checkpoint as a distinct message in the timeline

import { useState } from "react";
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "../ui/collapsible";
import { Bookmark, ChevronDown } from "lucide-react";
import type { CheckpointData, NodeId } from "@/types";

export interface CheckpointMessageCardProps {
  id: string;
  checkpointNodeId: NodeId;
  checkpointData: CheckpointData;
  timestamp: Date;
}

export function CheckpointMessageCard({
  id,
  checkpointNodeId,
  checkpointData,
  timestamp,
}: CheckpointMessageCardProps) {
  const [isExpanded, setIsExpanded] = useState(false);

  // Format timestamp
  const timeStr = timestamp.toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
  });

  // Strategy label
  const strategyLabels: Record<string, string> = {
    auto_turns: "Auto (Turn Limit)",
    auto_token_limit: "Auto (Token Limit)",
    auto_large_content: "Auto (Large Content)",
    manual: "Manual",
  };
  const strategyLabel = checkpointData.strategy
    ? strategyLabels[checkpointData.strategy] || checkpointData.strategy
    : "Unknown";

  // Summary preview (first 120 chars)
  const summaryPreview =
    checkpointData.summary.length > 120
      ? checkpointData.summary.slice(0, 120) + "..."
      : checkpointData.summary;

  return (
    <div className="mb-4 w-full">
      <Collapsible open={isExpanded} onOpenChange={setIsExpanded}>
        <div
          className="border-l-4 border-[hsl(var(--role-checkpoint))] bg-[hsl(var(--role-checkpoint)/0.05)] rounded-r-lg overflow-hidden transition-colors hover:bg-[hsl(var(--role-checkpoint)/0.08)]"
          data-checkpoint-id={id}
          data-node-id={checkpointNodeId}
        >
          {/* Header */}
          <div className="px-4 py-3 border-b border-[hsl(var(--role-checkpoint)/0.15)]">
            <div className="flex items-center justify-between gap-3">
              <div className="flex items-center gap-2 min-w-0">
                <Bookmark
                  className="w-4 h-4 flex-shrink-0 text-[hsl(var(--role-checkpoint))]"
                  fill="currentColor"
                />
                <span className="text-sm font-semibold text-[hsl(var(--role-checkpoint))]">
                  Checkpoint Created
                </span>
                <span className="text-xs text-muted-foreground">
                  {timeStr}
                </span>
                <span className="text-xs px-2 py-0.5 rounded-full bg-[hsl(var(--role-checkpoint)/0.15)] text-[hsl(var(--role-checkpoint))]">
                  {strategyLabel}
                </span>
              </div>
            </div>
          </div>

          {/* Collapsible Trigger */}
          <CollapsibleTrigger className="w-full px-4 py-3 text-left hover:bg-[hsl(var(--role-checkpoint)/0.10)] transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[hsl(var(--role-checkpoint))] focus-visible:ring-offset-2">
            <div className="flex items-start justify-between gap-3">
              <div className="flex-1 min-w-0">
                <p className="text-sm text-foreground line-clamp-2">
                  {summaryPreview}
                </p>
                {checkpointData.stats && (
                  <div className="mt-2 flex items-center gap-4 text-xs text-muted-foreground">
                    <span>
                      {checkpointData.stats.nodes_covered} messages compressed
                    </span>
                    <span>
                      {(checkpointData.stats.compression_ratio * 100).toFixed(
                        1
                      )}
                      % reduction
                    </span>
                  </div>
                )}
              </div>
              <ChevronDown
                className={`w-4 h-4 flex-shrink-0 text-muted-foreground transition-transform ${
                  isExpanded ? "rotate-180" : ""
                }`}
              />
            </div>
          </CollapsibleTrigger>

          {/* Collapsible Content */}
          <CollapsibleContent className="data-[state=open]:animate-collapsible-down data-[state=closed]:animate-collapsible-up overflow-hidden">
            <div className="px-4 py-4 space-y-4 bg-[hsl(var(--role-checkpoint)/0.03)]">
              {/* Full Summary */}
              <div>
                <h4 className="text-xs font-semibold text-muted-foreground uppercase tracking-wide mb-2">
                  Summary
                </h4>
                <div className="prose prose-sm dark:prose-invert max-w-none">
                  <p className="text-sm leading-relaxed whitespace-pre-wrap">
                    {checkpointData.summary}
                  </p>
                </div>
              </div>

              {/* Stats Grid */}
              {checkpointData.stats && (
                <div>
                  <h4 className="text-xs font-semibold text-muted-foreground uppercase tracking-wide mb-2">
                    Compression Statistics
                  </h4>
                  <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
                    <div className="bg-background/50 rounded-lg p-3">
                      <div className="text-xs text-muted-foreground mb-1">
                        Messages
                      </div>
                      <div className="text-lg font-mono font-semibold text-foreground">
                        {checkpointData.stats.nodes_covered}
                      </div>
                    </div>
                    <div className="bg-background/50 rounded-lg p-3">
                      <div className="text-xs text-muted-foreground mb-1">
                        Original Tokens
                      </div>
                      <div className="text-lg font-mono font-semibold text-foreground">
                        {checkpointData.stats.total_tokens.toLocaleString()}
                      </div>
                    </div>
                    <div className="bg-background/50 rounded-lg p-3">
                      <div className="text-xs text-muted-foreground mb-1">
                        Summary Tokens
                      </div>
                      <div className="text-lg font-mono font-semibold text-foreground">
                        {checkpointData.stats.summary_tokens.toLocaleString()}
                      </div>
                    </div>
                    <div className="bg-background/50 rounded-lg p-3">
                      <div className="text-xs text-muted-foreground mb-1">
                        Compression
                      </div>
                      <div className="text-lg font-mono font-semibold text-[hsl(var(--role-checkpoint))]">
                        {(
                          checkpointData.stats.compression_ratio * 100
                        ).toFixed(1)}
                        %
                      </div>
                    </div>
                  </div>
                </div>
              )}

              {/* Time Range (if available) */}
              {checkpointData.stats?.covered_time_range && (
                <div>
                  <h4 className="text-xs font-semibold text-muted-foreground uppercase tracking-wide mb-2">
                    Time Range Covered
                  </h4>
                  <div className="text-sm text-muted-foreground font-mono">
                    {new Date(
                      checkpointData.stats.covered_time_range[0] * 1000
                    ).toLocaleString()}{" "}
                    →{" "}
                    {new Date(
                      checkpointData.stats.covered_time_range[1] * 1000
                    ).toLocaleString()}
                  </div>
                </div>
              )}
            </div>
          </CollapsibleContent>
        </div>
      </Collapsible>
    </div>
  );
}
