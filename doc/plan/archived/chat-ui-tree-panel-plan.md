# Chat UI Tree Navigation Panel Plan

- Feature name: `chat-ui-tree-panel`
- Status: ✅ **ACHIEVED** - Ready for Archive
- Created: 2026-01-12
- Updated: 2026-01-15
- Completed: 2026-01-15
- Parent plan: [chat-ui-component-plan.md](./chat-ui-component-plan.md)

## 1) Overview

### Goal
Build an interactive tree visualization panel that displays conversation history as a navigable tree structure with nodes, edges, hover previews, and click-to-scroll navigation.

### Design Principles

1. **Tree structure reflects conversation flow** - Parent-child relationships visible
2. **Active path clearly highlighted** - Bold edges from root to current message
3. **Efficient space usage** - Collapse inactive branches after threshold
4. **Rich interaction** - Hover for preview, click to navigate
5. **Real-time updates** - Tree updates as conversation grows

## 2) Reference Implementation

Based on `doc/tree-visualization-demo.html`:

**Key Features Demonstrated:**
- SVG-based rendering with D3.js layout
- Circular nodes with role-based colors
- Edge highlighting for active vs inactive paths
- Hover tooltips showing message previews
- Click navigation to scroll message into view
- Collapsed branch groups for deep inactive paths
- Status indicators: loading spinner, error icon, checkpoint marker

## 3) Component Architecture

```
TreeNavigationPanel
├── TreeControls (top controls)
│   ├── Show/Hide inactive branches toggle
│   └── Center tree button
├── TreeVisualization (SVG container)
│   ├── TreeEdges (path elements for connections)
│   ├── TreeNodes (circle + status indicators)
│   └── TreeTooltip (hover preview)
└── TreeLegend (role color legend)
```

## 4) Data Structures

### TreeNode

```typescript
interface TreeNode {
  id: string;                    // Node ID (unique)
  parent_id: string | null;      // Parent node ID (null for root)
  role: 'user' | 'assistant' | 'system' | 'tool';
  content: string;               // Full message content
  seq: number;                   // Sequence number within siblings
  
  // Optional metadata
  hasCheckpoint?: boolean;       // Node has checkpoint
  status?: 'loading' | 'error' | 'complete';
  timestamp?: Date;
}
```

### TreeLayout

```typescript
interface PositionedNode extends TreeNode {
  x: number;                     // SVG x coordinate
  y: number;                     // SVG y coordinate
  depth: number;                 // Distance from root
  isActive: boolean;             // On active path to current leaf
  isCollapsed?: boolean;         // Part of collapsed group
}
```

### TreeConfig

```typescript
interface TreeConfig {
  nodeSpacing: number;           // Vertical spacing between nodes (60px)
  branchSpacing: number;         // Horizontal spacing between branches (120px)
  nodeSize: number;              // Node radius (16px standard, 20px checkpoint)
  collapseThreshold: number;     // Depth threshold for collapsing (5)
  edgeStrokeWidth: number;       // Edge line width (2px active, 1px inactive)
}
```

## 5) Layout Algorithm

Based on tree-visualization-demo.html implementation:

### Step 1: Build Children Map
```typescript
function buildChildrenMap(nodes: TreeNode[]): Map<string, TreeNode[]> {
  const map = new Map<string, TreeNode[]>();
  
  for (const node of nodes) {
    if (!node.parent_id) continue;
    
    if (!map.has(node.parent_id)) {
      map.set(node.parent_id, []);
    }
    
    map.get(node.parent_id)!.push(node);
  }
  
  // Sort siblings by seq
  for (const siblings of map.values()) {
    siblings.sort((a, b) => a.seq - b.seq);
  }
  
  return map;
}
```

### Step 2: Calculate Active Path
```typescript
function getActivePath(
  nodes: TreeNode[],
  activeLeafId: string
): Set<string> {
  const path = new Set<string>();
  const nodeMap = new Map(nodes.map(n => [n.id, n]));
  
  let current = nodeMap.get(activeLeafId);
  
  while (current) {
    path.add(current.id);
    current = current.parent_id ? nodeMap.get(current.parent_id) : null;
  }
  
  return path;
}
```

### Step 3: Depth-First Layout
```typescript
function layoutTree(
  nodes: TreeNode[],
  activeLeafId: string,
  config: TreeConfig
): PositionedNode[] {
  const childrenMap = buildChildrenMap(nodes);
  const activePath = getActivePath(nodes, activeLeafId);
  const positioned: PositionedNode[] = [];
  
  let xOffset = 0;  // Track horizontal position
  
  function dfs(
    nodeId: string,
    depth: number,
    parentX: number,
    parentY: number
  ): number {
    const node = nodes.find(n => n.id === nodeId)!;
    const isActive = activePath.has(nodeId);
    const children = childrenMap.get(nodeId) || [];
    
    // Calculate this node's Y position
    const y = depth * config.nodeSpacing;
    
    let x: number;
    
    if (children.length === 0) {
      // Leaf node: use current xOffset
      x = xOffset;
      xOffset += config.branchSpacing;
    } else {
      // Branch node: position at midpoint of children
      const childXPositions: number[] = [];
      
      for (const child of children) {
        const childMidX = dfs(child.id, depth + 1, x, y);
        childXPositions.push(childMidX);
      }
      
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
  
  // Start DFS from root
  const root = nodes.find(n => n.parent_id === null)!;
  dfs(root.id, 0, 0, 0);
  
  return positioned;
}
```

### Step 4: Collapse Inactive Branches
```typescript
function collapseInactiveBranches(
  positioned: PositionedNode[],
  threshold: number
): PositionedNode[] {
  return positioned.map(node => {
    if (!node.isActive && node.depth >= threshold) {
      return { ...node, isCollapsed: true };
    }
    return node;
  });
}
```

## 6) SVG Rendering

### Node Rendering

```typescript
function renderNode(node: PositionedNode, config: TreeConfig) {
  const color = NODE_COLORS[node.role];
  const size = node.hasCheckpoint ? config.nodeSize + 4 : config.nodeSize;
  
  return (
    <g
      key={node.id}
      className="node-tree"
      transform={`translate(${node.x},${node.y})`}
      onClick={() => handleNodeClick(node.id)}
    >
      {/* Background circle */}
      <circle
        className="node-circle"
        r={size}
        fill={color}
        opacity={node.isActive ? 1.0 : 0.3}
      />
      
      {/* Checkpoint marker */}
      {node.hasCheckpoint && (
        <circle
          className="checkpoint-marker"
          r={size + 4}
          fill="none"
          stroke="#a855f7"
          strokeWidth={2}
        />
      )}
      
      {/* Status indicators */}
      {node.status === 'loading' && <LoadingSpinner />}
      {node.status === 'error' && <ErrorIcon />}
      
      {/* Hover tooltip */}
      <title>{node.content.slice(0, 100)}...</title>
    </g>
  );
}
```

### Edge Rendering

```typescript
function renderEdge(
  parent: PositionedNode,
  child: PositionedNode,
  isActive: boolean
) {
  const strokeWidth = isActive ? 2 : 1;
  const opacity = isActive ? 1.0 : 0.3;
  const color = isActive ? '#6b7280' : '#374151';
  
  return (
    <line
      key={`${parent.id}-${child.id}`}
      className={isActive ? 'edge-active' : 'edge-inactive'}
      x1={parent.x}
      y1={parent.y}
      x2={child.x}
      y2={child.y}
      stroke={color}
      strokeWidth={strokeWidth}
      opacity={opacity}
    />
  );
}
```

## 7) Component Implementation

### TreeNavigationPanel Component

```typescript
// web/src/components/tree/TreeNavigationPanel.tsx

import React, { useState, useEffect, useRef } from 'react';
import { TreeVisualization } from './TreeVisualization';
import { TreeControls } from './TreeControls';

export interface TreeNavigationPanelProps {
  nodes: TreeNode[];
  activeLeafId: string;
  selectedNodeId?: string | null;
  onNodeSelect?: (nodeId: string) => void;
}

export function TreeNavigationPanel({
  nodes,
  activeLeafId,
  selectedNodeId,
  onNodeSelect,
}: TreeNavigationPanelProps) {
  const [showInactive, setShowInactive] = useState(true);
  const containerRef = useRef<HTMLDivElement>(null);

  const handleCenterTree = () => {
    if (containerRef.current) {
      const svg = containerRef.current.querySelector('svg');
      if (svg) {
        // Center SVG in viewport
        const bbox = svg.getBBox();
        const viewBox = `${bbox.x - 50} ${bbox.y - 50} ${bbox.width + 100} ${bbox.height + 100}`;
        svg.setAttribute('viewBox', viewBox);
      }
    }
  };

  return (
    <div className="tree-panel h-full flex flex-col bg-gray-900 border-r border-gray-700">
      {/* Controls */}
      <TreeControls
        showInactive={showInactive}
        onToggleInactive={() => setShowInactive(!showInactive)}
        onCenterTree={handleCenterTree}
      />

      {/* Visualization */}
      <div ref={containerRef} className="flex-1 overflow-auto">
        <TreeVisualization
          nodes={nodes}
          activeLeafId={activeLeafId}
          selectedNodeId={selectedNodeId}
          showInactive={showInactive}
          onNodeSelect={onNodeSelect}
        />
      </div>
    </div>
  );
}
```

### TreeVisualization Component

```typescript
// web/src/components/tree/TreeVisualization.tsx

import React, { useMemo } from 'react';
import { layoutTree, collapseInactiveBranches } from './treeLayout';
import { TreeNode } from './TreeNode';
import { TreeEdge } from './TreeEdge';

const CONFIG = {
  nodeSpacing: 60,
  branchSpacing: 120,
  nodeSize: 16,
  collapseThreshold: 5,
  edgeStrokeWidth: 2,
};

export function TreeVisualization({
  nodes,
  activeLeafId,
  selectedNodeId,
  showInactive,
  onNodeSelect,
}: TreeVisualizationProps) {
  const positioned = useMemo(() => {
    let layout = layoutTree(nodes, activeLeafId, CONFIG);
    
    if (!showInactive) {
      layout = collapseInactiveBranches(layout, CONFIG.collapseThreshold);
    }
    
    return layout;
  }, [nodes, activeLeafId, showInactive]);

  const edges = useMemo(() => {
    const result: Array<{ parent: PositionedNode; child: PositionedNode }> = [];
    const nodeMap = new Map(positioned.map(n => [n.id, n]));
    
    for (const node of positioned) {
      if (node.parent_id) {
        const parent = nodeMap.get(node.parent_id);
        if (parent && !node.isCollapsed) {
          result.push({ parent, child: node });
        }
      }
    }
    
    return result;
  }, [positioned]);

  // Calculate SVG dimensions
  const maxX = Math.max(...positioned.map(n => n.x), 0);
  const maxY = Math.max(...positioned.map(n => n.y), 0);
  const width = maxX + CONFIG.branchSpacing;
  const height = maxY + CONFIG.nodeSpacing;

  return (
    <svg
      width="100%"
      height="100%"
      viewBox={`0 0 ${width} ${height}`}
      className="tree-svg"
    >
      {/* Edges (render first, behind nodes) */}
      <g className="edges">
        {edges.map(({ parent, child }) => (
          <TreeEdge
            key={`${parent.id}-${child.id}`}
            parent={parent}
            child={child}
            isActive={parent.isActive && child.isActive}
          />
        ))}
      </g>

      {/* Nodes */}
      <g className="nodes">
        {positioned
          .filter(n => !n.isCollapsed)
          .map(node => (
            <TreeNode
              key={node.id}
              node={node}
              isSelected={node.id === selectedNodeId}
              onSelect={onNodeSelect}
            />
          ))}
      </g>
    </svg>
  );
}
```

### TreeNode Component

```typescript
// web/src/components/tree/TreeNode.tsx

import React from 'react';

const NODE_COLORS = {
  user: '#3b82f6',       // Blue-500
  assistant: '#6b7280',  // Gray-500
  system: '#fbbf24',     // Yellow-400
  tool: '#06b6d4',       // Cyan-500
};

export function TreeNode({
  node,
  isSelected,
  onSelect,
}: TreeNodeProps) {
  const color = NODE_COLORS[node.role];
  const size = node.hasCheckpoint ? 20 : 16;
  const opacity = node.isActive ? 1.0 : 0.3;

  return (
    <g
      className={`node-tree ${isSelected ? 'selected' : ''}`}
      transform={`translate(${node.x},${node.y})`}
      onClick={() => onSelect?.(node.id)}
      style={{ cursor: 'pointer' }}
    >
      {/* Main circle */}
      <circle
        className="node-circle"
        r={size}
        fill={color}
        opacity={opacity}
        stroke={isSelected ? '#fff' : 'none'}
        strokeWidth={isSelected ? 2 : 0}
      />

      {/* Checkpoint marker */}
      {node.hasCheckpoint && (
        <circle
          className="checkpoint-marker"
          r={size + 4}
          fill="none"
          stroke="#a855f7"
          strokeWidth={2}
          opacity={0.6}
        />
      )}

      {/* Loading spinner */}
      {node.status === 'loading' && (
        <circle
          className="spinner-ring"
          r={size + 6}
          fill="none"
          stroke="#3b82f6"
          strokeWidth={2}
          strokeDasharray="10 5"
          opacity={0.8}
        >
          <animateTransform
            attributeName="transform"
            type="rotate"
            from="0 0 0"
            to="360 0 0"
            dur="1s"
            repeatCount="indefinite"
          />
        </circle>
      )}

      {/* Error icon */}
      {node.status === 'error' && (
        <text
          className="error-icon"
          textAnchor="middle"
          y={5}
          fontSize="16"
        >
          ⚠️
        </text>
      )}

      {/* Tooltip */}
      <title>{node.content.slice(0, 100)}...</title>
    </g>
  );
}
```

### TreeEdge Component

```typescript
// web/src/components/tree/TreeEdge.tsx

import React from 'react';

export function TreeEdge({
  parent,
  child,
  isActive,
}: TreeEdgeProps) {
  const strokeWidth = isActive ? 2 : 1;
  const opacity = isActive ? 1.0 : 0.3;
  const color = isActive ? '#6b7280' : '#374151';

  return (
    <line
      className={isActive ? 'edge-active' : 'edge-inactive'}
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
```

## 8) Interaction Features

### Hover Tooltip
```typescript
// Show brief message preview on hover
<title>
  {node.role === 'user' 
    ? `You: ${node.content.slice(0, 100)}...`
    : `Assistant: ${node.content.slice(0, 100)}...`}
</title>
```

### Click Navigation
```typescript
function handleNodeClick(nodeId: string) {
  // Callback to parent component
  onNodeSelect?.(nodeId);
  
  // Scroll message into view
  const messageCard = document.getElementById(`message-${nodeId}`);
  if (messageCard) {
    messageCard.scrollIntoView({
      behavior: 'smooth',
      block: 'center',
    });
  }
}
```

### Selection State
```typescript
// Highlight selected node with white ring
<circle
  stroke={isSelected ? '#fff' : 'none'}
  strokeWidth={isSelected ? 2 : 0}
/>
```

## 9) Performance Optimizations

### Memoization
```typescript
const positioned = useMemo(() => {
  return layoutTree(nodes, activeLeafId, CONFIG);
}, [nodes, activeLeafId]);
```

### Virtual Rendering
For very large trees (>100 nodes), consider:
```typescript
// Only render nodes in viewport
const visibleNodes = positioned.filter(node => {
  return isInViewport(node.x, node.y, viewBox);
});
```

### Debounced Layout
```typescript
const [layoutDebounced, setLayoutDebounced] = useState(positioned);

useEffect(() => {
  const timer = setTimeout(() => {
    setLayoutDebounced(positioned);
  }, 100);
  
  return () => clearTimeout(timer);
}, [positioned]);
```

## 10) Acceptance Criteria - Core Features ✅

### Visual Rendering
- [x] Nodes render with role-specific colors
- [x] Active path edges are bold and opaque
- [x] Inactive edges are thin and dimmed
- [x] Loading nodes show animated spinner
- [x] Error nodes show warning icon

### Layout
- [x] Tree layout is hierarchical top-to-bottom
- [x] Sibling nodes are ordered by seq number
- [x] Branch spacing prevents overlaps
- [x] Root node at top center
- [x] Active path clearly visible

### Interaction
- [x] Hover shows tooltip with message preview
- [x] Click node triggers onSelect callback
- [x] Selected node has white ring highlight
- [x] Toggle inactive branches hides/shows dimmed paths
- [x] Center button resets viewport

### Integration
- [x] Tree updates when new messages arrive
- [x] Active leaf changes reflect in highlighting
- [x] Selected node syncs with message list scroll
- [x] Status changes (loading, error) update in real-time

## 11) Testing Checklist - Completed ✅

- [x] Render tree with 10 nodes (linear path)
- [x] Test hover tooltips display correctly
- [x] Test click navigation scrolls to message
- [x] Test selection highlight syncs
- [x] Test toggle inactive branches
- [x] Test center tree button
- [x] Test tree updates on new message

---

## 12) Implementation Summary (2026-01-14)

### Components Created

1. **`treeLayout.ts`** - Core layout algorithm
   - `buildChildrenMap()` - Build parent → children map
   - `getActivePath()` - Calculate active path from root to leaf
   - `layoutTree()` - DFS layout with x/y positioning
   - `collapseInactiveBranches()` - Hide deep inactive branches

2. **`TreeEdge.tsx`** - Edge rendering component
   - Renders SVG lines between parent and child nodes
   - Active vs inactive styling (stroke width, opacity)

3. **`TreeNode.tsx`** - Node rendering component
   - Role-based colors (user=blue, assistant=gray, system=yellow, tool=cyan)
   - Status indicators: loading spinner, error icon
   - Checkpoint marker (purple ring)
   - Hover tooltips

4. **`TreeVisualization.tsx`** - SVG tree renderer
   - Memoized layout calculation
   - Edge and node rendering
   - Viewport calculation

5. **`TreeControls.tsx`** - Control panel
   - Toggle inactive branches visibility
   - Center tree button

6. **`TreeNavigationPanel.tsx`** - Main panel component
   - Integrates controls + visualization
   - Handles node selection and scroll-to-message

7. **`treeHelpers.ts`** - Data conversion utilities
   - Convert `MessageData` to `TreeNode` format
   - Convert backend `Node` to `TreeNode` format

### Integration (Updated 2026-01-15)

✅ **Fully integrated into Chat page**
- TreeNavigationPanel added as left sidebar (320px width)
- SessionConfigPanel moved to right side (chat area only)
- Layout: Tree Panel | Chat Area (Session Settings + Messages + Input)
- Connected to Zustand store for selection state
- Syncs selection with message list highlighting
- Click node scrolls to message card in chat container

**Files Modified:**
- `web/src/pages/Chat.tsx` - Added tree panel to layout, moved session config
- `web/src/hooks/useChat.ts` - Added `treeNodes` and `activeLeafId` exports
- `web/src/types/backend.ts` - Added `ToolResultData` type
- `web/src/store/useChatStore.ts` - Added `expandedToolCalls` to UIState
- `web/src/components/tree/treeLayout.ts` - Fixed type errors (null → undefined)

**Type System Fixes:**
- Fixed MessageData ↔ MessageCardProps conversion (removed deprecated `toolCall`/`toolResult` fields)
- Standardized on `tool_calls` array and `tool_call_id` + `is_error` fields
- Deleted obsolete `MessageCard.old.tsx`

---

## 13) Future Enhancements

The following features depend on backend tree implementation (Phase 7 of `TREE_MESSAGE_MODEL_PLAN.md`):

### Backend-Dependent Features

1. **Branching Support**
   - Requires backend to send `parent_id` with messages
   - Need API endpoint for fetching full tree structure
   - Testing: Render tree with branching (2+ children)
   - Related: `TREE_MESSAGE_MODEL_PLAN.md` Phase 7 (JSONL storage)

2. **Checkpoint Detection**
   - Need backend to indicate which nodes have checkpoints
   - Need to fetch checkpoint data from API
   - Visual: Purple ring marker on checkpoint nodes
   - Related: `TREE_MESSAGE_MODEL_PLAN.md` Phase 3 (Checkpoint system)

3. **Parent ID Inference**
   - Currently inferring parent from message order
   - Should use actual `parent_id` from backend Node structure

### UI Improvements (Independent of Backend)

4. **Enhanced Center Tree Button**
   - Calculate actual tree bounds and center viewport
   - Add zoom/pan controls for large trees
   - Better viewport management

5. **Performance Optimization**
   - Test with 100+ nodes
   - Implement virtual rendering for very large trees
   - Optimize layout recalculation

6. **Advanced Interactions**
   - Double-click to branch from node
   - Right-click context menu (create checkpoint, pin branch)
   - Keyboard navigation (arrow keys)

---

## 14) Completion Summary

**Status:** ✅ **ACHIEVED** - Core tree visualization complete and integrated  
**Completion Date:** 2026-01-15  
**Dependencies:** Backend tree API (for branching/checkpoints - tracked in `TREE_MESSAGE_MODEL_PLAN.md`)  
**Time Spent:** ~3 hours (2 hours initial implementation + 1 hour integration)

**What Was Delivered:**
- ✅ Full tree visualization with SVG rendering
- ✅ Interactive navigation (click, hover, selection)
- ✅ Real-time updates as conversation grows
- ✅ Integration with chat UI and Zustand store
- ✅ Layout algorithm with active path highlighting
- ✅ Status indicators (loading, error)
- ✅ Responsive controls (toggle branches, center tree)

**What's Deferred:**
- Backend-dependent features moved to Future Enhancements
- These will be enabled when backend Phase 7 (JSONL storage) is complete
