// TreeEdge component - Renders edges (lines) between parent and child nodes

import type { PositionedNode } from "./treeLayout";

export interface TreeEdgeProps {
  parent: PositionedNode;
  child: PositionedNode;
  isActive: boolean;
}

export function TreeEdge({ parent, child, isActive }: TreeEdgeProps) {
  // Check if both nodes are in context (part of current history)
  const isInContext = parent.inContext && child.inContext;

  // In-context edges are much more prominent: thicker and bright color
  const strokeWidth = isInContext ? 10 : 4;
  const opacity = isInContext ? 1.0 : (isActive ? 0.4 : 0.15);
  // Use a bright, contrasting color for in-context (cyan/teal) vs muted gray for others
  const color = isInContext
    ? "hsl(190, 80%, 50%)" // Bright cyan for in-context
    : "hsl(var(--muted-foreground))";

  return (
    <line
      className={`${isActive ? "edge-active" : "edge-inactive"} ${isInContext ? "edge-in-context" : ""}`}
      x1={parent.x}
      y1={parent.y}
      x2={child.x}
      y2={child.y}
      stroke={color}
      strokeWidth={strokeWidth}
      opacity={opacity}
      strokeLinecap="round"
    />
  );
}
