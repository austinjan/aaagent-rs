// TreeVisualization component - SVG rendering of the conversation tree

import { useMemo } from "react";
import {
  layoutTree,
  collapseInactiveBranches,
  DEFAULT_CONFIG,
} from "./treeLayout";
import type { TreeNode, PositionedNode, TreeConfig } from "./treeLayout";
import { TreeNode as TreeNodeComponent } from "./TreeNode";
import { TreeEdge } from "./TreeEdge";

export interface TreeVisualizationProps {
  nodes: TreeNode[];
  activeLeafId: string;
  selectedNodeId?: string | null;
  showInactive: boolean;
  onNodeSelect?: (nodeId: string) => void;
  config?: Partial<TreeConfig>;
}

export function TreeVisualization({
  nodes,
  activeLeafId,
  selectedNodeId,
  showInactive,
  onNodeSelect,
  config: configOverride,
}: TreeVisualizationProps) {
  const config = { ...DEFAULT_CONFIG, ...configOverride };

  // Layout tree with memoization
  const positioned = useMemo(() => {
    if (nodes.length === 0) return [];

    let layout = layoutTree(nodes, activeLeafId, config);

    // Collapse inactive branches if showInactive is false
    if (!showInactive) {
      layout = collapseInactiveBranches(layout, config.collapseThreshold);
    }

    return layout;
  }, [nodes, activeLeafId, showInactive, config]);

  // Build edges from positioned nodes
  const edges = useMemo(() => {
    const result: Array<{
      parent: PositionedNode;
      child: PositionedNode;
      isActive: boolean;
    }> = [];
    const nodeMap = new Map(positioned.map((n) => [n.id, n]));

    for (const node of positioned) {
      if (node.parent_id && !node.isCollapsed) {
        const parent = nodeMap.get(node.parent_id);
        if (parent) {
          const isActive = parent.isActive && node.isActive;
          result.push({ parent, child: node, isActive });
        }
      }
    }

    return result;
  }, [positioned]);

  // Calculate SVG dimensions with padding
  const padding = 50;
  const maxX =
    positioned.length > 0 ? Math.max(...positioned.map((n) => n.x), 0) : 0;
  const maxY =
    positioned.length > 0 ? Math.max(...positioned.map((n) => n.y), 0) : 0;
  const width = maxX + config.branchSpacing + padding * 2;
  const height = maxY + config.nodeSpacing + padding * 2;

  // Filter out collapsed nodes
  const visibleNodes = positioned.filter((n) => !n.isCollapsed);

  if (nodes.length === 0) {
    return (
      <div className="flex items-center justify-center h-full text-muted-foreground">
        No conversation history yet
      </div>
    );
  }

  return (
    <svg
      width={width}
      height={height}
      className="tree-svg"
      style={{ display: "block" }}
    >
      {/* Offset group to add padding */}
      <g transform={`translate(${padding}, ${padding})`}>
        {/* Edges (render first, behind nodes) */}
        <g className="edges">
          {edges.map(({ parent, child, isActive }) => (
            <TreeEdge
              key={`${parent.id}-${child.id}`}
              parent={parent}
              child={child}
              isActive={isActive}
            />
          ))}
        </g>

        {/* Nodes */}
        <g className="nodes">
          {visibleNodes.map((node) => (
            <TreeNodeComponent
              key={node.id}
              node={node}
              isSelected={node.id === selectedNodeId}
              onSelect={onNodeSelect}
            />
          ))}
        </g>
      </g>
    </svg>
  );
}
