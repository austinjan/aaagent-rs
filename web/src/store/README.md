# Chat Store (Zustand)

Centralized state management for the chat UI using Zustand.

## Architecture

Following the design from `doc/plan/chat-ui-state-management.md`, the store provides:

### State Structure

```typescript
ChatStore {
  // Server-authoritative data (backend owns this)
  session: SessionState
  nodes: Map<NodeId, Node>
  messages: MessageData[]
  activePath: NodeId[]
  checkpoints: Map<NodeId, CheckpointMessage>
  
  // Client-authoritative UI state (frontend owns this)
  ui: UIState {
    selectedNodeId
    expandedToolPairs
    expandedCheckpoints
    scrollPosition
    visibleRange
    loadedChunks
  }
  
  // Transient streaming state
  streaming: StreamingState {
    isStreaming
    currentMessageId
    toolPairGroups
    pendingEvents
  }
  
  // Performance metrics
  metrics: PerformanceMetrics
  
  // Error/loading state
  error
  isLoading
}
```

## Key Principles

### 1. Single Source of Truth
All state lives in one store. No local `useState` for shared state.

```typescript
// ✅ CORRECT
const selectedNodeId = useChatStore(selectSelectedNodeId);
const selectNode = useChatStore(state => state.selectNode);

// ❌ WRONG
const [selected, setSelected] = useState<string | null>(null);
```

### 2. Optimistic vs Server-Authoritative

**Optimistic actions** (instant UI update, no server call):
- `selectNode()` - Select message in UI
- `toggleToolPair()` - Expand/collapse tool groups
- `toggleCheckpoint()` - Expand/collapse checkpoint summaries
- `updateScrollPosition()` - Track scroll position

**Server-authoritative actions** (wait for SSE confirmation):
- `addMessage()` - Add new message from SSE
- `updateMessage()` - Update message content
- `addToolCalls()` - Add tool calls to streaming state
- `addToolResult()` - Add tool result
- `addCheckpoint()` - Add checkpoint from SSE

### 3. Tool Pair Grouping

Tool calls are grouped by assistant message for better UI organization:

```typescript
ToolPairGroup {
  assistantMessageId: NodeId
  pairs: ToolPair[] {
    toolCall: ToolCall
    result?: ToolResultData
    state: 'pending' | 'slow' | 'complete' | 'error' | 'orphaned'
    elapsedMs: number
  }
  isCollapsed: boolean
  completionSummary: {
    total: number
    complete: number
    pending: number
    errors: number
  }
}
```

## Important: Avoiding Infinite Loops

When using Zustand with callbacks (like SSE event handlers), avoid dependencies on store state:

```typescript
// ❌ WRONG - causes infinite loop
const handleEvent = useCallback((event) => {
  const lastMsg = messages[messages.length - 1]; // Closure reference
  // ...
}, [messages]); // Recreates on every message change

// ✅ CORRECT - stable callback
const handleEvent = useCallback((event) => {
  const store = useChatStore.getState(); // Direct access
  const messages = store.messages;
  // ...
}, []); // No dependencies
```

**Why this matters:**
1. Callback with dependencies → Recreates on state change
2. Recreated callback → Triggers effect cleanup/rerun
3. Effect rerun → Callback recreates again → Infinite loop

**Solution:** Use `useChatStore.getState()` inside callbacks to access current state without dependencies.

## Usage

### Basic Usage

```typescript
import { useChatStore, selectMessages } from '@/store';

function MyComponent() {
  // Select specific state
  const messages = useChatStore(selectMessages);
  const isLoading = useChatStore(state => state.isLoading);
  
  // Get actions
  const addMessage = useChatStore(state => state.addMessage);
  const selectNode = useChatStore(state => state.selectNode);
  
  // Use them
  const handleClick = () => {
    selectNode('node_123');
  };
}
```

### Selectors (Performance Optimization)

Use provided selectors to prevent unnecessary re-renders:

```typescript
import {
  selectSession,
  selectMessages,
  selectSelectedNodeId,
  selectIsStreaming,
  selectToolPairGroups,
  selectCheckpoints,
  selectError,
  selectIsLoading,
} from '@/store';

// Only re-renders when messages change
const messages = useChatStore(selectMessages);
```

### DevTools

Zustand DevTools are enabled in development. Open Redux DevTools extension to:
- Inspect state
- Track actions
- Time-travel debug

## Migration from useState

Before (scattered state):
```typescript
const [selectedMessageId, setSelectedMessageId] = useState<string>();
const [messages, setMessages] = useState<MessageData[]>([]);
const [isLoading, setIsLoading] = useState(false);
```

After (centralized):
```typescript
const selectedNodeId = useChatStore(selectSelectedNodeId);
const messages = useChatStore(selectMessages);
const isLoading = useChatStore(selectIsLoading);

const selectNode = useChatStore(state => state.selectNode);
const addMessage = useChatStore(state => state.addMessage);
```

## Future Enhancements

From the original plan, these features are ready to implement:

1. **Event Queue** - Explicit ordering guarantees with UI yielding
2. **State Validation** - Dev-mode invariant checking
3. **Performance Metrics** - Track `renderTime`, `memoryUsage`, `fps`
4. **Virtual Scrolling** - `visibleRange` and `loadedChunks` are already in state
5. **Mini Map** - Tree visualization using `activePath` and `selectedNodeId`

## Files

- `useChatStore.ts` - Main store implementation
- `index.ts` - Exports and selectors
- `README.md` - This file
