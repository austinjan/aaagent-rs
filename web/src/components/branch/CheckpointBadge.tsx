import { useState } from "react";
import { Bookmark, ChevronDown, ChevronUp } from "lucide-react";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";

export interface CheckpointStats {
  nodesCovered: number;
  originalTokens: number;
  compressedTokens: number;
  compressionRatio: number;
}

export interface CheckpointBadgeProps {
  summary: string;
  stats?: CheckpointStats;
  isExpanded?: boolean;
  onToggle?: (expanded: boolean) => void;
}

export function CheckpointBadge({
  summary,
  stats,
  isExpanded: controlledExpanded,
  onToggle,
}: CheckpointBadgeProps) {
  const [internalExpanded, setInternalExpanded] = useState(false);

  const isExpanded = controlledExpanded ?? internalExpanded;
  const handleToggle = () => {
    const newState = !isExpanded;
    setInternalExpanded(newState);
    onToggle?.(newState);
  };

  return (
    <div className="flex flex-col">
      {/* Badge button */}
      <Button
        variant="ghost"
        size="sm"
        className={cn(
          "h-auto py-1 px-2 gap-1.5",
          "text-[hsl(var(--role-checkpoint))]",
          "hover:bg-[hsl(var(--role-checkpoint)/0.1)]",
          "transition-colors"
        )}
        onClick={handleToggle}
        aria-expanded={isExpanded}
        aria-label={isExpanded ? "Collapse checkpoint" : "Expand checkpoint"}
      >
        <Bookmark className="h-3.5 w-3.5 fill-current" />
        <span className="text-xs font-medium">Checkpoint</span>
        {isExpanded ? (
          <ChevronUp className="h-3 w-3" />
        ) : (
          <ChevronDown className="h-3 w-3" />
        )}
      </Button>

      {/* Expanded content */}
      {isExpanded && (
        <div
          className={cn(
            "mt-2 p-3 rounded-md",
            "bg-[hsl(var(--role-checkpoint)/0.08)]",
            "border border-[hsl(var(--role-checkpoint)/0.25)]"
          )}
        >
          {/* Summary */}
          <p className="text-sm text-[hsl(var(--foreground))]">{summary}</p>

          {/* Stats */}
          {stats && (
            <div
              className={cn(
                "mt-2 pt-2",
                "border-t border-[hsl(var(--role-checkpoint)/0.2)]",
                "flex flex-wrap gap-x-4 gap-y-1"
              )}
            >
              <StatItem label="Messages" value={stats.nodesCovered} />
              <StatItem
                label="Tokens"
                value={`${formatNumber(stats.originalTokens)} → ${formatNumber(stats.compressedTokens)}`}
              />
              <StatItem
                label="Compression"
                value={`${Math.round(stats.compressionRatio * 100)}%`}
              />
            </div>
          )}
        </div>
      )}
    </div>
  );
}

function StatItem({ label, value }: { label: string; value: string | number }) {
  return (
    <div className="flex items-center gap-1 text-xs">
      <span className="text-[hsl(var(--muted-foreground))]">{label}:</span>
      <span className="font-medium text-[hsl(var(--foreground))]">{value}</span>
    </div>
  );
}

function formatNumber(num: number): string {
  if (num >= 1000) {
    return `${(num / 1000).toFixed(1)}k`;
  }
  return num.toString();
}

export default CheckpointBadge;
