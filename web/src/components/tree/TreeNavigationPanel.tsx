// TreeNavigationPanel - Main component for tree-based conversation navigation

import { useState, useRef } from "react";
import { TreeVisualization } from "./TreeVisualization";
import { TreeControls } from "./TreeControls";
import type { TreeNode } from "./treeLayout";

export interface TreeNavigationPanelProps {
  nodes: TreeNode[];
  activeLeafId: string;
  selectedNodeId?: string | null;
  onNodeSelect?: (nodeId: string) => void;
  onBranchSwitch?: (nodeId: string) => void;
}

export function TreeNavigationPanel({
  nodes,
  activeLeafId,
  selectedNodeId,
  onNodeSelect,
  onBranchSwitch,
}: TreeNavigationPanelProps) {
  const [showInactive, setShowInactive] = useState(true);
  const containerRef = useRef<HTMLDivElement>(null);

  const handleCenterTree = () => {
    if (containerRef.current) {
      const svg = containerRef.current.querySelector("svg");
      if (svg) {
        // Reset viewBox to show entire tree
        // The TreeVisualization component handles viewBox calculation
        // This triggers a re-center by scrolling to top-left
        containerRef.current.scrollTo({
          top: 0,
          left: 0,
          behavior: "smooth",
        });
      }
    }
  };

  const handleNodeSelect = (nodeId: string) => {
    // Scroll message card into view in the chat container
    setTimeout(() => {
      // First try exact match, then try prefix match (for tool call messages)
      let messageCard = document.getElementById(`message-${nodeId}`);
      let actualMessageId = nodeId;

      if (!messageCard) {
        // For assistant messages with tool calls but no content, the ID might be like:
        // message-{nodeId}-tc-{toolCallId}
        // Find any element that starts with the nodeId prefix
        const allCards = document.querySelectorAll(`[id^="message-${nodeId}"]`);
        if (allCards.length > 0) {
          messageCard = allCards[0] as HTMLElement;
          // Extract the actual message ID from the element ID
          const elementId = messageCard.id;
          if (elementId.startsWith("message-")) {
            actualMessageId = elementId.substring("message-".length);
          }
        }
      }

      // Select the message with the actual ID (might be different from nodeId for tool calls)
      onNodeSelect?.(actualMessageId);

      const chatContainer = document.querySelector(".chat-container");

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
      } else if (!messageCard) {
        // No message card found - still select the node ID
        // This handles cases where the message might not be rendered yet
        onNodeSelect?.(nodeId);
      }
    }, 100);
  };

  const handleNodeDoubleClick = (nodeId: string) => {
    // Double-click switches the active branch to this node
    onBranchSwitch?.(nodeId);
  };

  return (
    <div className="tree-panel h-full flex flex-col bg-background border-r border-border">
      {/* Controls */}
      <TreeControls
        showInactive={showInactive}
        onToggleInactive={() => setShowInactive(!showInactive)}
        onCenterTree={handleCenterTree}
      />

      {/* Visualization */}
      <div ref={containerRef} className="flex-1 overflow-auto bg-background">
        <TreeVisualization
          nodes={nodes}
          activeLeafId={activeLeafId}
          selectedNodeId={selectedNodeId}
          showInactive={showInactive}
          onNodeSelect={handleNodeSelect}
          onNodeDoubleClick={handleNodeDoubleClick}
        />
      </div>

    </div>
  );
}
