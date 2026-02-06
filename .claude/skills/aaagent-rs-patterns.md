---
name: aaagent-rs-patterns
description: Coding patterns extracted from aaagent-rs repository
version: 1.0.0
source: local-git-analysis
analyzed_commits: 200
repository: aaagent-rs
contributors: austin, Austin Jan, austinjan, Claude
files_analyzed: 138 (Rust + TypeScript/React)
---

# aaagent-rs Development Patterns

**Context**: This skill captures the development patterns, conventions, and workflows used in the aaagent-rs project - a unified LLM provider abstraction library with tree-based conversation history and agent orchestration.

## Commit Conventions

This project uses **mixed commit styles** with a preference for descriptive commits:

### Conventional Commits (18% of commits)
When using conventional commits, prefer:
- `feat:` - New features (most common)
- `fix:` - Bug fixes
- `docs:` - Documentation updates
- `chore:` - Maintenance tasks
- `refactor:` - Code restructuring
- `test:` - Testing changes

### Descriptive Commits (82% of commits)
The project primarily uses highly descriptive commit messages without prefixes:
- Start with verb: "Add", "Fix", "Merge", "Improve", "Update"
- Be specific: "Add checkpoint creation UI and improve tree visualization"
- Reference affected components: "Fix minimap ID mismatch by sending full node data in SSE done event"

**Pattern**: When adding features, commit messages often follow this structure:
```
Add [feature name] and [related changes]
```
Examples:
- "Add session list sidebar and session ID copy button in chat UI"
- "Add Zustand chat store for centralized state management in UI"
- "Add JSONLStore for append-only session and node storage"

## Architecture Patterns

### Module Organization

```
src/
├── agent/              # High-level Agent API and orchestration
│   ├── mod.rs         # Agent struct, chat loop
│   ├── agent_factory.rs  # AgentFactory for creating agents
│   ├── announce.rs    # Sub-agent announcement system
│   └── runtime.rs     # AgentRuntime, SubAgentRegistry
├── llm/               # LLM provider implementations
│   ├── provider.rs    # LLMProvider trait (stateless)
│   ├── openai.rs      # OpenAI provider
│   ├── anthropic.rs   # Anthropic/Claude provider
│   ├── gemini.rs      # Google Gemini provider
│   ├── registry.rs    # Tool registry and execution
│   ├── loop_detector.rs  # Loop detection
│   └── helpers.rs     # Common utilities
├── history/           # Tree-based conversation history
│   ├── session.rs     # Session API with branching
│   ├── node.rs        # Tree node implementation
│   ├── storage.rs     # Storage trait
│   ├── compressor.rs  # Tool result compression
│   └── validator.rs   # Tree integrity validation
├── tools/             # Built-in tools (Bash, Editor, Read, etc.)
├── config/            # Configuration management
├── api/               # REST API with axum
└── web/               # Embedded frontend (rust-embed)

web/src/
├── components/        # React components
│   ├── chat/         # Chat UI components
│   └── tree/         # Tree visualization components
├── hooks/            # Custom React hooks
├── services/         # API client
├── store/            # Zustand state management
├── types/            # TypeScript types
└── pages/            # Top-level page components
```

### Import Patterns

**Use local crate imports**, not absolute paths:

```rust
// PREFERRED
use crate::agent::{Agent, AgentRuntime};
use crate::history::{Session, TreeStore};
use crate::llm::{LLMProvider, ToolRegistry};

// AVOID
use aaagent::agent::Agent;  // Don't use the crate name
```

**Pattern observed**: After refactoring, commits explicitly fix this:
- "Use local crate imports in API module instead of aaagent paths"

### Stateless Providers, Stateful Sessions

**Critical architectural principle**:

1. **Providers are stateless** - `LLMProvider` trait takes linear message history and returns responses
2. **Sessions are stateful** - `Session` manages tree structure, branching, context optimization
3. **Agent combines both** - Uses Session for tree operations, Provider for LLM calls

```rust
// Providers don't store history
trait LLMProvider {
    async fn chat(&self, messages: Vec<Message>) -> Result<Response>;
}

// Sessions manage the tree
impl Session {
    pub async fn add_user_message(&mut self, content: String) -> NodeId;
    pub async fn get_context(&self) -> Vec<Message>;
}

// Agent orchestrates
impl Agent {
    pub async fn chat(&mut self, user_msg: String) -> Result<String> {
        self.session.add_user_message(user_msg).await?;
        let context = self.session.get_context().await?;
        let response = self.provider.chat(context).await?;
        self.session.add_assistant_message(response).await?;
    }
}
```

## File Change Patterns

### High-Frequency Files (Iterative Development)

When implementing features, these files are modified together most often:

**Backend Core Loop** (21 changes):
```
src/api/mod.rs → Core API orchestration
src/agent/mod.rs → Agent implementation
src/history/session.rs → Session management
src/llm/provider.rs → Provider interface
```

**Frontend Chat UI** (15+ changes):
```
web/src/pages/Chat.tsx → Main chat page
web/src/hooks/useChat.ts → Chat logic hook
web/src/components/chat/MessageCard.tsx → Message rendering
web/src/services/api.ts → API client
web/src/types/backend.ts → Type definitions
```

**Tree Visualization** (7+ changes):
```
web/src/components/tree/TreeNavigationPanel.tsx
web/src/components/tree/TreeNode.tsx
web/src/components/tree/treeLayout.ts
```

**Pattern**: When adding a feature, changes typically span:
1. Backend API (`src/api/mod.rs`)
2. Frontend page (`web/src/pages/Chat.tsx`)
3. Frontend hook (`web/src/hooks/useChat.ts`)
4. Type definitions (`web/src/types/backend.ts`)

## Workflow Patterns

### Feature Implementation Workflow

**Pattern observed from commit history**:

1. **Create Feature Plan** (`doc/plan/`)
   - Write detailed plan document
   - Archive when complete

2. **Backend First**
   - Add to `src/api/mod.rs` (API endpoint)
   - Add to `src/agent/mod.rs` or `src/llm/` (core logic)
   - Add to `src/history/` if storage needed

3. **Frontend Integration**
   - Update types (`web/src/types/backend.ts`)
   - Update API client (`web/src/services/api.ts`)
   - Add hook (`web/src/hooks/use*.ts`)
   - Update UI component (`web/src/components/` or `web/src/pages/`)

4. **Archive Plan**
   - Move completed plan to `doc/plan/archived/`

**Example from history**:
```
feat: Add branching and checkpoint UI
├── Backend: src/history/session.rs (checkpoint logic)
├── API: src/api/mod.rs (checkpoint endpoints)
├── Types: web/src/types/backend.ts (checkpoint types)
├── UI: web/src/components/ (checkpoint components)
└── Archive: doc/plan/archived/chat-ui-sse-streaming.md
```

### LLM Provider Implementation Pattern

When adding a new LLM provider:

1. Create `src/llm/{provider_name}.rs`
2. Implement `LLMProvider` trait
3. Handle:
   - SSE/streaming parsing
   - Tool calls (parallel execution)
   - Token usage tracking
4. Add feature flag to `Cargo.toml`
5. Register in `src/api/provider_factory.rs`

**Pattern observed**:
- "Add Anthropic provider implementation with streaming support"
- "feat: Add Gemini provider"
- "feat: Add web search grounding for Gemini"

### Tool Implementation Pattern

When adding a new tool:

1. Create tool in `src/tools/{tool_name}.rs`
2. Implement `Tool` trait or similar
3. Register in `src/tools/mod.rs`
4. Register in `src/llm/registry.rs`

**Pattern observed**:
- "Add ReadTool for large file reading with chunking and search"
- "Add EditorEditTool for literal text file editing"
- "Add dynamic tool registry with pick_tools support and logging"

### Storage Implementation Pattern

When implementing storage:

1. Define trait in `src/history/storage.rs`
2. Implement concrete store (MemoryStore, JSONLStore, etc.)
3. Add to `TreeStore` enum
4. Initialize in agent factory

**Pattern observed**:
- "Add JSONLStore for append-only session and node storage"
- "Initialize MemoryStore with disk persistence"

### Configuration Pattern

When adding configuration:

1. Add to `src/config/` module
2. Add API endpoints in `src/api/mod.rs`
3. Add frontend panel in `web/src/components/`
4. Store in `config.yaml` (working directory) or `data/` (persistent)

**Pattern observed**:
- "Add session config management API and frontend support"
- "Add comprehensive chat UI config panels and temporary overrides"
- "feat: add per-skill configuration support"

## Data Directory Convention

**CRITICAL**: All persistent data MUST go in `data/` directory:

```
project_root/
├── config.yaml        # User config (working directory)
├── secrets.yaml       # API keys (working directory, gitignored)
└── data/              # All persistent data (gitignored)
    ├── sessions/      # Session JSON files
    ├── nodes/         # Tree node storage
    ├── archived/      # Archived sessions
    └── audit/         # Logs (future)
```

**Rules**:
- ✅ `data/` for ALL persistent files
- ✅ `testing/` for test files
- ✅ `temp-doc/` for temporary docs
- ❌ NEVER write data to project root

## Testing Patterns

### Backend Testing

**Pattern**: 41 Rust test modules with inline tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_feature() {
        // Use MemoryStore for in-memory testing
        let store = MemoryStore::new();
        let session = Session::new(store, config);

        // Test async operations
        let result = session.operation().await;
        assert!(result.is_ok());
    }
}
```

**Common test patterns**:
- Use `MemoryStore` for in-memory history
- Mock providers for testing Agent without real API calls
- Async tests with `#[tokio::test]`

### Frontend Testing

**Pattern**: Minimal frontend tests currently

**Observed patterns**:
- "Add dev-mode state validation and sync tests for chat store"
- Focus on Zustand store testing
- No extensive component tests yet

## Development Tools

### Python Development Script

**Pattern**: Use `develop.py` for development workflow

```bash
# Start both frontend dev server (5173) + backend (3000)
python develop.py start

# Rebuild/restart backend only (keeps frontend running)
python develop.py restart

# Stop both
python develop.py stop
```

**Observed in commits**:
- "Fix develop.py to use correct binary name (aaagent-serve)"
- Backend runs with `--features dev-server` for CORS

### CLI Tools

**Pattern**: Modern Rust CLI tools (from CLAUDE.md)

- `rg` instead of `grep`
- `fd` instead of `find`
- `bat` instead of `cat`
- `xh` instead of `curl`

### Binary Structure

**Pattern**: Multiple binaries in `src/bin/`

- `serve.rs` - Web server binary
- Other task-specific binaries

**Observed**:
- "Move km binary to src/bin and remove unused Serve command"
- "feat: separate serve functionality into aaagent-serve binary"

## Frontend Styling

**Pattern**: Tailwind v4 CSS-first with daisyUI

```css
/* web/src/index.css */
@import "tailwindcss";
@config "../tailwind.config.js";
```

**Theme**: BlackBear TechHive
- Yellow: `#E8C236`
- Black: `#000000`

**Pattern observed**:
- "Add frontend Tailwind v4 styling guidelines and improve UI consistency"
- "feat(brand): Add BlackBear TechHive brand guidelines skill"

## State Management

**Pattern**: Zustand for React state

```typescript
// web/src/store/useChatStore.ts
export const useChatStore = create<ChatState>((set) => ({
  messages: [],
  addMessage: (msg) => set((state) => ({
    messages: [...state.messages, msg]
  })),
}));
```

**Observed**:
- "Add Zustand chat store for centralized state management in UI"
- "Add dev-mode state validation and sync tests for chat store"

## SSE Streaming Pattern

**Pattern**: Server-Sent Events for real-time updates

1. Backend emits SSE events via `GlobalEventBus`
2. Frontend hooks connect via `useSSEStream`
3. Auto-reconnection with exponential backoff

**Observed**:
- "feat: Implement global event bus for SSE"
- "Add auto-reconnection with exponential backoff to SSE hook"
- "Add initial React chat UI components and SSE streaming support"

## Skills Framework

**Pattern**: Built-in skills system

1. Skills stored in `.claude/skills/`
2. Per-skill configuration support
3. Skills registered in agent runtime

**Observed**:
- "Add skills framework and update API/agent/web UI"
- "feat: add per-skill configuration support"
- "Add spec-ui-component skill for generating UI component specifications"
- "Add managing-feature-plans skill with plan template and archiving script"

## Context Optimization

**Pattern**: Two-tier context management

### Compression
- Long tool results → summaries
- Full content stored in `archived_tool_results`
- LLM can call `recall_tool_result` to retrieve

### Checkpoints
- Conversation segments → checkpoint summaries
- Auto-checkpointing based on turns or tokens
- Checkpoint nodes in tree structure

**Observed**:
- "Render checkpoint summaries as distinct cards in chat timeline"
- "Add checkpoint creation UI and improve tree visualization"

## Sub-Agent System

**Pattern**: Tree-based agent orchestration

1. `AgentRuntime` manages sub-agent lifecycle
2. `SubAgentRegistry` tracks active agents
3. `GlobalEventBus` for event propagation
4. `SpawnSubAgentTool` for spawning

**Observed**:
- "feat: Implement Agent Runtime and Sub-Agent System"
- "feat: Enhance tool executor registration and output handling in chat loop"
- Updates to `examples/interactive_agent_tree.rs` for sub-agent events

## Documentation Pattern

### Feature Plans

**Pattern**: `doc/plan/` for active plans, `doc/plan/archived/` for completed

1. Create plan: `doc/plan/{feature-name}.md`
2. Work on implementation
3. Archive: `doc/plan/archived/{feature-name}.md`

**Observed commits**:
- "archived finished plan"
- "Archive completed chat-ui-sse-streaming plan document"
- "docs: Create SKILL_IMPLEMENTATION_PLAN.md."

### README Pattern

**Pattern**: Comprehensive CLAUDE.md for AI assistance

- Project overview
- Architecture details
- Common commands
- Important implementation details
- Project conventions

**Observed**:
- "docs: simplify README with configuration guide"
- CLAUDE.md contains extensive guidance

## Merge Strategy

**Pattern**: Feature branches merged via PRs

```
18 merge commits out of 200 (9%)
```

**Branch naming**:
- `feature/branch-navigation`
- `ui-state-management`
- `agent-skills`
- `web-search`
- `feature/checkpoint`

**Pattern observed**:
- "Merge pull request #11 from austinjan/agent-skills"
- "Merge pull request #10 from austinjan/web-search"
- "Merge remote-tracking branch 'origin/master' into feature/branch-navigation"

## Error Handling Pattern

**Pattern**: `anyhow::Result` for application-level errors

```rust
use anyhow::{Context, Result};

pub async fn operation() -> Result<T> {
    let result = risky_operation()
        .await
        .context("Failed to perform operation")?;
    Ok(result)
}
```

## Async Runtime

**Pattern**: Tokio with full features

```rust
#[tokio::main]
async fn main() -> Result<()> {
    // All operations are async
    let session = Session::new(store, config).await?;
    Ok(())
}
```

**All LLM calls and Session operations are async**.

## Logging Pattern

**Pattern**: Simple file-based logger

```rust
use aaagent::logger::log;

log("Operation completed successfully");
```

**Current**: Logs to `app.log` (to be moved to `data/audit/`)

## Summary

### When Implementing New Features:

1. ✅ Create feature plan in `doc/plan/`
2. ✅ Use descriptive commit messages ("Add X and Y")
3. ✅ Modify high-frequency files together (api/mod.rs, Chat.tsx, useChat.ts, backend.ts)
4. ✅ Use local crate imports (`crate::` not `aaagent::`)
5. ✅ Store data in `data/` directory
6. ✅ Use `python develop.py restart` for backend changes
7. ✅ Archive plan when complete
8. ✅ Keep providers stateless, sessions stateful
9. ✅ Use async/await with tokio
10. ✅ Add tests with `#[cfg(test)]` and `MemoryStore`

### Project Philosophy:

- **Tree-based history** over linear conversation
- **Stateless providers** with **stateful sessions**
- **Context optimization** via compression + checkpoints
- **Feature plans** for complex work
- **Skills framework** for reusable AI patterns
- **SSE streaming** for real-time updates
- **Sub-agent orchestration** for complex tasks

---

**Generated by**: /skill-create (local git analysis)
**Analysis Date**: 2026-02-05
**Commits Analyzed**: 200
**Files Tracked**: 138 source files
**Contributors**: austin, Austin Jan, austinjan, Claude
