# Conversation Flow Analysis in aaagent-rs

This document provides a comprehensive analysis of how conversations are initiated, managed, and processed in the aaagent-rs LLM provider implementation.

## Table of Contents

- [Architecture Overview](#architecture-overview)
- [Key Components](#key-components)
- [Provider Trait](#provider-trait)
- [Chat Loop](#chat-loop)
- [Message Passing](#message-passing)
- [Tool Execution](#tool-execution)
- [Stream Processing](#stream-processing)
- [Loop Detection](#loop-detection)
- [Skills System](#skills-system)
- [Event Flow Summary](#event-flow-summary)
- [Key Data Structures](#key-data-structures)

---

## Architecture Overview

The conversation system follows a **provider-based streaming** architecture with bidirectional channels:

```
User/CLI → Provider.chat_loop() → Background Task → LLM API
                                        ↓
                    LoopStep Events ← SSE Stream
```

### Channel Structure

```
┌─────────────────────────────────────────────────────────────┐
│                    Caller (CLI/SDK)                         │
└──────┬────────────────────────────────────────┬─────────────┘
       │ command_tx                             │ events
       ▼                                        ▲
┌──────────────────────────────────────────────────────────────┐
│  command_tx: UnboundedSender<ChatLoopCommand>               │
│  events: Pin<Box<dyn Stream<Item = LoopStep>>>              │
└──────┬────────────────────────────────────────┬─────────────┘
       ▼                                        │
   Background Task → API Request → SSE Stream → Parse → Emit
```

---

## Key Components

| Component | File | Purpose |
|-----------|------|---------|
| `LLMProvider` | `src/llm/provider.rs:35` | Unified trait for all LLM providers |
| `ProviderState` | `src/llm/provider.rs:80` | Token usage and request tracking |
| `ProviderConfig` | `src/llm/provider.rs:130` | Per-provider configuration |
| `ChatLoopHandle` | `src/llm/provider.rs:600` | Bidirectional handle for chat loop |
| `ToolRegistry` | `src/llm/registry.rs:1` | Tool registration and execution |
| `LoopDetector` | `src/llm/loop_detector.rs:1` | Detect and handle infinite loops |
| `OpenAIProvider` | `src/llm/openai.rs:1` | OpenAI/Responses API implementation |
| `AnthropicProvider` | `src/llm/anthropic.rs:1` | Anthropic Claude implementation |

---

## Provider Trait

**Location:** `src/llm/provider.rs:35`

The `LLMProvider` trait defines the unified interface for all LLM providers.

```rust
#[async_trait]
pub trait LLMProvider: Send + Sync {
    /// Create a new provider instance
    fn create(model: String, api_key: String) -> Result<Self, ProviderError>
    where
        Self: Sized;

    /// Get current state (tokens, request count)
    fn state(&self) -> ProviderState;

    /// Get current configuration
    fn config(&self) -> ProviderConfig;

    /// Update configuration
    fn update_config(&self, f: impl FnOnce(&mut ProviderConfig));

    /// Simple streaming chat (no tools)
    async fn chat(
        &self,
        prompt: &str,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String, ProviderError>> + Send>>, ProviderError>;

    /// Multi-turn chat loop with tool support
    async fn chat_loop(
        &self,
        history: Vec<Message>,
        tools: Option<Vec<Tool>>,
    ) -> Result<ChatLoopHandle, ProviderError>;

    /// Compact conversation history
    async fn compact(&self, history: Vec<Message>) -> Result<Vec<Message>, ProviderError>;

    /// Get accumulated conversation history
    fn get_history(&self) -> Vec<Message>;
}
```

### Provider Implementations

| Provider | File | Models Supported |
|----------|------|------------------|
| `OpenAIProvider` | `src/llm/openai.rs` | GPT-4, GPT-4o, o1, o3, etc. |
| `AnthropicProvider` | `src/llm/anthropic.rs` | Claude 3.5, Claude 3, etc. |
| `GeminiProvider` | `src/llm/gemini.rs` | Gemini Pro (partial) |

---

## Chat Loop

**Location:** `src/llm/provider.rs:600`

The chat loop is the core mechanism for multi-turn conversations with tool support.

### ChatLoopHandle

```rust
pub struct ChatLoopHandle {
    events: Pin<Box<dyn Stream<Item = Result<LoopStep, ProviderError>> + Send>>,
    command_tx: mpsc::UnboundedSender<ChatLoopCommand>,
}

impl ChatLoopHandle {
    /// Get next event from the stream
    pub async fn next(&mut self) -> Option<Result<LoopStep, ProviderError>>;

    /// Submit tool execution results
    pub fn submit_tool_results(&self, results: Vec<ToolResult>) -> Result<(), ProviderError>;

    /// Update available tools dynamically
    pub fn update_tools(&self, tools: Vec<Tool>) -> Result<(), ProviderError>;
}
```

### ChatLoopCommand

```rust
pub(crate) enum ChatLoopCommand {
    SubmitToolResults(Vec<ToolResult>),
    UpdateTools(Vec<Tool>),
}
```

### LoopStep Events

```rust
pub enum LoopStep {
    /// Extended thinking content (for reasoning models)
    Thinking(String),

    /// Text content chunk
    Content(String),

    /// Tool calls requested by the model
    ToolCallsRequested {
        tool_calls: Vec<ToolCall>,
        content: String,  // Partial content before tool calls
    },

    /// Tool results submitted and processed
    ToolResultsReceived { count: usize },

    /// Conversation turn complete
    Done {
        content: String,
        finish_reason: FinishReason,
        total_usage: TokenUsage,
        all_tool_calls: Vec<ToolCall>,
    },
}
```

### Chat Loop Implementation (OpenAI)

**Location:** `src/llm/openai.rs:556`

```rust
async fn chat_loop(
    &self,
    history: Vec<Message>,
    tools: Option<Vec<Tool>>,
) -> Result<ChatLoopHandle, ProviderError> {
    // 1. Create bidirectional channels
    let (command_tx, mut command_rx) = tokio::sync::mpsc::unbounded_channel();
    let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel();

    // 2. Clone state for background task
    let state = Arc::clone(&self.state);
    let config = Arc::clone(&self.config);
    let history_arc = Arc::clone(&self.history);

    // 3. Spawn background task
    tokio::spawn(async move {
        let mut messages = history;
        let mut current_tools = tools;

        loop {
            // Build and send API request
            let request = build_chat_request(&messages, &current_tools, &config);
            let response = client.post(url).json(&request).send().await?;

            // Process SSE stream
            let mut tool_call_assembler = ToolCallAssembler::new();
            let mut content = String::new();

            while let Some(event) = stream.next().await {
                match parse_chunk(event) {
                    ContentDelta(delta) => {
                        content.push_str(&delta);
                        event_tx.send(Ok(LoopStep::Content(delta)))?;
                    }
                    ToolCallDelta(id, name, args) => {
                        tool_call_assembler.process_delta(id, name, args);
                    }
                    Done => break,
                }
            }

            // Check for tool calls
            let tool_calls = tool_call_assembler.into_tool_calls()?;
            if !tool_calls.is_empty() {
                event_tx.send(Ok(LoopStep::ToolCallsRequested {
                    tool_calls: tool_calls.clone(),
                    content: content.clone(),
                }))?;

                // Wait for tool results
                match command_rx.recv().await {
                    Some(ChatLoopCommand::SubmitToolResults(results)) => {
                        // Add assistant message with tool calls
                        messages.push(Message::assistant_with_tools(content, tool_calls));

                        // Add tool result messages
                        for result in results {
                            messages.push(Message::tool_result(result));
                        }

                        event_tx.send(Ok(LoopStep::ToolResultsReceived {
                            count: results.len()
                        }))?;

                        continue; // Next iteration
                    }
                    Some(ChatLoopCommand::UpdateTools(new_tools)) => {
                        current_tools = Some(new_tools);
                    }
                    None => break, // Channel closed
                }
            } else {
                // No tool calls - conversation done
                event_tx.send(Ok(LoopStep::Done {
                    content,
                    finish_reason: FinishReason::Stop,
                    total_usage,
                    all_tool_calls: vec![],
                }))?;
                break;
            }
        }
    });

    Ok(ChatLoopHandle::new(event_rx, command_tx))
}
```

---

## Message Passing

### Message Structure

**Location:** `src/llm/provider.rs:180`

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
    pub tool_call_id: Option<String>,    // For tool responses
    pub tool_calls: Option<Vec<ToolCall>>, // For assistant tool calls
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolResult {
    pub tool_call_id: String,
    pub content: String,
    pub is_error: bool,
}
```

### Message Flow

```
┌────────────────────────────────────────────────────────────────┐
│ 1. User Input                                                  │
│    Message { role: User, content: "...", ... }                │
└───────────────────────────┬────────────────────────────────────┘
                            ▼
┌────────────────────────────────────────────────────────────────┐
│ 2. API Request (with history + tools)                         │
│    POST /v1/chat/completions                                  │
└───────────────────────────┬────────────────────────────────────┘
                            ▼
┌────────────────────────────────────────────────────────────────┐
│ 3. SSE Stream Processing                                      │
│    - Content deltas → LoopStep::Content                       │
│    - Tool call deltas → Assemble                              │
└───────────────────────────┬────────────────────────────────────┘
                            ▼
┌────────────────────────────────────────────────────────────────┐
│ 4. Tool Calls Requested                                       │
│    LoopStep::ToolCallsRequested { tool_calls, content }       │
└───────────────────────────┬────────────────────────────────────┘
                            ▼
┌────────────────────────────────────────────────────────────────┐
│ 5. Caller Executes Tools                                      │
│    for call in tool_calls { execute(call) }                   │
└───────────────────────────┬────────────────────────────────────┘
                            ▼
┌────────────────────────────────────────────────────────────────┐
│ 6. Submit Results                                             │
│    handle.submit_tool_results(results)                        │
└───────────────────────────┬────────────────────────────────────┘
                            ▼
┌────────────────────────────────────────────────────────────────┐
│ 7. Next API Call (loop back to step 2)                        │
│    OR                                                         │
│    LoopStep::Done { content, finish_reason, ... }            │
└────────────────────────────────────────────────────────────────┘
```

---

## Tool Execution

### ToolProvider Trait

**Location:** `src/tools/mod.rs:10`

```rust
#[async_trait]
pub trait ToolProvider: Send + Sync {
    /// Tool name for registration
    fn name(&self) -> &str;

    /// Tool definition for LLM
    fn definition(&self) -> Tool;

    /// Execute the tool
    async fn execute(&self, call: &ToolCall) -> Result<String, String>;
}
```

### ToolRegistry

**Location:** `src/llm/registry.rs:1`

```rust
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn ToolProvider>>,
}

impl ToolRegistry {
    pub fn new() -> Self;

    /// Register all built-in tools
    pub fn register_all_builtin(self) -> Self;

    /// Register a custom tool
    pub fn register<T: ToolProvider + 'static>(&mut self, tool: T);

    /// Execute a tool call
    pub async fn execute(&self, call: &ToolCall) -> Option<ToolResult>;

    /// Get tool definitions for LLM
    pub fn get_tools_for_llm(&self) -> Vec<Tool>;
}
```

### Built-in Tools

| Tool | File | Purpose |
|------|------|---------|
| `BashTool` | `src/tools/bash.rs` | Execute shell commands |
| `EditorEditTool` | `src/tools/editor_edit.rs` | File editing operations |

### BashTool Implementation

**Location:** `src/tools/bash.rs:1`

```rust
pub struct BashTool {
    timeout_secs: u64,
    working_dir: Option<PathBuf>,
}

impl ToolProvider for BashTool {
    fn name(&self) -> &str { "bash" }

    fn definition(&self) -> Tool {
        Tool {
            name: "bash".to_string(),
            description: "Execute a bash command".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The command to execute"
                    }
                },
                "required": ["command"]
            }),
            full_description: None,
        }
    }

    async fn execute(&self, call: &ToolCall) -> Result<String, String> {
        let command = call.arguments
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or("Missing command argument")?;

        let output = tokio::process::Command::new("bash")
            .arg("-c")
            .arg(command)
            .current_dir(self.working_dir.as_ref().unwrap_or(&PathBuf::from(".")))
            .output()
            .await
            .map_err(|e| e.to_string())?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        Ok(format!("{}{}", stdout, stderr))
    }
}
```

### ChatLoopConfig with Tool Executors

**Location:** `src/llm/helpers.rs:50`

```rust
pub struct ChatLoopConfig {
    /// Content callback
    pub on_content: Option<ContentCallback>,

    /// Tool call callback
    pub on_tool_calls: Option<ToolCallCallback>,

    /// Tool result callback
    pub on_tool_results: Option<ToolResultCallback>,

    /// Thinking callback (for reasoning models)
    pub on_thinking: Option<ContentCallback>,

    /// Loop detection callback
    pub on_loop_detected: Option<LoopDetectionCallback>,

    /// Tool registry for automatic execution
    pub registry: Option<Arc<ToolRegistry>>,

    /// Custom tool executors
    pub tool_executors: HashMap<String, ToolExecutor>,
}

pub type ToolExecutor = Box<dyn Fn(ToolCall) -> BoxFuture<'static, Result<String, String>> + Send + Sync>;
```

---

## Stream Processing

### SSE Event Processing (OpenAI)

**Location:** `src/llm/openai.rs:600`

```rust
// 1. Get SSE stream from response
let byte_stream = response.bytes_stream();
let event_stream = byte_stream.eventsource();

// 2. Initialize accumulators
let mut tool_call_assembler = ToolCallAssembler::new();
let mut content_accumulator = String::new();

// 3. Process each event
while let Some(event_result) = event_stream.next().await {
    match event_result {
        Ok(event) if event.data == "[DONE]" => break,
        Ok(event) => {
            let chunk: ChatCompletionChunk = serde_json::from_str(&event.data)?;

            for choice in chunk.choices {
                // Content delta
                if let Some(content) = choice.delta.content {
                    content_accumulator.push_str(&content);
                    event_tx.send(Ok(LoopStep::Content(content)))?;
                }

                // Tool call deltas (parallel tool calls)
                if let Some(tool_calls) = choice.delta.tool_calls {
                    for delta in tool_calls {
                        tool_call_assembler.process_delta(
                            delta.id.unwrap_or_default(),
                            delta.function.name,
                            delta.function.arguments,
                        );
                    }
                }

                // Finish reason
                if let Some(reason) = choice.finish_reason {
                    match reason.as_str() {
                        "tool_calls" => { /* Will emit ToolCallsRequested */ }
                        "stop" => { /* Will emit Done */ }
                        _ => {}
                    }
                }
            }
        }
        Err(e) => return Err(ProviderError::Stream(e.to_string())),
    }
}
```

### ToolCallAssembler

**Location:** `src/llm/provider.rs:400`

Handles parallel tool calls where deltas arrive interleaved.

```rust
pub struct ToolCallAssembler {
    calls: HashMap<String, PartialToolCall>,
}

struct PartialToolCall {
    id: String,
    name: Option<String>,
    arguments: String,
}

impl ToolCallAssembler {
    pub fn new() -> Self {
        Self { calls: HashMap::new() }
    }

    pub fn process_delta(
        &mut self,
        id: String,
        name: Option<String>,
        arguments_delta: Option<String>,
    ) {
        let call = self.calls.entry(id.clone()).or_insert_with(|| {
            PartialToolCall {
                id: id.clone(),
                name: None,
                arguments: String::new(),
            }
        });

        if let Some(n) = name {
            call.name = Some(n);
        }
        if let Some(delta) = arguments_delta {
            call.arguments.push_str(&delta);
        }
    }

    pub fn into_tool_calls(self) -> Result<Vec<ToolCall>, serde_json::Error> {
        self.calls
            .into_values()
            .map(|partial| {
                Ok(ToolCall {
                    id: partial.id,
                    name: partial.name.unwrap_or_default(),
                    arguments: serde_json::from_str(&partial.arguments)?,
                })
            })
            .collect()
    }
}
```

---

## Loop Detection

**Location:** `src/llm/loop_detector.rs:1`

Prevents infinite tool call loops.

### LoopDetectorConfig

```rust
pub struct LoopDetectorConfig {
    /// Max exact duplicate tool calls before detection
    pub max_exact_duplicates: usize,

    /// Window size for exact duplicate detection
    pub exact_window_size: usize,

    /// Enable pattern-based detection
    pub enable_pattern_detection: bool,

    /// Minimum pattern length to detect
    pub min_pattern_length: usize,

    /// Maximum pattern length to detect
    pub max_pattern_length: usize,

    /// Window size for pattern detection
    pub pattern_window_size: usize,

    /// Action on first detection
    pub first_detection_action: LoopAction,

    /// Action on second detection
    pub second_detection_action: LoopAction,

    /// Action on third detection
    pub third_detection_action: LoopAction,
}
```

### LoopAction

```rust
pub enum LoopAction {
    /// Continue execution with warning
    Continue,

    /// Warn but allow continuation
    Warn,

    /// Terminate the loop
    Terminate,
}
```

### LoopDetection Result

```rust
pub struct LoopDetection {
    pub detected: bool,
    pub loop_type: LoopType,
    pub confidence: f64,
    pub suggestion: String,
    pub action: LoopAction,
    pub detection_count: usize,
    pub warning_message: Option<String>,
}

pub enum LoopType {
    ExactDuplicate { call: ToolCall, count: usize },
    Pattern { pattern: Vec<ToolCall>, repetitions: usize },
}
```

### Usage in Chat Loop

```rust
// Configure loop detection callback
let config = ChatLoopConfig::new()
    .on_loop_detected(|detection| {
        eprintln!("Loop detected: {}", detection.suggestion);
        match detection.action {
            LoopAction::Continue => true,
            LoopAction::Warn => {
                eprintln!("Warning: {}", detection.warning_message.unwrap_or_default());
                true
            }
            LoopAction::Terminate => false, // Stop the loop
        }
    });
```

---

## Skills System

**Location:** `src/skills/`

Skills are markdown files that provide specialized knowledge, workflows, or tool integrations to extend the agent's capabilities.

### Skill Discovery Locations

Skills are discovered from multiple locations in priority order:

| Priority | Location | Scope |
|----------|----------|-------|
| 1 (highest) | `.aaagent/skills/` | Project |
| 2 | `~/.aaagent/skills/` | User |
| 3 (lowest) | `~/.aaagent/skills/.system/` | System |

When skills have the same name, the first one found wins (project overrides user, user overrides system).

### Skill File Format

Each skill is a directory containing a `SKILL.md` file with YAML frontmatter:

```markdown
---
name: code-review
description: Guide for performing thorough code reviews
metadata:
  short-description: Code review guidelines
---

# Code Review Skill

Detailed instructions and content...
```

### Key Components

| Component | File | Purpose |
|-----------|------|---------|
| `SkillMetadata` | `src/skills/model.rs` | Skill name, description, path, scope |
| `SkillsManager` | `src/skills/manager.rs` | Load and cache skills by cwd |
| `SkillInjection` | `src/skills/injection.rs` | Format skills for model context |

### SkillsManager

```rust
pub struct SkillsManager {
    home: PathBuf,
    cache_by_cwd: RwLock<HashMap<PathBuf, SkillLoadOutcome>>,
}

impl SkillsManager {
    /// Create with default home (~/.aaagent/)
    pub fn with_default_home() -> Option<Self>;

    /// Load skills for a working directory (cached)
    pub fn skills_for_cwd(&self, cwd: &Path) -> SkillLoadOutcome;

    /// Force reload skills
    pub fn skills_for_cwd_with_options(&self, cwd: &Path, force: bool) -> SkillLoadOutcome;

    /// Find a skill by name
    pub fn find_skill(&self, cwd: &Path, name: &str) -> Option<SkillMetadata>;
}
```

### Skill Injection

Skills are injected into conversations as XML-formatted user messages:

```xml
<skill>
<name>code-review</name>
<path>/path/to/skills/code-review/SKILL.md</path>
[full SKILL.md contents]
</skill>
```

### Skill References

Skills can be referenced in user messages using `/skill:name` syntax:

```rust
// Parse skill references from text
let refs = parse_skill_references("Please /skill:code-review this PR");
// refs = [SkillReference { name: "code-review", path: None }]

// Build injections
let injections = build_skill_injections(&refs, Some(&outcome));
```

### Integration with ChatLoopConfig

```rust
let config = ChatLoopConfig::new()
    .with_skills_manager(Arc::new(SkillsManager::with_default_home().unwrap()))
    .with_cwd(std::env::current_dir().unwrap())
    .with_auto_parse_skills(true)  // Auto-parse /skill:name from messages
    .on_skill_warning(|warning| {
        eprintln!("Skill warning: {}", warning);
    });
```

---

## Event Flow Summary

### Complete Multi-turn Conversation Flow

```
1. INITIALIZATION
   ┌─────────────────────────────────────────────────────────┐
   │ let provider = OpenAIProvider::create(model, key)?;    │
   │ provider.update_config(|cfg| { /* settings */ });      │
   └─────────────────────────────────────────────────────────┘

2. START CHAT LOOP
   ┌─────────────────────────────────────────────────────────┐
   │ let mut handle = provider.chat_loop(history, tools)?;  │
   │ // Spawns background task                              │
   │ // Returns ChatLoopHandle                              │
   └─────────────────────────────────────────────────────────┘

3. EVENT PROCESSING
   ┌─────────────────────────────────────────────────────────┐
   │ while let Some(event) = handle.next().await {          │
   │     match event? {                                     │
   │         LoopStep::Thinking(text) => { /* display */ } │
   │         LoopStep::Content(text) => { /* display */ }  │
   │         LoopStep::ToolCallsRequested { calls, .. } => {│
   │             // Execute tools                           │
   │             // Submit results                          │
   │         }                                              │
   │         LoopStep::Done { content, .. } => break,       │
   │     }                                                  │
   │ }                                                      │
   └─────────────────────────────────────────────────────────┘

4. TOOL EXECUTION
   ┌─────────────────────────────────────────────────────────┐
   │ let results: Vec<ToolResult> = tool_calls              │
   │     .iter()                                            │
   │     .map(|call| execute_tool(call))                    │
   │     .collect();                                        │
   │ handle.submit_tool_results(results)?;                  │
   └─────────────────────────────────────────────────────────┘

5. COMPLETION
   ┌─────────────────────────────────────────────────────────┐
   │ // After LoopStep::Done:                               │
   │ let history = provider.get_history();                  │
   │ // Full conversation available                         │
   └─────────────────────────────────────────────────────────┘
```

### Background Task State Machine

```
                    ┌─────────────┐
                    │    START    │
                    └──────┬──────┘
                           │
                           ▼
              ┌────────────────────────┐
              │   Build API Request    │◄──────────────────┐
              │   (messages + tools)   │                   │
              └───────────┬────────────┘                   │
                          │                                │
                          ▼                                │
              ┌────────────────────────┐                   │
              │    Send to LLM API     │                   │
              └───────────┬────────────┘                   │
                          │                                │
                          ▼                                │
              ┌────────────────────────┐                   │
              │   Process SSE Stream   │                   │
              │   - Emit Content       │                   │
              │   - Assemble ToolCalls │                   │
              └───────────┬────────────┘                   │
                          │                                │
                          ▼                                │
              ┌────────────────────────┐                   │
              │   Tool Calls Found?    │                   │
              └───────────┬────────────┘                   │
                          │                                │
              ┌───────────┴───────────┐                   │
              │ YES                   │ NO                │
              ▼                       ▼                   │
    ┌─────────────────┐     ┌─────────────────┐          │
    │ Emit ToolCalls- │     │   Emit Done     │          │
    │ Requested       │     └────────┬────────┘          │
    └────────┬────────┘              │                   │
             │                       ▼                   │
             ▼                 ┌───────────┐             │
    ┌─────────────────┐        │    END    │             │
    │ Wait for        │        └───────────┘             │
    │ SubmitToolRes   │                                  │
    └────────┬────────┘                                  │
             │                                           │
             ▼                                           │
    ┌─────────────────┐                                  │
    │ Add messages    │                                  │
    │ Emit Received   │──────────────────────────────────┘
    └─────────────────┘
```

---

## Key Data Structures

### ProviderConfig

**Location:** `src/llm/provider.rs:130`

```rust
pub struct ProviderConfig {
    /// Temperature for generation (0.0 - 2.0)
    pub temperature: f32,

    /// Maximum tokens to generate
    pub max_tokens: u32,

    /// Top-p sampling
    pub top_p: Option<f32>,

    /// Top-k sampling (Anthropic/Gemini)
    pub top_k: Option<u32>,

    /// Enable extended thinking (o1, claude reasoning)
    pub enable_reasoning: bool,

    /// System prompt
    pub system_prompt: Option<String>,

    /// Stop sequences
    pub stop_sequences: Vec<String>,

    /// Extra provider-specific options
    pub extra_options: HashMap<String, serde_json::Value>,

    /// Max tool turns to keep (prune old ones)
    pub max_tool_turns: Option<usize>,
}
```

### ProviderState

**Location:** `src/llm/provider.rs:80`

```rust
pub struct ProviderState {
    /// Total input tokens used
    pub input_tokens: u64,

    /// Total output tokens used
    pub output_tokens: u64,

    /// Cached tokens (prompt caching)
    pub cached_tokens: u64,

    /// Total API requests made
    pub request_count: u64,

    /// Last request timestamp
    pub last_request_time: Option<SystemTime>,

    /// Additional metadata
    pub metadata: HashMap<String, String>,

    /// Conversation turn count
    pub conversation_turns: u32,
}
```

### TokenUsage

**Location:** `src/llm/provider.rs:110`

```rust
pub struct TokenUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub cached_tokens: u32,
}
```

### Tool

**Location:** `src/llm/provider.rs:300`

```rust
pub struct Tool {
    /// Tool name (function name)
    pub name: String,

    /// Brief description for LLM
    pub description: String,

    /// JSON Schema for parameters
    pub parameters: serde_json::Value,

    /// Detailed description (for reference)
    pub full_description: Option<String>,
}
```

---

## File Summary

| File | Lines | Purpose |
|------|-------|---------|
| `src/lib.rs` | ~10 | Module exports |
| `src/main.rs` | ~100 | CLI utility commands |
| `src/llm/mod.rs` | ~30 | LLM module organization |
| `src/llm/provider.rs` | ~850 | Core traits and types |
| `src/llm/helpers.rs` | ~400 | ChatLoopConfig, utilities |
| `src/llm/registry.rs` | ~130 | ToolRegistry implementation |
| `src/llm/loop_detector.rs` | ~350 | Loop detection logic |
| `src/llm/openai.rs` | ~1200 | OpenAI provider implementation |
| `src/llm/anthropic.rs` | ~900 | Anthropic provider implementation |
| `src/llm/gemini.rs` | ~TBD | Gemini provider (partial) |
| `src/tools/mod.rs` | ~40 | Tool trait and exports |
| `src/tools/bash.rs` | ~200 | BashTool implementation |
| `src/tools/editor_edit.rs` | ~200 | EditorEditTool implementation |

---

## Example Usage

### Basic Chat Loop

```rust
use aaagent::{LLMProvider, OpenAIProvider, Message, Role};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. Create provider
    let provider = OpenAIProvider::create(
        "gpt-4o".to_string(),
        std::env::var("OPENAI_API_KEY")?,
    )?;

    // 2. Configure
    provider.update_config(|cfg| {
        cfg.temperature = 0.7;
        cfg.max_tokens = 4096;
    });

    // 3. Build history
    let history = vec![
        Message {
            role: Role::User,
            content: "Hello!".to_string(),
            tool_call_id: None,
            tool_calls: None,
        },
    ];

    // 4. Start chat loop
    let mut handle = provider.chat_loop(history, None).await?;

    // 5. Process events
    while let Some(event) = handle.next().await {
        match event? {
            LoopStep::Content(text) => print!("{}", text),
            LoopStep::Done { content, .. } => {
                println!("\n\nFinal: {}", content);
                break;
            }
            _ => {}
        }
    }

    Ok(())
}
```

### With Tool Execution

```rust
use aaagent::{LLMProvider, OpenAIProvider, Tool, ToolResult};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let provider = OpenAIProvider::create(model, api_key)?;

    // Define tools
    let tools = vec![
        Tool {
            name: "get_weather".to_string(),
            description: "Get weather for a city".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "city": { "type": "string" }
                },
                "required": ["city"]
            }),
            full_description: None,
        },
    ];

    let mut handle = provider.chat_loop(history, Some(tools)).await?;

    while let Some(event) = handle.next().await {
        match event? {
            LoopStep::ToolCallsRequested { tool_calls, .. } => {
                let results: Vec<ToolResult> = tool_calls
                    .iter()
                    .map(|call| {
                        // Execute tool
                        let output = execute_my_tool(call);
                        ToolResult {
                            tool_call_id: call.id.clone(),
                            content: output,
                            is_error: false,
                        }
                    })
                    .collect();

                handle.submit_tool_results(results)?;
            }
            LoopStep::Done { .. } => break,
            _ => {}
        }
    }

    Ok(())
}
```
立即補上 (High Priority)

| 功能 | 理由 | 複雜度 |
|------|------|--------|
| **Auto Compact** | 長對話必需，避免 context window 溢出 | 低 |
| **基本 Telemetry** | 追蹤 token 使用、成本估算 | 低 |
| **錯誤重試機制** | API 不穩定時的基本可靠性 | 中 |

### 建議補上 (Medium Priority)

| 功能 | 理由 | 複雜度 |
|------|------|--------|
| **簡易審批回調** | 讓 caller 可選擇性審批危險操作 | 低 |
| **History 持久化** | 支援會話恢復 | 中 |
| **Rate Limit 處理** | 自動退避和重試 | 中 |

### 可選 (Low Priority)

| 功能 | 理由 | 複雜度 |
|------|------|--------|
| MCP 支援 | 如需擴展工具生態 | 高 |
| Sandbox | 如需安全執行 | 高 |
| Undo
