# Chat UI Feature Plan (Master)

- Feature name: `chat-ui`
- Status: Draft
- Created: 2026-01-06
- Last updated: 2026-01-08

## 1) Overview

### Goal
Deliver a web-based chat UI that visualizes the conversation history tree path with real-time streaming, tool execution tracking, AI-enhanced error handling, and performance optimization for long sessions.

### Architecture
**Single Binary + Embedded Frontend**: Rust backend embeds compiled React frontend, serving both API and UI from one executable.

**Frontend:** Vite + React 18 + TypeScript + shadcn/ui + Tailwind CSS  
**Backend:** Rust + axum + rust-embed  
**Theme:** BlackBear TechHive (Yellow #E8C236, Black #000000)

### Scope (In)
- Web UI with chat container and minimal mini map
- Real-time SSE streaming from backend
- Tool call/result pairing with state tracking
- AI-enhanced error analysis and suggestions
- Performance optimization (virtualization, lazy loading)
- State management with Zustand

### Non-goals (Out)
- Full tree branch visualization
- Provider configuration UI
- Multi-session management UI
- Bidirectional WebSocket communication

## 2) Feature Breakdown

This master plan is broken down into focused sub-plans:

### 2.1) Configuration System ([chat-ui-config.md](./chat-ui-config.md))
**Status**: ✅ Implemented (API keys pending)  
**Scope**: Intent-first API configuration, presets, API key management

**Key Topics**:
- Intent over parameters (creativity, verbosity, rounds)
- 4 presets: general, coding, research, quick
- Temperature profiles with model-specific handling
- API key management - see [api-key-management.md](./api-key-management.md)
- Session metadata for resolved config
- Immutable system prompts

**Deliverables**:
- ✅ Config module with types, presets, resolver
- ✅ ConfigManager with config.yaml loading
- ✅ POST `/api/sessions/{id}/chat` with config support
- ✅ Session metadata integration
- [ ] API key management system ([api-key-management.md](./api-key-management.md))
- [ ] Frontend config panel

### 2.2) Foundation ([chat-ui-foundation.md](./chat-ui-foundation.md))
**Status**: Draft  
**Scope**: Architecture, tech stack, build process, deployment

**Key Topics**:
- Single binary + embedded frontend approach
- Vite + React + TypeScript + shadcn/ui + Zustand
- Development workflow (two terminals: Rust + Vite)
- Production build (one executable with embedded assets)
- rust-embed for asset bundling
- axum router with SPA fallback

**Deliverables**:
- Project structure (`src/web/`, `src/api/`, `web/`)
- Build scripts and feature flags
- Development proxy configuration
- Embedded asset serving

### 2.2) SSE Streaming ([chat-ui-sse-streaming.md](./chat-ui-sse-streaming.md))
**Status**: Draft  
**Scope**: Server-Sent Events transport, event types, reconnection strategy

**Key Topics**:
- Two-step SSE pattern (POST to initiate, GET to stream)
- Direct mapping from `AgentEvent` to SSE events
- Event types: content, thinking, tool_calls_requested, tool_result, checkpoint_created, done, error
- EventSource client implementation
- Automatic reconnection with exponential backoff
- Event IDs for replay on reconnect

**Deliverables**:
- `POST /api/sessions/{id}/chat` endpoint
- `GET /api/sessions/{id}/stream/{stream_id}` SSE endpoint
- Frontend `ChatSSEClient` class
- Event handlers for all types

### 2.3) Tool Pair Management ([chat-ui-tool-pairs.md](./chat-ui-tool-pairs.md))
**Status**: Draft  
**Scope**: Tool call/result pairing, state machine, timeout handling

**Key Topics**:
- State machine: pending (0-10s) → slow (10-60s) → orphaned (>60s)
- Tool pair grouping by assistant turn
- Timeout timers (10s slow warning, 60s orphaned error)
- Out-of-order result handling (match by `tool_call_id`)
- Collapsible UI component with summary badges

**Deliverables**:
- `ToolPairTracker` class with timers
- `ToolPairCard` and `ToolPairItem` components
- State transitions (pending/slow/orphaned/complete/error)
- Summary calculations (total/complete/pending/errors)

### 2.4) AI Error Handling ([chat-ui-error-handling.md](./chat-ui-error-handling.md))
**Status**: Draft  
**Scope**: Error analysis agent, user-friendly explanations, actionable suggestions

**Key Topics**:
- Error analysis agent using quick provider
- Comprehensive error context capture (type, component, details)
- Sensitive data sanitization (API keys, tokens, passwords)
- AI-generated plain English explanations + 2-3 specific suggestions
- Severity-based styling (low=yellow, medium=orange, high=red)
- Category-based action buttons (Retry, Edit Input, View Billing)

**Deliverables**:
- `ErrorAnalyzer` backend struct
- `AgentEvent::ErrorAnalyzed` event type
- `ErrorCard` frontend component
- Sanitization functions
- Error analysis caching

### 2.5) Performance & Lazy Loading ([chat-ui-performance.md](./chat-ui-performance.md))
**Status**: Draft  
**Scope**: Virtualization, pagination, memory management

**Key Topics**:
- Performance targets: <200ms initial render, 60fps scroll, <50MB memory
- 4-tier adaptive strategy by session size (<100, 100-500, 500-1000, 1000+)
- Progressive lazy loading (metadata → viewport → chunks on scroll)
- Virtual scrolling for 100+ cards
- Card recycling pool for memory efficiency
- Backend path caching and pagination API

**Deliverables**:
- Paginated path endpoint (`/api/sessions/{id}/path?limit&offset`)
- Metadata endpoint (`/api/sessions/{id}/path/metadata`)
- `VirtualizedChatContainer` class
- `VirtualScrollManager` class
- `CardPool` for DOM recycling
- `PerformanceMonitor` for metrics

### 2.6) State Management ([chat-ui-state-management.md](./chat-ui-state-management.md))
**Status**: Draft  
**Scope**: Zustand store, event queue, sync patterns

**Key Topics**:
- Single source of truth for `selectedNodeId`
- Server-authoritative data (conversation nodes)
- Client-authoritative UI (expand/collapse, scroll)
- Optimistic updates for user actions
- Event queue with FIFO ordering and yield to UI
- Synchronization invariants (selection sync, no duplicate nodes, tool pair consistency)

**Deliverables**:
- Zustand `ChatStore` with full state structure
- Event queue with sequence numbers
- Optimistic action creators
- Server-authoritative action creators
- State validation functions (dev mode)

## 3) User Stories

- As a user, I can read the conversation as cards in order
- As a user, I can see tool calls and results grouped together
- As a user, I can collapse/expand tool pair cards
- As a user, I can see when tools are slow or stuck (timeout warnings)
- As a user, I get plain English error explanations instead of technical jargon
- As a user, I get specific suggestions for fixing errors
- As a user, I can navigate long histories smoothly (virtualization)
- As a user, I can click the mini map to jump to nodes
- As a user, I see checkpoints with summaries
- As a user, I can view the system prompt

## 4) Implementation Phases

### Phase 1: Foundation (Milestone 1)
**Goal**: Get basic infrastructure running  
**Duration**: 1-2 weeks

**Tasks** (see [chat-ui-foundation.md](./chat-ui-foundation.md)):
- [ ] Set up project structure (web/, src/web/, src/api/)
- [ ] Configure Vite + React + TypeScript + shadcn/ui
- [ ] Implement rust-embed asset serving
- [ ] Set up axum router with SPA fallback
- [ ] Test development workflow (two terminals)
- [ ] Test production build (single binary)

**Acceptance Criteria**:
- `cargo build --release` produces single binary
- Binary serves UI at `http://localhost:3000/`
- Hot reload works in development
- shadcn/ui components styled with BlackBear colors

### Phase 2: SSE Streaming (Milestone 2)
**Goal**: Real-time data flow from backend to frontend  
**Duration**: 1-2 weeks

**Tasks** (see [chat-ui-sse-streaming.md](./chat-ui-sse-streaming.md)):
- [ ] Implement POST `/api/sessions/{id}/chat` endpoint
- [ ] Implement GET `/api/sessions/{id}/stream/{stream_id}` SSE endpoint
- [ ] Map all `AgentEvent` types to SSE events
- [ ] Build `ChatSSEClient` frontend class
- [ ] Add event handlers for all types
- [ ] Implement reconnection with exponential backoff

**Acceptance Criteria**:
- SSE stream delivers all event types
- Frontend receives events in real-time (<100ms)
- Reconnection works after disconnect
- No events lost during reconnection

### Phase 3: Core UI Components (Milestone 3)
**Goal**: Display conversation with tool pairs and errors  
**Duration**: 2-3 weeks

**Tasks** (see tool-pairs + error-handling plans):
- [ ] Build `MessageCard` component
- [ ] Build `ToolPairCard` with state machine (pending/slow/orphaned)
- [ ] Implement timeout timers (10s/60s)
- [ ] Build `ErrorCard` with AI analysis integration
- [ ] Implement `ErrorAnalyzer` backend
- [ ] Build `CheckpointCard` component
- [ ] Build mini map component (minimalist design)

**Acceptance Criteria**:
- Tool pairs collapse/expand correctly
- Timeout states show appropriate warnings
- Error cards display AI explanations + suggestions
- Mini map syncs selection with chat container

### Phase 4: State Management (Milestone 4)
**Goal**: Consistent state across components and streaming  
**Duration**: 1 week

**Tasks** (see [chat-ui-state-management.md](./chat-ui-state-management.md)):
- [ ] Create Zustand store with full state structure
- [ ] Implement event queue with ordering
- [ ] Add optimistic actions (expand/collapse, select, scroll)
- [ ] Add server-authoritative actions (add node, update node)
- [ ] Integrate SSE handlers with store
- [ ] Add state validation (dev mode)

**Acceptance Criteria**:
- Single source of truth for selection
- Selection syncs between mini map and chat
- Event queue processes in order
- Optimistic UI updates instantly
- No lost SSE events

### Phase 5: Performance Optimization (Milestone 5)
**Goal**: Handle 1000+ node sessions smoothly  
**Duration**: 2 weeks

**Tasks** (see [chat-ui-performance.md](./chat-ui-performance.md)):
- [ ] Implement backend pagination API
- [ ] Add path caching to `Session`
- [ ] Build `VirtualizedChatContainer`
- [ ] Build `VirtualScrollManager`
- [ ] Implement 4-tier adaptive strategy
- [ ] Add card recycling pool
- [ ] Implement `PerformanceMonitor`

**Acceptance Criteria**:
- Initial render <200ms
- 60fps during scroll
- Memory <50MB for 1000+ nodes
- Virtual scrolling at 100+ cards

## 5) Testing Strategy

### Backend Tests
- Unit tests for each API endpoint
- SSE event mapping tests
- Path caching performance tests
- Error analysis prompt generation tests
- State validation tests

### Frontend Tests
- Component tests (MessageCard, ToolPairCard, ErrorCard)
- State management tests (Zustand store, event queue)
- Integration tests (SSE → state → UI)
- Performance tests (render time, memory, FPS)

### E2E Tests
- Full conversation flow (send message → stream → tool calls → done)
- Error handling flow (error → analysis → display → retry)
- Scroll performance with 500+ cards
- Reconnection after disconnect

## 6) Risks & Mitigations

**Risk: Large histories cause slow rendering**
- Mitigation: 4-tier adaptive lazy loading strategy (see performance plan)
- Mitigation: Virtual scrolling at 100+ cards
- Mitigation: Backend path caching

**Risk: Event stream races with UI state**
- Mitigation: Event queue with FIFO ordering (see state management plan)
- Mitigation: Single source of truth pattern

**Risk: Memory leaks in long sessions**
- Mitigation: Card recycling pool
- Mitigation: PerformanceMonitor triggers cleanup at 50MB threshold

**Risk: Complex build process**
- Mitigation: Automated build script (build.rs)
- Mitigation: Clear documentation for development workflow

## 7) Rollout Plan

**Phase 1: Internal Testing**
- Feature flag: `chat_ui_enabled` toggle
- Test with sample sessions (short, medium, long histories)
- Verify all event types work
- Check error handling with real errors

**Phase 2: Gradual Rollout**
- Enable per environment (dev → staging → production)
- Monitor performance metrics
- Collect user feedback

**Phase 3: Rollback (if needed)**
- Disable feature flag
- Fall back to CLI-only mode

## 8) Acceptance Criteria (Master)

### Foundation
- [ ] Single binary serves both API and UI
- [ ] Development mode has hot reload
- [ ] Production build completes successfully

### Streaming
- [ ] All `AgentEvent` types stream to frontend
- [ ] <100ms latency from backend to frontend
- [ ] Automatic reconnection works

### UI Components
- [ ] Cards render for all node types
- [ ] Tool pairs collapse/expand
- [ ] Timeout warnings show (slow, orphaned)
- [ ] Error cards show AI explanations
- [ ] Mini map syncs with chat container

### State Management
- [ ] Single source of truth for selection
- [ ] Event queue processes in order
- [ ] No lost events
- [ ] State validation passes

### Performance
- [ ] <200ms initial render
- [ ] 60fps scroll
- [ ] <50MB memory for 1000+ nodes
- [ ] Virtualization at 100+ cards

## 9) Success Metrics

**User Experience**:
- Time to first meaningful render: <200ms
- Scroll smoothness: 60fps maintained
- Error comprehension: 80%+ users understand error explanations
- Task completion: Users successfully retry/fix errors 70%+ of the time

**Technical Performance**:
- Memory usage: <50MB for 1000+ node sessions
- SSE throughput: 100+ events/sec without UI lag
- Backend response time: <50ms for path queries
- Cache hit rate: >90% for repeated requests

**Reliability**:
- Reconnection success rate: >95%
- Event loss rate: 0%
- UI freeze incidents: 0 per session

---

## References

### Sub-Plans (Detailed Implementation)
1. [chat-ui-foundation.md](./chat-ui-foundation.md) - Architecture & build system
2. [chat-ui-sse-streaming.md](./chat-ui-sse-streaming.md) - Real-time data transport
3. [chat-ui-tool-pairs.md](./chat-ui-tool-pairs.md) - Tool execution tracking
4. [chat-ui-error-handling.md](./chat-ui-error-handling.md) - AI-enhanced errors
5. [chat-ui-performance.md](./chat-ui-performance.md) - Optimization & lazy loading
6. [chat-ui-state-management.md](./chat-ui-state-management.md) - State & sync patterns

### Code References
- `src/agent/mod.rs` - AgentEvent definitions
- `src/history/session.rs` - Session & history management
- `src/history/node.rs` - Node data structures

### Related Plans
- [TREE_MESSAGE_MODEL_PLAN.md](./TREE_MESSAGE_MODEL_PLAN.md) - Tree history architecture

---

## Changelog
- 2026-01-06: Created master plan
- 2026-01-06: Added single binary + embedded frontend architecture
- 2026-01-06: Broke down into 6 focused sub-plans for maintainability
- 2026-01-08: Updated from daisyUI to shadcn/ui component library
