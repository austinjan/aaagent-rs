// TreeNavigationPanel - Main component for tree-based conversation navigation

import { useState, useRef, useMemo } from "react";
import { TreeVisualization } from "./TreeVisualization";
import { NodeDetailsPanel } from "./NodeDetailsPanel";
import { TreeStatsPanel } from "./TreeStatsPanel";
import type { TreeNode } from "./treeLayout";

export interface TreeNavigationPanelProps {
  nodes: TreeNode[];
  activeLeafId: string;
  selectedNodeId?: string | null;
  sessionId?: string | null;
  onNodeSelect?: (nodeId: string) => void;
  onBranchSwitch?: (nodeId: string) => void;
}

export function TreeNavigationPanel({
  nodes,
  activeLeafId,
  selectedNodeId,
  sessionId,
  onNodeSelect,
  onBranchSwitch,
}: TreeNavigationPanelProps) {
  const [showInactive] = useState(true); // Always show inactive branches
  const [showStats, setShowStats] = useState(true); // Show stats by default
  const containerRef = useRef<HTMLDivElement>(null);

  // Find the selected node from nodes array
  const selectedNode = useMemo(() => {
    if (!selectedNodeId) return null;
    return nodes.find((node) => node.id === selectedNodeId) || null;
  }, [selectedNodeId, nodes]);

  const handleNodeSelect = (nodeId: string) => {
    // Scroll message card into view in the chat container
    setTimeout(() => {
      // Direct lookup - no synthetic ID handling needed
      const messageCard = document.getElementById(`message-${nodeId}`);
      const chatContainer = document.querySelector(".chat-container");

      // Scroll to the message card if found
      if (messageCard && chatContainer) {
        const containerRect = chatContainer.getBoundingClientRect();
        const cardRect = messageCard.getBoundingClientRect();
        const scrollTop = chatContainer.scrollTop;

        // Calculate position to center the card in the container
        const targetScroll =
          scrollTop +
          cardRect.top -
          containerRect.top -
          containerRect.height / 2 +
          cardRect.height / 2;

        chatContainer.scrollTo({
          top: targetScroll,
          behavior: "smooth",
        });
      }

      // Always select the node ID
      onNodeSelect?.(nodeId);
    }, 100);
  };

  const handleNodeDoubleClick = (nodeId: string) => {
    // Double-click switches the active branch to this node
    onBranchSwitch?.(nodeId);
  };

  return (
    <div className="tree-panel h-full flex flex-col bg-background">
      {/* Stats Toggle Button */}
      <div className="p-2 border-b border-border bg-background flex justify-end">
        <button
          onClick={() => setShowStats(!showStats)}
          className="px-3 py-1 text-sm rounded-lg border border-border hover:bg-base-200 transition-colors flex items-center gap-2"
        >
          <span>📊</span>
          <span>{showStats ? "Hide Stats" : "Show Stats"}</span>
        </button>
      </div>

      {/* Main Content */}
      <div className="flex-1 flex overflow-hidden">
        {/* Tree Visualization */}
        <div ref={containerRef} className="flex-1 overflow-auto bg-background">
          {/* Stats Panel (Collapsible) */}
          {showStats && sessionId && (
            <div className="p-4">
              <TreeStatsPanel sessionId={sessionId} />
            </div>
          )}

          <TreeVisualization
            nodes={nodes}
            activeLeafId={activeLeafId}
            selectedNodeId={selectedNodeId}
            showInactive={showInactive}
            onNodeSelect={handleNodeSelect}
            onNodeDoubleClick={handleNodeDoubleClick}
          />
        </div>

        {/* Node Details Panel */}
        {selectedNode && (
          <div className="w-96 flex-shrink-0">
            <NodeDetailsPanel
              node={selectedNode}
              onClose={() => onNodeSelect?.("")}
            />
          </div>
        )}
      </div>
    </div>
  );
}
