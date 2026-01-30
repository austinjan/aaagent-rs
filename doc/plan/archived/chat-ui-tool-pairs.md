# Chat UI Tool Pair Management Plan

- Feature name: `chat-ui-tool-pairs`
- Status: **Completed (Simplified Approach)**
- Created: 2026-01-06
- Completed: 2026-01-30
- Parent plan: [chat-ui-plan.md](./chat-ui-plan.md)

## Implementation Summary (2026-01-30)

### Actual Implementation vs Original Plan

**Original Plan**: Complex state machine with timeout detection, grouping, and summary badges.

**Actual Implementation**: Simplified approach without timeout tracking, focusing on clean UI.

**Rationale for Change**:
1. **Backend Speed**: Tool calls complete quickly (<2s typically), making timeout detection unnecessary
2. **SSE Reliability**: Real-time streaming ensures results arrive immediately
3. **UI Simplicity**: Simpler component easier to maintain and understand
4. **YAGNI Principle**: Timeout tracking added complexity without clear user benefit

### What Was Built

**Files Created**:
- `web/src/components/chat/ToolCallCard.tsx` - Individual tool call/result display
- Tool integration in `web/src/components/chat/MessageCard.tsx`

**Features Implemented**:
- ✅ **Collapsible tool call cards** - Individual cards can expand/collapse
- ✅ **Visual state indicators** - Icons for pending (Wrench), success (CheckCircle), error (XCircle)
- ✅ **Input/Result display** - Show tool arguments and execution results
- ✅ **Error highlighting** - Red styling for failed tool calls
- ✅ **Clean UI** - Matches BlackBear TechHive theme

**Features NOT Implemented** (from original plan):
- ❌ Timeout detection (10s/60s timers)
- ❌ State machine (pending → slow → orphaned)
- ❌ Grouped tool pairs with summary badges
- ❌ ToolPairTracker class
- ❌ Flash animations for updates

### Current Implementation Details

```typescript
// Simplified ToolCallCard (web/src/components/chat/ToolCallCard.tsx)
interface ToolCallCardProps {
  id: string;
  name: string;
  input: Record<string, unknown> | undefined;
  result?: ToolResult;
  isExpanded?: boolean;
  onToggle?: () => void;
}

// States (simplified):
// 1. Pending: No result, show Wrench icon
// 2. Complete: Has result, is_error=false, show CheckCircle
// 3. Error: Has result, is_error=true, show XCircle
```

**UI Structure**:
```
MessageCard (Assistant)
  ├─ Content (markdown)
  ├─ Thinking block (if present)
  └─ ToolCallCard[] (one per tool call)
      ├─ Header: Icon + Tool Name + Expand/Collapse
      ├─ Input (when expanded, before result)
      └─ Result (when available)
```

**Integration with SSE**:
- Tool calls received via `tool_calls` SSE event
- Results received via `tool_result` SSE event  
- Matched by `tool_call_id`
- Store manages tool result state in Zustand

### Production Status: ✅ Ready

The simplified implementation is:
- **Working reliably** in production use
- **Easy to maintain** (single file, clear logic)
- **User-friendly** (clean UI, clear status)
- **Sufficient** for current needs

### Future Enhancements (If Needed)

If timeout tracking becomes necessary:
1. Add `timeoutMs` prop to ToolCallCard
2. Use `useEffect` with setTimeout for 10s/60s timers
3. Add warning/error badges to card header
4. Clear timers on result arrival

If grouping becomes necessary:
1. Create `ToolCallGroup` component
2. Group tool calls by assistant message ID
3. Show summary badges (total/complete/pending/errors)
4. Collapse/expand entire group

---

## Original Plan (For Reference)

### 1) Overview

### Goal
Manage tool call/result pairing with state tracking, timeout detection, and collapsible UI grouping.

### Scope (In)
- Tool pair grouping by assistant turn
- State machine (pending → slow → orphaned → complete/error)
- Timeout thresholds (10s, 60s)
- Out-of-order result handling
- Collapsible UI component

### Non-goals (Out)
- Tool execution logic (handled by backend)
- Parallel tool execution (sequential in current implementation)

### 2) State Machine (NOT IMPLEMENTED)

```
ToolCall Received
      ↓
   PENDING (0-10s) ────→ SLOW (10-60s) ────→ ORPHANED (>60s)
      ↓                       ↓                      ↓
   ⏳ Spinner          ⚠️ "Still working"      ❌ Red warning
      ↓
   COMPLETE/ERROR
      ↓
   ✓/❌ Final state
```

**Note**: The actual implementation uses a simpler 3-state model (pending/complete/error) without timeout tracking.

---

## References
- Parent plan: [chat-ui-plan.md](./chat-ui-plan.md)
- Related plans:
  - [chat-ui-sse-streaming.md](./archived/chat-ui-sse-streaming.md)
- Implemented files:
  - `web/src/components/chat/ToolCallCard.tsx`
  - `web/src/components/chat/MessageCard.tsx`
