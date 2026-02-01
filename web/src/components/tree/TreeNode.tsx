// TreeNode component - Renders individual tree nodes with status indicators

import type { PositionedNode } from "./treeLayout";
import { DEFAULT_CONFIG } from "./treeLayout";

export interface TreeNodeProps {
  node: PositionedNode;
  isSelected: boolean;
  onSelect?: (nodeId: string) => void;
  onDoubleClick?: (nodeId: string) => void;
}

const NODE_COLORS: Record<string, string> = {
  user: "hsl(var(--role-user))",
  assistant: "hsl(var(--role-assistant))",
  system: "hsl(var(--role-system))",
  tool: "hsl(var(--role-tool))",
  checkpoint: "hsl(var(--role-checkpoint))",
};

export function TreeNode({
  node,
  isSelected,
  onSelect,
  onDoubleClick,
}: TreeNodeProps) {
  const checkpointColor = NODE_COLORS.checkpoint;
  // Checkpoint nodes use gold color, others use role color
  const strokeColor = node.hasCheckpoint
    ? checkpointColor
    : NODE_COLORS[node.role] || NODE_COLORS.system;
  const baseSize = node.hasCheckpoint
    ? DEFAULT_CONFIG.nodeSize + 4
    : DEFAULT_CONFIG.nodeSize;
  const size = isSelected ? baseSize * 1.3 : baseSize;

  // In-context nodes are fully visible, others are dimmed
  const opacity = node.inContext ? 1.0 : node.isActive ? 0.5 : 0.2;

  const handleClick = () => {
    onSelect?.(node.id);
  };

  const handleDoubleClick = () => {
    onDoubleClick?.(node.id);
  };

  // Check if this assistant node has tool calls
  const hasToolCalls = node.role === "assistant" && node.tool_calls && node.tool_calls.length > 0;

  // Center fill color based on status and context
  const getCenterFill = () => {
    if (node.status === "error") return "hsl(var(--destructive))";
    if (node.status === "loading") return strokeColor;
    // Checkpoint nodes are always filled with gold
    if (node.hasCheckpoint) return checkpointColor;
    // Assistant nodes with tool calls get tool color (orange) inner fill
    if (hasToolCalls) return NODE_COLORS.tool;
    // In-context nodes get a filled center
    if (node.inContext) return strokeColor;
    return "transparent";
  };

  // Rich tooltip with role, timestamp, and content preview
  const getRoleLabel = () => {
    const labels: Record<string, string> = {
      user: "👤 User",
      assistant: "🤖 Assistant",
      system: "⚙️ System",
      tool: "🔧 Tool",
    };
    return labels[node.role] || node.role;
  };

  const getTooltipContent = () => {
    if (node.hasCheckpoint) {
      const summary = node.checkpointSummary || "Checkpoint created";
      return `📌 Checkpoint\n\n${summary.slice(0, 200)}${summary.length > 200 ? "..." : ""}`;
    }

    const preview = node.content.slice(0, 200);
    const toolCallInfo = hasToolCalls
      ? `\n\n🔧 Tool calls: ${node.tool_calls!.map(tc => tc.name).join(", ")}`
      : "";
    return `${getRoleLabel()}${toolCallInfo}\n\n${preview}${node.content.length > 200 ? "..." : ""}`;
  };

  const tooltipText = getTooltipContent();

  // Format timestamp
  const getTimeText = () => {
    if (!node.timestamp) return "";
    const date = new Date(node.timestamp);
    const hours = date.getHours().toString().padStart(2, "0");
    const minutes = date.getMinutes().toString().padStart(2, "0");
    return `${hours}:${minutes}`;
  };

  // Get content preview (first line or 30 chars)
  const getContentPreview = () => {
    if (node.hasCheckpoint && node.checkpointSummary) {
      return node.checkpointSummary.slice(0, 30);
    }
    const firstLine = node.content.split("\n")[0];
    return firstLine.length > 30 ? firstLine.slice(0, 30) + "..." : firstLine;
  };

  return (
    <g
      className={`node-tree ${isSelected ? "selected" : ""} ${node.isActive ? "active" : "inactive"}`}
      transform={`translate(${node.x},${node.y})`}
      onClick={handleClick}
      onDoubleClick={handleDoubleClick}
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

      {/* Text labels - role and content preview */}
      <g>
        {/* Role label */}
        <text
          x={size + 12}
          y={-8}
          fontSize="11"
          fontWeight="600"
          fill={strokeColor}
          opacity={opacity}
          style={{ userSelect: "none" }}
        >
          {node.hasCheckpoint ? "📌 Checkpoint" : hasToolCalls ? "🤖🔧 Assistant" : getRoleLabel()}
        </text>

        {/* Content preview */}
        <text
          x={size + 12}
          y={6}
          fontSize="10"
          fill="hsl(var(--muted-foreground))"
          style={{ userSelect: "none" }}
        >
          {getContentPreview()}
        </text>

        {/* Timestamp */}
        {node.timestamp && (
          <text
            x={size + 12}
            y={18}
            fontSize="9"
            fill="hsl(var(--muted-foreground))"
            opacity={0.6}
            style={{ userSelect: "none" }}
          >
            {getTimeText()}
          </text>
        )}
      </g>

      {/* Rich tooltip - show on hover */}
      <title>{tooltipText}</title>
    </g>
  );
}
