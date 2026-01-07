# Chat UI Tool Pair Management Plan

- Feature name: `chat-ui-tool-pairs`
- Status: Draft
- Created: 2026-01-06
- Parent plan: [chat-ui-plan.md](./chat-ui-plan.md)

## 1) Overview

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

## 2) State Machine

### States

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

**State Definitions:**
- **Pending (0-10s)**: Normal operation, show spinner ⏳
- **Slow (10-60s)**: Show "Still working..." with warning icon ⚠️
- **Orphaned (>60s)**: No result received, mark as failed with red warning ❌
- **Complete**: Tool executed successfully, green checkmark ✓
- **Error**: Tool returned `is_error: true`, red X with error message ❌

### TypeScript Types

```typescript
type ToolCallState = 
  | 'pending'     // 0-10s, normal
  | 'slow'        // 10-60s, warning
  | 'orphaned'    // >60s, error
  | 'complete'    // Result received, is_error=false
  | 'error';      // Result received, is_error=true

interface ToolPair {
  toolCall: ToolCall;                // From tool_calls_requested event
  result?: ToolResult;               // From tool_result event (undefined = pending)
  state: ToolCallState;
  elapsedMs: number;                 // Time since tool_calls_requested
}

interface ToolPairGroup {
  assistantMessageId: string;        // Parent assistant message node_id
  pairs: ToolPair[];                 // All tool call/result pairs
  isCollapsed: boolean;              // UI collapse state
  completionSummary: {               // For collapsed header
    total: number;
    complete: number;
    pending: number;
    errors: number;
  };
}
```

## 3) Timeout Management

### Timer Implementation

```typescript
class ToolPairTracker {
  private timers = new Map<string, { slow: NodeJS.Timeout, orphan: NodeJS.Timeout }>();
  
  onToolCallRequested(toolCall: ToolCall) {
    // Set 10s timer for "slow" warning
    const slowTimer = setTimeout(() => 
      this.updateState(toolCall.id, 'slow'), 10000);
    
    // Set 60s timer for "orphaned" error
    const orphanTimer = setTimeout(() => 
      this.updateState(toolCall.id, 'orphaned'), 60000);
    
    this.timers.set(toolCall.id, { slow: slowTimer, orphan: orphanTimer });
  }
  
  onToolResult(result: ToolResult) {
    // Clear timers when result arrives
    const timers = this.timers.get(result.tool_call_id);
    if (timers) {
      clearTimeout(timers.slow);
      clearTimeout(timers.orphan);
      this.timers.delete(result.tool_call_id);
    }
    
    // Update state
    this.updateState(
      result.tool_call_id,
      result.is_error ? 'error' : 'complete'
    );
  }
  
  private updateState(toolCallId: string, state: ToolCallState) {
    const store = useChatStore.getState();
    store.updateToolPairState(toolCallId, state);
  }
}
```

### Rationale for Thresholds

- **10 seconds (slow)**: Most tool calls complete in <5s. 10s is reasonable warning threshold.
- **60 seconds (orphaned)**: Very slow tools (file processing, API calls) take 30-60s. Beyond 60s likely stuck.

## 4) Grouping Logic

### Multiple Tool Calls in Single Turn

**Backend behavior** (from `src/agent/mod.rs:250-280`):
- Tool calls executed **sequentially** (not parallel)
- All tool calls from one assistant message grouped together

**Frontend grouping:**
```typescript
// Group by assistant message ID
function groupToolPairs(toolCalls: ToolCall[], results: ToolResult[]): ToolPairGroup[] {
  const groups = new Map<string, ToolPairGroup>();
  
  for (const toolCall of toolCalls) {
    const group = groups.get(toolCall.assistant_message_id) || {
      assistantMessageId: toolCall.assistant_message_id,
      pairs: [],
      isCollapsed: true,  // Default collapsed
      completionSummary: { total: 0, complete: 0, pending: 0, errors: 0 },
    };
    
    const result = results.find(r => r.tool_call_id === toolCall.id);
    const state = result 
      ? (result.is_error ? 'error' : 'complete')
      : 'pending';
    
    group.pairs.push({
      toolCall,
      result,
      state,
      elapsedMs: Date.now() - toolCall.timestamp,
    });
    
    groups.set(toolCall.assistant_message_id, group);
  }
  
  // Calculate summaries
  for (const group of groups.values()) {
    group.completionSummary = {
      total: group.pairs.length,
      complete: group.pairs.filter(p => p.state === 'complete').length,
      pending: group.pairs.filter(p => p.state === 'pending' || p.state === 'slow').length,
      errors: group.pairs.filter(p => p.state === 'error' || p.state === 'orphaned').length,
    };
  }
  
  return Array.from(groups.values());
}
```

## 5) Out-of-Order Results

### Current Behavior
Backend executes sequentially, so results arrive in order.

### Future-Proof Design
If parallel execution is added:

```typescript
// Match by tool_call_id, NOT arrival order
function handleToolResult(result: ToolResult) {
  const pair = findPairById(result.tool_call_id);
  
  if (!pair) {
    console.error(`Unexpected result for ${result.tool_call_id}`);
    return; // Ignore orphaned result
  }
  
  if (pair.result) {
    console.error(`Duplicate result for ${result.tool_call_id}`);
    return; // Ignore duplicate
  }
  
  // Update pair
  pair.result = result;
  pair.state = result.is_error ? 'error' : 'complete';
  
  // Flash animation to highlight new result
  flashToolPair(pair.toolCall.id);
}
```

## 6) UI Component

### ToolPairCard Component

```typescript
import { useState } from 'react';

interface ToolPairCardProps {
  group: ToolPairGroup;
  onToggle: () => void;
}

function ToolPairCard({ group, onToggle }: ToolPairCardProps) {
  const { pairs, isCollapsed, completionSummary } = group;
  const { total, complete, pending, errors } = completionSummary;
  
  return (
    <div className="card bg-base-200 shadow-sm">
      {/* Header (always visible) */}
      <div 
        className="card-body p-3 cursor-pointer hover:bg-base-300"
        onClick={onToggle}
      >
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <span className="text-lg">{isCollapsed ? '▶' : '▼'}</span>
            <h4 className="font-semibold">
              Tool Calls ({total})
            </h4>
          </div>
          
          {/* Summary badges */}
          <div className="flex gap-2">
            {complete > 0 && (
              <span className="badge badge-success gap-1">
                ✓ {complete}
              </span>
            )}
            {pending > 0 && (
              <span className="badge badge-warning gap-1">
                ⏳ {pending}
              </span>
            )}
            {errors > 0 && (
              <span className="badge badge-error gap-1">
                ❌ {errors}
              </span>
            )}
          </div>
        </div>
      </div>
      
      {/* Expanded view */}
      {!isCollapsed && (
        <div className="card-body pt-0 space-y-2">
          {pairs.map((pair, i) => (
            <ToolPairItem key={pair.toolCall.id} pair={pair} index={i} />
          ))}
        </div>
      )}
    </div>
  );
}

function ToolPairItem({ pair, index }: { pair: ToolPair; index: number }) {
  const icon = getToolCallIcon(pair.state);
  const iconColor = getIconColor(pair.state);
  
  return (
    <div className="flex items-start gap-3 p-2 rounded bg-base-100">
      {/* Icon */}
      <span className={`text-xl ${iconColor}`}>{icon}</span>
      
      {/* Content */}
      <div className="flex-1">
        {/* Tool call */}
        <div className="font-mono text-sm">
          <span className="font-semibold">{pair.toolCall.name}</span>
          <span className="text-base-content/60">
            ({pair.toolCall.id})
          </span>
        </div>
        
        {/* Arguments */}
        <pre className="text-xs mt-1 text-base-content/80 overflow-x-auto">
          {JSON.stringify(pair.toolCall.arguments, null, 2)}
        </pre>
        
        {/* Result (if available) */}
        {pair.result && (
          <div className="mt-2 p-2 rounded bg-base-200">
            <div className="text-xs font-semibold text-base-content/60">Result:</div>
            <div className="text-sm mt-1 font-mono">
              {pair.result.result}
            </div>
          </div>
        )}
        
        {/* Warning for slow/orphaned */}
        {pair.state === 'slow' && (
          <div className="alert alert-warning mt-2 py-2">
            <span className="text-sm">Still working... ({Math.floor(pair.elapsedMs / 1000)}s)</span>
          </div>
        )}
        
        {pair.state === 'orphaned' && (
          <div className="alert alert-error mt-2 py-2">
            <span className="text-sm">Tool call timed out after 60s</span>
          </div>
        )}
      </div>
    </div>
  );
}

function getToolCallIcon(state: ToolCallState): string {
  switch(state) {
    case 'pending': return '⏳';
    case 'slow': return '⚠️';
    case 'orphaned': return '❌';
    case 'complete': return '✓';
    case 'error': return '❌';
  }
}

function getIconColor(state: ToolCallState): string {
  switch(state) {
    case 'pending': return 'text-info';
    case 'slow': return 'text-warning';
    case 'orphaned': return 'text-error';
    case 'complete': return 'text-success';
    case 'error': return 'text-error';
  }
}
```

## 7) Testing Plan

**State Machine Tests:**
- [ ] Pending state (0-10s) shows spinner
- [ ] Slow state (10-60s) shows warning
- [ ] Orphaned state (>60s) shows error
- [ ] Complete state shows checkmark
- [ ] Error state shows red X

**Timer Tests:**
- [ ] 10s timer triggers slow state
- [ ] 60s timer triggers orphaned state
- [ ] Timers cleared on result arrival
- [ ] Multiple tool calls have independent timers

**Grouping Tests:**
- [ ] Tool calls from same message grouped
- [ ] Completion summary calculated correctly
- [ ] Out-of-order results matched by ID
- [ ] Duplicate results ignored

**UI Tests:**
- [ ] Collapsed view shows summary
- [ ] Expanded view shows all pairs
- [ ] Click toggles collapse state
- [ ] Flash animation on result update

## 8) Acceptance Criteria

- [ ] Tool pairs default to collapsed
- [ ] Clicking header toggles expansion
- [ ] Pending tools show spinner (0-10s)
- [ ] Slow tools show warning (10-60s)
- [ ] Orphaned tools show error (>60s)
- [ ] Completed tools show checkmark
- [ ] Error tools show red X
- [ ] Summary badges show counts
- [ ] Results update correct pair by tool_call_id
- [ ] Timers cleared on result arrival

## 9) Implementation Tasks

- [ ] Create `ToolPairTracker` class with timers
- [ ] Implement state machine transitions
- [ ] Add tool pair grouping logic
- [ ] Build `ToolPairCard` component
- [ ] Build `ToolPairItem` component
- [ ] Integrate with SSE event handlers
- [ ] Add flash animation for updates
- [ ] Write unit tests for state machine
- [ ] Write integration tests for grouping

---

## References
- Parent plan: [chat-ui-plan.md](./chat-ui-plan.md)
- Related plans:
  - [chat-ui-sse-streaming.md](./chat-ui-sse-streaming.md)
  - [chat-ui-state-management.md](./chat-ui-state-management.md)
- Spec: `src/agent/mod.rs:250-280` (sequential tool execution)
