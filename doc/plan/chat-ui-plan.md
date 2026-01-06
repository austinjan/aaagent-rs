# Chat UI Feature Plan

- Feature name: `chat-ui`
- Status: Draft
- Created: 2026-01-06
- Last updated: 2026-01-06 (SSE transport + tool pair edge cases + AI error handling + performance targets + state management specified)

## 1) Overview

### Goal
- Deliver a chat UI that visualizes the history tree path, agent events, tool activity, and checkpoints with collapsible cards and a minimal mini map.

### Scope (In)
- Chat container with per-node cards and fixed input.
- Mini map showing only the current path.
- Collapsible tool call/result pairs with warning states for missing pairs.
- Checkpoint cards with summary toggle (short/full).
- System prompt viewer via toolbar.

### Non-goals (Out)
- Full tree branch visualization.
- Provider configuration UI.
- Multi-session management UI.

### User stories
- As a user, I can read the conversation as cards in order.
- As a user, I can collapse/expand tool call/result pairs.
- As a user, I can quickly locate nodes using the mini map.
- As a user, I can view checkpoint summaries in full.

## 2) Requirements

### Functional requirements
- [ ] Render nodes on the active path (`root` → `active_leaf`) as cards.
- [ ] Group tool call/result pairs into a default-collapsed card.
- [ ] Highlight incomplete tool pairs with warning color in both views.
- [ ] Render checkpoint nodes as dedicated cards with summary toggle.
- [ ] Provide a toolbar button to view system prompt content.
- [ ] Enable mini map click-to-scroll and card click-to-highlight.

### Non-functional requirements

**Performance:**
- **Initial Load**: Render first visible messages in <200ms (50 cards viewport)
- **Virtualization Threshold**: Enable virtual scrolling at 100+ cards
- **Memory Ceiling**: UI state should not exceed 50MB for 1000+ node sessions
- **Scroll Performance**: Maintain 60fps during scroll with virtualization
- **SSE Throughput**: Handle 100+ events/second without UI lag
- **Error Analysis Latency**: Display raw error immediately, analysis within 5s

**Reliability:**
- UI state must stay consistent with stream updates.
- Handle SSE reconnection without data loss.
- Graceful degradation when backend is slow or unavailable.

**Security:**
- Never display secret values (API keys, tokens) in UI.
- Keep prompt view read-only.
- Sanitize error messages before display.

**Observability:**
- Log UI errors during stream rendering.
- Track performance metrics (render time, memory usage).
- Monitor error analysis success rate.

**Compatibility:**
- Supports modern Chromium-based browsers (Chrome, Edge, Arc).
- Fallback for browsers without native EventSource support.

## 3) References
- Docs: `doc/plan/TREE_MESSAGE_MODEL_PLAN.md`
- Related issues/PRs: TBD
- Designs/diagrams: TBD
- APIs/specs: `src/history/session.rs`, `src/history/node.rs`, `src/agent/mod.rs`

## 4) Design

### Proposed approach
- Build a web UI that renders a linear list derived from the active path.
- Maintain a view model that groups tool call/result nodes by `tool_call_id`.
- Sync selection state between mini map and chat container.

### Data model / schema changes
- None required; map directly from `Session`, `Node`, and `AgentEvent`.

### API changes

#### Streaming Transport: Server-Sent Events (SSE)

**Primary Endpoint**: `POST /api/sessions/{session_id}/chat`
- **Request**: `{ "message": "user input" }`
- **Response**: SSE stream with event types matching `AgentEvent` enum
- **Connection**: Keep-alive until `done` or `error` event

**Event Types** (maps directly from `src/agent/mod.rs::AgentEvent`):

| Event Type | Payload | Description |
|------------|---------|-------------|
| `content` | `{ "content": "text chunk" }` | Streaming text chunks from LLM (maps to `AgentEvent::Content`) |
| `thinking` | `{ "thinking": "reasoning text" }` | Reasoning/thinking content from models like Claude or o1 (maps to `AgentEvent::Thinking`) |
| `tool_calls_requested` | `{ "tool_calls": [{ "id": "...", "name": "...", "arguments": {...} }] }` | Tool execution initiated with list of requested tools (maps to `AgentEvent::ToolCallsRequested`) |
| `tool_result` | `{ "tool_call_id": "...", "tool_name": "...", "result": "...", "is_error": false }` | Tool execution completed with result (maps to `AgentEvent::ToolResult`) |
| `loop_detected` | `{ "detection": { "suggestion": "...", "action": "...", "warning_message": "..." } }` | Loop warning triggered by detector (maps to `AgentEvent::LoopDetected`) |
| `checkpoint_created` | `{ "node_id": "...", "strategy": "auto_turns/manual/..." }` | Checkpoint created at node (maps to `AgentEvent::CheckpointCreated`) |
| `done` | `{ "total_usage": {...}, "all_tool_calls": [...], "rounds": N }` | Chat completed with final stats (maps to `AgentEvent::Done`) |
| `error` | `{ "error": "error message" }` | Error occurred during processing |

**Additional Endpoints**:
- `GET /api/sessions/{session_id}/path` - Get active path nodes for initial render
- `GET /api/sessions/{session_id}/checkpoints` - Get all checkpoint metadata
- `GET /api/sessions/{session_id}/system-prompt` - Get system prompt content

**Rationale**:
- **Direct mapping**: SSE event types match existing `AgentEvent` callback pattern in `agent.chat_with_callback()`
- **Unidirectional**: Server→client stream matches current architecture (no bidirectional needed yet)
- **Simple client**: Native browser `EventSource` API, no WebSocket complexity
- **Automatic reconnection**: Browser handles connection drops transparently
- **Text-based**: Natural fit for JSON event payloads
- **Upgrade path**: Can switch to WebSockets later if bidirectional control needed (cancel, pause, etc.)

**Client Example**:
```javascript
const eventSource = new EventSource(`/api/sessions/${sessionId}/chat?message=${encodeURIComponent(msg)}`);

eventSource.addEventListener('content', (e) => {
  const { content } = JSON.parse(e.data);
  appendToCurrentMessage(content); // Stream text as it arrives
});

eventSource.addEventListener('tool_calls_requested', (e) => {
  const { tool_calls } = JSON.parse(e.data);
  showToolCallsCard(tool_calls); // Create collapsed card
});

eventSource.addEventListener('done', (e) => {
  const { total_usage, rounds } = JSON.parse(e.data);
  updateStats(total_usage, rounds);
  eventSource.close();
});
```

### UI/UX changes (if any)
- Card components: `MessageCard`, `ToolPairCard`, `CheckpointCard`.
- Mini map: node dots per type, hover tooltip with metadata.
- Toolbar: system prompt viewer modal/panel.

### Tool Pair Grouping & Edge Cases

#### Multiple Tool Calls in a Single Assistant Turn
- **Grouping**: Tool calls from a single assistant message are grouped into one collapsible card
- **Sequential Execution**: Backend executes tool calls sequentially (not parallel) per `src/agent/mod.rs:250-280`
- **Display Structure**:
  ```
  ┌─ Assistant Message Card ─────────────────────┐
  │ "Let me check both the weather and time..."  │
  │                                               │
  │ ┌─ Tool Calls (3) [collapsed] ─────────────┐ │
  │ │ ▶ get_weather, get_time, calculate       │ │
  │ │   2/3 complete, 1 pending                │ │
  │ └──────────────────────────────────────────┘ │
  └──────────────────────────────────────────────┘
  
  When expanded:
  ┌─ Tool Calls (3) [expanded] ──────────────────┐
  │ ✓ get_weather (call_1): "Sunny, 72°F"       │
  │ ✓ get_time (call_2): "2:30 PM"              │
  │ ⏳ calculate (call_3): pending...            │
  └──────────────────────────────────────────────┘
  ```
- **Summary Line**: Shows "N tools (X complete, Y pending, Z errors)"

#### Tool Call State Machine & Timeout Thresholds

**State Progression**:
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

**State Definitions**:
- **Pending (0-10s)**: Normal operation, show spinner ⏳
- **Slow (10-60s)**: Show "Still working..." with warning icon ⚠️
- **Orphaned (>60s)**: No result received, mark as failed with red warning ❌
- **Complete**: Tool executed successfully, green checkmark ✓
- **Error**: Tool returned `is_error: true`, red X with error message ❌

**Timeout Rationale**:
- Most tool calls complete in <5s
- File operations/API calls: 5-30s
- Very slow tools (large file processing): 30-60s
- Beyond 60s likely indicates a stuck tool or backend error

**Timer Implementation**:
```typescript
class ToolPairTracker {
  private timers = new Map<string, { slow: NodeJS.Timeout, orphan: NodeJS.Timeout }>();
  
  onToolCallRequested(toolCall: ToolCall) {
    const slowTimer = setTimeout(() => 
      this.updateState(toolCall.id, 'slow'), 10000);
    const orphanTimer = setTimeout(() => 
      this.updateState(toolCall.id, 'orphaned'), 60000);
    this.timers.set(toolCall.id, { slow: slowTimer, orphan: orphanTimer });
  }
  
  onToolResult(result: ToolResult) {
    const timers = this.timers.get(result.tool_call_id);
    if (timers) {
      clearTimeout(timers.slow);
      clearTimeout(timers.orphan);
      this.timers.delete(result.tool_call_id);
    }
  }
}
```

#### Out-of-Order Tool Results

**Current Behavior**: Results arrive in-order (sequential execution in backend)

**Future-Proof Design** (if parallel execution is added):
- UI maintains request order from `tool_calls_requested` event
- Results update their corresponding slot by `tool_call_id` (not by arrival order)
- Each tool pair slot is identified by `tool_call_id`, not index
- Flash animation highlights newly completed tools

**Example**:
```
Request order: [call_1, call_2, call_3]
Results arrive: [call_3 ✓, call_1 ✓, call_2 ✓]  ← Out-of-order

UI maintains request order:
┌─ Tool Calls ───────────────────────────┐
│ ✓ tool_1 (call_1) ← arrived 2nd       │
│ ✓ tool_2 (call_2) ← arrived 3rd       │
│ ✓ tool_3 (call_3) ← arrived 1st 💫    │  ← Flash on update
└────────────────────────────────────────┘
```

#### View Model Structure

```typescript
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

interface ToolPair {
  toolCall: ToolCall;                // From tool_calls_requested event
  result?: ToolResult;               // From tool_result event (undefined = pending)
  state: ToolCallState;
  elapsedMs: number;                 // Time since tool_calls_requested
}

type ToolCallState = 
  | 'pending'     // 0-10s, normal
  | 'slow'        // 10-60s, warning
  | 'orphaned'    // >60s, error
  | 'complete'    // Result received, is_error=false
  | 'error';      // Result received, is_error=true

function getToolCallIcon(state: ToolCallState): string {
  switch(state) {
    case 'pending': return '⏳';
    case 'slow': return '⚠️';
    case 'orphaned': return '❌';
    case 'complete': return '✓';
    case 'error': return '❌';
  }
}
```

#### Edge Case Handling

**Mismatched tool_call_id**:
- Backend validates Tool Sandwich pattern via `MessageValidator::validate_tool_sandwich()` (see `src/history/validator.rs`)
- UI defensively handles unexpected results:
  ```typescript
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
    pair.result = result;
    pair.state = result.is_error ? 'error' : 'complete';
  }
  ```

**Stream Connection Loss**:
- SSE auto-reconnects, but tool calls mid-execution may appear orphaned
- UI marks in-flight tool calls as 'orphaned' after 60s timeout
- Reconnection delivers final `done` event with all tool calls for reconciliation

### AI-Enhanced Error Handling

#### Overview
Instead of showing raw error messages, use a **lightweight agent** (with quick provider) to analyze errors and generate:
1. **User-friendly explanation** (what went wrong in plain English)
2. **Actionable suggestions** (specific steps to try)
3. **Contextual guidance** (based on user intent and error type)

#### Error Types Covered
- **Tool Execution**: File not found, permission denied, invalid arguments
- **API Calls**: Rate limits, timeouts, authentication failures, HTTP errors
- **LLM Provider**: Quota exceeded, invalid request, model unavailable
- **Network**: Connection timeouts, DNS failures, SSL errors
- **Validation**: Invalid input format, missing parameters, type mismatches
- **System**: Out of memory, disk full, internal errors

#### Architecture Flow

```
Error Occurs
    ↓
Capture error context (type, component, details)
    ↓
Send to Error Analysis Agent (quick provider)
    ↓
Agent generates:
  - Plain English explanation
  - 2-3 prioritized suggestions
  - Retry guidance
  - Severity level
    ↓
Display Enhanced Error Card in UI
```

#### Error Analysis Agent (Backend)

**Comprehensive Error Context:**
```rust
pub struct ErrorContext {
    error_type: ErrorType,        // ToolExecution, ApiCall, LlmProvider, Network, etc.
    component: String,             // Tool name, API endpoint, provider, etc.
    raw_error: String,             // Original error message
    details: ErrorDetails {
        tool_arguments: Option<Value>,
        http_status: Option<u16>,
        endpoint: Option<String>,
        model_name: Option<String>,
        token_count: Option<u32>,
        recent_successful_operations: Vec<String>,
        operation_count: usize,
        user_intent: String,
    },
}
```

**Analysis Prompt Structure:**
```rust
fn build_analysis_prompt(context: &ErrorContext) -> String {
    format!(r#"
You are an error analysis assistant. A user's AI agent encountered an error.

**Error Context:**
- Component: {component}
- Error Type: {error_type}
- Raw Error: {raw_error}
- User's intent: "{user_intent}"
- Recent successful operations: {recent_ops}
- This operation attempted {op_count} time(s)

**Your Task:**
Analyze this error and provide helpful guidance for the user (not the developer).

**Response Format (JSON):**
{{
  "explanation": "Brief, user-friendly explanation in plain English (1-2 sentences)",
  "suggestions": [
    "First specific, actionable suggestion",
    "Second suggestion with clear steps",
    "Third suggestion (optional)"
  ],
  "is_retryable": true_or_false,
  "retry_hint": "When/how to retry (e.g., 'Wait 1 minute')",
  "severity": "low|medium|high",
  "category": "transient|configuration|invalid_input|quota|permission|system"
}}

**Guidelines:**
1. Be user-friendly: Plain language, not technical jargon
2. Be specific: Concrete actions, not vague advice
3. Be honest: If unrecoverable, say so clearly
4. Be helpful: Suggest workarounds or alternatives
5. Prioritize: Most likely solution first
6. Context-aware: Use user's intent to tailor suggestions

**Examples:**

Tool execution (file_read failed):
- Explanation: "The file you're trying to read doesn't exist or isn't accessible."
- Suggestions: ["Check if the file path is correct", "Verify you have permission", "List directory first"]

API call (HTTP 429 rate limit):
- Explanation: "You've made too many requests to this API. The service is asking you to slow down."
- Suggestions: ["Wait 60 seconds before trying again", "Use webhooks for frequent updates", "Check for rate limit increase"]

LLM provider (quota exceeded):
- Explanation: "You've reached your usage limit for this AI model. Your account needs more credits."
- Suggestions: ["Add credits in billing settings", "Switch to cheaper model", "Wait until quota resets"]

Network error (timeout):
- Explanation: "The connection timed out - the server didn't respond in time."
- Suggestions: ["Try again - server might be busy", "Check internet connection", "Service might be down"]

Validation error (invalid JSON):
- Explanation: "The input data isn't in the correct format."
- Suggestions: ["Check input for syntax errors", "Ensure all required fields provided", "Refer to documentation"]

Now analyze the error above and provide the JSON response.
"#)
}
```

**Security: Sanitize Sensitive Data:**
```rust
fn sanitize_error(error: &str) -> String {
    // Remove API keys, tokens, passwords before sending to analysis agent
    let patterns = [
        r"(api[_-]?key[:\s=]+)[\w-]+",
        r"(token[:\s=]+)[\w-]+",
        r"(bearer\s+)[\w-]+",
        r"(password[:\s=]+)[\w-]+",
    ];
    // Replace with "***REDACTED***"
}
```

#### Enhanced AgentEvent

```rust
pub enum AgentEvent {
    // ... existing events ...
    
    /// Error with AI-generated analysis
    ErrorAnalyzed {
        error_id: String,           // Unique error ID for tracking
        error_type: ErrorType,      // ToolExecution, ApiCall, etc.
        component: String,          // What failed
        raw_error: String,          // Original error message
        analysis: ErrorAnalysis {
            explanation: String,
            suggestions: Vec<String>,
            is_retryable: bool,
            retry_hint: Option<String>,
            severity: ErrorSeverity,  // low, medium, high
            category: ErrorCategory,  // transient, quota, permission, etc.
        },
    },
}
```

#### SSE Event Type

```json
{
  "event": "error_analyzed",
  "data": {
    "error_id": "err_xyz789",
    "error_type": "ApiCall",
    "component": "fetch_url",
    "raw_error": "HTTP 429: Rate limit exceeded",
    "analysis": {
      "explanation": "You've made too many requests to this API. The service is asking you to slow down.",
      "suggestions": [
        "Wait 60 seconds before trying again",
        "If you need frequent updates, consider using a webhook instead",
        "Check if you have a rate limit increase available in your API settings"
      ],
      "is_retryable": true,
      "retry_hint": "Wait 1 minute",
      "severity": "medium",
      "category": "quota"
    }
  }
}
```

#### UI Error Card Component

**Severity-Based Styling:**
- **Low**: Yellow border/background, ⚠️ icon (user can easily fix or ignore)
- **Medium**: Orange border/background, ⚠️ icon (requires action but not urgent)
- **High**: Red border/background, ❌ icon (blocks progress, immediate attention needed)

**Component Structure:**
```typescript
interface ErrorCardProps {
  errorId: string;
  errorType: ErrorType;
  component: string;
  rawError: string;
  analysis: ErrorAnalysis;
  onRetry?: () => void;
  onEditInput?: () => void;
}

function ErrorCard({ component, analysis, rawError, onRetry }: ErrorCardProps) {
  const severityStyles = {
    low: { bg: 'bg-yellow-50', border: 'border-yellow-500', text: 'text-yellow-900', icon: '⚠️' },
    medium: { bg: 'bg-orange-50', border: 'border-orange-500', text: 'text-orange-900', icon: '⚠️' },
    high: { bg: 'bg-red-50', border: 'border-red-500', text: 'text-red-900', icon: '❌' }
  };
  
  const style = severityStyles[analysis.severity];
  
  return (
    <div className={`error-card border-l-4 ${style.border} ${style.bg} p-4 rounded`}>
      {/* Header with icon and component name */}
      <div className="flex items-start gap-3">
        <span className="text-2xl">{style.icon}</span>
        <div className="flex-1">
          <div className="flex items-center gap-2">
            <h4 className={`font-semibold ${style.text}`}>{component}</h4>
            <span className="text-xs px-2 py-0.5 rounded bg-gray-200 text-gray-700">
              {analysis.category}
            </span>
          </div>
          
          {/* AI-generated explanation */}
          <p className={`mt-2 ${style.text}`}>{analysis.explanation}</p>
        </div>
      </div>

      {/* AI-generated suggestions (numbered list) */}
      <div className="mt-4 ml-11">
        <p className={`text-sm font-medium ${style.text} mb-2`}>
          💡 What you can do:
        </p>
        <ol className="space-y-2">
          {analysis.suggestions.map((suggestion, i) => (
            <li key={i} className={`text-sm ${style.text} flex gap-2`}>
              <span className="font-semibold">{i + 1}.</span>
              <span>{suggestion}</span>
            </li>
          ))}
        </ol>
      </div>

      {/* Action buttons based on error category */}
      {analysis.is_retryable && (
        <div className="mt-4 ml-11 flex gap-2">
          <button onClick={onRetry} className="px-3 py-1.5 text-sm bg-blue-600 text-white rounded">
            🔄 Retry {analysis.retry_hint && `(${analysis.retry_hint})`}
          </button>
          {analysis.category === 'invalid_input' && (
            <button className="px-3 py-1.5 text-sm bg-gray-600 text-white rounded">
              ✏️ Edit Input
            </button>
          )}
          {analysis.category === 'quota' && (
            <button className="px-3 py-1.5 text-sm bg-purple-600 text-white rounded">
              💳 View Billing
            </button>
          )}
        </div>
      )}

      {/* Collapsible technical details */}
      <details className="mt-4 ml-11">
        <summary className="text-xs text-gray-600 cursor-pointer hover:underline">
          Technical details
        </summary>
        <pre className="mt-2 text-xs text-gray-900 bg-gray-100 p-2 rounded overflow-x-auto">
          {rawError}
        </pre>
      </details>
    </div>
  );
}
```

#### Visual Examples

**API Rate Limit (Medium Severity):**
```
╔════════════════════════════════════════════════════════════╗
║ ⚠️  fetch_url                              [quota]         ║
║                                                            ║
║ You've made too many requests to this API. The service    ║
║ is asking you to slow down.                               ║
║                                                            ║
║ 💡 What you can do:                                       ║
║    1. Wait 60 seconds before trying again                 ║
║    2. If you need frequent updates, consider using a      ║
║       webhook instead                                     ║
║    3. Check if you have a rate limit increase available   ║
║       in your API settings                                ║
║                                                            ║
║ [ 🔄 Retry (Wait 1 minute) ] [ 💳 View Billing ]          ║
║                                                            ║
║ ▸ Technical details                                       ║
╚════════════════════════════════════════════════════════════╝
```

**LLM Quota Exceeded (High Severity):**
```
╔════════════════════════════════════════════════════════════╗
║ ❌ LLM Provider                            [quota]         ║
║                                                            ║
║ You've reached your usage limit for this AI model. Your   ║
║ account needs more credits to continue.                   ║
║                                                            ║
║ 💡 What you can do:                                       ║
║    1. Check your billing settings and add credits         ║
║    2. Switch to a cheaper model like gpt-4o-mini          ║
║    3. Wait until your quota resets (resets on Jan 15)     ║
║                                                            ║
║ [ 💳 View Billing ] [ 🔄 Switch Model ]                   ║
║                                                            ║
║ ▸ Technical details                                       ║
╚════════════════════════════════════════════════════════════╝
```

**File Not Found (Low Severity):**
```
╔════════════════════════════════════════════════════════════╗
║ ⚠️  file_read                         [invalid_input]     ║
║                                                            ║
║ The file you're trying to read doesn't exist or isn't     ║
║ accessible.                                               ║
║                                                            ║
║ 💡 What you can do:                                       ║
║    1. Check if the file path is correct                   ║
║    2. Verify you have permission to read this file        ║
║    3. Try listing the directory first to see available    ║
║       files                                               ║
║                                                            ║
║ [ ✏️ Edit Input ] [ 🔄 Retry ]                            ║
║                                                            ║
║ ▸ Technical details                                       ║
╚════════════════════════════════════════════════════════════╝
```

#### Benefits

1. **User-Friendly**: Non-technical users understand what went wrong
2. **Actionable**: Specific steps users can take immediately
3. **Educational**: Users learn to use the system better over time
4. **Cost-Effective**: Uses quick/cheap model for error analysis
5. **Async**: Analysis happens in background, doesn't block
6. **Graceful Degradation**: Falls back to raw error if analysis fails
7. **Secure**: Sanitizes sensitive data before analysis

#### Edge Cases & Fallbacks

**Analysis Failure:**
- If error analyzer itself fails, display raw error card
- Log meta-error for debugging
- Show simplified message: "Error details unavailable"

**Analysis Timeout:**
- Show raw error immediately (don't block UI)
- Stream in analysis when ready (update card in place)
- Timeout after 5s, keep raw error visible

**Sensitive Data:**
- Sanitize API keys, tokens, passwords before sending to analyzer
- Never send user PII to analysis agent
- Filter common secret patterns with regex

**Cost Control:**
- Only analyze errors with quick provider (cheap model)
- Cache common error patterns to avoid re-analysis
- Rate limit: max 10 analyses per minute per session

### State Management Architecture

#### Overview

With synced selection between mini map and chat container, plus real-time streaming updates, we need a **clear state management pattern** to avoid race conditions and inconsistent UI state.

#### Core Principles

1. **Single Source of Truth**: All state lives in one store
2. **Server-Authoritative Data**: Backend owns the conversation data
3. **Client-Authoritative UI**: Frontend owns UI state (expand/collapse, scroll position)
4. **Optimistic Updates**: User actions update UI immediately, no waiting
5. **Event Ordering Guarantees**: Stream events applied in order via queue

#### State Structure

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
    pendingEvents: AgentEvent[];        // Event queue for ordering
  };
  
  // ===== PERFORMANCE METRICS =====
  metrics: {
    renderTime: number;
    memoryUsage: number;
    fps: number;
  };
}
```

#### State Management Pattern: Unidirectional Data Flow

```
User Action → Dispatch Action → Reducer → New State → UI Re-render
     ↑                                                      ↓
     └──────────────── Side Effects (API calls) ───────────┘

SSE Event → Event Queue → Ordered Processing → Reducer → New State → UI Re-render
```

#### Single Source of Truth: Selected Node

**Problem**: Selection state must sync between mini map and chat container.

**Solution**: Store selection in one place, derive UI from it.

```typescript
// ✅ CORRECT: Single source of truth
const state = {
  ui: {
    selectedNodeId: "node_123"  // ONLY place selection is stored
  }
};

// Derive UI state from single source
function isMiniMapNodeSelected(nodeId: string): boolean {
  return state.ui.selectedNodeId === nodeId;
}

function isChatCardSelected(nodeId: string): boolean {
  return state.ui.selectedNodeId === nodeId;
}

// Update selection (both views react automatically)
function selectNode(nodeId: string) {
  dispatch({ type: 'SELECT_NODE', nodeId });
  // Both mini map and chat container re-render with new selection
}
```

**Anti-pattern**: Storing selection in multiple places
```typescript
// ❌ WRONG: Multiple sources of truth
const miniMapState = { selectedNodeId: "node_123" };
const chatState = { selectedNodeId: "node_456" };  // OUT OF SYNC!
```

#### Event Queue for Stream Updates

**Problem**: SSE events can arrive faster than UI can process them, causing:
- Race conditions (events processed out of order)
- UI lag (main thread blocked)
- Inconsistent state (partial updates)

**Solution**: Event queue with ordering guarantees

```typescript
class EventQueue {
  private queue: AgentEvent[] = [];
  private processing = false;
  private sequenceNumber = 0;
  
  enqueue(event: AgentEvent) {
    // Assign sequence number for ordering
    const sequencedEvent = { ...event, seq: this.sequenceNumber++ };
    this.queue.push(sequencedEvent);
    
    // Start processing if not already running
    if (!this.processing) {
      this.processQueue();
    }
  }
  
  private async processQueue() {
    this.processing = true;
    
    while (this.queue.length > 0) {
      const event = this.queue.shift()!;
      
      // Process event (sync or async)
      await this.processEvent(event);
      
      // Yield to browser for rendering (avoid blocking UI)
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
    switch (event.type) {
      case 'content':
        store.dispatch({ type: 'APPEND_CONTENT', ...event });
        break;
      case 'tool_calls_requested':
        store.dispatch({ type: 'START_TOOL_CALLS', ...event });
        break;
      case 'tool_result':
        store.dispatch({ type: 'ADD_TOOL_RESULT', ...event });
        break;
      // ... handle all event types
    }
  }
}
```

**Ordering Guarantees:**
1. Events processed in arrival order (FIFO)
2. No concurrent processing (one event at a time)
3. Yields to UI between events (maintains 60fps)

#### Optimistic UI vs Server-Authoritative

**Client-Authoritative (Optimistic UI):**
User actions that only affect UI state update immediately without server confirmation.

```typescript
// Expand/collapse tool pair (instant feedback)
function toggleToolPair(toolPairId: string) {
  // Update UI immediately (no server round-trip)
  if (state.ui.expandedToolPairs.has(toolPairId)) {
    dispatch({ type: 'COLLAPSE_TOOL_PAIR', toolPairId });
  } else {
    dispatch({ type: 'EXPAND_TOOL_PAIR', toolPairId });
  }
  // UI re-renders instantly
}

// Scroll position (local only)
function updateScrollPosition(offset: number) {
  dispatch({ type: 'UPDATE_SCROLL', offset });
}

// Select node (instant highlight)
function selectNode(nodeId: string) {
  dispatch({ type: 'SELECT_NODE', nodeId });
  // Mini map and chat both highlight immediately
}
```

**Server-Authoritative (Wait for Confirmation):**
Actions that modify backend data must wait for server confirmation.

```typescript
// Send message (optimistic display, then confirm)
async function sendMessage(text: string) {
  // 1. Optimistic: Show message immediately with pending state
  const tempId = `temp_${Date.now()}`;
  dispatch({ 
    type: 'ADD_OPTIMISTIC_MESSAGE', 
    tempId,
    content: text,
    isPending: true 
  });
  
  // 2. Send to server (SSE stream will send back real node)
  const stream = await fetch(`/api/sessions/${sessionId}/chat`, {
    method: 'POST',
    body: JSON.stringify({ message: text })
  });
  
  // 3. Server responds with real node_id via SSE
  // EventQueue processes 'done' event and replaces temp node
}

// Retry failed tool call (server decides)
async function retryToolCall(toolCallId: string) {
  // NO optimistic update - wait for server
  dispatch({ type: 'SHOW_RETRY_LOADING', toolCallId });
  
  await fetch(`/api/sessions/${sessionId}/retry`, {
    method: 'POST',
    body: JSON.stringify({ toolCallId })
  });
  
  // Server sends new events via SSE
  // EventQueue will update UI when events arrive
}
```

#### State Update Flow Examples

**Example 1: User expands tool pair (optimistic)**

```
User clicks "Expand" button
    ↓
dispatch({ type: 'EXPAND_TOOL_PAIR', toolPairId: 'call_123' })
    ↓
Reducer adds 'call_123' to state.ui.expandedToolPairs
    ↓
UI re-renders (tool pair expands immediately)
    ↓
No server call needed (UI state only)
```

**Example 2: User selects node in mini map (optimistic + side effect)**

```
User clicks node in mini map
    ↓
dispatch({ type: 'SELECT_NODE', nodeId: 'node_456' })
    ↓
Reducer sets state.ui.selectedNodeId = 'node_456'
    ↓
UI re-renders:
  - Mini map highlights node_456
  - Chat container scrolls to node_456
  - Both views show same selection (single source of truth)
```

**Example 3: SSE stream delivers tool result (server-authoritative)**

```
SSE event: { type: 'tool_result', tool_call_id: 'call_123', result: '...' }
    ↓
EventQueue.enqueue(event)
    ↓
EventQueue processes in order
    ↓
dispatch({ type: 'ADD_TOOL_RESULT', ... })
    ↓
Reducer updates state.streaming.toolPairGroups
    ↓
UI re-renders (tool result appears, changes pending → complete)
```

**Example 4: Multiple rapid SSE events (queued)**

```
SSE events arrive rapidly:
  1. content: "Let me"
  2. content: " check"
  3. tool_calls_requested: [...]
  4. tool_result: ...
    ↓
All enqueued immediately (non-blocking)
    ↓
EventQueue processes in order:
  - Process event 1 → yield to UI (browser paints)
  - Process event 2 → yield to UI
  - Process event 3 → yield to UI
  - Process event 4 → yield to UI
    ↓
UI stays responsive (60fps maintained)
```

#### State Management Library Choice

**Recommended: Zustand (lightweight, no boilerplate)**

```typescript
import create from 'zustand';

interface ChatStore extends ChatUIState {
  // Actions
  selectNode: (nodeId: string) => void;
  toggleToolPair: (toolPairId: string) => void;
  addNode: (node: Node) => void;
  updateToolPairState: (toolCallId: string, state: ToolCallState) => void;
}

const useChatStore = create<ChatStore>((set, get) => ({
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
    pendingEvents: [],
  },
  metrics: {
    renderTime: 0,
    memoryUsage: 0,
    fps: 60,
  },
  
  // Actions (optimistic)
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
  
  // Actions (server-authoritative)
  addNode: (node) => set((state) => {
    const nodes = new Map(state.nodes);
    nodes.set(node.node_id, node);
    return { nodes };
  }),
  
  updateToolPairState: (toolCallId, newState) => set((state) => {
    const groups = new Map(state.streaming.toolPairGroups);
    const group = groups.get(toolCallId);
    if (group) {
      group.state = newState;
      groups.set(toolCallId, group);
    }
    return { streaming: { ...state.streaming, toolPairGroups: groups } };
  }),
}));

// Usage in components
function MiniMap() {
  const selectedNodeId = useChatStore(state => state.ui.selectedNodeId);
  const selectNode = useChatStore(state => state.selectNode);
  
  return (
    <div>
      {nodes.map(node => (
        <NodeDot 
          key={node.id}
          isSelected={node.id === selectedNodeId}  // Derived from single source
          onClick={() => selectNode(node.id)}
        />
      ))}
    </div>
  );
}

function ChatContainer() {
  const selectedNodeId = useChatStore(state => state.ui.selectedNodeId);
  
  return (
    <div>
      {cards.map(card => (
        <Card 
          key={card.id}
          isHighlighted={card.id === selectedNodeId}  // Same single source
        />
      ))}
    </div>
  );
}
```

#### Synchronization Invariants

**Guarantees to maintain:**

1. **Selection Sync**: `miniMap.selected === chatContainer.highlighted === state.ui.selectedNodeId`
2. **Tool Pair Consistency**: Every `tool_calls_requested` event has matching results or timeout
3. **Event Ordering**: Events processed in arrival order (seq numbers)
4. **No Lost Updates**: All SSE events either processed or queued, never dropped
5. **Memory Bounds**: `nodes.size <= loadedChunks.size * 50` (only loaded chunks in memory)

**Validation checks (dev mode):**

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

### Performance Optimization & Lazy Loading Strategy

#### Performance Targets (Concrete Metrics)

| Metric | Target | Measurement Point |
|--------|--------|------------------|
| Initial render (first paint) | <200ms | 50 visible cards in viewport |
| Scroll frame rate | 60fps | During virtualized scroll with 1000+ cards |
| Memory usage | <50MB | UI state for 1000-node session |
| SSE event throughput | 100+ events/sec | No dropped frames or UI lag |
| Error analysis latency | <5s | From error → analysis display |
| Virtualization threshold | 100 cards | When to enable virtual scrolling |
| Card render budget | <4ms per card | To maintain 60fps (16ms frame budget) |

#### Lazy Loading Architecture

**Problem**: Loading 1000+ nodes upfront would:
- Block initial render (>1s load time)
- Consume excessive memory (>100MB)
- Cause scroll jank (rendering thousands of DOM nodes)

**Solution**: Progressive lazy loading with virtualization

```
┌─────────────────────────────────────────────┐
│  Viewport (50 cards visible)                │
│  ┌─────────────────────────────────────┐   │
│  │ Card 95                             │   │ ← Rendered
│  │ Card 96                             │   │
│  │ Card 97                             │   │
│  └─────────────────────────────────────┘   │
│                                             │
│  [Virtual Spacer: 44 cards above]          │ ← Placeholder height
│  [Virtual Spacer: 905 cards below]         │
│                                             │
└─────────────────────────────────────────────┘

Total: 1000 cards, only 50 rendered in DOM
```

#### Backend API Support

**1. Paginated Path Endpoint**

```rust
GET /api/sessions/{session_id}/path?limit=50&offset=0&direction=newest_first

Response:
{
  "nodes": [...],           // 50 nodes
  "total_count": 1234,      // Total nodes in active path
  "has_more": true,
  "next_offset": 50,
  "estimated_heights": {    // For virtual scrolling placeholder
    "message_avg": 120,     // px
    "tool_pair_avg": 180,
    "checkpoint_avg": 150
  }
}
```

**2. Node Range Endpoint (for jump-to)**

```rust
GET /api/sessions/{session_id}/path/range?start_node_id=abc&end_node_id=xyz

Response:
{
  "nodes": [...],           // Nodes between start and end
  "start_offset": 450,      // Position in full path
  "end_offset": 500
}
```

**3. Metadata Endpoint (for quick initial render)**

```rust
GET /api/sessions/{session_id}/path/metadata

Response:
{
  "total_nodes": 1234,
  "active_leaf_id": "xyz",
  "root_node_id": "abc",
  "checkpoint_positions": [100, 450, 890],  // Offsets
  "estimated_total_height": 148080          // px (for scrollbar)
}
```

#### Frontend Lazy Loading Strategy

**Phase 1: Initial Load (0-200ms target)**

```typescript
async function initialLoad(sessionId: string) {
  // 1. Fetch metadata (fast, <50ms)
  const metadata = await fetch(`/api/sessions/${sessionId}/path/metadata`);
  
  // 2. Set virtual scroll total height
  setVirtualScrollHeight(metadata.estimated_total_height);
  
  // 3. Fetch only visible viewport (newest 50 cards)
  const visible = await fetch(
    `/api/sessions/${sessionId}/path?limit=50&offset=0&direction=newest_first`
  );
  
  // 4. Render immediately
  renderCards(visible.nodes);  // <200ms total
}
```

**Phase 2: Scroll-Triggered Lazy Loading**

```typescript
class VirtualizedChatContainer {
  private loadedRanges: Set<string> = new Set();  // Track loaded chunks
  private chunkSize = 50;
  
  onScroll(scrollTop: number) {
    const visibleRange = this.calculateVisibleRange(scrollTop);
    const chunksToLoad = this.getUnloadedChunks(visibleRange);
    
    for (const chunk of chunksToLoad) {
      this.loadChunk(chunk.offset, chunk.limit);
    }
  }
  
  async loadChunk(offset: number, limit: number) {
    const chunkKey = `${offset}-${limit}`;
    if (this.loadedRanges.has(chunkKey)) return;
    
    const nodes = await fetch(
      `/api/sessions/${sessionId}/path?offset=${offset}&limit=${limit}`
    );
    
    this.loadedRanges.add(chunkKey);
    this.insertNodesAtOffset(nodes, offset);
  }
  
  calculateVisibleRange(scrollTop: number): Range {
    // Calculate which cards are visible based on scroll position
    // Add buffer (±1 chunk) for smooth scrolling
    const avgCardHeight = 120;  // From metadata
    const startIndex = Math.floor(scrollTop / avgCardHeight) - 50;  // -1 chunk buffer
    const endIndex = startIndex + 150;  // viewport + 2 chunk buffers
    
    return { start: Math.max(0, startIndex), end: endIndex };
  }
}
```

**Phase 3: Virtual Scrolling (100+ cards)**

```typescript
class VirtualScrollManager {
  private totalHeight = 0;
  private renderedCards = new Map<number, CardElement>();
  
  render() {
    const { scrollTop, viewportHeight } = this.getScrollInfo();
    const visibleIndices = this.getVisibleIndices(scrollTop, viewportHeight);
    
    // Render only visible + buffer
    const toRender = this.expandWithBuffer(visibleIndices, 25);  // ±25 cards buffer
    
    // Mount new cards
    for (const idx of toRender) {
      if (!this.renderedCards.has(idx)) {
        this.mountCard(idx);
      }
    }
    
    // Unmount far-away cards (keep memory low)
    for (const [idx, card] of this.renderedCards) {
      if (!toRender.includes(idx)) {
        this.unmountCard(idx);
      }
    }
  }
  
  mountCard(index: number) {
    const node = this.loadedNodes.get(index);
    if (!node) {
      this.triggerLazyLoad(index);  // Request from backend
      return;
    }
    
    const card = this.createCardElement(node);
    const offset = this.calculateOffset(index);
    card.style.position = 'absolute';
    card.style.top = `${offset}px`;
    this.container.appendChild(card);
    this.renderedCards.set(index, card);
  }
}
```

#### Memory Management

**Card Recycling Pool:**
```typescript
class CardPool {
  private pool: CardElement[] = [];
  private maxPoolSize = 100;  // Recycle up to 100 cards
  
  acquire(type: CardType): CardElement {
    const recycled = this.pool.find(c => c.type === type);
    if (recycled) {
      this.pool = this.pool.filter(c => c !== recycled);
      return recycled;
    }
    return this.createNew(type);
  }
  
  release(card: CardElement) {
    if (this.pool.length < this.maxPoolSize) {
      card.reset();  // Clear data, keep DOM structure
      this.pool.push(card);
    } else {
      card.destroy();  // GC will collect
    }
  }
}
```

**Aggressive Cleanup:**
```typescript
function cleanupOffscreenCards() {
  const { scrollTop, viewportHeight } = getScrollInfo();
  const keepRange = {
    start: scrollTop - viewportHeight * 2,  // Keep 2 viewports above
    end: scrollTop + viewportHeight * 3     // Keep 3 viewports below
  };
  
  for (const [offset, card] of renderedCards) {
    if (offset < keepRange.start || offset > keepRange.end) {
      cardPool.release(card);
      renderedCards.delete(offset);
    }
  }
}

// Run cleanup every 2 seconds during idle
setInterval(() => requestIdleCallback(cleanupOffscreenCards), 2000);
```

#### Progressive Enhancement

**Strategy by Session Size:**

| Session Size | Strategy | Rationale |
|-------------|----------|-----------|
| <100 nodes | No virtualization, load all | Fast enough, simple |
| 100-500 nodes | Virtualization only | Smooth scroll, low overhead |
| 500-1000 nodes | Virtual + lazy load | Balance memory & requests |
| 1000+ nodes | Aggressive lazy + recycle | Keep <50MB, 60fps |

**Adaptive Loading:**
```typescript
function selectStrategy(totalNodes: number): LoadStrategy {
  if (totalNodes < 100) {
    return new LoadAllStrategy();
  } else if (totalNodes < 500) {
    return new VirtualScrollStrategy();
  } else if (totalNodes < 1000) {
    return new LazyVirtualStrategy(chunkSize: 50);
  } else {
    return new AggressiveLazyStrategy(chunkSize: 25, recyclePool: true);
  }
}
```

#### Backend Optimization (Session.get_context)

**Problem**: Current `get_context()` walks entire tree from leaf to root/checkpoint.

**Optimization**: Add indexed path cache

```rust
// src/history/session.rs

impl Session {
    /// Get paginated slice of active path (optimized for UI)
    pub async fn get_path_slice(
        &self,
        offset: usize,
        limit: usize,
    ) -> Result<PathSlice> {
        let store = self.store()?;
        
        // Check if we have cached path
        if let Some(cached) = self.cached_path.as_ref() {
            let slice = cached.get(offset..offset + limit).unwrap_or_default();
            return Ok(PathSlice {
                nodes: slice.to_vec(),
                total_count: cached.len(),
                has_more: offset + limit < cached.len(),
            });
        }
        
        // Build path (walk tree once)
        let full_path = self.build_full_path().await?;
        
        // Cache for future requests
        self.cached_path = Some(full_path.clone());
        
        // Return slice
        let slice = full_path.get(offset..offset + limit).unwrap_or_default();
        Ok(PathSlice {
            nodes: slice.to_vec(),
            total_count: full_path.len(),
            has_more: offset + limit < full_path.len(),
        })
    }
    
    /// Invalidate path cache (call on new messages)
    pub fn invalidate_path_cache(&mut self) {
        self.cached_path = None;
    }
}
```

#### Performance Monitoring

**Client-Side Metrics:**
```typescript
class PerformanceMonitor {
  trackInitialRender() {
    performance.mark('render-start');
    // ... render code ...
    performance.mark('render-end');
    performance.measure('initial-render', 'render-start', 'render-end');
    
    const measure = performance.getEntriesByName('initial-render')[0];
    console.log(`Initial render: ${measure.duration}ms`);  // Target: <200ms
    
    if (measure.duration > 200) {
      analytics.trackSlow('initial-render', measure.duration);
    }
  }
  
  trackMemoryUsage() {
    if ('memory' in performance) {
      const memory = (performance as any).memory;
      const usedMB = memory.usedJSHeapSize / 1024 / 1024;
      
      console.log(`Memory usage: ${usedMB.toFixed(2)}MB`);  // Target: <50MB
      
      if (usedMB > 50) {
        analytics.trackHighMemory(usedMB);
        this.triggerAggressiveCleanup();
      }
    }
  }
  
  trackScrollPerformance() {
    let frameCount = 0;
    let lastTime = performance.now();
    
    const measureFPS = () => {
      const now = performance.now();
      frameCount++;
      
      if (now - lastTime >= 1000) {
        console.log(`Scroll FPS: ${frameCount}`);  // Target: 60fps
        
        if (frameCount < 50) {
          analytics.trackLowFPS(frameCount);
        }
        
        frameCount = 0;
        lastTime = now;
      }
      
      requestAnimationFrame(measureFPS);
    };
    
    requestAnimationFrame(measureFPS);
  }
}
```

### Migration / backward compatibility
- No breaking changes to existing APIs; UI can be additive.

## 5) Implementation plan

### Milestones
- M1: Data pipeline for active path + events + error analysis agent.
- M2: Chat container cards + tool pair collapse + error cards.
- M3: Mini map + toolbar + checkpoint summary toggle.

### Task breakdown (TODO)

**Backend:**
- [ ] Implement SSE endpoint for `/api/sessions/{session_id}/chat` with AgentEvent streaming.
- [ ] Implement paginated path endpoint: `GET /api/sessions/{session_id}/path?limit&offset&direction`.
- [ ] Implement node range endpoint: `GET /api/sessions/{session_id}/path/range?start_node_id&end_node_id`.
- [ ] Implement metadata endpoint: `GET /api/sessions/{session_id}/path/metadata`.
- [ ] Implement supporting GET endpoints (checkpoints, system-prompt).
- [ ] Add `Session::get_path_slice()` with cached path for pagination.
- [ ] Implement path cache invalidation on new messages.
- [ ] Implement ErrorAnalyzer with comprehensive error context capture.
- [ ] Build error sanitization (remove API keys, tokens, passwords).
- [ ] Add `AgentEvent::ErrorAnalyzed` with severity and category fields.
- [ ] Integrate error analysis into agent chat loop (tool errors, API errors, LLM errors).
- [ ] Implement error analysis caching to reduce costs.

**Frontend - State Management:**
- [ ] Implement ChatUIState interface with server/client/streaming/metrics sections.
- [ ] Set up Zustand store with unidirectional data flow.
- [ ] Implement EventQueue with ordering guarantees (FIFO + yield to UI).
- [ ] Implement single source of truth for selectedNodeId.
- [ ] Separate optimistic actions (expand/collapse) from server-authoritative (add node).
- [ ] Add state validation checks (dev mode) for sync invariants.

**Frontend - View Models:**
- [ ] Implement view model: ToolPairGroup with state machine (pending/slow/orphaned/complete/error).
- [ ] Implement ToolPairTracker with 10s/60s timeout timers.
- [ ] Implement error state management with severity-based styling.
- [ ] Implement VirtualScrollManager for 100+ card scenarios.
- [ ] Implement VirtualizedChatContainer with chunk-based loading.
- [ ] Implement CardPool for DOM element recycling.
- [ ] Implement adaptive loading strategy selector (4 tiers based on session size).

**Frontend - Components:**
- [ ] Build MessageCard component.
- [ ] Build ToolPairCard with collapsible groups and state icons.
- [ ] Build ErrorCard with AI-generated suggestions and action buttons.
- [ ] Build CheckpointCard with summary toggle.
- [ ] Implement mini map with hover detail + click-to-scroll.
- [ ] Add system prompt viewer in toolbar.

**Frontend - SSE Integration:**
- [ ] Implement SSE client with event handlers for all AgentEvent types.
- [ ] Add `error_analyzed` event handler with analysis display.
- [ ] Build streaming update logic for real-time card rendering with tool_call_id matching.
- [ ] Implement flash animation for out-of-order tool result updates.
- [ ] Handle SSE reconnection with in-flight error recovery.

**Frontend - Performance:**
- [ ] Implement PerformanceMonitor with initial render tracking (<200ms target).
- [ ] Add memory usage monitoring (<50MB target for 1000+ nodes).
- [ ] Add scroll FPS tracking (60fps target).
- [ ] Implement aggressive cleanup for offscreen cards (2 viewport buffer).
- [ ] Add performance regression tests for all targets.

### Completed (DONE)
- [x] Initial UI behavior requirements captured.

## 6) Testing plan

**Backend Tests:**
- Unit tests:
  - `Session::get_path_slice()` pagination correctness
  - Path cache invalidation on new messages
  - Metadata endpoint returns correct estimated heights
  - Node range endpoint returns correct offsets
  - ErrorAnalyzer prompt generation for all error types
  - Error sanitization (API keys, tokens, passwords redacted)
  - Error context capture for tool/API/LLM/network errors
  - JSON parsing of error analysis responses
  - Fallback behavior when analysis fails
  
- Integration tests:
  - Paginated path loading with 1000+ node sessions
  - Path cache hit rate (should be >90% for repeated requests)
  - Error analysis with quick provider (mock responses)
  - Full error flow: capture → analyze → emit event
  - Cost control: caching common errors
  - Rate limiting (max 10 analyses per minute)
  
- Performance tests:
  - Path slice query <50ms for 10,000 node sessions
  - Metadata endpoint <20ms response time
  - Memory usage of cached paths

**Frontend Tests:**
- Unit tests: 
  - State management: single source of truth for selectedNodeId
  - State management: optimistic vs server-authoritative actions
  - EventQueue: FIFO ordering with sequence numbers
  - EventQueue: yield to UI between events (maintains 60fps)
  - State validation: sync invariants (selection, tool pairs, no duplicates)
  - ToolPairGroup state machine transitions (pending → slow → orphaned)
  - Timer logic (10s slow warning, 60s orphaned timeout)
  - tool_call_id matching for out-of-order results
  - Mismatched/duplicate result handling
  - Error severity styling (low/medium/high)
  - Error category action button rendering
  - VirtualScrollManager mount/unmount logic
  - CardPool acquire/release/recycle logic
  - Adaptive strategy selection (4 tiers by session size)
  
- Integration tests: 
  - State sync: mini map selection updates chat container highlight
  - State sync: chat container scroll updates mini map highlight
  - EventQueue: rapid SSE events processed in order without lag
  - EventQueue: no lost updates during reconnection
  - Stream updates with multiple tool calls in single turn
  - Tool result arrival during different timeout phases
  - Error analysis streaming (raw error → analysis arrives later)
  - SSE reconnection with in-flight tool calls and errors
  - Lazy loading on scroll (fetch new chunks)
  - Virtual scroll with rapid scrolling
  - Memory cleanup during long scroll sessions
  - Optimistic UI: expand/collapse instant without server round-trip
  
- Performance tests:
  - Initial render <200ms with 50 cards
  - Scroll at 60fps with 1000+ cards (virtual scroll)
  - Memory usage <50MB with 1000 nodes loaded
  - SSE event processing 100+ events/sec without lag
  - Card render time <4ms per card
  - Cleanup reduces memory by 30%+ after scrolling
  
- E2E tests (if any): 
  - Collapsed/expanded state sync with mini map
  - Visual states (spinner, warning, error icons) during tool execution
  - Error card display with AI suggestions
  - Retry/Edit/Billing action buttons
  - Smooth scrolling through 500+ card history
  - Jump-to from mini map with lazy loading
  
- Edge cases: 
  - Tool call timeout (slow/orphaned states)
  - Out-of-order results (future parallel execution)
  - Rapid stream updates with multiple tool rounds
  - Large history with many tool pairs
  - SSE connection loss during tool execution
  - Error analysis timeout (5s limit)
  - Error analysis failure (fallback to raw error)
  - Multiple errors in single turn
  - Sensitive data in error messages
  - Scroll to unloaded region (trigger lazy load)
  - Rapid scroll without loading all chunks
  - Memory pressure with 10,000+ node session

## 7) Rollout plan
- Feature flag: optional `chat_ui_enabled` toggle.
- Staging validation: run sample session with tool calls + checkpoints.
- Gradual rollout: enable per environment.
- Rollback: disable feature flag.

## 8) Risks & mitigations
- Risk: large histories cause slow rendering.
  - Mitigation: Progressive lazy loading with 4-tier adaptive strategy (<100, 100-500, 500-1000, 1000+).
  - Mitigation: Virtual scrolling at 100+ cards with card recycling pool.
  - Mitigation: Aggressive cleanup (keep only 2-3 viewport buffers in memory).
  - Mitigation: Backend path caching to avoid repeated tree walks.
- Risk: event stream races with UI state.
  - Mitigation: queue events, apply in order, guard missing pairs.
- Risk: virtualization introduces complexity and bugs.
  - Mitigation: Comprehensive performance tests with concrete targets.
  - Mitigation: Start simple (<100 cards no virtualization) and progressively enhance.
- Risk: memory leaks in long-running sessions.
  - Mitigation: PerformanceMonitor tracks memory, triggers cleanup at 50MB threshold.
  - Mitigation: Automated tests verify memory doesn't grow unbounded.
- Risk: lazy loading causes flickering during scroll.
  - Mitigation: Pre-load ±1 chunk buffer (50 cards above/below viewport).
  - Mitigation: Show skeleton placeholders while loading chunks.

## 9) Acceptance criteria

**State Management:**
- [ ] Single source of truth: selectedNodeId stored in one place.
- [ ] Selection sync: mini map and chat container both reflect state.ui.selectedNodeId.
- [ ] EventQueue processes SSE events in order (FIFO with sequence numbers).
- [ ] Optimistic updates: expand/collapse/scroll update UI instantly.
- [ ] Server-authoritative: conversation data waits for SSE confirmation.
- [ ] State validation passes all invariants (no duplicate nodes, tool pairs consistent).
- [ ] No lost SSE events (all enqueued and processed).

**Message & Tool Display:**
- [ ] Cards render for every node on the active path.
- [ ] Tool pairs from single assistant turn are grouped into one collapsible card.
- [ ] Tool pair cards default to collapsed and can expand.
- [ ] Tool call states are visually distinct:
  - [ ] Pending (0-10s): spinner icon
  - [ ] Slow (10-60s): warning icon with "Still working..."
  - [ ] Orphaned (>60s): red error icon
  - [ ] Complete: green checkmark
  - [ ] Error: red X with error message
- [ ] Collapsed tool pair cards show summary: "N tools (X complete, Y pending, Z errors)".
- [ ] Tool results update correct slot by tool_call_id (not arrival order).

**Error Handling:**
- [ ] All errors (tool, API, LLM, network, validation) trigger error analysis.
- [ ] Error cards display AI-generated plain English explanation.
- [ ] Error cards show 2-3 specific, actionable suggestions.
- [ ] Error severity is visually distinct (low=yellow, medium=orange, high=red).
- [ ] Error category badge is displayed (transient, quota, invalid_input, etc.).
- [ ] Appropriate action buttons render based on error category:
  - [ ] Retry button for retryable errors
  - [ ] Edit Input button for validation errors
  - [ ] View Billing button for quota errors
- [ ] Technical details are collapsible and hidden by default.
- [ ] Sensitive data (API keys, tokens) is sanitized before display.
- [ ] Error analysis failures gracefully fall back to raw error display.

**Performance:**
- [ ] Initial render completes in <200ms for 50 visible cards.
- [ ] Virtual scrolling maintains 60fps with 1000+ cards.
- [ ] Memory usage stays under 50MB for 1000+ node sessions.
- [ ] SSE processes 100+ events/second without UI lag.
- [ ] Lazy loading fetches chunks on demand without flickering.
- [ ] Card recycling pool reduces DOM creation overhead.
- [ ] Adaptive strategy correctly selects tier based on session size.
- [ ] PerformanceMonitor logs metrics and triggers cleanup at thresholds.

**Other Features:**
- [ ] Checkpoint cards show short summary with full toggle.
- [ ] System prompt is accessible from toolbar.

---

## Changelog
- 2026-01-06: Created
- 2026-01-06: Specified SSE (Server-Sent Events) as streaming transport mechanism with detailed API design
- 2026-01-06: Added detailed tool pair grouping & edge case handling (state machine, timeout thresholds, out-of-order results)
- 2026-01-06: Added AI-enhanced error handling with comprehensive error analysis agent (all error types, severity levels, actionable suggestions)
- 2026-01-06: Added concrete performance targets and comprehensive lazy loading strategy (virtualization, pagination, memory management)
- 2026-01-06: Added state management architecture (single source of truth, event queue, optimistic UI, sync invariants)
