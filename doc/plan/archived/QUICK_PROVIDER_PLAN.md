# Quick Provider Plan

**Status: ✅ COMPLETED**

*Archived: 2026-01-06*

## Overview

Add an optional `quick_provider` to Agent for handling simple internal tasks (e.g., checkpoint summaries) with a cheaper/faster model, while the main provider handles complex reasoning.

## Motivation

Currently, `generate_summary()` uses the same provider as the main conversation. This means:
- Using expensive models (GPT-4o, Claude Opus) for simple summarization tasks
- Unnecessary cost and latency for internal operations

## Design

### Agent Structure Change

```rust
pub struct Agent<P: LLMProvider> {
    pub session: Session,
    provider: P,
    quick_provider: Option<Box<dyn LLMProvider>>,  // NEW
    tools: ToolRegistry,
    config: AgentConfig,
}
```

### API

```rust
// Builder pattern
let agent = Agent::new(session, main_provider, tools)
    .with_quick_provider(quick_provider);
```

### Behavior

1. If `quick_provider` is set, use it for:
   - Checkpoint summary generation
   - Future: context compression, classification, etc.

2. If `quick_provider` is None, fallback to main provider

## Implementation Summary

### Phase 1: Core Changes ✅
- [x] Add `quick_provider: Option<Box<dyn LLMProvider>>` to Agent struct
- [x] Add `with_quick_provider()` builder method
- [x] Add `set_quick_provider()` method for post-construction setting
- [x] Update `generate_summary()` to use quick provider with fallback
- [x] Made `LLMProvider` trait dyn-compatible by changing `update_config` signature

### Phase 2: Testing ✅
- [x] All 76 existing tests pass
- [x] Update `interactive_agent_tree` example to demonstrate usage via QUICK_MODEL env var

### Phase 3: Documentation ✅
- [x] Add doc comments to `with_quick_provider()`
- [x] Update examples/README.md with quick_provider usage

## Usage Example

```rust
use aaagent::agent::Agent;
use aaagent::llm::{OpenAIProvider, GeminiProvider};

// Main provider: powerful model for complex reasoning
let main_provider = OpenAIProvider::create("gpt-4o".into(), api_key)?;

// Quick provider: cheap/fast model for simple tasks  
let quick_provider = OpenAIProvider::create("gpt-4o-mini".into(), api_key)?;

let agent = Agent::new(session, main_provider, tools)
    .with_quick_provider(Box::new(quick_provider));
```

## Key Technical Changes

1. **Made `LLMProvider` trait dyn-compatible**:
   - Changed `update_config(&self, f: impl FnOnce(&mut ProviderConfig))` 
   - To `update_config(&self, f: Box<dyn FnOnce(&mut ProviderConfig) + Send>)`

2. **Added quick_provider to Agent**:
   - Stored as `Option<Box<dyn LLMProvider>>`
   - Used in `generate_summary()` with fallback to main provider

3. **Example supports QUICK_MODEL env var**:
   - `QUICK_MODEL=gpt-4o-mini cargo run --example interactive_agent_tree`

## Trade-offs

### Pros
- Cost reduction for internal operations
- Faster checkpoint creation
- Simple API change, backward compatible

### Cons
- Additional complexity (two providers)
- Need to manage two API keys potentially
- Summary quality may differ between models

## Future Extensions

The quick_provider pattern can be extended for:
- Context compression summaries
- Message classification/routing
- Tool result summarization
- Any internal task that doesn't need full reasoning power
