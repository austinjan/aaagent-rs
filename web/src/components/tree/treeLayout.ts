// Tree layout algorithm for conversation history visualization
// Based on doc/plan/chat-ui-tree-panel-plan.md and tree-visualization-demo.html

export interface TreeNode {
  id: string;
  parent_id: string | null;
  role: "user" | "assistant" | "system" | "tool";
  content: string;
  seq: number;

  // Optional metadata
  hasCheckpoint?: boolean;
  inContext?: boolean;
  checkpointSummary?: string;
  status?: "loading" | "error" | "complete";
  timestamp?: Date;
}

export interface PositionedNode extends TreeNode {
  x: number;
  y: number;
  depth: number;
  isActive: boolean;
  isCollapsed?: boolean;
  // Inherited from TreeNode but explicitly listed for clarity
  inContext?: boolean;
  checkpointSummary?: string;
}

export interface TreeConfig {
  nodeSpacing: number; // Vertical spacing between nodes (60px)
  branchSpacing: number; // Horizontal spacing between branches (120px)
  nodeSize: number; // Node radius (16px standard, 20px checkpoint)
  collapseThreshold: number; // Depth threshold for collapsing (5)
  edgeStrokeWidth: number; // Edge line width (2px active, 1px inactive)
}

export const DEFAULT_CONFIG: TreeConfig = {
  nodeSpacing: 80, // Increased for text labels
  branchSpacing: 400, // Increased for content preview
  nodeSize: 12,
  collapseThreshold: 5,
  edgeStrokeWidth: 6,
};

/**
 * Build a map of parent_id -> children[] for efficient tree traversal
 */
export function buildChildrenMap(nodes: TreeNode[]): Map<string, TreeNode[]> {
  const map = new Map<string, TreeNode[]>();

  for (const node of nodes) {
    if (!node.parent_id) continue;

    if (!map.has(node.parent_id)) {
      map.set(node.parent_id, []);
    }

    map.get(node.parent_id)!.push(node);
  }

  // Sort siblings by seq number
  for (const siblings of map.values()) {
    siblings.sort((a, b) => a.seq - b.seq);
  }

  return map;
}

/**
 * Get the set of node IDs on the active path from root to activeLeafId
 */
export function getActivePath(
  nodes: TreeNode[],
  activeLeafId: string,
): Set<string> {
  const path = new Set<string>();
  const nodeMap = new Map(nodes.map((n) => [n.id, n]));

  let current = nodeMap.get(activeLeafId);

  while (current) {
    path.add(current.id);
    current = current.parent_id ? nodeMap.get(current.parent_id) : undefined;
  }

  return path;
}

/**
 * Layout tree nodes using depth-first traversal
 * Returns positioned nodes with x, y coordinates
 */
export function layoutTree(
  nodes: TreeNode[],
  activeLeafId: string,
  config: TreeConfig = DEFAULT_CONFIG,
): PositionedNode[] {
  if (nodes.length === 0) return [];

  const childrenMap = buildChildrenMap(nodes);
  const activePath = getActivePath(nodes, activeLeafId);
  const nodeMap = new Map(nodes.map((n) => [n.id, n]));
  const positioned: PositionedNode[] = [];

  let xOffset = 0; // Track horizontal position for leaf nodes

  /**
   * Depth-first traversal to position nodes
   * Returns the x-coordinate of this node (midpoint of children if branch)
   */
  function dfs(
    nodeId: string,
    depth: number,
    _parentX: number,
    _parentY: number,
  ): number {
    const node = nodeMap.get(nodeId);
    if (!node) return xOffset;

    const isActive = activePath.has(nodeId);
    const children = childrenMap.get(nodeId) || [];

    // Calculate this node's Y position
    const y = depth * config.nodeSpacing;

    let x: number;

    if (children.length === 0) {
      // Leaf node: use current xOffset and increment
      x = xOffset;
      xOffset += config.branchSpacing;
    } else {
      // Branch node: recursively layout children first, then position at midpoint
      const childXPositions: number[] = [];

      for (const child of children) {
        const childMidX = dfs(child.id, depth + 1, 0, y);
        childXPositions.push(childMidX);
      }

      // Position this node at the midpoint of its children
      x = (Math.min(...childXPositions) + Math.max(...childXPositions)) / 2;
    }

    positioned.push({
      ...node,
      x,
      y,
      depth,
      isActive,
    });

    return x;
  }

  // Find root node (node with parent_id === null)
  const root = nodes.find((n) => n.parent_id === null);
  if (!root) {
    console.error("No root node found in tree");
    return [];
  }

  // Start DFS from root
  dfs(root.id, 0, 0, 0);

  return positioned;
}

/**
 * Collapse inactive branches
 * When hideAll is true, collapse all inactive nodes
 * Otherwise, only collapse inactive nodes deeper than threshold
 */
export function collapseInactiveBranches(
  positioned: PositionedNode[],
  threshold: number,
  hideAll: boolean = false,
): PositionedNode[] {
  return positioned.map((node) => {
    // When hideAll is true, collapse all inactive nodes
    // Otherwise, only collapse inactive nodes at depth >= threshold
    if (!node.isActive && (hideAll || node.depth >= threshold)) {
      return { ...node, isCollapsed: true };
    }
    return node;
  });
}
