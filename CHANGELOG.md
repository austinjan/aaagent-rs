# Changelog

## [Unreleased]

### Added

#### LLM Provider System
- **OpenAI Provider** with full tool calling support
  - Streaming chat completions
  - Tool call delta assembly (handles index-based deltas)
  - Bidirectional chat loop with tool execution
  - Token usage tracking across multiple rounds
  - Support for gpt-5, gpt-4o, and o1 models

#### Helper API
- **`chat_loop_with_tools`** - High-level helper function for AI agents
  - Automatic tool execution via registered executors
  - Callback system for content, tool calls, and thinking
  - Built-in error handling and round limiting
  - Reduces boilerplate by ~70% compared to manual API

#### Tools
- **BashTool** - Execute shell commands
  - Cross-platform (Windows cmd / Unix sh)
  - Configurable timeout
  - Working directory support
  - stdout/stderr capture

#### Type System
- `LLMProvider` trait - Unified interface for all LLM providers
- `ChatLoopHandle` - Bidirectional communication for tool execution
- `LoopStep` enum - Events during chat loop (Thinking, Content, ToolCallsRequested, etc.)
- `Tool`, `ToolCall`, `ToolResult` - Tool calling types
- `ToolCallAssembler` - Helper for assembling streaming tool calls
- `TokenUsage` with `total()` method

### Changed
- Simplified all examples to use the helper API
- Reduced example count from 7 to 3 (removed duplicates)
- Updated documentation with clear API comparison

### Examples

Now includes 3 focused examples:

1. **`simple_agent.rs`** ⭐ - Recommended starting point
   - Shows helper API usage
   - BashTool integration
   - ~40 lines of code

2. **`openai_tools.rs`** - Manual API for learning
   - Low-level event loop
   - Educational purposes

3. **`openai_basic.rs`** - Basic chat without tools
   - Simple streaming example

### Documentation
- Added `examples/README.md` with quick start guide
- Added comparison between helper and manual APIs
- Documented when to use each approach

## Implementation Notes

### OpenAI Tool Calling
- Fixed critical issue with tool call delta assembly
- OpenAI sends first delta with ID, subsequent deltas with only index
- Implemented index-to-ID mapping to correctly assemble all deltas
- Handles tool results properly by saving completed tool calls

### API Design Philosophy
- **Helper API**: For 99% of use cases - simple, clean, automatic
- **Manual API**: For advanced customization - full control, more code

### Code Metrics
- Helper API examples: ~40 lines
- Manual API examples: ~150 lines
- **Reduction: ~73% less code** with helper API
