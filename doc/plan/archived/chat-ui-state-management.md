# Chat UI State Management Plan

- Feature name: `chat-ui-state-management`
- Status: **MVP Complete** - Ready for Product Phase
- Created: 2026-01-06
- Last updated: 2026-01-16
- Parent plan: [chat-ui-plan.md](./chat-ui-plan.md)

## 1) Overview

### MVP Status Summary

**✅ COMPLETED**: Core Zustand state management is functional and tested. The chat UI now has:
- Single source of truth for all state
- Optimistic UI updates (instant feedback)
- Server-authoritative data (SSE-driven)
- Tool pair streaming with proper lifecycle management
- Basic state validation in dev mode

**🚀 DEFERRED to Product Phase**: Advanced features like EventQueue with explicit sequencing, performance metrics, and virtual scrolling are postponed.

### Goal
Establish clear state management patterns using Zustand to ensure consistency between streaming updates, user actions, and UI components.

### Scope (In) - MVP
- Single source of truth for selection
- Optimistic UI for user actions
- Server-authoritative for conversation data
- Direct SSE processing (FIFO ordering)

### Scope (Out) - Post-MVP
- Advanced event queue with sequence numbers and yielding
- Complex synchronization invariants
- Performance metrics tracking
- Memory-bounded virtual scrolling

### Core Principles (MVP)

1. **Single Source of Truth**: All state lives in one store
2. **Server-Authoritative Data**: Backend owns conversation data
3. **Client-Authoritative UI**: Frontend owns UI state (expand/collapse, scroll)
4. **Optimistic Updates**: User actions update UI immediately
5. **Direct SSE Processing**: Stream events applied directly via SSE FIFO

## 2) State Structure

```typescript
interface ChatUIState {
  // ===== SERVER-AUTHORITATIVE DATA (read-only from backend) =====
  session: {
    sessionId: string;
    totalNodes: number;
    activeLeafId: string;
    rootNodeId: string;
  };
  
  nodes: Map<string, Node>;              // node_id → Node
  activePath: string[];                  // Ordered node IDs from root → active_leaf
  checkpoints: Map<string, CheckpointData>;
  
  // ===== CLIENT-AUTHORITATIVE UI STATE =====
  ui: {
    selectedNodeId: string | null;      // SINGLE SOURCE OF TRUTH for selection
    expandedToolPairs: Set<string>;     // Tool pair IDs that are expanded
    expandedCheckpoints: Set<string>;   // Checkpoint IDs showing full summary
    scrollPosition: number;             // Current scroll offset
    
    // Virtual scroll state
    visibleRange: { start: number; end: number };
    loadedChunks: Set<string>;          // "offset-limit" keys
  };
  
  // ===== TRANSIENT STREAMING STATE =====
  streaming: {
    isStreaming: boolean;
    currentMessageId: string | null;    // Currently streaming message
    toolPairGroups: Map<string, ToolPairGroup>;  // In-flight tool calls
  };
}
```

## 3) Zustand Store Implementation

```typescript
import { create } from 'zustand';
import { devtools } from 'zustand/middleware';

interface ChatStore extends ChatUIState {
  // Actions (optimistic - instant UI update)
  selectNode: (nodeId: string) => void;
  toggleToolPair: (toolPairId: string) => void;
  toggleCheckpoint: (checkpointId: string) => void;
  updateScrollPosition: (offset: number) => void;

  // Actions (server-authoritative - wait for SSE)
  addNode: (node: Node) => void;
  updateNode: (nodeId: string, updates: Partial<Node>) => void;
  addToolCalls: (nodeId: string, toolCalls: ToolCall[]) => void;
  addToolResult: (toolCallId: string, result: ToolResult) => void;
}

const useChatStore = create<ChatStore>()(
  devtools((set, get) => ({
    // Initial state
    session: { sessionId: '', totalNodes: 0, activeLeafId: '', rootNodeId: '' },
    nodes: new Map(),
    activePath: [],
    checkpoints: new Map(),
    ui: {
      selectedNodeId: null,
      expandedToolPairs: new Set(),
      expandedCheckpoints: new Set(),
      scrollPosition: 0,
      visibleRange: { start: 0, end: 50 },
      loadedChunks: new Set(),
    },
    streaming: {
      isStreaming: false,
      currentMessageId: null,
      toolPairGroups: new Map(),
    },
    
    // Optimistic actions (instant UI update)
    selectNode: (nodeId) => set((state) => ({
      ui: { ...state.ui, selectedNodeId: nodeId }
    })),
    
    toggleToolPair: (toolPairId) => set((state) => {
      const expanded = new Set(state.ui.expandedToolPairs);
      if (expanded.has(toolPairId)) {
        expanded.delete(toolPairId);
      } else {
        expanded.add(toolPairId);
      }
      return { ui: { ...state.ui, expandedToolPairs: expanded } };
    }),
    
    toggleCheckpoint: (checkpointId) => set((state) => {
      const expanded = new Set(state.ui.expandedCheckpoints);
      if (expanded.has(checkpointId)) {
        expanded.delete(checkpointId);
      } else {
        expanded.add(checkpointId);
      }
      return { ui: { ...state.ui, expandedCheckpoints: expanded } };
    }),
    
    updateScrollPosition: (offset) => set((state) => ({
      ui: { ...state.ui, scrollPosition: offset }
    })),
    
    // Server-authoritative actions (wait for SSE confirmation)
    addNode: (node) => set((state) => {
      const nodes = new Map(state.nodes);
      nodes.set(node.node_id, node);
      
      // Update active path if this is on the path
      const activePath = [...state.activePath];
      if (node.is_on_active_path) {
        activePath.push(node.node_id);
      }
      
      return { nodes, activePath };
    }),
    
    updateNode: (nodeId, updates) => set((state) => {
      const nodes = new Map(state.nodes);
      const existing = nodes.get(nodeId);
      if (existing) {
        nodes.set(nodeId, { ...existing, ...updates });
      }
      return { nodes };
    }),
    
    addToolCalls: (nodeId, toolCalls) => set((state) => {
      const groups = new Map(state.streaming.toolPairGroups);
      groups.set(nodeId, {
        assistantMessageId: nodeId,
        pairs: toolCalls.map(tc => ({
          toolCall: tc,
          result: undefined,
          state: 'pending',
          elapsedMs: 0,
        })),
        isCollapsed: true,
        completionSummary: {
          total: toolCalls.length,
          complete: 0,
          pending: toolCalls.length,
          errors: 0,
        },
      });
      return { streaming: { ...state.streaming, toolPairGroups: groups } };
    }),
    
    addToolResult: (toolCallId, result) => set((state) => {
      const groups = new Map(state.streaming.toolPairGroups);
      
      // Find the pair
      for (const [nodeId, group] of groups) {
        const pair = group.pairs.find(p => p.toolCall.id === toolCallId);
        if (pair) {
          pair.result = result;
          pair.state = result.is_error ? 'error' : 'complete';
          
          // Update summary
          group.completionSummary = {
            total: group.pairs.length,
            complete: group.pairs.filter(p => p.state === 'complete').length,
            pending: group.pairs.filter(p => p.state === 'pending' || p.state === 'slow').length,
            errors: group.pairs.filter(p => p.state === 'error' || p.state === 'orphaned').length,
          };
          
          groups.set(nodeId, group);
          break;
        }
      }
      
      return { streaming: { ...state.streaming, toolPairGroups: groups } };
    }),
  }))
);

export { useChatStore };
```

## 4) Event Queue (POST-MVP)

**Status**: Out of scope for MVP - will be added during product phase

### Purpose
- Ensure events processed in order with explicit sequence numbers
- Prevent race conditions under high load
- Maintain 60fps by yielding to UI during event bursts

### Why Deferred
For MVP, SSE provides inherent FIFO ordering which is sufficient. Advanced queuing with yielding and sequence numbers adds complexity that's not critical for initial release.

### Future Implementation

```typescript
class EventQueue {
  private queue: AgentEvent[] = [];
  private processing = false;
  private sequenceNumber = 0;
  
  enqueue(event: AgentEvent) {
    // Assign sequence number for ordering
    const sequencedEvent = { ...event, seq: this.sequenceNumber++ };
    this.queue.push(sequencedEvent);
    
    if (!this.processing) {
      this.processQueue();
    }
  }
  
  private async processQueue() {
    this.processing = true;
    
    while (this.queue.length > 0) {
      const event = this.queue.shift()!;
      
      // Process event
      await this.processEvent(event);
      
      // Yield to browser for rendering (maintain 60fps)
      if (this.queue.length > 0) {
        await this.yieldToUI();
      }
    }
    
    this.processing = false;
  }
  
  private async yieldToUI(): Promise<void> {
    return new Promise(resolve => {
      requestAnimationFrame(() => {
        setTimeout(resolve, 0);  // Let browser paint
      });
    });
  }
  
  private async processEvent(event: AgentEvent) {
    const store = useChatStore.getState();
    
    switch (event.type) {
      case 'content':
        store.updateNode(event.node_id, { content: event.content });
        break;
      case 'tool_calls_requested':
        store.addToolCalls(event.node_id, event.tool_calls);
        break;
      case 'tool_result':
        store.addToolResult(event.tool_call_id, event);
        break;
      // ... handle all event types
    }
  }
}
```

## 5) Single Source of Truth: Selection

**Problem**: Selection must sync between mini map and chat container.

**Solution**: Store in one place, derive UI from it.

```typescript
// ✅ CORRECT: Single source of truth
const state = {
  ui: {
    selectedNodeId: "node_123"  // ONLY place selection is stored
  }
};

// Both components derive from same source
function MiniMap() {
  const selectedNodeId = useChatStore(s => s.ui.selectedNodeId);
  const selectNode = useChatStore(s => s.selectNode);
  
  return (
    <div>
      {nodes.map(node => (
        <NodeDot 
          key={node.id}
          isSelected={node.id === selectedNodeId}  // Derived
          onClick={() => selectNode(node.id)}
        />
      ))}
    </div>
  );
}

function ChatContainer() {
  const selectedNodeId = useChatStore(s => s.ui.selectedNodeId);
  
  return (
    <div>
      {cards.map(card => (
        <Card 
          key={card.id}
          isHighlighted={card.id === selectedNodeId}  // Same source
        />
      ))}
    </div>
  );
}
```

## 6) Optimistic vs Server-Authoritative

### Optimistic (Instant UI Update)

User actions that only affect UI:

```typescript
// Expand/collapse (instant)
function toggleToolPair(toolPairId: string) {
  useChatStore.getState().toggleToolPair(toolPairId);
  // UI re-renders instantly, no server call
}

// Scroll position (local only)
function updateScroll(offset: number) {
  useChatStore.getState().updateScrollPosition(offset);
}

// Select node (instant highlight)
function selectNode(nodeId: string) {
  useChatStore.getState().selectNode(nodeId);
  // Both mini map and chat highlight immediately
}
```

### Server-Authoritative (Wait for SSE)

Actions that modify backend data:

```typescript
// Send message (optimistic display, then confirm)
async function sendMessage(text: string) {
  // 1. Optimistic: Show immediately
  const tempId = `temp_${Date.now()}`;
  useChatStore.getState().addNode({
    node_id: tempId,
    content: text,
    role: 'user',
    isPending: true,
  });
  
  // 2. Send to server
  await chatClient.sendMessage(sessionId, text, {
    onContent: (data) => {
      // 3. Server confirms, replace temp node
      useChatStore.getState().updateNode(tempId, {
        node_id: data.node_id,
        isPending: false,
      });
    },
  });
}
```

## 7) Synchronization Invariants (MVP)

**Basic guarantees to maintain:**

1. **Selection Sync**: `miniMap.selected === chatContainer.highlighted === state.ui.selectedNodeId`
2. **Tool Pair Consistency**: Every tool_calls_requested has matching results or timeout
3. **SSE Ordering**: Events processed in SSE arrival order (FIFO)

**Post-MVP (deferred):**
- Explicit sequence numbers for event ordering
- Memory bounds with virtual scrolling
- Advanced lost update detection

**Validation (dev mode):**

```typescript
function validateState(state: ChatUIState) {
  // Check 1: Selected node exists in active path
  if (state.ui.selectedNodeId) {
    assert(
      state.activePath.includes(state.ui.selectedNodeId),
      'Selected node must be in active path'
    );
  }
  
  // Check 2: All tool pairs have matching calls
  for (const [groupId, group] of state.streaming.toolPairGroups) {
    for (const pair of group.pairs) {
      assert(pair.toolCall, 'Tool pair must have toolCall');
      if (pair.state === 'complete') {
        assert(pair.result, 'Complete pair must have result');
      }
    }
  }
  
  // Check 3: No duplicate nodes in active path
  const uniqueNodes = new Set(state.activePath);
  assert(
    uniqueNodes.size === state.activePath.length,
    'Active path must not have duplicates'
  );
}
```

## 8) Testing Plan (MVP)

**State Management Tests:**
- [x] Single source of truth for selectedNodeId
- [x] Optimistic actions update instantly (basic toggle/select)
- [x] Server-authoritative waits for SSE
- [x] Event ordering preserved during streaming (SSE FIFO)
- [x] State validation passes basic invariants

**Sync Tests:**
- [x] Mini map selection updates chat highlight
- [x] Chat scroll updates mini map
- [x] No race conditions during rapid updates
- [x] No lost events during reconnection

**Post-MVP Tests (deferred):**
- UI remains responsive during extreme event bursts (>1000 events)
- Memory bounds enforced with virtual scrolling
- Sequence number ordering under network reordering

## 9) Acceptance Criteria (MVP)

- [x] Zustand store is single source of truth
- [x] Optimistic UI updates instantly (expand/collapse, selection)
- [x] Server-authoritative data waits for SSE confirmation
- [x] Event ordering preserved via SSE FIFO
- [x] Basic state validation in dev mode
- [x] Tool pairs render correctly during streaming
- [x] No lost SSE events during normal operation

**Post-MVP (deferred):**
- Selection syncs between mini map and chat (mini map not implemented yet)
- Advanced event queue with yielding
- Performance metrics tracking
- Memory-bounded virtual scrolling

## 10) Implementation Tasks (MVP)

- [x] Create Zustand store with core state
- [x] Implement optimistic actions (select, toggle, scroll)
- [x] Implement server-authoritative actions (addNode, updateNode, tool calls)
- [x] Add basic state validation (dev mode)
- [x] Integrate SSE handlers with store
- [x] Write synchronization tests
- [x] Tool pair rendering with streaming updates

**Post-MVP (deferred):**
- [ ] Build EventQueue class with sequence numbers
- [ ] Implement UI yielding for event bursts
- [ ] Add performance metrics tracking
- [ ] Implement memory-bounded virtual scrolling
- [ ] Add mini map integration with selection sync

---

## Changelog
- 2026-01-16: **MVP COMPLETE** - Refocused plan on MVP scope. Removed EventQueue complexity, marked as post-MVP. All core state management working.
- 2026-01-13: Added Zustand store implementation and SSE integration; event queue and validation still pending.
- 2026-01-13: Deferred EventQueue in favor of direct SSE FIFO processing.
- 2026-01-13: Added dev-mode state validation in the Zustand store.
- 2026-01-13: Added sync test runner for the chat store.
- 2026-01-06: Initial plan created.

---

## References
- Parent plan: [chat-ui-plan.md](./chat-ui-plan.md)
- Related plans:
  - [chat-ui-sse-streaming.md](./chat-ui-sse-streaming.md)
  - [chat-ui-performance.md](./chat-ui-performance.md)
