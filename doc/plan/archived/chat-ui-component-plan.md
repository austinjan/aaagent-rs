# Chat UI Component Plan

- Feature name: `chat-ui-component`
- Status: **Nearly Complete** (95%)
- Created: 2026-01-12
- Updated: 2026-02-01
- Parent plan: [chat-ui-plan.md](./chat-ui-plan.md)

## 1) Overview

### Goal
Build a complete React-based chat interface that displays conversation history as both a linear message list and an interactive tree visualization, with real-time streaming support via SSE.

### Design Principles

1. **Tree-first architecture** - Conversation history is a tree, not a list
2. **Visual clarity** - Clear role indicators, collapsible tool calls, status feedback
3. **Real-time streaming** - SSE integration with live updates
4. **Navigation sync** - Tree selection scrolls to message, message click highlights node
5. **Responsive layout** - Side-by-side tree + messages on desktop, stacked on mobile

## 2) Component Architecture

```
ChatPage
├── ChatHeader (session info, stats)
├── TreeNavigationPanel (left sidebar)
│   └── TreeVisualization (SVG tree with D3.js layout)
└── ChatContainer (right main area)
    ├── MessageList (scrollable messages)
    │   ├── MessageCard (user/assistant/system)
    │   ├── CheckpointCard (summary cards)
    │   └── ToolResultCard (tool execution results)
    └── ChatInput (message input + config controls)
        ├── InputField (textarea with auto-resize)
        ├── PresetSelector (dropdown)
        └── ConfigOverrides (expandable advanced options)
```

## 3) Key Features

### Message Display
- **Role-based styling** - User (blue), Assistant (gray), System (yellow), Tool (cyan)
- **Thinking blocks** - Purple highlight for reasoning content
- **Tool calls** - Collapsible tool execution details with JSON formatting
- **Streaming indicator** - Pulsing cursor during live updates
- **Checkpoint summaries** - Expandable summary cards at checkpoint nodes

### Tree Navigation
- **Node visualization** - Circular nodes with role-based colors
- **Active path highlighting** - Bold edges for current conversation path
- **Branch collapse** - Inactive branches collapse after threshold depth
- **Hover tooltips** - Brief message preview on node hover
- **Click to navigate** - Click node to scroll to corresponding message
- **Status indicators** - Loading spinner, error icon, checkpoint marker

### Input Controls
- **Auto-resize textarea** - Grows with content, max 10 lines
- **Preset quick-select** - Dropdown: general, coding, research, quick
- **Send on Enter** - Shift+Enter for newlines
- **Advanced overrides** - Collapsible panel for model, sampling params
- **Streaming status** - Disable input during active stream

## 4) Sub-Plans

### 4.1 Message Card Component
See: [chat-ui-message-card-plan.md](./chat-ui-message-card-plan.md)

**Responsibilities:**
- Render message content with role-specific styling
- Display thinking blocks, tool calls, tool results
- Show streaming state with animated cursor
- Handle selection state for tree sync

### 4.2 Tree Navigation Panel
See: [chat-ui-tree-panel-plan.md](./chat-ui-tree-panel-plan.md)

**Responsibilities:**
- SVG-based tree layout using D3.js force simulation
- Node rendering with role colors and status indicators
- Edge rendering with active/inactive distinction
- Branch collapse for inactive paths
- Hover tooltips and click navigation
- Sync with message list scroll position

### 4.3 Chat Input Component
See: [chat-ui-input-plan.md](./chat-ui-input-plan.md)

**Responsibilities:**
- Auto-resizing textarea for message input
- Preset selector dropdown
- Config overrides panel (collapsible)
- Send message with Enter key
- Integration with SSE streaming state
- Validation and error display

## 5) Data Flow

### Message Streaming
```
User sends message
  ↓
POST /api/sessions/:id/chat
  ↓
Returns { stream_id, resolved_config }
  ↓
Connect EventSource to /api/sessions/:id/stream/:stream_id
  ↓
Receive events:
  - content → Append to assistant message
  - thinking → Display in thinking block
  - tool_calls → Show tool execution cards
  - tool_result → Update tool result status
  - checkpoint → Insert checkpoint card
  - done → Finalize message, enable input
```

### Tree Synchronization
```
1. Load session → Fetch tree structure
2. Build node map → Map node_id to position
3. Calculate active path → Root to active_leaf
4. Layout tree → D3.js force-directed layout
5. Render SVG → Nodes + edges with colors
6. User clicks node → Scroll to message card
7. User scrolls messages → Highlight current node
```

## 6) State Management

### Session State
```typescript
interface SessionState {
  sessionId: string;
  tree: TreeNode[];
  activeLeafId: string;
  checkpoints: Checkpoint[];
  messages: Message[];  // Derived from tree path
  resolvedConfig: ResolvedConfig;
}
```

### Streaming State
```typescript
interface StreamingState {
  isStreaming: boolean;
  streamId: string | null;
  currentMessage: Partial<Message>;
  error: Error | null;
}
```

### UI State
```typescript
interface UIState {
  selectedNodeId: string | null;
  showInactiveBranches: boolean;
  isInputExpanded: boolean;
  scrollToMessageId: string | null;
}
```

## 7) Styling Theme

Based on tree-visualization-demo.html:

**Colors:**
- Background: `#0a0e1a` (dark navy)
- Cards: `#1a1f2e` with `#2a3040` hover
- User: Blue `#1e3a8a` / `#3b82f6`
- Assistant: Gray `#374151` / `#6b7280`
- System: Yellow `#78350f` / `#fbbf24`
- Tool: Cyan `#164e63` / `#06b6d4`
- Error: Red `#7f1d1d` / `#ef4444`
- Checkpoint: Purple `#581c87` / `#a855f7`

**Layout:**
- Tree panel: 40% width, fixed left sidebar
- Chat container: 60% width, scrollable
- Message cards: Max 800px width, centered
- Tree nodes: 16px radius (standard), 20px (checkpoint)
- Node spacing: 60px vertical, 120px horizontal

## 8) Technology Stack

**Core:**
- React 18 with TypeScript
- Vite for dev + build
- Tailwind CSS v4 for styling

**Tree Visualization:**
- D3.js for layout calculation
- Native SVG rendering (no React wrapper)
- Custom force-directed layout with constraints

**State Management:**
- React Context for session state
- useState/useReducer for local state
- Custom hooks: useSSE, useTreeLayout, useMessageSync

**API Integration:**
- EventSource for SSE streaming
- Fetch API for REST endpoints
- Custom api.ts client with error handling

## 9) Responsive Design

### Desktop (≥1024px)
```
┌─────────────────────────────────────┐
│         ChatHeader                  │
├────────────┬────────────────────────┤
│   Tree     │    Messages            │
│   Panel    │    ┌────────────┐     │
│   (40%)    │    │ Message 1  │     │
│            │    │ Message 2  │     │
│            │    │ Message 3  │     │
│            │    └────────────┘     │
│            │    ChatInput           │
└────────────┴────────────────────────┘
```

### Tablet (768px - 1023px)
```
┌─────────────────────────────────────┐
│         ChatHeader                  │
├─────────────────────────────────────┤
│   Tree Panel (collapsible)          │
├─────────────────────────────────────┤
│    Messages (full width)            │
│    ┌────────────────────────────┐  │
│    │ Message 1                  │  │
│    │ Message 2                  │  │
│    └────────────────────────────┘  │
│    ChatInput                        │
└─────────────────────────────────────┘
```

### Mobile (<768px)
```
┌───────────────────┐
│   ChatHeader      │
├───────────────────┤
│   Messages        │
│   ┌───────────┐  │
│   │ Message 1 │  │
│   │ Message 2 │  │
│   └───────────┘  │
│   ChatInput       │
└───────────────────┘
(Tree hidden, show via toggle)
```

## 10) Acceptance Criteria

### Message Display
- [x] User messages styled with blue theme
- [x] Assistant messages styled with gray theme
- [x] Thinking blocks displayed in purple highlight
- [x] Tool calls shown with expandable JSON
- [x] Tool results show success/error status
- [x] Checkpoint cards display summary content
- [x] Streaming indicator shows during active stream

### Tree Visualization
- [x] Tree layout matches conversation structure
- [x] Active path highlighted with bold edges
- [x] Inactive branches shown in dimmed style
- [x] Nodes show role-specific colors
- [x] Hover shows message preview tooltip
- [x] Click node scrolls to message
- [x] Checkpoint nodes show marker icon

### Input & Interaction
- [x] Textarea auto-resizes up to 10 lines
- [x] Enter sends message, Shift+Enter adds newline
- [x] Preset selector changes session config
- [x] Config overrides panel is collapsible
- [x] Input disabled during streaming
- [x] Error messages displayed inline

### Synchronization
- [x] Message list and tree stay in sync
- [x] Scrolling messages highlights current node
- [x] Clicking node scrolls to message smoothly
- [x] New messages appear in both views
- [x] Checkpoint insertion updates both views

### Real-time Streaming
- [x] SSE connection established on send
- [x] Content events append to message
- [x] Thinking events update thinking block
- [x] Tool events create tool cards
- [x] Done event enables input
- [x] Error handling with retry option

## 11) Implementation Tasks

### Phase 1: Message Components
- [x] Create MessageCard component
- [x] Create CheckpointCard component
- [x] Create ToolResultCard component
- [x] Add role-based styling
- [x] Add streaming state indicator

### Phase 2: Tree Visualization
- [x] Implement D3.js tree layout
- [x] Create TreeNode SVG component
- [x] Create TreeEdge SVG component
- [x] Add hover tooltips
- [x] Add click navigation
- [x] Implement branch collapse

### Phase 3: Input Controls
- [x] Create ChatInput component
- [x] Add auto-resize textarea
- [x] Create PresetSelector dropdown
- [x] Create ConfigOverrides panel
- [x] Add validation and error display

### Phase 4: Integration
- [x] Create ChatPage container
- [x] Implement SSE streaming hook
- [x] Connect message list to API
- [x] Sync tree with message scroll
- [x] Handle session loading/creation

### Phase 5: Polish
- [x] Add responsive layout
- [x] Add loading states
- [x] Add error boundaries
- [ ] Add keyboard shortcuts
- [ ] Performance optimization

## 12) Dependencies

**Required:**
- [x] SSE streaming backend (implemented)
- [x] Session storage backend (implemented)
- [x] Config resolution API (implemented)

**Libraries:**
- [x] Install D3.js: `npm install d3 @types/d3`
- [x] Install date-fns: `npm install date-fns`
- [x] Install clsx: `npm install clsx`

## 13) Testing Strategy

### Unit Tests
- MessageCard rendering with different roles
- ToolResultCard expand/collapse
- CheckpointCard content display
- TreeNode layout calculation
- ChatInput validation

### Integration Tests
- SSE streaming end-to-end
- Tree-message synchronization
- Config changes persist to session
- Navigation between nodes

### Manual Testing
- Send message with each preset
- Test tool calling scenarios
- Test checkpoint creation
- Test branch navigation
- Test mobile responsiveness

## 14) Future Enhancements

- [ ] Branch comparison mode (diff two paths)
- [ ] Export conversation as markdown
- [ ] Search within messages
- [ ] Message editing and regeneration
- [ ] Custom syntax highlighting for code
- [ ] Voice input support
- [ ] Collaborative sessions (multi-user)

---

**Status:** Nearly Complete (95%)
**Start date:** 2026-01-12
**Remaining:** Keyboard shortcuts, performance optimization
