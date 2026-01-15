// TreeNode component - Renders individual tree nodes with status indicators

import type { PositionedNode } from "./treeLayout";
import { DEFAULT_CONFIG } from "./treeLayout";

export interface TreeNodeProps {
  node: PositionedNode;
  isSelected: boolean;
  onSelect?: (nodeId: string) => void;
}

const NODE_COLORS: Record<string, string> = {
  user: "hsl(var(--role-user))",
  assistant: "hsl(var(--role-assistant))",
  system: "hsl(var(--role-system))",
  tool: "hsl(var(--role-tool))",
};

export function TreeNode({ node, isSelected, onSelect }: TreeNodeProps) {
  const strokeColor = NODE_COLORS[node.role] || NODE_COLORS.system;
  const baseSize = node.hasCheckpoint
    ? DEFAULT_CONFIG.nodeSize + 4
    : DEFAULT_CONFIG.nodeSize;
  const size = isSelected ? baseSize * 1.3 : baseSize;
  const opacity = node.isActive ? 1.0 : 0.3;

  const handleClick = () => {
    onSelect?.(node.id);
  };

  // Center fill color based on status
  const getCenterFill = () => {
    if (node.status === "error") return "hsl(var(--destructive))";
    if (node.status === "loading") return strokeColor;
    return "transparent";
  };

  // Tooltip: show brief preview of content
  const tooltipText =
    node.content.slice(0, 100) + (node.content.length > 100 ? "..." : "");

  return (
    <g
      className={`node-tree ${isSelected ? "selected" : ""} ${node.isActive ? "active" : "inactive"}`}
      transform={`translate(${node.x},${node.y})`}
      onClick={handleClick}
      style={{ cursor: "pointer", transition: "all 0.2s ease" }}
    >
      {/* Outer ring (hollow circle) */}
      <circle
        className="node-circle"
        r={size}
        fill="hsl(var(--background))"
        stroke={strokeColor}
        strokeWidth={6}
        opacity={opacity}
      />

      {/* Inner center (for status indication) */}
      <circle
        className="node-center"
        r={size - 7}
        fill={getCenterFill()}
        opacity={opacity}
      />

      {/* Selection glow - multiple rings */}
      {isSelected && (
        <>
          <circle
            r={size + 8}
            fill="none"
            stroke={strokeColor}
            strokeWidth={2}
            opacity={0.3}
          />
          <circle
            r={size + 12}
            fill="none"
            stroke={strokeColor}
            strokeWidth={1}
            opacity={0.15}
          />
        </>
      )}

      {/* Checkpoint marker - rotated square */}
      {node.hasCheckpoint && (
        <rect
          className="checkpoint-marker"
          width={5}
          height={5}
          x={-2.5}
          y={-2.5}
          fill="hsl(var(--primary))"
          opacity={0.9}
          transform="rotate(45)"
        />
      )}

      {/* Loading spinner - dashed ring */}
      {node.status === "loading" && (
        <circle
          className="spinner-ring"
          r={size}
          fill="none"
          stroke={strokeColor}
          strokeWidth={1}
          strokeDasharray="3 2"
          opacity={0.6}
        >
          <animateTransform
            attributeName="transform"
            type="rotate"
            from="0 0 0"
            to="360 0 0"
            dur="2s"
            repeatCount="indefinite"
          />
        </circle>
      )}

      {/* Tooltip - show on hover */}
      <title>{tooltipText}</title>
    </g>
  );
}
