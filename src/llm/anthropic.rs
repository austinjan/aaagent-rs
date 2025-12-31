// Anthropic Provider implementation using reqwest + SSE streaming
// Supports Claude models including Opus 4.5, Sonnet 3.5/4, and Haiku 3.5

use crate::llm::provider::*;
use eventsource_stream::Eventsource;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::pin::Pin;
use std::sync::{Arc, RwLock};
use tokio::sync::mpsc;

const ANTHROPIC_API_BASE: &str = "https://api.anthropic.com/v1";
const ANTHROPIC_VERSION: &str = "2023-06-01";

#[derive(Clone)]
pub struct AnthropicProvider {
    client: reqwest::Client,
    api_key: String,
    model: String,
    config: Arc<RwLock<ProviderConfig>>,
    state: Arc<RwLock<ProviderState>>,
    history: Arc<RwLock<Vec<Message>>>,
}

impl AnthropicProvider {
    pub fn new(model: String, api_key: String) -> Result<Self, ProviderError> {
        if !Self::is_supported_model(&model) {
            return Err(ProviderError::ConfigError(format!(
                "Model '{}' may not be supported. Continue at your own risk.",
                model
            )));
        }

        if api_key.trim().is_empty() {
            return Err(ProviderError::ConfigError(
                "Anthropic API key must not be empty".to_string(),
            ));
        }

        Ok(Self {
            client: reqwest::Client::new(),
            api_key,
            model,
            config: Arc::new(RwLock::new(ProviderConfig::default())),
            state: Arc::new(RwLock::new(ProviderState::default())),
            history: Arc::new(RwLock::new(Vec::new())),
        })
    }

    fn is_supported_model(model: &str) -> bool {
        // Common Claude models (non-exhaustive list)
        matches!(
            model,
            "claude-opus-4-5-20251101"
                | "claude-sonnet-4-5-20250929"
                | "claude-3-5-haiku-20241022"
                | "claude-3-7-sonnet-20250219"
                | "claude-sonnet-4-20250514"
        ) || model.starts_with("claude-")
    }

    fn build_request_body(
        &self,
        history: &[Message],
        cfg: &ProviderConfig,
        tools: Option<&[Tool]>,
    ) -> (Vec<AnthropicMessage>, Option<AnthropicSystemPrompt>) {
        let mut messages = Vec::new();
        let mut system_prompt: Option<AnthropicSystemPrompt> = None;

        // Extract system prompt from history
        for msg in history {
            match msg.role {
                Role::System => {
                    if system_prompt.is_none() {
                        system_prompt = Some(AnthropicSystemPrompt::Text(msg.content.clone()));
                    }
                }
                Role::User => {
                    // Check if this is a tool result message
                    if msg.tool_call_id.is_some() {
                        // This should be a tool_result content block
                        // We'll handle this differently
                        continue;
                    }

                    messages.push(AnthropicMessage {
                        role: "user".to_string(),
                        content: AnthropicContent::Text(msg.content.clone()),
                    });
                }
                Role::Assistant => {
                    let mut content_blocks = Vec::new();

                    // Add text content if present
                    if !msg.content.is_empty() {
                        content_blocks.push(AnthropicContentBlock::Text {
                            text: msg.content.clone(),
                        });
                    }

                    // Add tool use blocks if present
                    if let Some(tool_calls) = &msg.tool_calls {
                        for call in tool_calls {
                            content_blocks.push(AnthropicContentBlock::ToolUse {
                                id: call.id.clone(),
                                name: call.name.clone(),
                                input: call.arguments.clone(),
                            });
                        }
                    }

                    if !content_blocks.is_empty() {
                        messages.push(AnthropicMessage {
                            role: "assistant".to_string(),
                            content: AnthropicContent::Blocks(content_blocks),
                        });
                    }
                }
                Role::Tool => {
                    // Tool results go in user messages as tool_result blocks
                    if let Some(tool_call_id) = &msg.tool_call_id {
                        messages.push(AnthropicMessage {
                            role: "user".to_string(),
                            content: AnthropicContent::Blocks(vec![
                                AnthropicContentBlock::ToolResult {
                                    tool_use_id: tool_call_id.clone(),
                                    content: msg.content.clone(),
                                    is_error: Some(false),
                                },
                            ]),
                        });
                    }
                }
            }
        }

        (messages, system_prompt)
    }

    fn convert_tools(tools: &[Tool]) -> Vec<AnthropicTool> {
        tools
            .iter()
            .map(|tool| AnthropicTool {
                name: tool.name.clone(),
                description: Some(tool.description.clone()),
                input_schema: tool.parameters.clone(),
            })
            .collect()
    }

    fn update_usage_state(&self, usage: &AnthropicUsage) {
        if let Ok(mut state) = self.state.write() {
            state.input_tokens += usage.input_tokens as u64;
            state.output_tokens += usage.output_tokens as u64;
            state.cached_tokens += (usage.cache_creation_input_tokens.unwrap_or(0)
                + usage.cache_read_input_tokens.unwrap_or(0))
                as u64;
            state.request_count += 1;
            state.last_request_time = Some(std::time::SystemTime::now());
        }
    }

    fn convert_usage(usage: &AnthropicUsage) -> TokenUsage {
        TokenUsage {
            input_tokens: usage.input_tokens,
            output_tokens: usage.output_tokens,
            cached_tokens: usage.cache_creation_input_tokens.unwrap_or(0)
                + usage.cache_read_input_tokens.unwrap_or(0),
        }
    }

    fn parse_finish_reason(reason: Option<String>) -> FinishReason {
        match reason.as_deref() {
            Some("end_turn") => FinishReason::Stop,
            Some("max_tokens") => FinishReason::Length,
            Some("tool_use") => FinishReason::ToolCalls,
            Some("stop_sequence") => FinishReason::Stop,
            Some(other) => FinishReason::Other(other.to_string()),
            None => FinishReason::Stop,
        }
    }

    fn build_create_message_request(
        &self,
        messages: Vec<AnthropicMessage>,
        system: Option<AnthropicSystemPrompt>,
        tools: Option<Vec<AnthropicTool>>,
        cfg: &ProviderConfig,
        stream: bool,
    ) -> CreateMessageRequest {
        CreateMessageRequest {
            model: self.model.clone(),
            max_tokens: cfg.max_tokens,
            messages,
            system,
            temperature: Some(1.0),
            top_p: None,
            top_k: None,
            stop_sequences: None,
            stream: Some(stream),
            tool_choice: if tools.is_some() {
                Some(AnthropicToolChoice::Auto)
            } else {
                None
            },
            tools,
            thinking: None,
            metadata: None,
        }
    }
}

#[async_trait::async_trait]
impl LLMProvider for AnthropicProvider {
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
        let mut history = Vec::new();

        if let Some(system_prompt) = &cfg.system_prompt {
            history.push(Message {
                role: Role::System,
                content: system_prompt.clone(),
                tool_call_id: None,
                tool_calls: None,
            });
        }

        history.push(Message {
            role: Role::User,
            content: prompt.to_string(),
            tool_call_id: None,
            tool_calls: None,
        });

        let (messages, system) = self.build_request_body(&history, &cfg, None);
        let request_body = self.build_create_message_request(messages, system, None, &cfg, true);

        let response = self
            .client
            .post(format!("{}/messages", ANTHROPIC_API_BASE))
            .header("Content-Type", "application/json")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .json(&request_body)
            .send()
            .await
            .map_err(|e| ProviderError::ApiError(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(ProviderError::ApiError(format!(
                "Anthropic HTTP {}: {}",
                status, text
            )));
        }

        let usage_state = self.state.clone();
        let byte_stream = response.bytes_stream();
        let event_stream = byte_stream.eventsource();

        let output_stream = async_stream::stream! {
            let mut full_content = String::new();
            futures::pin_mut!(event_stream);

            while let Some(event_result) = event_stream.next().await {
                match event_result {
                    Ok(event) => {
                        if event.event == "message_stop" {
                            break;
                        }

                        if event.data.trim().is_empty() {
                            continue;
                        }

                        match event.event.as_str() {
                            "content_block_delta" => {
                                if let Ok(delta) = serde_json::from_str::<ContentBlockDelta>(&event.data) {
                                    match delta.delta {
                                        ContentDelta::TextDelta { text } => {
                                            full_content.push_str(&text);
                                            yield Ok(StreamChunk::Content(text));
                                        }
                                        _ => {}
                                    }
                                }
                            }
                            "message_delta" => {
                                if let Ok(msg_delta) = serde_json::from_str::<MessageDelta>(&event.data) {
                                    if let Some(usage) = msg_delta.usage {
                                        if let Ok(mut state) = usage_state.write() {
                                            state.output_tokens += usage.output_tokens as u64;
                                        }

                                        yield Ok(StreamChunk::Done {
                                            finish_reason: Self::parse_finish_reason(msg_delta.delta.stop_reason),
                                            usage: TokenUsage {
                                                input_tokens: 0,
                                                output_tokens: usage.output_tokens,
                                                cached_tokens: 0,
                                            },
                                            full_content: full_content.clone(),
                                        });
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    Err(err) => {
                        yield Err(ProviderError::ApiError(format!(
                            "Anthropic stream error: {}",
                            err
                        )));
                        break;
                    }
                }
            }
        };

        Ok(Box::pin(output_stream))
    }

    async fn chat_loop(
        &self,
        mut history: Vec<Message>,
        tools: Option<Vec<Tool>>,
    ) -> Result<ChatLoopHandle, ProviderError> {
        let (event_tx, event_rx) = mpsc::unbounded_channel::<Result<LoopStep, ProviderError>>();
        let (command_tx, mut command_rx) = mpsc::unbounded_channel::<ChatLoopCommand>();

        let client = self.client.clone();
        let api_key = self.api_key.clone();
        let model = self.model.clone();
        let cfg = self.config();
        let history_store = self.history.clone();
        let provider_clone = self.clone();

        tokio::spawn(async move {
            let mut accumulated_usage = TokenUsage::default();
            let _all_tool_calls = Vec::<ToolCall>::new();
            let tools_opt = tools;

            loop {
                // Apply pruning if needed
                if let Some(max_turns) = cfg.max_tool_turns {
                    if max_turns > 0 {
                        Self::prune_message_tool_turns(&mut history, max_turns);
                    }
                }

                let (messages, system) =
                    provider_clone.build_request_body(&history, &cfg, tools_opt.as_deref());

                let anthropic_tools = tools_opt.as_ref().map(|t| Self::convert_tools(t));

                let request_body = CreateMessageRequest {
                    model: model.clone(),
                    max_tokens: cfg.max_tokens,
                    messages,
                    system,
                    temperature: Some(1.0),
                    top_p: None,
                    top_k: None,
                    stop_sequences: None,
                    stream: Some(true),
                    tools: anthropic_tools,
                    tool_choice: if tools_opt.is_some() {
                        Some(AnthropicToolChoice::Auto)
                    } else {
                        None
                    },
                    thinking: None,
                    metadata: None,
                };

                let response = match client
                    .post(format!("{}/messages", ANTHROPIC_API_BASE))
                    .header("Content-Type", "application/json")
                    .header("x-api-key", &api_key)
                    .header("anthropic-version", ANTHROPIC_VERSION)
                    .json(&request_body)
                    .send()
                    .await
                {
                    Ok(resp) => resp,
                    Err(err) => {
                        let _ = event_tx.send(Err(ProviderError::ApiError(err.to_string())));
                        break;
                    }
                };

                if !response.status().is_success() {
                    let status = response.status();
                    let text = response
                        .text()
                        .await
                        .unwrap_or_else(|_| "Unknown error".to_string());
                    let _ = event_tx.send(Err(ProviderError::ApiError(format!(
                        "Anthropic HTTP {}: {}",
                        status, text
                    ))));
                    break;
                }

                let byte_stream = response.bytes_stream();
                let event_stream = byte_stream.eventsource();

                futures::pin_mut!(event_stream);

                let mut content_accumulator = String::new();
                let mut pending_tool_calls: Vec<ToolCall> = Vec::new();
                let mut finish_reason: Option<String> = None;
                let mut loop_usage: Option<AnthropicUsage> = None;
                let mut current_tool_input: Option<(String, String, String)> = None; // (id, name, partial_json)

                while let Some(event_result) = event_stream.next().await {
                    match event_result {
                        Ok(event) => {
                            if event.event == "message_stop" {
                                break;
                            }

                            if event.data.trim().is_empty() {
                                continue;
                            }

                            match event.event.as_str() {
                                "message_start" => {
                                    if let Ok(msg_start) =
                                        serde_json::from_str::<MessageStart>(&event.data)
                                    {
                                        loop_usage = Some(msg_start.message.usage);
                                    }
                                }
                                "content_block_start" => {
                                    if let Ok(block_start) =
                                        serde_json::from_str::<ContentBlockStart>(&event.data)
                                    {
                                        match block_start.content_block {
                                            AnthropicContentBlock::ToolUse { id, name, input } => {
                                                // Start collecting tool use
                                                let input_str = serde_json::to_string(&input)
                                                    .unwrap_or_else(|_| "{}".to_string());
                                                current_tool_input = Some((id, name, input_str));
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                                "content_block_delta" => {
                                    if let Ok(delta) =
                                        serde_json::from_str::<ContentBlockDelta>(&event.data)
                                    {
                                        match delta.delta {
                                            ContentDelta::TextDelta { text } => {
                                                content_accumulator.push_str(&text);
                                                let _ = event_tx.send(Ok(LoopStep::Content(text)));
                                            }
                                            ContentDelta::ThinkingDelta { thinking } => {
                                                let _ =
                                                    event_tx.send(Ok(LoopStep::Thinking(thinking)));
                                            }
                                            ContentDelta::InputJsonDelta { partial_json } => {
                                                // Accumulate tool input JSON
                                                if let Some((_, _, ref mut json)) =
                                                    current_tool_input
                                                {
                                                    json.push_str(&partial_json);
                                                }
                                            }
                                        }
                                    }
                                }
                                "content_block_stop" => {
                                    // Content block finished - finalize tool use if present
                                    if let Some((id, name, json)) = current_tool_input.take() {
                                        let input = serde_json::from_str(&json)
                                            .unwrap_or(serde_json::json!({}));

                                        pending_tool_calls.push(ToolCall {
                                            id,
                                            name,
                                            arguments: input,
                                        });
                                    }
                                }
                                "message_delta" => {
                                    if let Ok(msg_delta) =
                                        serde_json::from_str::<MessageDelta>(&event.data)
                                    {
                                        finish_reason = msg_delta.delta.stop_reason;

                                        if let Some(usage) = msg_delta.usage {
                                            if let Some(ref mut total_usage) = loop_usage {
                                                total_usage.output_tokens = usage.output_tokens;
                                            }
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                        Err(err) => {
                            log::error!("❌ Anthropic stream error: {}", err);
                            let _ = event_tx.send(Err(ProviderError::ApiError(format!(
                                "Anthropic stream error: {}",
                                err
                            ))));
                            return;
                        }
                    }
                }

                // Update usage
                if let Some(usage) = loop_usage {
                    provider_clone.update_usage_state(&usage);
                    let token_usage = Self::convert_usage(&usage);
                    accumulated_usage.input_tokens += token_usage.input_tokens;
                    accumulated_usage.output_tokens += token_usage.output_tokens;
                    accumulated_usage.cached_tokens += token_usage.cached_tokens;
                }

                // Handle tool calls if present
                if !pending_tool_calls.is_empty() {
                    // Add assistant message with tool calls
                    history.push(Message {
                        role: Role::Assistant,
                        content: content_accumulator.clone(),
                        tool_call_id: None,
                        tool_calls: Some(pending_tool_calls.clone()),
                    });

                    // Notify that tool calls are requested
                    let _ = event_tx.send(Ok(LoopStep::ToolCallsRequested {
                        tool_calls: pending_tool_calls.clone(),
                        content: content_accumulator.clone(),
                    }));

                    // Wait for tool results
                    match command_rx.recv().await {
                        Some(ChatLoopCommand::SubmitToolResults(results)) => {
                            let _ = event_tx.send(Ok(LoopStep::ToolResultsReceived {
                                count: results.len(),
                            }));

                            // Add tool results to history
                            for result in results {
                                history.push(Message {
                                    role: Role::Tool,
                                    content: result.content.clone(),
                                    tool_call_id: Some(result.tool_call_id.clone()),
                                    tool_calls: None,
                                });
                            }

                            // Continue the loop for next turn
                            continue;
                        }
                        Some(ChatLoopCommand::UpdateTools(_new_tools)) => {
                            // Anthropic doesn't support dynamic tool updates in the same way
                            // Tools are specified per-request, not per-session
                            continue;
                        }
                        None => {
                            break;
                        }
                    }
                } else {
                    // No tool calls - conversation is done
                    history.push(Message {
                        role: Role::Assistant,
                        content: content_accumulator.clone(),
                        tool_call_id: None,
                        tool_calls: None,
                    });

                    let finish = Self::parse_finish_reason(finish_reason);

                    let _ = event_tx.send(Ok(LoopStep::Done {
                        content: content_accumulator,
                        finish_reason: finish,
                        total_usage: accumulated_usage.clone(),
                        all_tool_calls: Vec::new(),
                    }));
                    break;
                }
            }

            if let Ok(mut store) = history_store.write() {
                *store = history;
            }
        });

        Ok(ChatLoopHandle::new(event_rx, command_tx))
    }

    fn prompt_cache(&mut self, _cache_prompt: String) -> Result<(), ProviderError> {
        // TODO: Implement prompt caching
        Err(ProviderError::CachingNotSupported)
    }

    async fn compact(&self, _history: Vec<Message>) -> Result<Vec<Message>, ProviderError> {
        Err(ProviderError::ApiError(
            "Anthropic compact not implemented".to_string(),
        ))
    }

    fn get_history(&self) -> Vec<Message> {
        self.history.read().map(|h| h.clone()).unwrap_or_default()
    }
}

impl AnthropicProvider {
    fn prune_message_tool_turns(history: &mut Vec<Message>, max_turns: usize) {
        if max_turns == 0 {
            return;
        }

        let mut tool_turn_ranges: Vec<(usize, usize)> = Vec::new();
        let mut i = 0;

        while i < history.len() {
            if let Message {
                role: Role::Assistant,
                tool_calls: Some(_),
                ..
            } = &history[i]
            {
                let start = i;
                i += 1;

                while i < history.len() {
                    if matches!(
                        history[i],
                        Message {
                            role: Role::Tool,
                            ..
                        }
                    ) {
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

        if tool_turn_ranges.len() > max_turns {
            let to_remove = tool_turn_ranges.len() - max_turns;

            for &(start, end) in tool_turn_ranges.iter().take(to_remove).rev() {
                history.drain(start..end);
            }
        }
    }
}

// ============================== API types ==============================

#[derive(Debug, Serialize)]
struct CreateMessageRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<AnthropicMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<AnthropicSystemPrompt>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_k: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop_sequences: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<AnthropicTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<AnthropicToolChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    thinking: Option<AnthropicThinking>,
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
enum AnthropicSystemPrompt {
    Text(String),
    Blocks(Vec<AnthropicSystemBlock>),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct AnthropicSystemBlock {
    #[serde(rename = "type")]
    block_type: String,
    text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    cache_control: Option<CacheControl>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct CacheControl {
    #[serde(rename = "type")]
    cache_type: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct AnthropicMessage {
    role: String,
    content: AnthropicContent,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
enum AnthropicContent {
    Text(String),
    Blocks(Vec<AnthropicContentBlock>),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type")]
enum AnthropicContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image")]
    Image { source: ImageSource },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
    },
    #[serde(rename = "thinking")]
    Thinking { thinking: String },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct ImageSource {
    #[serde(rename = "type")]
    source_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    media_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct AnthropicTool {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    input_schema: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type")]
enum AnthropicToolChoice {
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "any")]
    Any {
        #[serde(skip_serializing_if = "Option::is_none")]
        disable_parallel_tool_use: Option<bool>,
    },
    #[serde(rename = "tool")]
    Tool { name: String },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct AnthropicThinking {
    #[serde(rename = "type")]
    thinking_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    budget_tokens: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct CreateMessageResponse {
    id: String,
    #[serde(rename = "type")]
    response_type: String,
    role: String,
    content: Vec<AnthropicContentBlock>,
    model: String,
    stop_reason: Option<String>,
    usage: AnthropicUsage,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct AnthropicUsage {
    input_tokens: u32,
    output_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    cache_creation_input_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cache_read_input_tokens: Option<u32>,
}

// Streaming event types
#[derive(Debug, Deserialize)]
struct MessageStart {
    message: MessageStartContent,
}

#[derive(Debug, Deserialize)]
struct MessageStartContent {
    id: String,
    usage: AnthropicUsage,
}

#[derive(Debug, Deserialize)]
struct ContentBlockStart {
    index: usize,
    content_block: AnthropicContentBlock,
}

#[derive(Debug, Deserialize)]
struct ContentBlockDelta {
    index: usize,
    delta: ContentDelta,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum ContentDelta {
    #[serde(rename = "text_delta")]
    TextDelta { text: String },
    #[serde(rename = "thinking_delta")]
    ThinkingDelta { thinking: String },
    #[serde(rename = "input_json_delta")]
    InputJsonDelta { partial_json: String },
}

#[derive(Debug, Deserialize)]
struct MessageDelta {
    delta: MessageDeltaContent,
    #[serde(skip_serializing_if = "Option::is_none")]
    usage: Option<AnthropicUsageDelta>,
}

#[derive(Debug, Deserialize)]
struct MessageDeltaContent {
    stop_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop_sequence: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AnthropicUsageDelta {
    output_tokens: u32,
}
