# Frontend Sub-Agent UI Implementation Plan

**Status**: Ready to Start  
**Backend Status**: ✅ 100% Complete (SSE Broadcast ready)  
**Frontend Status**: 🟡 15% Complete (Event parsing done)  
**Estimated Time**: 2-3 days

---

## Overview

Build UI components to visualize sub-agent activity in real-time using the SSE broadcast infrastructure that's already in place.

### What Users Will See:

1. **Status Indicator** - Small icon in header showing active sub-agents count
2. **Detail Side Panel** - Expandable panel with full sub-agent details and history
3. **Inline Status Cards** - Status cards inserted into chat flow (started/completed)
4. **Message Badges** - Visual indicators showing which messages came from sub-agents
5. **Tool Timeline** (Optional) - Visual representation of tool execution flow

---

## Current State

### ✅ Already Implemented (Backend):

```
Agent → GlobalEventBus → broadcast::channel → SSE Handler → Frontend
```

- ✅ Events broadcast to multiple clients
- ✅ Session-based filtering
- ✅ Sequence numbering
- ✅ Timestamp tracking
- ✅ All 11 event types supported

### ✅ Already Implemented (Frontend):

**Event Type Definitions** (`web/src/types/backend.ts`):
```typescript
type AgentEventType = 
  | 'content'
  | 'thinking'
  | 'tool_calls_requested'
  | 'tool_result'
  | 'done'
  | 'queued_messages'      // ✅ Sub-agent queue
  | 'followup_processed'   // ✅ Sub-agent followup
  // ... etc
```

**SSE Hook** (`web/src/hooks/useSSEStream.ts`):
```typescript
// ✅ Already parsing these events:
case 'queued_messages':
  console.log('[Queue] Processing N queued message(s)');
  break;
  
case 'followup_processed':
  console.log('[Queue] Processing followup 2/5 from SubAgent(run-abc)');
  break;
```

**What's Missing**: UI components to display these events visually!

---

## Implementation Plan

### Phase 1: State Management (Day 1 Morning)

#### 1.1 Create Sub-Agent Store

**File**: `web/src/stores/subAgentStore.ts`

**Purpose**: Centralized state for tracking sub-agent activity

```typescript
import { create } from 'zustand';

interface ToolCallInfo {
  name: string;
  status: 'pending' | 'running' | 'completed' | 'error';
  startTime?: number;
  endTime?: number;
  result?: string;
}

interface SubAgentInfo {
  runId: string;
  taskLabel: string;
  startTime: number;
  endTime?: number;
  phase: 'running' | 'completed' | 'error';
  toolCalls: ToolCallInfo[];
  errors: string[];
  source: string; // "SubAgent(run-123)" or "System"
}

interface SubAgentStore {
  // State
  activeRuns: Map<string, SubAgentInfo>;
  completedRuns: Map<string, SubAgentInfo>;
  
  // Actions
  addRun: (runId: string, info: Partial<SubAgentInfo>) => void;
  updateRun: (runId: string, updates: Partial<SubAgentInfo>) => void;
  completeRun: (runId: string, success: boolean) => void;
  pruneOldRuns: () => void; // Remove runs older than 60s
}

export const useSubAgentStore = create<SubAgentStore>((set) => ({
  activeRuns: new Map(),
  completedRuns: new Map(),
  
  addRun: (runId, info) => set((state) => {
    const newRuns = new Map(state.activeRuns);
    newRuns.set(runId, {
      runId,
      taskLabel: info.taskLabel || 'Unknown Task',
      startTime: Date.now(),
      phase: 'running',
      toolCalls: [],
      errors: [],
      source: info.source || 'SubAgent',
      ...info,
    });
    return { activeRuns: newRuns };
  }),
  
  updateRun: (runId, updates) => set((state) => {
    const run = state.activeRuns.get(runId);
    if (!run) return state;
    
    const newRuns = new Map(state.activeRuns);
    newRuns.set(runId, { ...run, ...updates });
    return { activeRuns: newRuns };
  }),
  
  completeRun: (runId, success) => set((state) => {
    const run = state.activeRuns.get(runId);
    if (!run) return state;
    
    const newActive = new Map(state.activeRuns);
    const newCompleted = new Map(state.completedRuns);
    
    newActive.delete(runId);
    newCompleted.set(runId, {
      ...run,
      endTime: Date.now(),
      phase: success ? 'completed' : 'error',
    });
    
    return { 
      activeRuns: newActive, 
      completedRuns: newCompleted 
    };
  }),
  
  pruneOldRuns: () => set((state) => {
    const now = Date.now();
    const cutoff = 60 * 1000; // 60 seconds
    
    const newCompleted = new Map(state.completedRuns);
    for (const [runId, run] of newCompleted.entries()) {
      if (run.endTime && now - run.endTime > cutoff) {
        newCompleted.delete(runId);
      }
    }
    
    return { completedRuns: newCompleted };
  }),
}));
```

**Tests**: `web/src/stores/__tests__/subAgentStore.test.ts`

#### 1.2 Connect Store to SSE Events

**File**: `web/src/hooks/useChat.ts` (enhance existing)

```typescript
import { useSubAgentStore } from '../stores/subAgentStore';

export function useChat(sessionId: string) {
  const { addRun, updateRun, completeRun } = useSubAgentStore();
  
  // Inside onEvent handler:
  const handleEvent = (event: SSEEvent) => {
    switch (event.type) {
      case 'queued_messages':
        // Extract run_id from event data if available
        // For now, we might not have run_id yet
        console.log('[Queue] Processing', event.data.count, 'messages');
        break;
        
      case 'followup_processed':
        const { message_index, total_queued, source } = event.data;
        // Parse source: "SubAgent(run-abc123)"
        const match = source.match(/SubAgent\((.+)\)/);
        if (match) {
          const runId = match[1];
          // Update or create run info
          if (!useSubAgentStore.getState().activeRuns.has(runId)) {
            addRun(runId, {
              taskLabel: 'Processing queued message',
              source: source,
            });
          }
        }
        break;
        
      // TODO: Handle other events when backend sends them
    }
  };
}
```

**Estimate**: 3-4 hours

---

### Phase 2: Status Indicator & Detail Panel (Day 1 Afternoon)

#### 2.1 Status Indicator Component

**File**: `web/src/components/layout/SubAgentIndicator.tsx`

```typescript
import React, { useState } from 'react';
import { useSubAgentStore } from '../../stores/subAgentStore';
import { SubAgentDetailPanel } from '../agent/SubAgentDetailPanel';

export function SubAgentIndicator() {
  const { activeRuns } = useSubAgentStore();
  const [isPanelOpen, setIsPanelOpen] = useState(false);
  
  // Hide indicator when no active runs
  if (activeRuns.size === 0) return null;
  
  return (
    <>
      {/* Small indicator button in header */}
      <button 
        className="btn btn-ghost btn-circle relative"
        onClick={() => setIsPanelOpen(true)}
        title={`${activeRuns.size} sub-agent${activeRuns.size > 1 ? 's' : ''} running`}
      >
        🤖
        
        {/* Badge showing count */}
        <span className="absolute top-0 right-0 badge badge-primary badge-sm">
          {activeRuns.size}
        </span>
        
        {/* Spinning indicator */}
        <span className="loading loading-spinner loading-xs absolute bottom-1 right-1"></span>
      </button>
      
      {/* Detail panel (opens on click) */}
      {isPanelOpen && (
        <SubAgentDetailPanel onClose={() => setIsPanelOpen(false)} />
      )}
    </>
  );
}
```

#### 2.2 Detail Side Panel

**File**: `web/src/components/agent/SubAgentDetailPanel.tsx`

```typescript
import React from 'react';
import { useSubAgentStore } from '../../stores/subAgentStore';

interface SubAgentDetailPanelProps {
  onClose: () => void;
}

export function SubAgentDetailPanel({ onClose }: SubAgentDetailPanelProps) {
  const { activeRuns, completedRuns } = useSubAgentStore();
  
  return (
    <div className="fixed inset-0 z-50">
      {/* Backdrop */}
      <div 
        className="absolute inset-0 bg-black/20" 
        onClick={onClose}
      ></div>
      
      {/* Side Panel */}
      <div className="absolute right-0 top-0 bottom-0 w-96 bg-base-100 shadow-2xl overflow-y-auto animate-slide-in-right">
        <div className="p-4">
          {/* Header */}
          <div className="flex items-center justify-between mb-4">
            <h2 className="text-xl font-bold">🤖 Sub-Agents</h2>
            <button 
              className="btn btn-ghost btn-sm btn-circle" 
              onClick={onClose}
            >
              ✕
            </button>
          </div>
          
          {/* Active Runs Section */}
          {activeRuns.size > 0 && (
            <div className="mb-6">
              <h3 className="text-sm font-semibold mb-3 text-blue-600 flex items-center gap-2">
                <span className="loading loading-spinner loading-xs"></span>
                Running ({activeRuns.size})
              </h3>
              {Array.from(activeRuns.values()).map(run => (
                <SubAgentDetailCard key={run.runId} run={run} status="running" />
              ))}
            </div>
          )}
          
          {/* Completed Runs Section */}
          {completedRuns.size > 0 && (
            <div>
              <h3 className="text-sm font-semibold mb-3 text-green-600">
                ✅ Completed ({completedRuns.size})
              </h3>
              {Array.from(completedRuns.values()).map(run => (
                <SubAgentDetailCard key={run.runId} run={run} status="completed" />
              ))}
            </div>
          )}
          
          {/* Empty State */}
          {activeRuns.size === 0 && completedRuns.size === 0 && (
            <div className="text-center text-gray-500 py-8">
              No sub-agents yet
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

// Detail card for each sub-agent
function SubAgentDetailCard({ run, status }: { run: SubAgentInfo; status: string }) {
  const elapsed = Math.floor((Date.now() - run.startTime) / 1000);
  const duration = run.endTime 
    ? Math.floor((run.endTime - run.startTime) / 1000) 
    : null;
  
  return (
    <div className="card bg-base-200 shadow-sm mb-3">
      <div className="card-body p-3 text-sm">
        <div className="font-semibold">{run.taskLabel}</div>
        
        {status === 'running' && (
          <div className="text-xs text-gray-600">
            Running for {elapsed}s
          </div>
        )}
        
        {status === 'completed' && duration && (
          <div className="text-xs text-gray-600">
            Completed in {duration}s
          </div>
        )}
        
        {/* Tool calls */}
        {run.toolCalls.length > 0 && (
          <details className="mt-2">
            <summary className="text-xs cursor-pointer text-blue-600">
              {run.toolCalls.length} tool{run.toolCalls.length > 1 ? 's' : ''}
            </summary>
            <ul className="text-xs ml-4 mt-1 space-y-1">
              {run.toolCalls.map((tool, idx) => (
                <li key={idx} className="flex items-center gap-1">
                  <span>{tool.status === 'completed' ? '✓' : '⋯'}</span>
                  <span>{tool.name}</span>
                </li>
              ))}
            </ul>
          </details>
        )}
        
        {/* Errors */}
        {run.errors.length > 0 && (
          <div className="alert alert-error alert-sm mt-2">
            {run.errors[0]}
          </div>
        )}
      </div>
    </div>
  );
}
```

**Integration**: Indicator in header, panel opens on click

**Estimate**: 3-4 hours

---

### Phase 3: Inline Status Cards (Day 2 Morning)

#### 3.1 Inline Status Card Component

**File**: `web/src/components/chat/SubAgentStatusCard.tsx`

```typescript
import React, { useState, useEffect } from 'react';
import { useSubAgentStore } from '../../stores/subAgentStore';

interface SubAgentStatusCardProps {
  runId: string;
  status: 'running' | 'completed' | 'error';
}

export function SubAgentStatusCard({ runId, status }: SubAgentStatusCardProps) {
  const run = useSubAgentStore(state => 
    state.activeRuns.get(runId) || state.completedRuns.get(runId)
  );
  
  const [elapsed, setElapsed] = useState(0);
  
  useEffect(() => {
    if (status === 'running') {
      const interval = setInterval(() => {
        setElapsed(Math.floor((Date.now() - (run?.startTime || 0)) / 1000));
      }, 1000);
      return () => clearInterval(interval);
    }
  }, [status, run]);
  
  if (!run) return null;
  
  const colors = {
    running: 'bg-blue-50 border-blue-300 text-blue-900',
    completed: 'bg-green-50 border-green-300 text-green-900',
    error: 'bg-red-50 border-red-300 text-red-900',
  };
  
  const icons = {
    running: '🚀',
    completed: '✅',
    error: '❌',
  };
  
  const duration = run.endTime 
    ? Math.floor((run.endTime - run.startTime) / 1000) 
    : null;
  
  return (
    <div className={`card ${colors[status]} border-2 my-4 mx-auto max-w-2xl`}>
      <div className="card-body p-4">
        <div className="flex items-center gap-3">
          <span className="text-3xl">{icons[status]}</span>
          <div className="flex-1">
            <h4 className="font-bold">
              Sub-Agent {status === 'running' ? 'Running' : status === 'completed' ? 'Completed' : 'Failed'}
            </h4>
            <p className="text-sm opacity-90">{run.taskLabel}</p>
            
            {status === 'running' && (
              <div className="flex items-center gap-2 mt-2">
                <span className="loading loading-spinner loading-xs"></span>
                <span className="text-xs opacity-75">{elapsed}s elapsed</span>
              </div>
            )}
            
            {status === 'completed' && duration && (
              <div className="text-xs opacity-75 mt-1">
                Duration: {duration}s
              </div>
            )}
          </div>
        </div>
        
        {/* Tool calls list (collapsible) */}
        {run.toolCalls.length > 0 && (
          <details className="mt-2">
            <summary className="text-xs cursor-pointer opacity-75">
              {run.toolCalls.length} tool{run.toolCalls.length > 1 ? 's' : ''} executed
            </summary>
            <ul className="text-xs ml-4 mt-1 space-y-1">
              {run.toolCalls.map((tool, idx) => (
                <li key={idx}>• {tool.name}</li>
              ))}
            </ul>
          </details>
        )}
      </div>
    </div>
  );
}
```

**Integration**: Insert into chat message flow when sub-agent starts/completes

**Estimate**: 2-3 hours

---

### Phase 4: Message Badges (Day 2 Afternoon)

#### 4.1 Enhance Message Component

**File**: `web/src/components/chat/MessageBubble.tsx` (enhance existing)

```typescript
interface MessageBubbleProps {
  // ... existing props ...
  sourceRunId?: string; // NEW: Identify sub-agent messages
}

export function MessageBubble({ content, role, sourceRunId, ...props }: MessageBubbleProps) {
  const { activeRuns, completedRuns } = useSubAgentStore();
  
  // Check if this message is from a sub-agent
  const isSubAgentMessage = sourceRunId && sourceRunId !== 'main-agent';
  
  // Get task label from store
  const subAgentInfo = 
    activeRuns.get(sourceRunId || '') || 
    completedRuns.get(sourceRunId || '');
  
  return (
    <div className={`chat ${role === 'user' ? 'chat-end' : 'chat-start'}`}>
      <div className="chat-bubble">
        {/* Sub-Agent Badge */}
        {isSubAgentMessage && (
          <div className="badge badge-warning mb-2" title={subAgentInfo?.taskLabel}>
            🤖 Sub-Agent Result
          </div>
        )}
        
        {/* Message Content */}
        <div>{content}</div>
      </div>
    </div>
  );
}
```

#### 4.2 Connect to Message Data

**Challenge**: Need to track which messages came from sub-agents

**Solution**: Modify message storage to include `sourceRunId`

**File**: `web/src/hooks/useChat.ts`

```typescript
// When processing followup_processed event:
case 'followup_processed':
  const { source } = event.data;
  const match = source.match(/SubAgent\((.+)\)/);
  const sourceRunId = match ? match[1] : undefined;
  
  // Store sourceRunId with the message
  // (Implementation depends on current message storage structure)
  break;
```

**Estimate**: 2-3 hours

---

### Phase 5: Testing & Polish (Day 3)

#### 5.1 Unit Tests

**Files to Test**:
1. `subAgentStore.test.ts` - Store logic
2. `SubAgentNotificationToast.test.tsx` - Toast component
3. `ActiveSubAgentPanel.test.tsx` - Panel component
4. `MessageBubble.test.tsx` - Badge rendering

**Testing Approach**:
```typescript
import { renderHook, act } from '@testing-library/react-hooks';
import { useSubAgentStore } from '../subAgentStore';

describe('SubAgentStore', () => {
  it('should add new run', () => {
    const { result } = renderHook(() => useSubAgentStore());
    
    act(() => {
      result.current.addRun('run-123', {
        taskLabel: 'Test Task',
        source: 'SubAgent(run-123)',
      });
    });
    
    expect(result.current.activeRuns.size).toBe(1);
    expect(result.current.activeRuns.get('run-123')?.taskLabel).toBe('Test Task');
  });
  
  // ... more tests
});
```

#### 5.2 Integration Testing

**Manual Test Scenarios**:
1. Spawn sub-agent → See toast notification
2. Sub-agent runs → See in active panel
3. Sub-agent completes → Toast + panel update
4. Sub-agent message → See badge on message bubble
5. Multiple sub-agents → All tracked separately

#### 5.3 Styling Polish

**Custom Styles** (`web/src/index.css`):
```css
/* Sub-agent badge */
.badge-subagent {
  @apply bg-yellow-500 text-black font-semibold;
}

/* Active sub-agent panel animation */
.subagent-panel-enter {
  animation: slideInRight 0.3s ease-out;
}

@keyframes slideInRight {
  from {
    transform: translateX(100%);
    opacity: 0;
  }
  to {
    transform: translateX(0);
    opacity: 1;
  }
}

/* Toast stack spacing */
.toast > * + * {
  margin-top: 0.5rem;
}
```

**Estimate**: 4-5 hours

---

## Optional: Tool Timeline (Priority 2)

**File**: `web/src/components/agent/ToolExecutionTimeline.tsx`

**Concept**: Horizontal timeline showing tool execution sequence

```
[ShellTool: rg "pattern"] ───▶ [ReadTool: file.rs] ───▶ [Done]
   2.3s                          0.8s                     
```

**Implementation**: Can be added later if requested

**Estimate**: 4-6 hours

---

## Summary

### Total Estimate: **2-3 days**

**Day 1**:
- Morning: State management (3-4h)
- Afternoon: Toast notifications (2-3h)

**Day 2**:
- Morning: Active sub-agent panel (3-4h)
- Afternoon: Message badges (2-3h)

**Day 3**:
- Testing & polish (4-5h)

### Key Files to Create:

1. `web/src/stores/subAgentStore.ts` - State management
2. `web/src/components/layout/SubAgentIndicator.tsx` - Header indicator
3. `web/src/components/agent/SubAgentDetailPanel.tsx` - Detail side panel
4. `web/src/components/chat/SubAgentStatusCard.tsx` - Inline status cards
5. `web/src/components/chat/MessageBubble.tsx` - Enhance existing

### Key Files to Modify:

1. `web/src/hooks/useChat.ts` - Connect events to store, insert status cards
2. `web/src/App.tsx` or `web/src/components/layout/Header.tsx` - Add SubAgentIndicator
3. `web/src/index.css` - Custom styling and animations

### Dependencies:

- ✅ `zustand` - Already in package.json
- ✅ `daisyUI` - Already configured
- ✅ React 18 - Already using
- ✅ TypeScript - Already configured

**No new dependencies needed!**

---

## Next Steps

1. **Review this plan** - Confirm approach and priorities
2. **Start Day 1** - Implement state management
3. **Iterate** - Test each component as it's built
4. **Deploy** - Ship when Day 3 complete

Ready to start? 🚀
