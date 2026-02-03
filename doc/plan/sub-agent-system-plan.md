# Feature Plan: Sub-Agent System

**Status**: Draft  
**Owner**: Development Team  
**Created**: 2026-02-02  
**Last Updated**: 2026-02-02  
**Target Release**: v0.3.0

---

## Executive Summary

Implement a sub-agent system that enables the main agent to spawn background agents for parallel task execution, long-running operations, and specialized processing. This mirrors the OpenClaw architecture where sub-agents execute independently and report results back to the main agent through a message injection mechanism.

**Key Benefits**:
- 3x speed improvement for parallel tasks
- Non-blocking background operations
- Cost optimization through specialized agents
- Context overflow prevention
- Fault isolation

---

## Goals

- [x] Enable spawning background agents that execute independently
- [x] Implement message injection mechanism (sub-agent → main agent as User message)
- [x] Support concurrent execution with configurable limits (lane system)
- [x] Provide queue mechanism for handling results when main agent is busy
- [x] Ensure sub-agent results are properly integrated into conversation history
- [x] Support cleanup strategies (delete immediately vs keep for debugging)

---

## Non-Goals

- ❌ Multi-process architecture (single-process with tokio for Phase 1)
- ❌ Cross-machine deployment (future enhancement)
- ❌ Nested sub-agents (sub-agents cannot spawn sub-agents)
- ❌ Interactive sub-agents (bidirectional communication during execution)
- ❌ Streaming sub-agent output (Phase 1 - final result only)

---

## Requirements

### Functional Requirements

1. **Sub-Agent Spawning**
   - FR-1.1: Main agent can spawn sub-agents via `spawn_subagent` tool
   - FR-1.2: Each sub-agent gets unique session with isolated context
   - FR-1.3: Sub-agents cannot spawn other sub-agents (nesting prevention)
   - FR-1.4: Support configurable cleanup strategy (delete/keep)
   - FR-1.5: Return immediately with run_id (non-blocking)

2. **Execution Management**
   - FR-2.1: Sub-agents run concurrently with configurable limits (default: 8)
   - FR-2.2: Main agent can continue working while sub-agents execute
   - FR-2.3: Track sub-agent lifecycle (created → started → completed/failed/timeout)
   - FR-2.4: Support timeout configuration per sub-agent
   - FR-2.5: Persist registry to survive process restarts

3. **Result Communication**
   - FR-3.1: Sub-agent results inject as **User messages** (not System messages)
   - FR-3.2: Format announcement with: task label, status, findings, stats
   - FR-3.3: Queue announcements when main agent is busy
   - FR-3.4: Process queued messages in FIFO order after main turn completes
   - FR-3.5: Support multiple queue modes (followup, collect)

4. **Session Management**
   - FR-4.1: SessionManager caches active sessions in memory
   - FR-4.2: Support session persistence to `data/sessions/`
   - FR-4.3: AgentFactory creates agent instances with proper configuration
   - FR-4.4: Each agent instance tracked by unique session_key

### Non-Functional Requirements

1. **Performance**
   - NFR-1.1: Sub-agent spawn latency < 100ms
   - NFR-1.2: Announcement delivery latency < 500ms
   - NFR-1.3: Support 8 concurrent sub-agents without degradation

2. **Reliability**
   - NFR-2.1: Registry survives process restarts (persist to disk)
   - NFR-2.2: Sub-agent failures isolated from main agent
   - NFR-2.3: Queue depth limited to prevent memory exhaustion (max: 100)

3. **Observability**
   - NFR-3.1: Log all lifecycle events (spawn, start, complete, fail)
   - NFR-3.2: Track metrics: active runs, queue depth, completion rate
   - NFR-3.3: Include run_id in all logs for tracing

4. **Security**
   - NFR-4.1: Sub-agents have isolated tool access (cannot access parent tools)
   - NFR-4.2: Cross-agent spawning requires allowlist (future)
   - NFR-4.3: Session keys cryptographically random (UUID v4)

---

## Design

### Architecture Overview

```
Agent (with AgentRuntime)
├── Session (tree-based history)
├── Provider (stateless LLM)
├── ToolRegistry (includes spawn_subagent)
└── AgentRuntime (run tracking + queue)
    ├── active_runs: HashMap<session_key, RunHandle>
    └── message_queues: HashMap<session_key, Vec<QueuedMessage>>

Sub-Agent Lifecycle:
1. spawn_subagent tool called
2. Registry registers run
3. Background task spawned (tokio::spawn)
4. Sub-agent executes with new Session
5. Completion event emitted
6. Announce flow triggered
7. Check main agent status
   ├─ Busy → enqueue message
   └─ Idle → inject immediately (emit InjectMessageEvent)
8. Inject listener receives event
9. Start new agent turn with message as User input
```

### Key Components

#### 1. AgentRuntime (`src/agent/runtime.rs`)

Manages agent run lifecycle and message queuing.

```rust
pub struct AgentRuntime {
    active_runs: Arc<Mutex<HashMap<String, AgentRunHandle>>>,
    message_queues: Arc<Mutex<HashMap<String, Vec<QueuedMessage>>>>,
}

pub struct AgentRunHandle {
    pub session_key: String,
    pub started_at: i64,
    pub is_streaming: bool,
    pub cancel_tx: mpsc::Sender<()>,
}

pub struct QueuedMessage {
    pub content: String,
    pub mode: QueueMode,
    pub source: MessageSource,
}
```

**Key Methods**:
- `register_run()` - Track active agent run
- `unregister_run()` - Remove completed run
- `is_run_active()` - Check if agent is busy
- `enqueue_message()` - Add to queue (returns true if queued)
- `drain_queue()` - Get and clear queued messages

#### 2. SubAgentRegistry (`src/agent/subagent_registry.rs`)

Tracks sub-agent runs with persistence.

```rust
pub struct SubAgentRegistry {
    runs: HashMap<String, SubAgentRun>,
    event_bus: Arc<GlobalEventBus>,
    persistence_path: PathBuf,  // data/subagent_registry.json
}

pub struct SubAgentRun {
    pub run_id: String,
    pub child_session_key: String,
    pub parent_session_key: String,
    pub task_label: String,
    pub cleanup: CleanupStrategy,
    pub created_at: i64,
    pub started_at: Option<i64>,
    pub ended_at: Option<i64>,
    pub outcome: Option<SubAgentOutcome>,
}
```

#### 3. GlobalEventBus (`src/agent/events.rs`)

Event system for lifecycle and injection events.

```rust
pub struct GlobalEventBus {
    lifecycle_tx: broadcast::Sender<SubAgentLifecycleEvent>,
    inject_tx: broadcast::Sender<InjectMessageEvent>,
}

pub enum SubAgentLifecycleEvent {
    Started { run_id, session_key, started_at },
    Completed { run_id, session_key, ended_at, outcome },
    Failed { run_id, error },
}

pub struct InjectMessageEvent {
    pub session_key: String,
    pub message: String,
    pub source: MessageSource,
}
```

#### 4. SessionManager (`src/agent/session_manager.rs`)

Manages session lifecycle and caching.

```rust
pub struct SessionManager {
    sessions: Arc<RwLock<HashMap<String, Arc<RwLock<Session>>>>>,
    store: Arc<dyn TreeStore>,
    default_config: SessionConfig,
}
```

#### 5. AgentFactory (`src/agent/agent_factory.rs`)

Creates agent instances with proper configuration.

```rust
pub struct AgentFactory {
    provider_config: ProviderConfig,
    tool_registry: Arc<ToolRegistry>,
    runtime: Arc<AgentRuntime>,
}
```

### Message Injection Flow

**Critical Design Decision**: Sub-agent results are injected as **User messages**, not System messages.

```
Sub-Agent Completes
    ↓
run_announce_flow()
    ↓
Check: runtime.is_run_active(parent_session_key)?
    ├─ YES → runtime.enqueue_message() → QUEUED
    └─ NO  → send_message_immediately()
              ↓
         emit InjectMessageEvent
              ↓
    Inject Listener (Web API layer)
              ↓
    session_manager.get_or_create(session_key)
              ↓
    agent_factory.create_agent(session)
              ↓
    agent.chat(announcement_text)  ← Starts new turn
              ↓
    session.append_message({ role: User, content: "..." })
```

### Queue Modes

1. **Followup** (Phase 1): Process messages sequentially after current turn
2. **Collect** (Phase 2): Batch multiple messages into one
3. **Steer** (Future): Inject into current turn to guide behavior
4. **Interrupt** (Future): Cancel current turn and process immediately

---

## Milestones

### Milestone 1: Core Infrastructure (Week 1)
**Target**: 2026-02-09

- [ ] AgentRuntime implementation with run tracking
- [ ] Message queue with FIFO ordering
- [ ] GlobalEventBus with lifecycle and inject events
- [ ] Agent integration (register/unregister runs)
- [ ] Basic tests for runtime and queue

**Acceptance Criteria**:
- AC-1.1: Can register and track agent runs
- AC-1.2: Messages queue when agent is active
- AC-1.3: drain_queue() returns messages in FIFO order
- AC-1.4: Unit tests pass (>80% coverage)

### Milestone 2: Spawn Tool + Announce (Week 2)
**Target**: 2026-02-16

- [ ] SpawnSubAgentTool implementation
- [ ] SubAgentRegistry with persistence
- [ ] Announce flow (read output, format, check status)
- [ ] Inject event emission
- [ ] Integration tests (spawn → complete → announce)

**Acceptance Criteria**:
- AC-2.1: spawn_subagent tool returns run_id immediately
- AC-2.2: Sub-agent executes in background
- AC-2.3: Completion triggers announce flow
- AC-2.4: Announcement formatted correctly (task, status, findings, stats)
- AC-2.5: Registry persists to data/subagent_registry.json

### Milestone 3: Session Management (Week 2-3)
**Target**: 2026-02-16

- [ ] SessionManager implementation
- [ ] AgentFactory implementation
- [ ] Inject listener (subscribes to InjectMessageEvent)
- [ ] End-to-end flow (sub-agent → inject → main agent processes)

**Acceptance Criteria**:
- AC-3.1: SessionManager caches sessions in memory
- AC-3.2: AgentFactory creates agents with correct config
- AC-3.3: Inject listener starts new agent turn
- AC-3.4: Message appears as User role in conversation history
- AC-3.5: End-to-end test passes (spawn → complete → inject → process)

### Milestone 4: Queue Processing (Week 3)
**Target**: 2026-02-23

- [ ] Followup mode implementation
- [ ] Collect mode implementation (batch messages)
- [ ] Queue depth limiting (max 100 messages)
- [ ] Queue timeout/expiration

**Acceptance Criteria**:
- AC-4.1: Followup mode processes messages sequentially
- AC-4.2: Collect mode merges messages before processing
- AC-4.3: Queue rejects messages when full (>100)
- AC-4.4: Old messages expire (>5 minutes)

### Milestone 5: Production Readiness (Week 4)
**Target**: 2026-03-02

- [ ] Backend event system (SSE broadcast upgrade)
- [ ] Frontend integration (SSE client enhancement + UI)
- [ ] Configuration (config.yaml)
- [ ] Monitoring and metrics
- [ ] Documentation (API, architecture, examples)
- [ ] All tests passing (unit + integration + e2e)

**Acceptance Criteria**:
- AC-5.1: SSE broadcast channel allows multiple clients to receive same events
- AC-5.2: `GlobalEventBus` emits events with session_id, run_id, seq, timestamp
- AC-5.3: Agent emits lifecycle, assistant, and tool events in real-time
- AC-5.4: SSE handler filters events by session_id (no cross-session leakage)
- AC-5.5: Frontend receives and displays sub-agent notifications (toast/panel)
- AC-5.6: Active sub-agents shown with run_id, task label, elapsed time
- AC-5.7: Message bubbles distinguish user vs sub-agent sources (yellow badge)
- AC-5.8: Sequence number validation detects out-of-order events
- AC-5.9: Configuration loaded from config.yaml (max concurrent, queue limits)
- AC-5.10: Metrics tracked (active runs, queue depth, SSE clients)
- AC-5.11: Documentation complete and reviewed
- AC-5.12: Test coverage >80% for new components
- AC-5.13: End-to-end test: spawn sub-agent → SSE event → UI toast verified
- AC-5.14: Multi-tab test: two browser tabs receive identical events

---

## Tasks

### Phase 1: Core Infrastructure (Week 1)

#### AgentRuntime Implementation
- [ ] Create `src/agent/runtime.rs`
- [ ] Implement `AgentRuntime` struct with HashMap storage
- [ ] Implement `AgentRunHandle` with cancel channel
- [ ] Implement `QueuedMessage` and `QueueMode` enums
- [ ] Implement `MessageSource` enum
- [ ] Method: `register_run()` with mutex locking
- [ ] Method: `unregister_run()` with cleanup
- [ ] Method: `is_run_active()` check
- [ ] Method: `enqueue_message()` with depth limit
- [ ] Method: `drain_queue()` with FIFO ordering
- [ ] Implement `RunGuard` RAII cleanup
- [ ] Unit tests for all methods
- [ ] Documentation with examples

**Owner**: Backend Team  
**Estimate**: 2 days

#### Event System Extension
- [ ] Modify `src/agent/events.rs`
- [ ] Add `InjectMessageEvent` struct
- [ ] Add `GlobalEventBus::inject_tx` channel
- [ ] Implement `emit_inject()` method
- [ ] Implement `subscribe_inject()` method
- [ ] Create global `GLOBAL_EVENT_BUS` singleton
- [ ] Helper function `emit_inject_message_event()`
- [ ] Unit tests for event emission and subscription

**Owner**: Backend Team  
**Estimate**: 1 day

#### Agent Integration
- [ ] Modify `src/agent/mod.rs` Agent struct
- [ ] Add `runtime: Arc<AgentRuntime>` field
- [ ] Add `session_key: String` field
- [ ] Update `Agent::new()` signature
- [ ] Modify `chat_with_callback()` to register run on start
- [ ] Modify `chat_with_callback()` to unregister on end (via RunGuard)
- [ ] Modify `chat_with_callback()` to drain queue before return
- [ ] Add `AgentEvent::QueuedMessagesReceived` variant
- [ ] Integration tests for run lifecycle

**Owner**: Backend Team  
**Estimate**: 2 days

---

### Phase 2: Spawn Tool + Announce (Week 2)

#### SubAgentRegistry
- [ ] Create `src/agent/subagent_registry.rs`
- [ ] Implement `SubAgentRegistry` struct
- [ ] Implement `SubAgentRun` struct
- [ ] Method: `register()` with persistence
- [ ] Method: `persist()` to `data/subagent_registry.json`
- [ ] Method: `restore()` from disk
- [ ] Method: `get_run()` lookup
- [ ] Method: `remove_run()` cleanup
- [ ] Ensure `data/` directory structure created
- [ ] Unit tests for persistence
- [ ] Error handling for I/O failures

**Owner**: Backend Team  
**Estimate**: 2 days

#### SpawnSubAgentTool
- [ ] Create `src/agent/tools/spawn_tool.rs`
- [ ] Implement `SpawnSubAgentTool` struct
- [ ] Validation: prevent nesting (check if current agent is sub-agent)
- [ ] Generate unique child_session_key (UUID v4)
- [ ] Register run in SubAgentRegistry
- [ ] Spawn background task (tokio::spawn)
- [ ] Acquire lane permit (concurrency control)
- [ ] Create new Session for sub-agent
- [ ] Execute agent.chat() with task
- [ ] Emit lifecycle events (Started, Completed/Failed)
- [ ] Return SpawnResult immediately (non-blocking)
- [ ] Error handling and logging
- [ ] Integration tests

**Owner**: Backend Team  
**Estimate**: 3 days

#### Announce Flow
- [ ] Create `src/agent/announce.rs`
- [ ] Implement `run_announce_flow()`
- [ ] Read sub-agent output (latest assistant message)
- [ ] Build stats (runtime, tokens, cost estimate)
- [ ] Format announcement message
- [ ] Check parent agent status via runtime
- [ ] Branch: if busy → enqueue, if idle → send immediately
- [ ] Implement `send_message_immediately()` with event emission
- [ ] Implement cleanup logic (delete/keep based on strategy)
- [ ] Error handling and retries
- [ ] Unit tests for formatting and flow
- [ ] Integration tests for end-to-end announce

**Owner**: Backend Team  
**Estimate**: 2 days

---

### Phase 3: Session Management (Week 2-3)

#### SessionManager
- [ ] Create `src/agent/session_manager.rs`
- [ ] Implement `SessionManager` struct
- [ ] In-memory cache with RwLock
- [ ] Method: `get_or_create()` with caching
- [ ] Method: `remove()` for cleanup
- [ ] Method: `persist()` to `data/sessions/`
- [ ] Method: `restore()` from disk
- [ ] Session expiration/eviction (future)
- [ ] Unit tests for caching logic
- [ ] Integration tests with storage

**Owner**: Backend Team  
**Estimate**: 2 days

#### AgentFactory
- [ ] Create `src/agent/agent_factory.rs`
- [ ] Implement `AgentFactory` struct
- [ ] Store provider config, tools, runtime
- [ ] Method: `create_agent()` with session
- [ ] Method: `create_provider()` based on config
- [ ] Support quick_provider configuration
- [ ] Unit tests for agent creation
- [ ] Integration tests with real providers

**Owner**: Backend Team  
**Estimate**: 1 day

#### Inject Listener
- [ ] Create `src/api/inject_listener.rs`
- [ ] Implement `start_inject_listener()` function
- [ ] Subscribe to InjectMessageEvent
- [ ] Get session from SessionManager
- [ ] Create agent from AgentFactory
- [ ] Call agent.chat() with injected message
- [ ] Handle responses (WebSocket/SSE push)
- [ ] Error handling and logging
- [ ] Integration tests with mock events

**Owner**: Backend Team  
**Estimate**: 2 days

#### Main Integration
- [ ] Modify `src/main.rs`
- [ ] Create AgentRuntime singleton
- [ ] Create SessionManager
- [ ] Create AgentFactory
- [ ] Start inject listener on startup
- [ ] Pass runtime to all agents
- [ ] Update API routes to use SessionManager
- [ ] End-to-end smoke tests

**Owner**: Backend Team  
**Estimate**: 1 day

---

### Phase 4: Queue Processing (Week 3)

#### Followup Mode
- [ ] Implement queue draining in Agent
- [ ] Process messages sequentially
- [ ] Start new turn for each message
- [ ] Prevent infinite loops (max depth: 10)
- [ ] Tests for sequential processing

**Owner**: Backend Team  
**Estimate**: 1 day

#### Collect Mode
- [ ] Implement message batching logic
- [ ] Format merged message with separators
- [ ] Process once with combined content
- [ ] Tests for batching

**Owner**: Backend Team  
**Estimate**: 1 day

#### Queue Management
- [ ] Implement depth limit (max 100)
- [ ] Implement message expiration (5 minutes)
- [ ] Log warnings when queue is full
- [ ] Metrics for queue depth
- [ ] Tests for limits and expiration

**Owner**: Backend Team  
**Estimate**: 1 day

---

### Phase 5: Production Readiness (Week 4)

> **Architecture Decision**: Reuse existing SSE (Server-Sent Events) infrastructure instead of WebSocket.
> Current codebase already has production-ready SSE with auto-reconnection, event routing, and frontend hooks.
> Only need to upgrade from single-consumer (`mpsc`) to broadcast (`broadcast::channel`) for multi-client support.

#### Backend Event System (SSE Broadcast Upgrade)

**Core Infrastructure** (`src/api/event_bus.rs` - NEW)
- [ ] Create `GlobalEventBus` struct
  - [ ] Use `broadcast::Sender<AgentEventEnvelope>` (supports N subscribers)
  - [ ] Implement `new()` with 1000-event buffer
  - [ ] Implement `subscribe() -> broadcast::Receiver` (one per SSE connection)
  - [ ] Implement `emit(session_id, run_id, event)` with auto-sequencing
  - [ ] Add global sequence tracking (`AtomicU64`)
  - [ ] Add per-run sequence tracking (`DashMap<run_id, u32>`)
  - [ ] Add timestamp generation (`chrono::Utc::now()`)
  - [ ] Unit tests for subscribe/emit/sequencing

- [ ] Define `AgentEventEnvelope` struct
  - [ ] Field: `session_id: String` (for filtering)
  - [ ] Field: `run_id: String` ("main-agent" or "subagent-xxx")
  - [ ] Field: `seq: u64` (global sequence number)
  - [ ] Field: `ts: i64` (Unix timestamp in milliseconds)
  - [ ] Field: `event: AgentEvent` (reuse existing enum from `src/agent/mod.rs:122`)
  - [ ] Implement `Serialize` for JSON conversion
  - [ ] Unit tests for serialization

**Modify Existing Components**

- [ ] Update `src/api/mod.rs` - AppState
  - [ ] Add `event_bus: Arc<GlobalEventBus>` field
  - [ ] Initialize in `AppState::new()`
  - [ ] Remove dependency on `stream_manager` for new SSE connections (keep for backward compat)

- [ ] Update `src/api/mod.rs` - SSE handler (mod sse)
  - [ ] Change signature: accept `session_id` from path param
  - [ ] Replace `stream_manager.take_stream()` with `event_bus.subscribe()`
  - [ ] Use `BroadcastStream::new(rx)` instead of `ReceiverStream`
  - [ ] Add session filter: `.filter_map(|envelope| if envelope.session_id == session_id)`
  - [ ] Update event mapping to include `run_id`, `seq`, `ts` in SSE data
  - [ ] Handle `RecvError::Lagged` (slow consumer warning)
  - [ ] Integration test: verify multiple SSE connections receive same events

- [ ] Update `src/agent/mod.rs` - Agent struct
  - [ ] Add optional `event_bus: Option<Arc<GlobalEventBus>>` field
  - [ ] Add `set_event_bus(&mut self, bus: Arc<GlobalEventBus>)` method
  - [ ] Modify `chat()` to emit events via `event_bus.emit()` if present
  - [ ] Emit at key points:
    - [ ] Before LLM call: `emit(session_id, run_id, Content("..."))`
    - [ ] After tool call: `emit(session_id, run_id, ToolResult {...})`
    - [ ] On completion: `emit(session_id, run_id, Done {...})`
  - [ ] Ensure `run_id` is passed through from `spawn_sub_agent()` context
  - [ ] Keep backward compat: if `event_bus.is_none()`, use old `mpsc` sender

**Delta Throttling (Optional - P2)**
- [ ] Create `src/api/delta_throttler.rs`
  - [ ] Implement `DeltaThrottler` struct
  - [ ] Track last_sent timestamp per run_id (`DashMap<run_id, SystemTime>`)
  - [ ] Track accumulated deltas per run_id (`DashMap<run_id, String>`)
  - [ ] Method: `should_send(run_id, delta) -> Option<String>`
  - [ ] Enforce 150ms minimum interval
  - [ ] Return accumulated text if interval elapsed
  - [ ] Return None if throttled (caller buffers the delta)
  - [ ] Unit tests for throttling behavior

- [ ] Integrate throttler in Agent
  - [ ] Add `delta_throttler: Arc<DeltaThrottler>` to Agent
  - [ ] Before emitting `Content(delta)`, check `should_send()`
  - [ ] If throttled, skip emit (throttler buffers internally)
  - [ ] On final `Done`, flush any remaining buffered deltas

#### Frontend UI Implementation

**Upgrade Existing SSE Hook** (`web/src/hooks/useSSEStream.ts`)
- [ ] Parse new event envelope format:
  - [ ] Extract `run_id` from event data
  - [ ] Extract `seq` (global sequence number)
  - [ ] Extract `ts` (timestamp)
  - [ ] Extract `data` (existing AgentEvent payload)
- [ ] Add sequence validation
  - [ ] Track `lastSeq` per `run_id` (`Map<run_id, number>`)
  - [ ] Warn if `seq <= lastSeq` (out-of-order or duplicate)
  - [ ] Warn if gap detected (`seq > lastSeq + 1`)
- [ ] Add multi-run state tracking
  - [ ] State: `activeRuns: Map<run_id, RunState>`
  - [ ] `RunState`: `{ phase, lastActivity, accumulatedText }`
  - [ ] Update `activeRuns` on each event
  - [ ] Remove completed runs after 60s (configurable)

**Agent Event State Management** (`web/src/stores/subAgentStore.ts` - NEW)
- [ ] Create Zustand store (or Context API)
  - [ ] State: `activeRuns: Map<run_id, SubAgentInfo>`
  - [ ] `SubAgentInfo`: `{ taskLabel, startTime, phase, toolCalls, errors }`
  - [ ] Actions: `addRun()`, `updateRun()`, `completeRun()`, `pruneOldRuns()`
- [ ] Subscribe to SSE events via `useSSEStream`
  - [ ] On `ToolCallsRequested`: add to `activeRuns[run_id].toolCalls`
  - [ ] On `ToolResult`: mark tool as completed
  - [ ] On `Done`: mark run as completed
  - [ ] On `LoopDetected`: add warning to run
- [ ] Expose hook: `useSubAgents() -> { activeRuns, completedRuns }`

**UI Components** (`web/src/components/agent/` - NEW)

- [ ] `SubAgentNotificationToast.tsx`
  - [ ] Use daisyUI `toast` component
  - [ ] Show on sub-agent lifecycle events (start, end, error)
  - [ ] Display: task label, status icon, brief summary
  - [ ] Auto-dismiss after 10s (use `setTimeout`)
  - [ ] Click to navigate to sub-agent details (future)
  - [ ] Accessibility: ARIA live region for screen readers

- [ ] `ActiveSubAgentPanel.tsx`
  - [ ] Sidebar or modal showing active sub-agents
  - [ ] List items: run_id, task label, elapsed time (use `ts`)
  - [ ] Loading spinner for `phase === 'running'`
  - [ ] Completed status badge (✓ success, ✗ error)
  - [ ] Collapsible tool execution list
  - [ ] Empty state: "No active sub-agents"

- [ ] `MessageBubble.tsx` enhancement
  - [ ] Add prop: `sourceRunId?: string`
  - [ ] If `sourceRunId !== 'main-agent'`, show badge "Sub-Agent Result"
  - [ ] Badge styling: yellow accent (`bg-yellow-500 text-black`)
  - [ ] Tooltip on badge: show task label from `subAgentStore`

- [ ] `ToolExecutionTimeline.tsx` (Optional - P2)
  - [ ] Horizontal timeline visualization
  - [ ] Each tool call as a segment (start → end)
  - [ ] Hover to show tool name, duration, result preview
  - [ ] Click to expand full input/output
  - [ ] Error highlighting (red segment for failed tools)

**Styling & Tests**
- [ ] Add daisyUI toast styles (already available in daisyUI)
- [ ] Add custom styles for sub-agent badge (`web/src/index.css`)
- [ ] Add loading spinner keyframes (use Tailwind `animate-spin`)
- [ ] Write tests (`web/src/__tests__/subAgentStore.test.ts`)
  - [ ] Mock SSE events with MSW (Mock Service Worker)
  - [ ] Test state updates on lifecycle events
  - [ ] Test sequence validation logic
  - [ ] Test run pruning after timeout
- [ ] Write component tests with React Testing Library
  - [ ] Test toast rendering and auto-dismiss
  - [ ] Test active runs panel display
  - [ ] Test message badge appearance

**Owner**: Backend Team (Event System) + Frontend Team (UI)  
**Estimate**: 2-3 days (reduced from 4-5 days by reusing existing SSE infrastructure)

#### Configuration
- [ ] Add `agent.runtime` section to config.yaml
- [ ] Configure max concurrent sub-agents (default: 8)
- [ ] Configure queue depth limit (default: 100)
- [ ] Configure queue timeout (default: 300s)
- [ ] Configure default cleanup strategy
- [ ] Documentation for configuration

**Owner**: DevOps Team  
**Estimate**: 0.5 days

#### Monitoring
- [ ] Add metrics: active_runs count
- [ ] Add metrics: queue_depth per session
- [ ] Add metrics: completion_rate (ok/error/timeout)
- [ ] Add metrics: announce_success_rate
- [ ] Log all lifecycle transitions
- [ ] Log queue operations
- [ ] Grafana dashboard (if applicable)

**Owner**: DevOps Team  
**Estimate**: 1 day

#### Documentation
- [ ] API documentation for spawn_subagent tool
- [ ] Architecture documentation (message injection)
- [ ] Usage examples (5 scenarios)
- [ ] Configuration guide
- [ ] Troubleshooting guide
- [ ] Update main README

**Owner**: Docs Team  
**Estimate**: 2 days

#### Testing
- [ ] Complete unit test coverage (>80%)
- [ ] Integration tests for all flows
- [ ] End-to-end tests (user → spawn → complete → inject)
- [ ] Load tests (8 concurrent sub-agents)
- [ ] Error scenario tests (timeout, failure, crash)
- [ ] CI/CD integration

**Owner**: QA Team  
**Estimate**: 3 days

---

## Risks and Mitigations

### Technical Risks

| Risk | Impact | Probability | Mitigation |
|------|--------|-------------|------------|
| Queue grows unbounded causing OOM | High | Medium | Implement depth limit (100), message expiration (5min), monitoring alerts |
| Sub-agent blocks main agent | High | Low | Use dedicated tokio tasks, lane-based concurrency, timeout enforcement |
| Registry corruption on crash | Medium | Low | Atomic writes with temp files, backup before persist, restore validation |
| Infinite loop in queue processing | Medium | Low | Max depth limit (10), loop detector in Agent, circuit breaker |
| Session cache memory leak | Medium | Low | Implement LRU eviction, periodic cleanup, monitoring |
| Race condition in run tracking | Medium | Low | Use Mutex/RwLock, atomic operations, comprehensive locking tests |

### Dependency Risks

| Dependency | Risk | Mitigation |
|------------|------|------------|
| Tokio runtime | Task spawning limits | Monitor task count, implement backpressure |
| Disk I/O | Persistence failures | Retry logic, fallback to in-memory, logging |
| Broadcast channels | Event loss | Increase buffer size (1000), monitor dropped events |

### Schedule Risks

- **Risk**: Complex integration takes longer than estimated (Week 3-4)
- **Mitigation**: Start integration early, incremental testing, daily standups

---

## Dependencies

### Internal Dependencies

- `src/history/session.rs` - Session tree structure
- `src/llm/provider.rs` - LLM provider trait
- `src/llm/registry.rs` - Tool registry
- `src/agent/mod.rs` - Agent implementation
- `src/api/*` - Web API layer

### External Dependencies

**Backend (Rust)**
- `tokio` (≥1.35) - ✅ Already in project - Async runtime for background tasks
- `serde` (≥1.0) - ✅ Already in project - Serialization for persistence
- `uuid` (≥1.6) - ✅ Already in project - Session key generation (using ULID in code)
- `log` (≥0.4) - ✅ Already in project - Logging
- `axum` (≥0.7) - ✅ Already in project - Web framework with SSE support
- `dashmap` (≥5.5) - **NEW** - Thread-safe HashMap for sequence tracking and run state
- `chrono` (≥0.4) - ✅ Already in project - Timestamp generation
- `tokio-stream` (≥0.1) - ✅ Already in project - Stream utilities (BroadcastStream)

**Frontend (TypeScript/React)**
- React 18 - ✅ Already in project - UI framework
- EventSource API - ✅ Native browser API - SSE client (no additional dependency)
- React Testing Library - ✅ Already in project - Component testing
- MSW (Mock Service Worker) - Optional - SSE mocking for tests (already in devDependencies)
- Zustand (≥4.4) - **NEW** (or use React Context API) - State management for sub-agent tracking

### Data Dependencies

- `data/subagent_registry.json` - Registry persistence
- `data/sessions/` - Session storage
- `config.yaml` - Runtime configuration

---

## Testing Strategy

### Unit Tests

**Target Coverage**: >80%

- AgentRuntime: run tracking, queue operations
- SubAgentRegistry: persistence, lookup, cleanup
- SpawnSubAgentTool: validation, spawning logic
- Announce flow: formatting, status checks
- SessionManager: caching, persistence
- AgentFactory: agent creation

### Integration Tests

- Spawn → Execute → Complete flow
- Announce → Inject → Process flow
- Queue when busy, process when idle
- Registry persistence and restore
- Error scenarios (timeout, failure)

### End-to-End Tests

- User chat → spawn sub-agent → receive notification
- Multiple sub-agents completing concurrently
- Main agent busy during sub-agent completion
- Sub-agent failure handling

### Performance Tests

- 8 concurrent sub-agents (baseline)
- Queue with 100 messages
- Session cache with 1000 sessions
- Registry with 10000 completed runs

### Acceptance Tests

See Acceptance Criteria in Milestones section.

---

## Acceptance Criteria

### Overall Acceptance

- [ ] All milestones completed
- [ ] All tasks checked off
- [ ] Test coverage >85%
- [ ] Documentation complete and reviewed
- [ ] No critical bugs (severity 1-2)
- [ ] Performance benchmarks met
- [ ] Code review approved
- [ ] Security review passed

### Feature Verification

- [ ] Can spawn sub-agent and get immediate run_id
- [ ] Sub-agent executes in background without blocking
- [ ] Completion triggers announcement to main agent
- [ ] Announcement appears as User message in history
- [ ] Main agent can respond naturally to announcement
- [ ] Queue handles messages when main agent is busy
- [ ] Registry survives process restart
- [ ] Cleanup strategies work (delete/keep)
- [ ] Concurrent sub-agents respect lane limits
- [ ] Frontend displays notifications correctly

---

## WebSocket Event Format

### Message Frame Structure

All WebSocket messages sent to clients follow this format:

```json
{
  "type": "event",
  "event": "agent",
  "seq": 1234,
  "payload": { ... }
}
```

### AgentEventPayload Schema

```rust
pub struct AgentEventPayload {
    pub run_id: String,           // Unique run identifier
    pub seq: u32,                 // Per-run sequence number (monotonic)
    pub ts: i64,                  // Unix timestamp (milliseconds)
    pub stream: AgentEventStream, // Event stream type
    pub session_key: Option<String>, // Parent session (for filtering)
    pub data: serde_json::Value,  // Stream-specific data
}

pub enum AgentEventStream {
    Lifecycle,  // start, end, error
    Assistant,  // text deltas
    Tool,       // tool execution
    Error,      // errors
}
```

### Event Examples

**Lifecycle: Sub-Agent Start**
```json
{
  "type": "event",
  "event": "agent",
  "seq": 100,
  "payload": {
    "run_id": "subagent-abc123",
    "seq": 1,
    "ts": 1738483200000,
    "stream": "lifecycle",
    "session_key": "agent:main",
    "data": {
      "phase": "start",
      "started_at": 1738483200000
    }
  }
}
```

**Tool Execution**
```json
{
  "type": "event",
  "event": "agent",
  "seq": 101,
  "payload": {
    "run_id": "subagent-abc123",
    "seq": 2,
    "ts": 1738483201000,
    "stream": "tool",
    "session_key": "agent:main",
    "data": {
      "name": "Grep",
      "phase": "start",
      "input": { "pattern": "TODO", "path": "src/" }
    }
  }
}
```

**Assistant Text Delta (Streaming)**
```json
{
  "type": "event",
  "event": "agent",
  "seq": 102,
  "payload": {
    "run_id": "subagent-abc123",
    "seq": 3,
    "ts": 1738483202000,
    "stream": "assistant",
    "session_key": "agent:main",
    "data": {
      "text": "Found 5 files with TODO comments...",
      "delta": true
    }
  }
}
```

**Lifecycle: Completion**
```json
{
  "type": "event",
  "event": "agent",
  "seq": 110,
  "payload": {
    "run_id": "subagent-abc123",
    "seq": 10,
    "ts": 1738483260000,
    "stream": "lifecycle",
    "session_key": "agent:main",
    "data": {
      "phase": "end",
      "ended_at": 1738483260000,
      "outcome": {
        "status": "ok"
      }
    }
  }
}
```

**Error Event**
```json
{
  "type": "event",
  "event": "agent",
  "seq": 105,
  "payload": {
    "run_id": "subagent-abc123",
    "seq": 5,
    "ts": 1738483205000,
    "stream": "error",
    "session_key": "agent:main",
    "data": {
      "message": "Tool execution failed: timeout after 30s",
      "kind": "timeout",
      "tool_name": "Grep"
    }
  }
}
```

### Client-Side Event Handling

```typescript
// WebSocket connection
const ws = new WebSocket('ws://localhost:3000/api/ws');

ws.onmessage = (event) => {
  const message = JSON.parse(event.data);
  
  if (message.type === 'event' && message.event === 'agent') {
    const payload = message.payload as AgentEventPayload;
    
    switch (payload.stream) {
      case 'lifecycle':
        handleLifecycleEvent(payload);
        break;
      case 'assistant':
        handleAssistantDelta(payload);
        break;
      case 'tool':
        handleToolEvent(payload);
        break;
      case 'error':
        handleErrorEvent(payload);
        break;
    }
  }
};
```

---

## Changelog

### 2026-02-02 (Update 3 - Architecture Pivot)
- **Major Architecture Change: WebSocket → SSE (Server-Sent Events)**
  - **Rationale**: Discovered existing production-ready SSE infrastructure in codebase
    - `src/api/stream_manager.rs` - Stream management with mpsc channels
    - `src/api/mod.rs:1639-1773` - SSE handler with axum
    - `web/src/hooks/useSSEStream.ts` - Frontend SSE hook with auto-reconnect
    - `src/agent/mod.rs:122` - Complete AgentEvent enum (7 event types)
  - **Key Insight**: Only need to upgrade from `mpsc` (single-consumer) to `broadcast` (multi-consumer)
  - **Impact**: Reduced Phase 5 estimate from 4-5 days to 2-3 days (40% faster)
- **Revised Phase 5: Backend Event System (SSE Broadcast Upgrade)**
  - Replace WebSocket infrastructure with SSE broadcast approach
  - New component: `GlobalEventBus` with `broadcast::channel` (supports N subscribers)
  - New component: `AgentEventEnvelope` wrapper (session_id, run_id, seq, ts, event)
  - Modify existing SSE handler to use `event_bus.subscribe()` + session filtering
  - Keep backward compatibility with existing Main Agent chat flow
- **Revised Phase 5: Frontend UI Implementation**
  - Upgrade existing `useSSEStream` hook (add seq validation, multi-run tracking)
  - New store: `subAgentStore.ts` (Zustand or Context API)
  - New components: `SubAgentNotificationToast`, `ActiveSubAgentPanel`, enhanced `MessageBubble`
  - Delta throttling moved to Optional (P2) - not critical for MVP
- **Updated External Dependencies**
  - Removed: `tokio-tungstenite` (WebSocket library - not needed)
  - Added: `dashmap` (thread-safe HashMap for sequence tracking)
  - Added: `zustand` (optional - can use React Context API)
  - Marked existing dependencies: ✅ Already in project (tokio, axum, serde, etc.)
- **Documentation Updates**
  - Created `temp-doc/existing-event-infrastructure-analysis.md` - Full codebase analysis
  - Created `temp-doc/event-system-reuse-recommendations.md` - Implementation guide
  - Removed WebSocket Event Format section (replaced with SSE envelope format in docs)
- **Benefits of SSE Approach**
  - ✅ Reuses 80% of existing code (minimal changes)
  - ✅ Lower risk (proven SSE infrastructure already in production)
  - ✅ Simpler implementation (HTTP-based, auto-reconnect built-in)
  - ✅ Faster delivery (2-3 days vs 5-7 days for WebSocket rewrite)

### 2026-02-02 (Update 2)
- **Expanded Phase 5 Frontend Integration** based on OpenClaw architecture
  - Added detailed Backend Event System tasks (events.rs, websocket.rs, agent_event_handler.rs)
  - Added detailed Frontend UI Implementation tasks (WebSocket client, state management, UI components)
  - Increased estimate from 2 days to 4-5 days (more realistic)
  - Added 60+ new subtasks with specific file paths and implementation details
- **Updated Milestone 5 Acceptance Criteria**
  - Expanded from 5 to 12 criteria
  - Added WebSocket-specific criteria (delta throttling, slow consumer handling)
  - Added end-to-end verification requirement
- **Updated External Dependencies**
  - Added Rust WebSocket dependencies (axum, tokio-tungstenite, dashmap, chrono)
  - Added Frontend testing dependencies (MSW for WebSocket mocking)
- **Added WebSocket Event Format section**
  - Documented message frame structure
  - Added AgentEventPayload schema
  - Included 5 event examples (lifecycle, tool, assistant, error)
  - Added TypeScript client-side handling example
- **References**: Based on OpenClaw implementation (`src/infra/agent-events.ts`, `src/gateway/server-broadcast.ts`)

### 2026-02-02 (Initial)
- Initial feature plan created
- Defined 5 milestones over 4 weeks
- Identified 140+ tasks across 5 phases
- Documented architecture and design decisions
- Added comprehensive testing strategy
- Risk assessment completed

---

## References

### Internal Documentation

- `temp-doc/sub_agent_analysis.md` - Architecture comparison and benefits
- `temp-doc/sub_agent_injection_mechanism.md` - Detailed injection flow
- `temp-doc/sub_agent_implementation_checklist.md` - Implementation checklist
- `E:\code\openclaw\docs\SUB_AGENT_SYSTEM.md` - OpenClaw reference architecture

### Code References

- OpenClaw announce: `src/agents/subagent-announce.ts:126-145`
- OpenClaw queue: `src/agents/subagent-announce.ts:179-199`
- OpenClaw runner: `src/agents/pi-embedded-runner/runs.ts:21-38`

### External Resources

- Tokio async/await: https://tokio.rs/
- Rust concurrency patterns: https://doc.rust-lang.org/book/ch16-00-concurrency.html

---

## Notes

### Design Rationale

**Why User message instead of System message?**
- Follows OpenClaw pattern (`deliver: true` triggers agent run with user role)
- Allows main agent to respond naturally in conversation
- Maintains clear message boundaries in history
- System messages reserved for instructions, not dynamic content

**Why event-based injection instead of direct append?**
- Decouples sub-agent from main agent execution
- Allows queue management and flow control
- Easier to test (mock event bus)
- Extensible to multi-process architecture later

**Why single-process for Phase 1?**
- Simpler implementation (no RPC, no serialization overhead)
- Faster (in-memory communication)
- Sufficient for current workload
- Can extend to multi-process with Redis pub-sub if needed

### Future Enhancements

- Streaming sub-agent output (real-time progress updates)
- Interactive sub-agents (bidirectional communication)
- Steer and Interrupt queue modes
- Cross-agent spawning with allowlist
- Multi-process deployment with gRPC
- Sub-agent priority levels
- Resource quotas per user/session
