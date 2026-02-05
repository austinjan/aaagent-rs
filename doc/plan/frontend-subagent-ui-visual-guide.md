# Frontend Sub-Agent UI Visual Guide

## Component Layout Overview

```
┌─────────────────────────────────────────────────────────────────────────┐
│ Chat Header                                                              │
│ ┌─────────────────────────────────────┐  ┌──────────────────────────┐  │
│ │ Chat                                 │  │ [🤖 2 ⚙️] [Metrics] [ID]  │  │
│ │ Conversational AI with tree-history  │  │  ↑                       │  │
│ └─────────────────────────────────────┘  │ SubAgentIndicator        │  │
│                                           └──────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## 1. SubAgentIndicator (Header)

**States**:

### No Active Sub-Agents
```
┌──────────────┐
│ [🤖]         │  ← Gray robot icon
└──────────────┘
```

### 1 Active Sub-Agent
```
┌──────────────────┐
│ [🤖] [1] ⚙️     │  ← Yellow icon + badge + spinner
└──────────────────┘
```

### Multiple Active Sub-Agents
```
┌──────────────────┐
│ [🤖] [3] ⚙️     │  ← Yellow icon + badge "3" + spinner
└──────────────────┘
```

### Panel Open (Active State)
```
┌──────────────────┐
│ [🤖] [2] ⚙️     │  ← Highlighted background
└──────────────────┘
```

---

## 2. SubAgentDetailPanel (Side Panel)

### Panel Closed
(Not visible)

### Panel Open - List View
```
┌────────────────────────────────────────┐
│ Sub-Agent Activity              [×]    │
├────────────────────────────────────────┤
│                                        │
│ ┌────────────────────────────────────┐ │
│ │ Search: find API endpoints         │ │
│ │ 5s elapsed                         │ │
│ │ [🚀 spawning]                      │ │
│ │ ⚙️ 0 tool calls                    │ │
│ └────────────────────────────────────┘ │
│                                        │
│ ┌────────────────────────────────────┐ │
│ │ Search: user authentication        │ │
│ │ 12s elapsed                        │ │
│ │ [⚙️ running]                       │ │
│ │ ⚙️ 3 tool calls                    │ │
│ └────────────────────────────────────┘ │
│                                        │
│ ┌────────────────────────────────────┐ │
│ │ Analyze: database schema           │ │
│ │ 8s ago                             │ │
│ │ [✅ completed]                     │ │
│ │ ⚙️ 5 tool calls                    │ │
│ └────────────────────────────────────┘ │
│                                        │
│ ┌────────────────────────────────────┐ │
│ │ Refactor: component structure      │ │
│ │ 15s ago                            │ │
│ │ [❌ error]                         │ │
│ │ ⚙️ 2 tool calls                    │ │
│ │ ⚠️ 1 error                          │ │
│ └────────────────────────────────────┘ │
│                                        │
└────────────────────────────────────────┘
```

### Panel Open - Detail View
```
┌────────────────────────────────────────┐
│ [← Back]                        [×]    │
├────────────────────────────────────────┤
│                                        │
│ Search: find API endpoints             │
│ Duration: 12s                          │
│ Started: 14:32:15                      │
│ Ended: 14:32:27                        │
│                                        │
│ Tool Calls                             │
│ ┌────────────────────────────────────┐ │
│ │ shell_tool             success ✓   │ │
│ │ 14:32:16                           │ │
│ │ > fd "api" src/ --type f           │ │
│ └────────────────────────────────────┘ │
│                                        │
│ ┌────────────────────────────────────┐ │
│ │ read_tool              success ✓   │ │
│ │ 14:32:18                           │ │
│ │ > Read src/api/routes.ts           │ │
│ └────────────────────────────────────┘ │
│                                        │
│ ┌────────────────────────────────────┐ │
│ │ grep_tool              success ✓   │ │
│ │ 14:32:20                           │ │
│ │ > rg "async fn" src/api/           │ │
│ └────────────────────────────────────┘ │
│                                        │
│ Progress                               │
│ [████████████░░░░░░░] 3 / 4            │
│                                        │
└────────────────────────────────────────┘
```

---

## 3. SubAgentStatusCard (Inline in Chat)

### Spawning Phase
```
┌────────────────────────────────────────────────────────┐
│ 🚀 Sub-Agent: Spawning                    [5s]        │
│ Search: find API endpoints                             │
│                                                        │
│ ⚙️ In progress...                                     │
└────────────────────────────────────────────────────────┘
│ Blue left border (info)
```

### Running Phase (with Progress)
```
┌────────────────────────────────────────────────────────┐
│ ⚙️ Sub-Agent: Running                     [12s]       │
│ Search: find API endpoints                             │
│                                                        │
│ Progress                            [████░░] 2 / 3     │
│                                                        │
│ ▼ Tool Calls (2)                                      │
│   ⏳ shell_tool           14:32:16                    │
│   ✓ read_tool            14:32:18                    │
│                                                        │
│ [View Details →]                                      │
└────────────────────────────────────────────────────────┘
│ Yellow left border (warning)
```

### Completed Phase
```
┌────────────────────────────────────────────────────────┐
│ ✅ Sub-Agent: Completed                   [8s]        │
│ Search: find API endpoints                             │
│                                                        │
│ ▼ Tool Calls (3)                                      │
│   ✓ shell_tool           14:32:16                    │
│   ✓ read_tool            14:32:18                    │
│   ✓ grep_tool            14:32:20                    │
│                                                        │
│ [View Details →]                                      │
└────────────────────────────────────────────────────────┘
│ Green left border (success)
```

### Error Phase
```
┌────────────────────────────────────────────────────────┐
│ ❌ Sub-Agent: Failed                      [5s]        │
│ Search: find API endpoints                             │
│                                                        │
│ ▼ Tool Calls (1)                                      │
│   ✗ shell_tool           14:32:16                    │
│                                                        │
│ ⚠️ Permission denied: cannot access /restricted        │
│                                                        │
│ [View Details →]                                      │
└────────────────────────────────────────────────────────┘
│ Red left border (error)
```

---

## 4. MessageCard with Sub-Agent Badge

### Regular User Message
```
┌────────────────────────────────────────────────────────┐
│ YOU                                      14:32:05      │
│                                                        │
│ Find all API endpoints in the codebase                 │
└────────────────────────────────────────────────────────┘
```

### Regular Assistant Message
```
┌────────────────────────────────────────────────────────┐
│ ASSISTANT                                14:32:08      │
│                                                        │
│ I'll search for API endpoints using a sub-agent.       │
└────────────────────────────────────────────────────────┘
```

### Sub-Agent Message (with Badge)
```
┌────────────────────────────────────────────────────────┐
│ ASSISTANT [🤖 Sub-Agent]                 14:32:28      │
│                                                        │
│ Found 12 API endpoints in the following files:         │
│ - src/api/routes.ts (8 endpoints)                     │
│ - src/api/auth.ts (2 endpoints)                       │
│ - src/api/webhooks.ts (2 endpoints)                   │
└────────────────────────────────────────────────────────┘
```

### Sub-Agent Tool Result (with Badge)
```
┌────────────────────────────────────────────────────────┐
│ TOOL RESULT [🤖 Search Agent]           14:32:20      │
│ shell_tool                                             │
│                                                        │
│ src/api/routes.ts                                      │
│ src/api/auth.ts                                        │
│ src/api/webhooks.ts                                    │
└────────────────────────────────────────────────────────┘
```

---

## Full Page Layout

```
┌─────────────────────────────────────────────────────────────────────────┐
│ HEADER: Chat | [🤖 2 ⚙️] [Metrics] [Session ID]                        │
├─────────┬───────────────────────────────────────────────┬───────────────┤
│ SESSION │ CHAT AREA                                     │ DETAIL PANEL  │
│ LIST    │                                               │ (when open)   │
│         │ ┌───────────────────────────────────────────┐ │               │
│ • Chat1 │ │ YOU: Find API endpoints        14:32:05  │ │ Sub-Agent     │
│ • Chat2 │ └───────────────────────────────────────────┘ │ Activity [×]  │
│ • Chat3 │                                               │               │
│         │ ┌───────────────────────────────────────────┐ │ ┌───────────┐ │
│ [+ New] │ │ ASSISTANT: I'll use sub-agent  14:32:08  │ │ │ Search... │ │
│         │ └───────────────────────────────────────────┘ │ │ 12s       │ │
│         │                                               │ │ [running] │ │
│         │ ┌─────────────────────────────────────────┐   │ └───────────┘ │
│         │ │ 🚀 Sub-Agent: Running           [12s] │   │               │
│         │ │ Search: find API endpoints           │   │ ┌───────────┐ │
│         │ │ Progress [████░] 2/3                  │   │ │ Analyze.. │ │
│         │ │ ▼ Tool Calls (2)                     │   │ │ 5s        │ │
│         │ │   ⏳ shell_tool                      │   │ │ [done]    │ │
│         │ │   ✓ read_tool                        │   │ └───────────┘ │
│         │ └─────────────────────────────────────────┘   │               │
│         │                                               │               │
│         │ ┌───────────────────────────────────────────┐ │               │
│         │ │ ASSISTANT [🤖]: Found 12 endpoints       │ │               │
│         │ │ - src/api/routes.ts                      │ │               │
│         │ └───────────────────────────────────────────┘ │               │
│         │                                               │               │
│         ├───────────────────────────────────────────────┤               │
│         │ INPUT: [Type message...] [🔍] [Send]        │               │
│         └───────────────────────────────────────────────┘               │
└─────────┴───────────────────────────────────────────────┴───────────────┘
```

---

## Color Scheme (BlackBear TechHive Theme)

### Primary Colors
- **Yellow/Gold**: `#E8C236` (active state, badges, indicators)
- **Black**: `#000000` (background, text on badges)
- **Base Colors**: DaisyUI theme defaults

### Phase Colors
- **Spawning**: Blue (`border-info`, `bg-info/10`)
- **Running**: Yellow (`border-warning`, `bg-warning/10`)
- **Completed**: Green (`border-success`, `bg-success/10`)
- **Error**: Red (`border-error`, `bg-error/10`)

### Status Icons
- **Spawning**: 🚀 (rocket)
- **Running**: ⚙️ (gear)
- **Completed**: ✅ (check mark)
- **Error**: ❌ (cross mark)
- **Robot**: 🤖 (robot face)

---

## Interaction Flows

### 1. User Triggers Sub-Agent

```
User types message
    ↓
Assistant decides to spawn sub-agent
    ↓
Backend emits: subagent_spawned
    ↓
Frontend: SubAgentIndicator badge appears (count +1)
    ↓
Frontend: Store adds new run (phase: spawning)
    ↓
(Optional) Frontend: Inline status card rendered
```

### 2. Sub-Agent Executes Tools

```
Sub-agent calls tool
    ↓
Backend emits: tool_calls event (with run_id)
    ↓
Frontend: Store adds tool call (status: running)
    ↓
Frontend: Detail panel updates (if open)
    ↓
Backend emits: tool_result event
    ↓
Frontend: Store updates tool call (status: success/error)
    ↓
Frontend: Detail panel updates
```

### 3. Sub-Agent Completes

```
Sub-agent finishes
    ↓
Backend emits: done event (with run_id)
    ↓
Frontend: Store moves run to completed
    ↓
Frontend: SubAgentIndicator badge decrements
    ↓
Frontend: Detail panel updates (if open)
    ↓
(Optional) Frontend: Inline completion card rendered
    ↓
After 5 minutes: Store prunes old completed run
```

### 4. User Views Details

```
User clicks SubAgentIndicator
    ↓
Detail panel opens (list view)
    ↓
User clicks a run
    ↓
Detail panel switches to detail view
    ↓
User sees: timeline, tool calls, errors
    ↓
User clicks "Back"
    ↓
Detail panel returns to list view
    ↓
User clicks [×] or clicks indicator again
    ↓
Detail panel closes
```

---

## Responsive Behavior

### Desktop (>1024px)
- Detail panel: 384px wide (w-96)
- Full feature set

### Tablet (768px - 1024px)
- Detail panel: 320px wide
- Reduced padding

### Mobile (<768px)
- Detail panel: Full screen overlay
- Bottom sheet style (slide up from bottom)
- Swipe down to close

---

## Accessibility

### Keyboard Navigation
- `Tab`: Focus indicator button
- `Enter/Space`: Toggle panel
- `Escape`: Close panel
- `Arrow keys`: Navigate list items
- `Enter`: Select run for detail view

### ARIA Labels
- `role="region"` on panel
- `aria-label="Sub-agent activity"` on indicator
- `aria-expanded` state on indicator
- `aria-live="polite"` on status updates

### Screen Reader Support
- Announces when sub-agent starts
- Announces completion/errors
- Reads tool call results
- Indicates loading states

---

## Performance Optimizations

### Render Optimization
- Zustand selectors (prevent unnecessary re-renders)
- Detail panel only renders when open
- List view virtualizes if >50 items (future)

### Memory Management
- Auto-prune completed runs (5 min)
- Max 20 completed runs in memory
- Tool results truncated in list view

### Network Efficiency
- Single SSE connection per session
- Client-side event filtering
- No polling (push-based updates)

---

## Future Enhancements

### Phase 2
- [ ] Tree visualization (nested sub-agents)
- [ ] Sub-agent logs export (JSON/CSV)
- [ ] Pause/resume sub-agents
- [ ] Sub-agent retry mechanism

### Phase 3
- [ ] Sub-agent performance metrics
- [ ] Cost tracking per sub-agent
- [ ] Sub-agent templates (reusable configs)
- [ ] Cross-session sub-agent history

---

## Mockup Screenshots

(TODO: Add actual screenshots after manual testing)

1. Header with 2 active sub-agents
2. Detail panel list view
3. Detail panel detail view
4. Inline status card (running)
5. Inline status card (completed)
6. Message with sub-agent badge

---

## Component Dependencies

```
SubAgentIndicator
    └─ useSubAgentStore (Zustand)

SubAgentDetailPanel
    ├─ useSubAgentStore (Zustand)
    └─ SubAgentInfo (type)

SubAgentStatusCard
    └─ SubAgentInfo (type)

MessageCard
    └─ (optional) subAgentRunId, subAgentLabel props

Chat (page)
    ├─ useSubAgentSSE (hook)
    ├─ SubAgentIndicator
    └─ SubAgentDetailPanel
```

---

## Testing Scenarios

### Scenario 1: Single Sub-Agent
1. User asks to search codebase
2. Sub-agent spawns
3. Indicator shows badge "1"
4. Sub-agent runs 3 tools
5. Sub-agent completes
6. Badge disappears
7. Run moves to completed list

### Scenario 2: Multiple Concurrent Sub-Agents
1. User asks complex question
2. 3 sub-agents spawn
3. Indicator shows badge "3"
4. Sub-agents run in parallel
5. First completes (badge → "2")
6. Second completes (badge → "1")
7. Third completes (badge disappears)

### Scenario 3: Sub-Agent Error
1. Sub-agent spawns
2. Tool call fails
3. Error appears in detail panel
4. Status card shows error message
5. Badge color changes to red
6. Completed with error status

### Scenario 4: Panel Interaction
1. User clicks indicator
2. Panel opens with list
3. User clicks a run
4. Detail view shows full info
5. User clicks "Back"
6. Returns to list view
7. User clicks [×]
8. Panel closes

---

This visual guide provides a complete reference for the sub-agent UI implementation!
