# Gemini Provider Implementation Plan

## Feature Name
Google Gemini LLM Provider Integration

## Status
📋 **TODO** - Not Started

## Priority
🔴 **HIGH** - Core functionality for multi-provider support

---

## Objective

Implement a complete Google Gemini provider for the `llm` module, providing feature parity with OpenAI and Anthropic providers including streaming, tool calling, and history management.

---

## Background

The current `aaagent-rs` project has fully implemented OpenAI and Anthropic providers. Adding Gemini support will:
- Enable multi-provider flexibility
- Provide access to Google's AI models (Gemini Pro, Gemini Flash, etc.)
- Complete the "big three" LLM provider coverage
- Allow users to choose based on cost, performance, or regional availability

---

## Requirements

### Functional Requirements

1. **Core Chat Interface**
   - Implement `LLMProvider` trait (`src/llm/provider.rs`)
   - Support `chat()` method with streaming
   - Support `chat_loop()` with bidirectional communication
   - Return `Stream<StreamChunk>` for progressive output

2. **Tool Calling (Function Calling)**
   - Support Gemini's Function Calling API
   - Parallel tool execution
   - Tool call/result history tracking
   - Automatic tool turn pruning (configurable via `max_tool_turns`)

3. **History Management**
   - Implement `get_history()` - retrieve conversation history
   - Automatic tool turn pruning to prevent token overflow
   - Implement `compact()` if Gemini supports history compression (TBD)

4. **Streaming Support**
   - Manual SSE (Server-Sent Events) parsing with `reqwest`
   - Progressive content delivery via `StreamChunk`
   - Handle partial JSON for tool calls

5. **Configuration**
   - `ProviderConfig` integration (temperature, max_tokens, etc.)
   - Model selection (gemini-pro, gemini-flash, etc.)
   - API key management via environment variable `GEMINI_API_KEY`
   - Thread-safe state with `Arc<RwLock<ProviderState>>`

6. **Context Caching**
   - Investigate and implement Gemini's context caching feature
   - Reduce costs for repetitive prompts
   - Expose cache configuration in `ProviderConfig`

### Non-Functional Requirements

1. **Error Handling**
   - Use `ProviderError` enum for all errors
   - Handle rate limits, API errors, network failures
   - Provide clear error messages with context

2. **Performance**
   - Async/await throughout
   - Efficient SSE parsing
   - Minimal memory overhead for streaming

3. **Testing**
   - Unit tests for core logic
   - Integration tests with mock HTTP responses
   - Tool calling scenario tests
   - Streaming tests

4. **Documentation**
   - API documentation (rustdoc)
   - Usage examples
   - Model compatibility notes

---

## Technical Design

### Implementation Approach

#### 1. Manual Implementation with `reqwest`

**Rationale**: Follow the same pattern as OpenAI and Anthropic providers
- Full control over HTTP requests and response parsing
- Consistent error handling across providers
- No dependency on third-party Gemini SDK

```rust
// src/llm/gemini.rs

use reqwest::Client;
use tokio_stream::Stream;
use crate::llm::provider::*;

pub struct GeminiProvider {
    client: Client,
    config: Arc<RwLock<ProviderConfig>>,
    state: Arc<RwLock<ProviderState>>,
    api_key: String,
}

impl GeminiProvider {
    pub fn new(api_key: String, config: ProviderConfig) -> Self {
        // Initialize provider
    }
    
    pub fn from_env(config: ProviderConfig) -> Result<Self, ProviderError> {
        let api_key = std::env::var("GEMINI_API_KEY")
            .map_err(|_| ProviderError::Configuration("GEMINI_API_KEY not set".into()))?;
        Ok(Self::new(api_key, config))
    }
}
```

#### 2. Gemini API Endpoints

- **Base URL**: `https://generativelanguage.googleapis.com/v1beta/`
- **Chat endpoint**: `models/{model}:generateContent`
- **Streaming endpoint**: `models/{model}:streamGenerateContent`
- **Authentication**: API key via query parameter `?key={api_key}` or header

#### 3. Request/Response Format

**Request structure:**
```json
{
  "contents": [
    {
      "role": "user",
      "parts": [{"text": "Hello"}]
    }
  ],
  "generationConfig": {
    "temperature": 0.7,
    "maxOutputTokens": 1000,
    "topP": 0.9,
    "topK": 40
  },
  "tools": [
    {
      "functionDeclarations": [
        {
          "name": "get_weather",
          "description": "Get weather for a location",
          "parameters": {
            "type": "object",
            "properties": {
              "location": {"type": "string"}
            },
            "required": ["location"]
          }
        }
      ]
    }
  ]
}
```

**Streaming response (SSE):**
```
data: {"candidates":[{"content":{"parts":[{"text":"Hello"}]}}]}

data: {"candidates":[{"content":{"parts":[{"functionCall":{"name":"get_weather","args":{"location":"SF"}}}]}}]}
```

#### 4. Message Conversion

Gemini uses a different message structure than OpenAI/Anthropic:

```rust
fn convert_message_to_gemini(msg: &Message) -> GeminiContent {
    match msg.role {
        Role::User => GeminiContent {
            role: "user",
            parts: vec![GeminiPart::Text(msg.content.clone())],
        },
        Role::Assistant => GeminiContent {
            role: "model", // Gemini uses "model" instead of "assistant"
            parts: /* handle text + tool calls */,
        },
        Role::System => {
            // Gemini doesn't have system role - prepend to first user message
            // or use systemInstruction field (v1beta API)
        }
    }
}
```

#### 5. SSE Parsing

```rust
use futures::stream::StreamExt;

async fn parse_sse_stream(response: reqwest::Response) 
    -> impl Stream<Item = Result<StreamChunk, ProviderError>> 
{
    let stream = response.bytes_stream();
    
    stream
        .map(|chunk| {
            // Parse SSE format: "data: {json}\n\n"
            // Handle partial JSON for tool calls
            // Convert to StreamChunk
        })
        .filter_map(|result| /* filter empty chunks */)
}
```

#### 6. Tool Calling Flow

1. User sends message
2. LLM responds with `functionCall` in parts
3. Provider converts to `ToolCall` structs
4. Executor runs tools in parallel
5. Provider sends tool results as new message with role="function"
6. LLM processes results and responds

**Gemini tool result format:**
```json
{
  "contents": [
    {
      "role": "function",
      "parts": [
        {
          "functionResponse": {
            "name": "get_weather",
            "response": {
              "temperature": 72,
              "conditions": "sunny"
            }
          }
        }
      ]
    }
  ]
}
```

#### 7. Context Caching (Advanced)

Gemini supports context caching via:
- `cachedContent` field in request
- Separate cache management API
- TTL-based expiration

**Implementation:**
```rust
pub struct GeminiCacheConfig {
    pub enabled: bool,
    pub ttl_seconds: u64,
    pub cache_id: Option<String>,
}

impl GeminiProvider {
    pub async fn create_cache(&self, contents: Vec<GeminiContent>) 
        -> Result<String, ProviderError> {
        // POST to /cachedContents
        // Return cache ID
    }
    
    pub async fn use_cache(&self, cache_id: &str) 
        -> Result<(), ProviderError> {
        // Set cache_id in next request
    }
}
```

---

## Implementation Tasks

### Phase 1: Core Infrastructure (Est. 4-6 hours)

- [ ] Create `src/llm/gemini.rs` module
- [ ] Define `GeminiProvider` struct with state management
- [ ] Implement `new()` and `from_env()` constructors
- [ ] Add Gemini feature flag to `Cargo.toml`
- [ ] Define Gemini-specific types (GeminiContent, GeminiPart, etc.)
- [ ] Implement message conversion (`Message` → `GeminiContent`)
- [ ] Handle system message mapping (prepend or systemInstruction)

### Phase 2: Basic Chat (Est. 3-4 hours)

- [ ] Implement non-streaming `chat()` method
- [ ] Build HTTP request with `reqwest`
- [ ] Parse JSON response
- [ ] Extract content from response
- [ ] Convert to `Message` struct
- [ ] Update token usage in `ProviderState`
- [ ] Write unit tests for message conversion

### Phase 3: Streaming Support (Est. 4-6 hours)

- [ ] Implement SSE stream parsing
- [ ] Handle `data:` prefix and JSON parsing
- [ ] Convert Gemini events to `StreamChunk`
- [ ] Handle partial content accumulation
- [ ] Implement `chat()` with streaming return type
- [ ] Test streaming with mock SSE responses
- [ ] Handle stream interruption and errors

### Phase 4: Tool Calling (Est. 6-8 hours)

- [ ] Implement tool schema conversion (`Tool` → `functionDeclarations`)
- [ ] Parse `functionCall` from streaming response
- [ ] Accumulate partial JSON for tool arguments
- [ ] Convert to `ToolCall` structs
- [ ] Implement `chat_loop()` with tool support
- [ ] Handle tool results → `functionResponse` conversion
- [ ] Test parallel tool execution
- [ ] Test multi-turn tool calling scenarios

### Phase 5: History Management (Est. 2-3 hours)

- [ ] Implement `get_history()` method
- [ ] Add automatic tool turn pruning
- [ ] Respect `max_tool_turns` configuration
- [ ] Investigate Gemini's history compression capabilities
- [ ] Implement `compact()` if supported (or return error)
- [ ] Test history pruning logic

### Phase 6: Context Caching (Est. 4-6 hours)

- [ ] Research Gemini caching API documentation
- [ ] Define `GeminiCacheConfig` struct
- [ ] Implement cache creation endpoint
- [ ] Implement cache usage in requests
- [ ] Add cache configuration to `ProviderConfig`
- [ ] Test cache creation and reuse
- [ ] Document caching best practices

### Phase 7: Error Handling (Est. 2-3 hours)

- [ ] Map Gemini error codes to `ProviderError`
- [ ] Handle rate limiting (429 errors)
- [ ] Handle quota exceeded errors
- [ ] Handle invalid API key errors
- [ ] Add retry logic for transient failures
- [ ] Test error scenarios

### Phase 8: Examples & Documentation (Est. 3-4 hours)

- [ ] Create `examples/gemini_basic.rs` - basic chat example
- [ ] Create `examples/gemini_tools.rs` - tool calling example
- [ ] Create `examples/gemini_cache.rs` - context caching example
- [ ] Add rustdoc comments to all public APIs
- [ ] Document supported models (gemini-pro, gemini-flash, etc.)
- [ ] Document rate limits and quotas
- [ ] Update main README with Gemini usage

### Phase 9: Testing (Est. 4-6 hours)

- [ ] Write unit tests for message conversion
- [ ] Write unit tests for tool schema conversion
- [ ] Create mock HTTP server for integration tests
- [ ] Test streaming with mock SSE responses
- [ ] Test tool calling with mock responses
- [ ] Test error handling paths
- [ ] Test history management
- [ ] Test context caching (if implemented)

### Phase 10: Integration (Est. 2-3 hours)

- [ ] Update `src/llm/mod.rs` to export GeminiProvider
- [ ] Update `LLM_IMPLEMENTATION_STATUS.md`
- [ ] Add Gemini to CI/CD testing
- [ ] Test with `chat_loop_with_tools()` helper
- [ ] Test with `ToolRegistry`
- [ ] Test with `LoopDetector`

---

## Dependencies

### Required Crates

```toml
[dependencies]
# Core async
tokio = { version = "1", features = ["full"] }
tokio-stream = "0.1"
futures = "0.3"

# HTTP client
reqwest = { version = "0.12", features = ["json", "stream"] }

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Error handling
thiserror = "2.0"

# Async traits
async-trait = "0.1"
```

### Optional Dependencies

```toml
[dev-dependencies]
# Testing
mockito = "1.0"  # Mock HTTP server for integration tests
tokio-test = "0.4"
```

---

## Supported Models

Based on Gemini API documentation:

| Model | Context Window | Use Case |
|-------|----------------|----------|
| `gemini-2.0-flash-exp` | 1M tokens | Experimental, fastest, multimodal |
| `gemini-1.5-pro` | 2M tokens | Advanced reasoning, complex tasks |
| `gemini-1.5-flash` | 1M tokens | Fast, cost-effective |
| `gemini-1.0-pro` | 32K tokens | Legacy, basic tasks |

**Recommendation**: Default to `gemini-1.5-flash` for balance of speed and cost

---

## Testing Strategy

### 1. Unit Tests
- Message conversion (Message ↔ GeminiContent)
- Tool schema conversion (Tool ↔ functionDeclarations)
- Error mapping
- History pruning logic

### 2. Integration Tests
- Full chat flow with mock HTTP responses
- Streaming with mock SSE data
- Tool calling with mock function calls
- Error handling with mock error responses

### 3. Manual Testing
- Real API calls with `GEMINI_API_KEY`
- Interactive chat via `examples/gemini_basic.rs`
- Multi-turn conversations
- Complex tool calling scenarios

### 4. Compatibility Testing
- Test with `ToolRegistry`
- Test with `LoopDetector`
- Test with `chat_loop_with_tools()`
- Ensure feature parity with OpenAI/Anthropic

---

## Success Criteria

- [ ] All `LLMProvider` trait methods implemented
- [ ] Streaming chat works with real Gemini API
- [ ] Tool calling works with parallel execution
- [ ] History management matches OpenAI/Anthropic behavior
- [ ] Context caching implemented (or documented as unsupported)
- [ ] All tests passing (unit + integration)
- [ ] Examples run successfully
- [ ] Documentation complete
- [ ] No regression in existing OpenAI/Anthropic providers

---

## Risks & Mitigations

### Risk 1: API Changes
**Likelihood**: Medium  
**Impact**: High  
**Mitigation**: Pin to v1beta API, monitor Google AI announcements

### Risk 2: SSE Format Differences
**Likelihood**: Medium  
**Impact**: Medium  
**Mitigation**: Comprehensive SSE parsing tests, handle edge cases

### Risk 3: Tool Calling Format Incompatibility
**Likelihood**: Low  
**Impact**: High  
**Mitigation**: Early prototype with real API, test parallel execution

### Risk 4: Context Caching Complexity
**Likelihood**: Medium  
**Impact**: Low  
**Mitigation**: Make caching optional, document clearly

### Risk 5: Rate Limiting
**Likelihood**: High (during testing)  
**Impact**: Low  
**Mitigation**: Implement exponential backoff, use mock responses for CI

---

## Open Questions

1. **System Message Handling**: Use `systemInstruction` field or prepend to first user message?
   - **Research needed**: Test both approaches for behavior differences

2. **Context Caching API Stability**: Is the caching API production-ready?
   - **Research needed**: Check Google AI documentation, community feedback

3. **Thinking/Reasoning Tokens**: Does Gemini expose reasoning tokens like Anthropic?
   - **Research needed**: Review API response structure

4. **Multimodal Support**: Should we support image/video inputs in this phase?
   - **Decision**: Defer to future phase, focus on text + tools first

5. **Safety Settings**: How to expose Gemini's safety filters?
   - **Decision**: Add to `ProviderConfig` as optional field

---

## Future Enhancements

### Phase 2 Features (Post-MVP)
- [ ] Multimodal support (images, audio, video)
- [ ] Grounding with Google Search
- [ ] Code execution capabilities
- [ ] Fine-tuned model support
- [ ] Batch API support
- [ ] Advanced safety settings configuration

### Performance Optimizations
- [ ] HTTP/2 connection pooling
- [ ] Request batching
- [ ] Smarter context caching strategies

### Developer Experience
- [ ] Better error messages with suggestions
- [ ] Automatic retry with exponential backoff
- [ ] Request/response logging for debugging
- [ ] Performance metrics and telemetry

---

## References

- [Google AI Gemini API Documentation](https://ai.google.dev/docs)
- [Gemini API Quickstart](https://ai.google.dev/tutorials/rest_quickstart)
- [Function Calling Guide](https://ai.google.dev/docs/function_calling)
- [Context Caching Documentation](https://ai.google.dev/docs/caching)
- [Safety Settings](https://ai.google.dev/docs/safety_setting_gemini)
- [Rate Limits and Quotas](https://ai.google.dev/docs/quota)

---

## Estimated Timeline

**Total Effort**: ~35-50 hours

- **Phase 1-2** (Core + Basic Chat): ~7-10 hours
- **Phase 3-4** (Streaming + Tools): ~10-14 hours
- **Phase 5-7** (History + Cache + Errors): ~8-12 hours
- **Phase 8-10** (Examples + Tests + Integration): ~9-13 hours

**Recommended Approach**: 
1. Start with basic chat (Phases 1-2) to validate API access
2. Add streaming (Phase 3) for better UX
3. Implement tool calling (Phase 4) for core functionality
4. Polish with remaining phases

**Milestone Checkpoints**:
- ✅ After Phase 2: Basic chat works
- ✅ After Phase 3: Streaming works
- ✅ After Phase 4: Tool calling works
- ✅ After Phase 10: Production ready
