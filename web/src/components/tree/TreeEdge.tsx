// TreeEdge component - Renders edges (lines) between parent and child nodes

import type { PositionedNode } from "./treeLayout";

export interface TreeEdgeProps {
  parent: PositionedNode;
  child: PositionedNode;
  isActive: boolean;
}

export function TreeEdge({ parent, child, isActive }: TreeEdgeProps) {
  const strokeWidth = 6;
  const opacity = isActive ? 1.0 : 0.3;
  const color = "hsl(var(--border))";

  return (
    <line
      className={isActive ? "edge-active" : "edge-inactive"}
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
