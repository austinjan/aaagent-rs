# Feature Plan: aaagent-core-features

## Overview

Core features needed to make aaagent-rs a production-ready LLM agent framework. This plan tracks the remaining work identified in the conversation flow analysis.

## Scope

- High/Medium priority features from conversation-description.md
- Skills system enhancements
- Provider improvements

## References

- [Conversation Description](../conversation-description.md) - Architecture overview and feature priorities
- [LLM Implementation Status](./achieved/LLM_IMPLEMENTATION_STATUS.md) - Completed provider work

---

## TODO

### High Priority

- [ ] **Auto Compact** - Automatic conversation compaction when approaching context window limits
  - Trigger compaction based on token count threshold
  - Use provider's compact() method
  - Configurable threshold in ChatLoopConfig

- [ ] **Basic Telemetry** - Token usage and cost tracking
  - Accumulate token usage across requests
  - Cost estimation based on model pricing
  - Expose via ProviderState or dedicated telemetry struct

### Medium Priority

- [ ] **Approval Callback** - Let caller approve/reject dangerous tool operations
  - Add `on_tool_approval` callback to ChatLoopConfig
  - Return bool to allow/deny execution
  - Configurable list of tools requiring approval

- [ ] **History Persistence** - Save and restore conversation sessions
  - Serialize Message history to JSON/file
  - Load history on startup
  - Session ID management

### Low Priority

- [ ] **Gemini Provider** - Complete implementation
  - Manual reqwest + SSE parsing
  - Tool calling support
  - Context caching

- [ ] **Anthropic Enhancements**
  - Prompt caching
  - Extended thinking mode (budget_tokens)
  - compact() implementation

- [ ] **MCP Support** - Model Context Protocol integration
  - Tool discovery from MCP servers
  - MCP message routing

- [ ] **Sandbox Execution** - Secure tool execution environment
  - Container/WASM isolation for bash tool
  - Resource limits

---

## DONE

### Skills System (2024-01)
- [x] SkillsManager - Load and cache skills by cwd
- [x] SkillMetadata - Skill name, description, path, scope
- [x] SkillInjection - XML formatting for model context
- [x] Skill discovery from project/user/system directories
- [x] `/skill:name` syntax parsing (explicit mode)
- [x] `invoke_skill` tool (implicit mode)
- [x] `on_skill_injected` callback
- [x] `on_skill_warning` callback
- [x] Example skills (code-review, rust-expert)

### Rate Limiting (2024-01)
- [x] RateLimitConfig with exponential backoff
- [x] RetryState tracking
- [x] Retry parsing for Gemini/OpenAI/Anthropic errors
- [x] `with_retry` async helper
- [x] `on_rate_limit_retry` callback in ChatLoopConfig
- [x] Integration with chat_loop_with_tools

### Tool Improvements (2024-01)
- [x] Fixed editor__Edit schema (removed oneOf for OpenAI compatibility)
- [x] InvokeSkillTool for implicit skill invocation

### Interactive Agent (2024-01)
- [x] Skill loading message display
- [x] Rate limit retry display with formatted box
- [x] Implicit skills mode enabled

---

## Acceptance Criteria

- [ ] All high priority features implemented and tested
- [ ] Medium priority features implemented
- [ ] Examples updated to demonstrate new features
- [ ] Documentation updated

---

## Notes

- Rate limiting/retry was listed as medium priority but completed early due to immediate need
- Skills system was not in original plan but added as key capability
