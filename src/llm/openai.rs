// OpenAI Provider implementation using reqwest + SSE streaming
// Following OpenAI's official Rust implementation pattern

use crate::llm::provider::*;
use eventsource_stream::Eventsource;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::pin::Pin;
use std::sync::{Arc, RwLock};

const OPENAI_API_BASE: &str = "https://api.openai.com/v1";
const OPENAI_RESPONSES_API_BASE: &str = "https://api.openai.com/v1/responses";

/// OpenAI provider implementation
#[derive(Clone)]
pub struct OpenAIProvider {
    client: reqwest::Client,
    api_key: String,
    model: String,
    config: Arc<RwLock<ProviderConfig>>,
    state: Arc<RwLock<ProviderState>>,
    /// Conversation history from the last chat_loop
    history: Arc<RwLock<Vec<Message>>>,
}

// Request/Response types matching OpenAI API spec

#[derive(Debug, Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_completion_tokens: Option<u32>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream_options: Option<StreamOptions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<OpenAITool>>,
}

#[derive(Debug, Serialize)]
struct StreamOptions {
    include_usage: bool,
}

#[derive(Debug, Serialize, Clone)]
struct OpenAITool {
    #[serde(rename = "type")]
    tool_type: String, // Always "function"
    function: OpenAIFunction,
}

#[derive(Debug, Serialize, Clone)]
struct OpenAIFunction {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "role")]
enum ChatMessage {
    #[serde(rename = "system")]
    System { content: String },
    #[serde(rename = "user")]
    User { content: String },
    #[serde(rename = "assistant")]
    Assistant {
        #[serde(skip_serializing_if = "Option::is_none")]
        content: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        tool_calls: Option<Vec<OpenAIToolCall>>,
    },
    #[serde(rename = "tool")]
    Tool {
        content: String,
        tool_call_id: String,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct OpenAIToolCall {
    id: String,
    #[serde(rename = "type")]
    tool_type: String, // Always "function"
    function: OpenAIFunctionCall,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct OpenAIFunctionCall {
    name: String,
    arguments: String, // JSON string
}

#[derive(Debug, Deserialize)]
struct ChatCompletionChunk {
    choices: Vec<Choice>,
    #[serde(default)]
    usage: Option<Usage>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    delta: Delta,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Delta {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<ToolCallDelta>>,
}

#[derive(Debug, Deserialize)]
struct ToolCallDelta {
    #[allow(dead_code)]
    index: usize,
    #[serde(default)]
    id: Option<String>,
    #[serde(rename = "type", default)]
    #[allow(dead_code)]
    tool_type: Option<String>,
    #[serde(default)]
    function: Option<FunctionDelta>,
}

#[derive(Debug, Deserialize)]
struct FunctionDelta {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    arguments: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
struct Usage {
    prompt_tokens: u32,
    completion_tokens: u32,
    #[serde(default)]
    #[allow(dead_code)]
    total_tokens: u32,
}

// Responses API types for compaction

#[derive(Debug, Serialize)]
struct ResponsesCompactRequest {
    model: String,
    input: Vec<ResponsesInput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    instructions: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
enum ResponsesInput {
    Message(ResponsesMessage),
    CompactedItem(CompactedItem),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct ResponsesMessage {
    role: String,
    content: ResponsesContent,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
enum ResponsesContent {
    Text(String),
    Parts(Vec<ResponsesContentPart>),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type")]
enum ResponsesContentPart {
    #[serde(rename = "input_text")]
    InputText { text: String },
    #[serde(rename = "output_text")]
    OutputText { text: String },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct CompactedItem {
    #[serde(rename = "type")]
    item_type: String, // "compacted"
    data: String, // Opaque encrypted data
}

#[derive(Debug, Deserialize)]
struct ResponsesCompactResponse {
    output: Vec<ResponsesInput>,
}

impl OpenAIProvider {
    /// Check if a model is supported (uses max_completion_tokens)
    fn is_supported_model(model: &str) -> bool {
        model.starts_with("gpt-5") || model.starts_with("o1") || model.starts_with("gpt-4o")
    }

    /// Prune old tool call/result turns from history
    /// Keeps only the most recent N turns, where one turn = assistant message with tool_calls + tool results
    fn prune_tool_turns(messages: &mut Vec<ChatMessage>, max_turns: usize) {
        if max_turns == 0 {
            return;
        }

        // Find all tool turns (assistant message with tool_calls followed by tool results)
        let mut tool_turn_ranges: Vec<(usize, usize)> = Vec::new();
        let mut i = 0;

        while i < messages.len() {
            // Look for Assistant message with tool_calls
            if let ChatMessage::Assistant {
                tool_calls: Some(_),
                ..
            } = &messages[i]
            {
                let start = i;
                i += 1;

                // Find all consecutive Tool messages that follow
                while i < messages.len() {
                    if matches!(&messages[i], ChatMessage::Tool { .. }) {
                        i += 1;
                    } else {
                        break;
                    }
                }

                let end = i;
                tool_turn_ranges.push((start, end));
            } else {
                i += 1;
            }
        }

        // If we have more tool turns than max_turns, remove the oldest ones
        if tool_turn_ranges.len() > max_turns {
            let turns_to_remove = tool_turn_ranges.len() - max_turns;

            // Remove from the end backwards to avoid index shifting issues
            for &(start, end) in tool_turn_ranges.iter().take(turns_to_remove).rev() {
                messages.drain(start..end);
            }
        }
    }

    /// Convert our Message type to Responses API format
    fn convert_to_responses_input(msg: &Message) -> ResponsesInput {
        let role = match msg.role {
            Role::System => "system",
            Role::User => "user",
            Role::Assistant => "assistant",
            Role::Tool => "tool",
        };

        ResponsesInput::Message(ResponsesMessage {
            role: role.to_string(),
            content: ResponsesContent::Text(msg.content.clone()),
        })
    }

    /// Convert ResponsesInput back to our Message type
    fn convert_from_responses_input(input: &ResponsesInput) -> Result<Message, ProviderError> {
        match input {
            ResponsesInput::Message(msg) => {
                let role = match msg.role.as_str() {
                    "system" => Role::System,
                    "user" => Role::User,
                    "assistant" => Role::Assistant,
                    "tool" => Role::Tool,
                    _ => {
                        return Err(ProviderError::ApiError(format!(
                            "Unknown role: {}",
                            msg.role
                        )))
                    }
                };

                let content = match &msg.content {
                    ResponsesContent::Text(text) => text.clone(),
                    ResponsesContent::Parts(parts) => {
                        // Combine all text parts
                        parts
                            .iter()
                            .filter_map(|part| match part {
                                ResponsesContentPart::InputText { text } => Some(text.as_str()),
                                ResponsesContentPart::OutputText { text } => Some(text.as_str()),
                            })
                            .collect::<Vec<_>>()
                            .join("\n")
                    }
                };

                Ok(Message {
                    role,
                    content,
                    tool_call_id: None,
                    tool_calls: None,
                })
            }
            ResponsesInput::CompactedItem(_) => {
                // Compacted items are opaque and can't be converted back
                // They should be passed through as-is in the next request
                Err(ProviderError::ApiError(
                    "Cannot convert compacted item to Message".to_string(),
                ))
            }
        }
    }

    /// Convert our Message type to OpenAI ChatMessage format
    fn convert_message(msg: &Message) -> ChatMessage {
        match msg.role {
            Role::System => ChatMessage::System {
                content: msg.content.clone(),
            },
            Role::User => ChatMessage::User {
                content: msg.content.clone(),
            },
            Role::Assistant => {
                let tool_calls = msg.tool_calls.as_ref().map(|calls| {
                    calls
                        .iter()
                        .map(|tc| OpenAIToolCall {
                            id: tc.id.clone(),
                            tool_type: "function".to_string(),
                            function: OpenAIFunctionCall {
                                name: tc.name.clone(),
                                arguments: serde_json::to_string(&tc.arguments).unwrap_or_default(),
                            },
                        })
                        .collect()
                });

                ChatMessage::Assistant {
                    content: Some(msg.content.clone()),
                    tool_calls,
                }
            }
            Role::Tool => ChatMessage::Tool {
                content: msg.content.clone(),
                tool_call_id: msg.tool_call_id.clone().unwrap_or_default(),
            },
        }
    }

    /// Convert our Tool type to OpenAI tool format
    fn convert_tools(tools: &[Tool]) -> Vec<OpenAITool> {
        tools
            .iter()
            .map(|t| OpenAITool {
                tool_type: "function".to_string(),
                function: OpenAIFunction {
                    name: t.name.clone(),
                    description: t.description.clone(),
                    parameters: t.parameters.clone(),
                },
            })
            .collect()
    }

    pub fn new(model: String, api_key: String) -> Result<Self, ProviderError> {
        // Only support GPT-5+ and newer models
        if !Self::is_supported_model(&model) {
            return Err(ProviderError::ConfigError(
                format!(
                    "Model '{}' not supported. Only GPT-5+ models supported (e.g., gpt-5-nano, o1, gpt-4o)",
                    model
                )
            ));
        }

        let client = reqwest::Client::new();

        Ok(Self {
            client,
            api_key,
            model,
            config: Arc::new(RwLock::new(ProviderConfig::default())),
            state: Arc::new(RwLock::new(ProviderState::default())),
            history: Arc::new(RwLock::new(Vec::new())),
        })
    }
}

#[async_trait::async_trait]
impl LLMProvider for OpenAIProvider {
    fn create(model: String, api_key: String) -> Result<Self, ProviderError> {
        Self::new(model, api_key)
    }

    fn state(&self) -> ProviderState {
        self.state.read().map(|s| s.clone()).unwrap_or_default()
    }

    fn config(&self) -> ProviderConfig {
        self.config.read().map(|c| c.clone()).unwrap_or_default()
    }

    fn update_config(&self, f: impl FnOnce(&mut ProviderConfig)) {
        if let Ok(mut config) = self.config.write() {
            f(&mut config);
        }
    }

    async fn chat(
        &self,
        prompt: &str,
    ) -> Result<
        Pin<Box<dyn futures::Stream<Item = Result<StreamChunk, ProviderError>> + Send>>,
        ProviderError,
    > {
        let cfg = self.config();

        // Build messages
        let mut messages = vec![];

        if let Some(system_prompt) = &cfg.system_prompt {
            messages.push(ChatMessage::System {
                content: system_prompt.clone(),
            });
        }

        messages.push(ChatMessage::User {
            content: prompt.to_string(),
        });

        // Build request
        let request = ChatCompletionRequest {
            model: self.model.clone(),
            messages,
            temperature: Some(cfg.temperature),
            max_completion_tokens: Some(cfg.max_tokens),
            stream: true,
            stream_options: Some(StreamOptions {
                include_usage: true,
            }),
            tools: None,
        };

        // Make HTTP request
        let response = self
            .client
            .post(format!("{}/chat/completions", OPENAI_API_BASE))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| ProviderError::ApiError(e.to_string()))?;

        // Check status
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(ProviderError::ApiError(format!(
                "HTTP {}: {}",
                status, error_text
            )));
        }

        // Create SSE stream
        let byte_stream = response.bytes_stream();
        let event_stream = byte_stream.eventsource();

        let state = self.state.clone();
        let mut full_content = String::new();

        // Convert SSE events to StreamChunk
        let output_stream = async_stream::stream! {
            futures::pin_mut!(event_stream);

            while let Some(event_result) = event_stream.next().await {
                match event_result {
                    Ok(event) => {
                        // Skip non-data events
                        if event.data == "[DONE]" {
                            break;
                        }

                        // Parse JSON chunk
                        match serde_json::from_str::<ChatCompletionChunk>(&event.data) {
                            Ok(chunk) => {
                                // Check if this is a usage-only chunk (comes after finish_reason)
                                if chunk.choices.is_empty() && chunk.usage.is_some() {
                                    // This is the final usage chunk
                                    let usage = chunk.usage.unwrap();
                                    let token_usage = TokenUsage {
                                        input_tokens: usage.prompt_tokens,
                                        output_tokens: usage.completion_tokens,
                                        cached_tokens: 0,
                                    };

                                    // Update state
                                    if let Ok(mut s) = state.write() {
                                        s.input_tokens += token_usage.input_tokens as u64;
                                        s.output_tokens += token_usage.output_tokens as u64;
                                        s.request_count += 1;
                                        s.last_request_time = Some(std::time::SystemTime::now());
                                    }

                                    // Send Done event with usage
                                    yield Ok(StreamChunk::Done {
                                        finish_reason: FinishReason::Stop,
                                        usage: token_usage,
                                        full_content: full_content.clone(),
                                    });
                                    continue;
                                }

                                // Process each choice
                                for choice in chunk.choices {
                                    // Content delta
                                    if let Some(content) = choice.delta.content {
                                        full_content.push_str(&content);
                                        yield Ok(StreamChunk::Content(content));
                                    }

                                    // Finish reason (without usage, as it comes in separate chunk)
                                    // OpenAI sends usage in a separate chunk after finish_reason
                                    if let Some(_finish_reason) = choice.finish_reason {
                                        // Skip - we'll handle completion when usage chunk arrives
                                    }
                                }
                            }
                            Err(e) => {
                                yield Err(ProviderError::ApiError(format!(
                                    "Failed to parse chunk: {}",
                                    e
                                )));
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        yield Err(ProviderError::ApiError(format!("Stream error: {}", e)));
                        break;
                    }
                }
            }
        };

        Ok(Box::pin(output_stream))
    }

    async fn chat_loop(
        &self,
        history: Vec<Message>,
        tools: Option<Vec<Tool>>,
    ) -> Result<ChatLoopHandle, ProviderError> {
        let (tool_result_tx, mut tool_result_rx) =
            tokio::sync::mpsc::unbounded_channel::<ToolResultSubmission>();
        let (event_tx, event_rx) =
            tokio::sync::mpsc::unbounded_channel::<Result<LoopStep, ProviderError>>();

        let client = self.client.clone();
        let api_key = self.api_key.clone();
        let model = self.model.clone();
        let cfg = self.config();
        let state = self.state.clone();
        let provider_history = self.history.clone();

        // Convert messages and tools
        let mut messages: Vec<ChatMessage> = history.iter().map(Self::convert_message).collect();
        let openai_tools = tools.as_ref().map(|t| Self::convert_tools(t));

        // Track history as our Message types (not ChatMessage)
        let mut current_history = history.clone();

        // Spawn the chat loop task
        tokio::spawn(async move {
            loop {
                // Build request
                // Note: When using tools, some models don't support custom temperature
                let request = ChatCompletionRequest {
                    model: model.clone(),
                    messages: messages.clone(),
                    temperature: if openai_tools.is_some() {
                        None
                    } else {
                        Some(cfg.temperature)
                    },
                    max_completion_tokens: Some(cfg.max_tokens),
                    stream: true,
                    stream_options: Some(StreamOptions {
                        include_usage: true,
                    }),
                    tools: openai_tools.clone(),
                };

                // Make HTTP request
                let response = match client
                    .post(format!("{}/chat/completions", OPENAI_API_BASE))
                    .header("Authorization", format!("Bearer {}", api_key))
                    .header("Content-Type", "application/json")
                    .json(&request)
                    .send()
                    .await
                {
                    Ok(r) => r,
                    Err(e) => {
                        let _ = event_tx.send(Err(ProviderError::ApiError(e.to_string())));
                        break;
                    }
                };

                // Check status
                if !response.status().is_success() {
                    let status = response.status();
                    let error_text = response
                        .text()
                        .await
                        .unwrap_or_else(|_| "Unknown error".to_string());
                    let _ = event_tx.send(Err(ProviderError::ApiError(format!(
                        "HTTP {}: {}",
                        status, error_text
                    ))));
                    break;
                }

                // Create SSE stream
                let byte_stream = response.bytes_stream();
                let event_stream = byte_stream.eventsource();

                let mut tool_call_assembler = ToolCallAssembler::new();
                let mut content_accumulator = String::new();
                let mut current_finish_reason = None;
                let mut tool_call_index_to_id: std::collections::HashMap<usize, String> =
                    std::collections::HashMap::new();
                let mut completed_tool_calls: Option<Vec<ToolCall>> = None;

                futures::pin_mut!(event_stream);

                while let Some(event_result) = event_stream.next().await {
                    match event_result {
                        Ok(event) => {
                            if event.data == "[DONE]" {
                                break;
                            }

                            match serde_json::from_str::<ChatCompletionChunk>(&event.data) {
                                Ok(chunk) => {
                                    // Handle usage-only chunk
                                    if chunk.choices.is_empty() && chunk.usage.is_some() {
                                        let usage = chunk.usage.unwrap();
                                        let token_usage = TokenUsage {
                                            input_tokens: usage.prompt_tokens,
                                            output_tokens: usage.completion_tokens,
                                            cached_tokens: 0,
                                        };

                                        // Update state
                                        if let Ok(mut s) = state.write() {
                                            s.input_tokens += token_usage.input_tokens as u64;
                                            s.output_tokens += token_usage.output_tokens as u64;
                                            s.request_count += 1;
                                            s.last_request_time =
                                                Some(std::time::SystemTime::now());
                                        }

                                        // Send appropriate completion event
                                        let tool_calls = std::mem::replace(
                                            &mut tool_call_assembler,
                                            ToolCallAssembler::new(),
                                        )
                                        .into_tool_calls()
                                        .unwrap_or_default();

                                        if !tool_calls.is_empty() {
                                            // Tool calls completed - save them for later use
                                            completed_tool_calls = Some(tool_calls.clone());
                                            let _ =
                                                event_tx.send(Ok(LoopStep::ToolCallsRequested {
                                                    tool_calls,
                                                    content: content_accumulator.clone(),
                                                }));
                                        } else {
                                            // Text response completed
                                            let _ = event_tx.send(Ok(LoopStep::Done {
                                                content: content_accumulator.clone(),
                                                finish_reason: FinishReason::Stop,
                                                total_usage: token_usage,
                                                all_tool_calls: vec![],
                                            }));
                                        }
                                        continue;
                                    }

                                    // Process choices
                                    for choice in chunk.choices {
                                        // Content delta
                                        if let Some(content) = choice.delta.content {
                                            content_accumulator.push_str(&content);
                                            let _ = event_tx.send(Ok(LoopStep::Content(content)));
                                        }

                                        // Tool call deltas
                                        if let Some(tool_calls) = choice.delta.tool_calls {
                                            for delta in tool_calls {
                                                // Store ID on first occurrence
                                                if let Some(id) = &delta.id {
                                                    tool_call_index_to_id
                                                        .insert(delta.index, id.clone());
                                                }

                                                // Look up ID by index for all deltas
                                                if let Some(id) =
                                                    tool_call_index_to_id.get(&delta.index)
                                                {
                                                    tool_call_assembler.process_delta(
                                                        id.clone(),
                                                        delta
                                                            .function
                                                            .as_ref()
                                                            .and_then(|f| f.name.clone()),
                                                        delta
                                                            .function
                                                            .as_ref()
                                                            .and_then(|f| f.arguments.clone()),
                                                    );
                                                }
                                            }
                                        }

                                        // Finish reason
                                        if let Some(finish_reason) = choice.finish_reason {
                                            current_finish_reason = Some(finish_reason);
                                        }
                                    }
                                }
                                Err(e) => {
                                    let _ = event_tx.send(Err(ProviderError::ApiError(format!(
                                        "Failed to parse chunk: {}",
                                        e
                                    ))));
                                    return;
                                }
                            }
                        }
                        Err(e) => {
                            let _ = event_tx
                                .send(Err(ProviderError::ApiError(format!("Stream error: {}", e))));
                            return;
                        }
                    }
                }

                // Check if we need to wait for tool results
                if current_finish_reason.as_deref() == Some("tool_calls") {
                    // Wait for tool results from user
                    match tool_result_rx.recv().await {
                        Some(submission) => {
                            // Get the saved tool calls from earlier
                            let tool_calls = completed_tool_calls.take().unwrap_or_default();

                            // Add assistant message with tool calls to history
                            messages.push(ChatMessage::Assistant {
                                content: if content_accumulator.is_empty() {
                                    None
                                } else {
                                    Some(content_accumulator.clone())
                                },
                                tool_calls: Some(
                                    tool_calls
                                        .iter()
                                        .map(|tc| OpenAIToolCall {
                                            id: tc.id.clone(),
                                            tool_type: "function".to_string(),
                                            function: OpenAIFunctionCall {
                                                name: tc.name.clone(),
                                                arguments: serde_json::to_string(&tc.arguments)
                                                    .unwrap_or_default(),
                                            },
                                        })
                                        .collect(),
                                ),
                            });

                            // Update current_history with assistant message
                            current_history.push(Message {
                                role: Role::Assistant,
                                content: content_accumulator.clone(),
                                tool_call_id: None,
                                tool_calls: Some(tool_calls.clone()),
                            });

                            // Signal that we received tool results
                            let result_count = submission.results.len();
                            let _ = event_tx.send(Ok(LoopStep::ToolResultsReceived {
                                count: result_count,
                            }));

                            // Add tool results to history
                            for result in submission.results.clone() {
                                messages.push(ChatMessage::Tool {
                                    content: result.content.clone(),
                                    tool_call_id: result.tool_call_id.clone(),
                                });

                                // Update current_history with tool result
                                current_history.push(Message {
                                    role: Role::Tool,
                                    content: result.content,
                                    tool_call_id: Some(result.tool_call_id),
                                    tool_calls: None,
                                });
                            }

                            // Prune old tool turns if configured
                            if let Some(max_turns) = cfg.max_tool_turns {
                                Self::prune_tool_turns(&mut messages, max_turns);
                                // TODO: Also prune current_history to match
                            }

                            // Reset for next iteration
                            content_accumulator.clear();
                            tool_call_assembler = ToolCallAssembler::new();
                            tool_call_index_to_id.clear();
                            #[allow(unused_assignments)]
                            {
                                current_finish_reason = None;
                            }

                            // Continue the loop to make another request with tool results
                            continue;
                        }
                        None => {
                            // Channel closed, exit loop
                            break;
                        }
                    }
                } else {
                    // Normal completion, exit loop
                    break;
                }
            }

            // Save the final history to provider
            if let Ok(mut hist) = provider_history.write() {
                *hist = current_history;
            }
        });

        Ok(ChatLoopHandle::new(event_rx, tool_result_tx))
    }

    fn prompt_cache(&mut self, _cache_prompt: String) -> Result<(), ProviderError> {
        Err(ProviderError::CachingNotSupported)
    }

    async fn compact(&self, history: Vec<Message>) -> Result<Vec<Message>, ProviderError> {
        // Convert our Message types to Responses API format
        let input: Vec<ResponsesInput> = history
            .iter()
            .map(Self::convert_to_responses_input)
            .collect();

        // Build compact request
        let cfg = self.config();
        let request = ResponsesCompactRequest {
            model: self.model.clone(),
            input,
            instructions: cfg.system_prompt.clone(),
        };

        // Make HTTP request to /responses/compact
        let response = self
            .client
            .post(format!("{}/compact", OPENAI_RESPONSES_API_BASE))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| ProviderError::ApiError(format!("Compact request failed: {}", e)))?;

        // Check status
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(ProviderError::ApiError(format!(
                "Compact API error HTTP {}: {}",
                status, error_text
            )));
        }

        // Parse response
        let compact_response: ResponsesCompactResponse = response.json().await.map_err(|e| {
            ProviderError::ApiError(format!("Failed to parse compact response: {}", e))
        })?;

        // Convert back to our Message format
        // Note: Compacted items will be preserved as opaque data
        let mut compacted_history = Vec::new();
        for input in compact_response.output {
            match Self::convert_from_responses_input(&input) {
                Ok(msg) => compacted_history.push(msg),
                Err(_) => {
                    // This is a compacted item - we can't convert it back
                    // For now, skip it (we'll need to handle this better later)
                    // TODO: Store compacted items separately and pass them through
                    continue;
                }
            }
        }

        Ok(compacted_history)
    }

    fn get_history(&self) -> Vec<Message> {
        self.history.read().map(|h| h.clone()).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_supported_models() {
        // GPT-5 models should be supported
        assert!(OpenAIProvider::is_supported_model("gpt-5-nano"));
        assert!(OpenAIProvider::is_supported_model("gpt-5-turbo"));
        assert!(OpenAIProvider::is_supported_model("gpt-5"));

        // o1 models should be supported
        assert!(OpenAIProvider::is_supported_model("o1"));
        assert!(OpenAIProvider::is_supported_model("o1-preview"));
        assert!(OpenAIProvider::is_supported_model("o1-mini"));

        // gpt-4o models should be supported
        assert!(OpenAIProvider::is_supported_model("gpt-4o"));
        assert!(OpenAIProvider::is_supported_model("gpt-4o-mini"));
    }

    #[test]
    fn test_unsupported_models() {
        // Old models should NOT be supported
        assert!(!OpenAIProvider::is_supported_model("gpt-4"));
        assert!(!OpenAIProvider::is_supported_model("gpt-4-turbo"));
        assert!(!OpenAIProvider::is_supported_model("gpt-3.5-turbo"));
        assert!(!OpenAIProvider::is_supported_model("text-davinci-003"));
    }

    #[test]
    fn test_create_with_unsupported_model() {
        let result = OpenAIProvider::new("gpt-3.5-turbo".to_string(), "test-key".to_string());
        assert!(result.is_err());

        if let Err(ProviderError::ConfigError(msg)) = result {
            assert!(msg.contains("not supported"));
            assert!(msg.contains("GPT-5+"));
        } else {
            panic!("Expected ConfigError");
        }
    }

    #[test]
    fn test_create_with_supported_model() {
        // Should not panic - just verify it creates successfully
        let result = OpenAIProvider::new("gpt-4o".to_string(), "test-key".to_string());
        assert!(result.is_ok());
    }

    #[test]
    fn test_provider_trait_create() {
        // Test through the trait interface
        let result = <OpenAIProvider as LLMProvider>::create(
            "o1-preview".to_string(),
            "test-key".to_string(),
        );
        assert!(result.is_ok());

        let result =
            <OpenAIProvider as LLMProvider>::create("gpt-4".to_string(), "test-key".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn test_prune_tool_turns_no_tools() {
        let mut messages = vec![
            ChatMessage::User {
                content: "Hello".to_string(),
            },
            ChatMessage::Assistant {
                content: Some("Hi".to_string()),
                tool_calls: None,
            },
        ];

        OpenAIProvider::prune_tool_turns(&mut messages, 3);
        assert_eq!(messages.len(), 2); // No changes
    }

    #[test]
    fn test_prune_tool_turns_under_limit() {
        let mut messages = vec![
            ChatMessage::User {
                content: "Hello".to_string(),
            },
            ChatMessage::Assistant {
                content: Some("Calling tool".to_string()),
                tool_calls: Some(vec![OpenAIToolCall {
                    id: "call_1".to_string(),
                    tool_type: "function".to_string(),
                    function: OpenAIFunctionCall {
                        name: "test".to_string(),
                        arguments: "{}".to_string(),
                    },
                }]),
            },
            ChatMessage::Tool {
                content: "result".to_string(),
                tool_call_id: "call_1".to_string(),
            },
            ChatMessage::Assistant {
                content: Some("Done".to_string()),
                tool_calls: None,
            },
        ];

        let original_len = messages.len();
        OpenAIProvider::prune_tool_turns(&mut messages, 3);
        assert_eq!(messages.len(), original_len); // No pruning, only 1 turn
    }

    #[test]
    fn test_prune_tool_turns_exceeds_limit() {
        let mut messages = vec![
            // Turn 1
            ChatMessage::Assistant {
                content: Some("Turn 1".to_string()),
                tool_calls: Some(vec![OpenAIToolCall {
                    id: "call_1".to_string(),
                    tool_type: "function".to_string(),
                    function: OpenAIFunctionCall {
                        name: "test".to_string(),
                        arguments: "{}".to_string(),
                    },
                }]),
            },
            ChatMessage::Tool {
                content: "result 1".to_string(),
                tool_call_id: "call_1".to_string(),
            },
            // Turn 2
            ChatMessage::Assistant {
                content: Some("Turn 2".to_string()),
                tool_calls: Some(vec![OpenAIToolCall {
                    id: "call_2".to_string(),
                    tool_type: "function".to_string(),
                    function: OpenAIFunctionCall {
                        name: "test".to_string(),
                        arguments: "{}".to_string(),
                    },
                }]),
            },
            ChatMessage::Tool {
                content: "result 2".to_string(),
                tool_call_id: "call_2".to_string(),
            },
            // Turn 3
            ChatMessage::Assistant {
                content: Some("Turn 3".to_string()),
                tool_calls: Some(vec![OpenAIToolCall {
                    id: "call_3".to_string(),
                    tool_type: "function".to_string(),
                    function: OpenAIFunctionCall {
                        name: "test".to_string(),
                        arguments: "{}".to_string(),
                    },
                }]),
            },
            ChatMessage::Tool {
                content: "result 3".to_string(),
                tool_call_id: "call_3".to_string(),
            },
            // Turn 4
            ChatMessage::Assistant {
                content: Some("Turn 4".to_string()),
                tool_calls: Some(vec![OpenAIToolCall {
                    id: "call_4".to_string(),
                    tool_type: "function".to_string(),
                    function: OpenAIFunctionCall {
                        name: "test".to_string(),
                        arguments: "{}".to_string(),
                    },
                }]),
            },
            ChatMessage::Tool {
                content: "result 4".to_string(),
                tool_call_id: "call_4".to_string(),
            },
        ];

        OpenAIProvider::prune_tool_turns(&mut messages, 3);

        // Should keep only last 3 turns (turn 2, 3, 4)
        // Turn 1 should be removed
        assert_eq!(messages.len(), 6); // 3 turns * 2 messages each

        // Verify turn 1 is gone
        if let ChatMessage::Assistant {
            content: Some(c), ..
        } = &messages[0]
        {
            assert_eq!(c, "Turn 2");
        } else {
            panic!("Expected Turn 2 to be first");
        }
    }

    #[test]
    fn test_get_history_initially_empty() {
        let provider = OpenAIProvider::new("gpt-4o".to_string(), "test-key".to_string()).unwrap();
        let history = provider.get_history();
        assert_eq!(history.len(), 0);
    }

    #[test]
    fn test_prune_tool_turns_multiple_tool_results() {
        let mut messages = vec![
            // Turn 1: multiple tool calls
            ChatMessage::Assistant {
                content: Some("Turn 1".to_string()),
                tool_calls: Some(vec![
                    OpenAIToolCall {
                        id: "call_1a".to_string(),
                        tool_type: "function".to_string(),
                        function: OpenAIFunctionCall {
                            name: "test".to_string(),
                            arguments: "{}".to_string(),
                        },
                    },
                    OpenAIToolCall {
                        id: "call_1b".to_string(),
                        tool_type: "function".to_string(),
                        function: OpenAIFunctionCall {
                            name: "test".to_string(),
                            arguments: "{}".to_string(),
                        },
                    },
                ]),
            },
            ChatMessage::Tool {
                content: "result 1a".to_string(),
                tool_call_id: "call_1a".to_string(),
            },
            ChatMessage::Tool {
                content: "result 1b".to_string(),
                tool_call_id: "call_1b".to_string(),
            },
            // Turn 2: single tool call
            ChatMessage::Assistant {
                content: Some("Turn 2".to_string()),
                tool_calls: Some(vec![OpenAIToolCall {
                    id: "call_2".to_string(),
                    tool_type: "function".to_string(),
                    function: OpenAIFunctionCall {
                        name: "test".to_string(),
                        arguments: "{}".to_string(),
                    },
                }]),
            },
            ChatMessage::Tool {
                content: "result 2".to_string(),
                tool_call_id: "call_2".to_string(),
            },
        ];

        OpenAIProvider::prune_tool_turns(&mut messages, 1);

        // Should keep only turn 2 (last turn)
        assert_eq!(messages.len(), 2); // 1 assistant + 1 tool result

        if let ChatMessage::Assistant {
            content: Some(c), ..
        } = &messages[0]
        {
            assert_eq!(c, "Turn 2");
        } else {
            panic!("Expected Turn 2");
        }
    }
}
