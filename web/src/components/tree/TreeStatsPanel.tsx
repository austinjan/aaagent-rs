// TreeStatsPanel - Statistics dashboard for conversation tree

import { useEffect, useState } from "react";
import type { TreeStatsResponse } from "../../types/backend";
import { getTreeStats } from "../../services/api";

interface TreeStatsPanelProps {
  sessionId: string | null;
  onClose?: () => void;
}

export function TreeStatsPanel({ sessionId, onClose }: TreeStatsPanelProps) {
  const [stats, setStats] = useState<TreeStatsResponse | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!sessionId) {
      setStats(null);
      return;
    }

    const fetchStats = async () => {
      setLoading(true);
      setError(null);
      try {
        const data = await getTreeStats(sessionId);
        setStats(data);
      } catch (err) {
        console.error("Failed to fetch tree stats:", err);
        setError(err instanceof Error ? err.message : "Unknown error");
      } finally {
        setLoading(false);
      }
    };

    fetchStats();

    // Refresh stats every 10 seconds
    const interval = setInterval(fetchStats, 10000);
    return () => clearInterval(interval);
  }, [sessionId]);

  if (!sessionId || loading) {
    return (
      <div className="p-4 bg-base-200 rounded-lg">
        <div className="animate-pulse">
          <div className="h-4 bg-base-300 rounded w-3/4 mb-2"></div>
          <div className="h-4 bg-base-300 rounded w-1/2"></div>
        </div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="p-4 bg-error/10 border border-error/20 rounded-lg">
        <p className="text-error text-sm">Failed to load statistics: {error}</p>
      </div>
    );
  }

  if (!stats) {
    return null;
  }

  const { token_usage, node_counts } = stats;

  // Calculate compression percentage for display
  const compressionPercentage = Math.round(
    (1 - 1 / token_usage.estimated_compression_ratio) * 100
  );

  // Calculate context usage percentage (assuming max 200k tokens)
  const maxContextTokens = 200000;
  const contextUsagePercent = Math.round(
    (token_usage.active_path_tokens / maxContextTokens) * 100
  );

  return (
    <div className="p-4 bg-base-200 rounded-lg space-y-4 border border-base-300">
      {/* Close button */}
      {onClose && (
        <button
          onClick={onClose}
          className="float-right text-base-content/50 hover:text-base-content"
        >
          ✕
        </button>
      )}

      {/* Title */}
      <div className="flex items-center gap-2">
        <span className="text-lg">📊</span>
        <h3 className="font-semibold text-base-content">Tree Statistics</h3>
      </div>

      {/* Token Usage Section */}
      <div className="space-y-2">
        <h4 className="text-sm font-medium text-base-content/70">
          💡 Token Efficiency
        </h4>
        <div className="space-y-1 text-sm">
          <div className="flex justify-between">
            <span className="text-base-content/70">Active Path:</span>
            <span className="font-mono font-semibold text-base-content">
              {token_usage.active_path_tokens.toLocaleString()} tokens
            </span>
          </div>

          {/* Progress bar */}
          <div className="relative w-full h-2 bg-base-300 rounded-full overflow-hidden">
            <div
              className={`absolute h-full rounded-full transition-all ${
                contextUsagePercent > 80
                  ? "bg-error"
                  : contextUsagePercent > 50
                    ? "bg-warning"
                    : "bg-success"
              }`}
              style={{ width: `${Math.min(contextUsagePercent, 100)}%` }}
            />
          </div>
          <div className="text-xs text-base-content/50 text-right">
            {contextUsagePercent}% of max context
          </div>
        </div>

        {/* Compression Potential */}
        {token_usage.potential_token_savings > 0 && (
          <div className="mt-3 p-3 bg-primary/10 border border-primary/20 rounded-lg">
            <div className="text-sm font-medium text-primary mb-1">
              Create Checkpoint Now
            </div>
            <div className="text-xs space-y-1 text-base-content/70">
              <div>
                Save ~{token_usage.potential_token_savings.toLocaleString()}{" "}
                tokens ({compressionPercentage}% reduction)
              </div>
              <div>
                {token_usage.active_path_tokens.toLocaleString()} →{" "}
                {token_usage.estimated_after_checkpoint.toLocaleString()} tokens
              </div>
            </div>
          </div>
        )}
      </div>

      {/* Node Counts Section */}
      <div className="space-y-2">
        <h4 className="text-sm font-medium text-base-content/70">
          📊 Node Statistics
        </h4>
        <div className="grid grid-cols-2 gap-2 text-sm">
          <div className="flex items-center gap-2">
            <span>👤</span>
            <span className="text-base-content/70">User:</span>
            <span className="font-semibold text-base-content">
              {node_counts.user}
            </span>
          </div>
          <div className="flex items-center gap-2">
            <span>🤖</span>
            <span className="text-base-content/70">Assistant:</span>
            <span className="font-semibold text-base-content">
              {node_counts.assistant}
            </span>
          </div>
          <div className="flex items-center gap-2">
            <span>🔧</span>
            <span className="text-base-content/70">Tool:</span>
            <span className="font-semibold text-base-content">
              {node_counts.tool}
            </span>
          </div>
          <div className="flex items-center gap-2">
            <span>📍</span>
            <span className="text-base-content/70">Checkpoints:</span>
            <span className="font-semibold text-base-content">
              {node_counts.checkpoint}
            </span>
          </div>
        </div>
        <div className="pt-2 border-t border-base-300 text-sm text-base-content/70">
          Total: {node_counts.total} nodes
        </div>
      </div>
    </div>
  );
}
