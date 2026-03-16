// TreeStatsPanel - Statistics dashboard for conversation tree

import { useEffect, useState } from "react";
import type { TreeStatsResponse } from "../../types/backend";
import { getTreeStats, getSessionTree } from "../../services/api";
import { copyToClipboard } from "@/lib/clipboard";

interface TreeStatsPanelProps {
  sessionId: string | null;
  onClose?: () => void;
}

// Helper to generate markdown export of tree
async function exportTreeAsMarkdown(sessionId: string, stats: TreeStatsResponse): Promise<string> {
  const treeData = await getSessionTree(sessionId);

  // Build node map
  const nodeMap = new Map(treeData.nodes.map(n => [n.node_id, n]));

  // Find root
  const root = treeData.nodes.find(n => n.parent_id === null);
  if (!root) return "# Empty Tree";

  // Build children map
  const childrenMap = new Map<string, typeof treeData.nodes>();
  for (const node of treeData.nodes) {
    if (node.parent_id) {
      if (!childrenMap.has(node.parent_id)) {
        childrenMap.set(node.parent_id, []);
      }
      childrenMap.get(node.parent_id)!.push(node);
    }
  }

  // Check if node is on active path
  const activePath = new Set<string>();
  let currentId: string | null = treeData.active_leaf_id;
  while (currentId) {
    activePath.add(currentId);
    const node = nodeMap.get(currentId);
    currentId = node?.parent_id || null;
  }

  // Recursive function to build tree string
  function buildTree(nodeId: string, indent: string = "", isLast: boolean = true): string {
    const node = nodeMap.get(nodeId);
    if (!node) return "";

    const children = childrenMap.get(nodeId) || [];
    const isActive = activePath.has(nodeId);
    const isLeaf = nodeId === treeData.active_leaf_id;

    // Format node line
    const prefix = indent + (isLast ? "└─" : "├─");
    const marker = isLeaf ? " ★ ACTIVE LEAF" : isActive ? " ★" : "";
    const checkpointMarker = node.has_checkpoint ? " 📍" : "";
    const roleIcon = node.role === "User" ? "👤" : node.role === "Assistant" ? "🤖" : node.role === "Tool" ? "🔧" : "⚙️";

    // Full content (preserve newlines for better readability)
    const content = node.content || "";
    // Indent multi-line content properly
    const contentIndent = indent + (isLast ? "  " : "│ ");
    const indentedContent = content.split('\n').map((line, i) =>
      i === 0 ? line : contentIndent + "  " + line
    ).join('\n');

    let line = `${prefix} ${roleIcon} [${node.role}]${checkpointMarker}${marker}\n`;
    line += `${contentIndent}  ${indentedContent}\n`;

    // Show tool calls if any
    if (node.tool_calls && node.tool_calls.length > 0) {
      line += `${indent}${isLast ? "  " : "│ "}  🔧 Tools: ${node.tool_calls.map(tc => tc.name).join(", ")}\n`;
    }

    // Recursively build children
    const childIndent = indent + (isLast ? "  " : "│ ");
    for (let i = 0; i < children.length; i++) {
      line += buildTree(children[i].node_id, childIndent, i === children.length - 1);
    }

    return line;
  }

  // Build markdown document
  const md = `# Conversation Tree Export

## Session Information
- **Session ID**: ${sessionId}
- **Created**: ${new Date(treeData.nodes[0]?.created_at * 1000 || 0).toLocaleString()}
- **Total Nodes**: ${stats.node_counts.total}

## Active Path
${Array.from(activePath).map(id => {
  const node = nodeMap.get(id);
  const roleIcon = node?.role === "User" ? "👤" : node?.role === "Assistant" ? "🤖" : node?.role === "Tool" ? "🔧" : "⚙️";
  return `${roleIcon} ${node?.node_id.substring(0, 8)}`;
}).reverse().join(" → ")}

## Statistics
- **User Messages**: ${stats.node_counts.user}
- **Assistant Messages**: ${stats.node_counts.assistant}
- **Tool Calls**: ${stats.node_counts.tool}
- **Checkpoints**: ${stats.node_counts.checkpoint}
- **Active Path Length**: ${stats.node_counts.active_path} nodes
- **Active Path Tokens**: ${stats.token_usage.active_path_tokens.toLocaleString()}
- **Compression Potential**: ${stats.token_usage.potential_token_savings.toLocaleString()} tokens (${Math.round((1 - 1 / stats.token_usage.estimated_compression_ratio) * 100)}% reduction)

## Tree Structure
\`\`\`
${buildTree(root.node_id)}
\`\`\`

---
*Exported from aaagent-rs at ${new Date().toLocaleString()}*
`;

  return md;
}

export function TreeStatsPanel({ sessionId, onClose }: TreeStatsPanelProps) {
  const [stats, setStats] = useState<TreeStatsResponse | null>(null);
  const [initialLoad, setInitialLoad] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [exporting, setExporting] = useState(false);
  const [exportSuccess, setExportSuccess] = useState(false);

  useEffect(() => {
    if (!sessionId) {
      setStats(null);
      setInitialLoad(true);
      return;
    }

    const fetchStats = async (isInitial = false) => {
      if (isInitial) {
        setInitialLoad(true);
      }
      setError(null);
      try {
        const data = await getTreeStats(sessionId);
        setStats(data);
      } catch (err) {
        console.error("Failed to fetch tree stats:", err);
        setError(err instanceof Error ? err.message : "Unknown error");
      } finally {
        if (isInitial) {
          setInitialLoad(false);
        }
      }
    };

    // Initial fetch
    fetchStats(true);

    // Refresh stats every 10 seconds (background refresh, no loading state)
    const interval = setInterval(() => fetchStats(false), 10000);
    return () => clearInterval(interval);
  }, [sessionId]);

  if (!sessionId || initialLoad) {
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

  // Export handler
  const handleExport = async () => {
    if (!sessionId || !stats) return;

    setExporting(true);
    setExportSuccess(false);

    try {
      const markdown = await exportTreeAsMarkdown(sessionId, stats);
      await copyToClipboard(markdown);
      setExportSuccess(true);
      setTimeout(() => setExportSuccess(false), 2000);
    } catch (err) {
      console.error("Failed to export tree:", err);
      alert("Failed to export tree: " + (err instanceof Error ? err.message : "Unknown error"));
    } finally {
      setExporting(false);
    }
  };

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

      {/* Export Button */}
      <div>
        <button
          onClick={handleExport}
          disabled={exporting}
          className={`w-full px-4 py-2 rounded-lg border transition-all flex items-center justify-center gap-2 ${
            exportSuccess
              ? "bg-success/10 border-success/20 text-success"
              : "bg-base-300 border-base-content/20 hover:bg-base-content/10 text-base-content"
          }`}
        >
          {exporting ? (
            <>
              <span className="animate-spin">⏳</span>
              <span>Exporting...</span>
            </>
          ) : exportSuccess ? (
            <>
              <span>✓</span>
              <span>Copied to Clipboard!</span>
            </>
          ) : (
            <>
              <span>📤</span>
              <span>Export Tree as Markdown</span>
            </>
          )}
        </button>
        {!exportSuccess && (
          <p className="mt-2 text-xs text-base-content/50 text-center">
            Exports full tree structure for LLM analysis
          </p>
        )}
      </div>
    </div>
  );
}
