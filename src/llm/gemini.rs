// Gemini Provider implementation using reqwest + SSE streaming
// Supports gemini-3-pro-preview and gemini-3-flash-preview models

use crate::llm::provider::*;
use eventsource_stream::Eventsource;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use tokio::sync::mpsc;

const GEMINI_API_BASE: &str = "https://generativelanguage.googleapis.com/v1beta";

/// Metadata for tracked tool calls
#[derive(Debug, Clone)]
struct GeminiToolCallMeta {
    signature: Option<String>,
    function_name: String,
}

#[derive(Clone)]
pub struct GeminiProvider {
    client: reqwest::Client,
    api_key: String,
    model: String,
    config: Arc<RwLock<ProviderConfig>>,
    state: Arc<RwLock<ProviderState>>,
    history: Arc<RwLock<Vec<Message>>>,
    tool_call_meta: Arc<RwLock<HashMap<String, GeminiToolCallMeta>>>,
    call_counter: Arc<AtomicU64>,
}

impl GeminiProvider {
    pub fn new(model: String, api_key: String) -> Result<Self, ProviderError> {
        if !Self::is_supported_model(&model) {
            return Err(ProviderError::ConfigError(format!(
                "Model '{}' is not supported. Supported models: gemini-3-pro-preview, gemini-3-flash-preview",
                model
            )));
        }

        if api_key.trim().is_empty() {
            return Err(ProviderError::ConfigError(
                "Gemini API key must not be empty".to_string(),
            ));
        }

        Ok(Self {
            client: reqwest::Client::new(),
            api_key,
            model,
            config: Arc::new(RwLock::new(ProviderConfig::default())),
            state: Arc::new(RwLock::new(ProviderState::default())),
            history: Arc::new(RwLock::new(Vec::new())),
            tool_call_meta: Arc::new(RwLock::new(HashMap::new())),
            call_counter: Arc::new(AtomicU64::new(1)),
        })
    }

    fn is_supported_model(model: &str) -> bool {
        matches!(model, "gemini-3-pro-preview" | "gemini-3-flash-preview")
    }

    fn next_call_id(&self) -> String {
        let idx = self.call_counter.fetch_add(1, Ordering::SeqCst);
        format!("gemini_call_{}", idx)
    }

    fn register_tool_call(&self, call_id: String, meta: GeminiToolCallMeta) {
        if let Ok(mut map) = self.tool_call_meta.write() {
            map.insert(call_id, meta);
        }
    }

    fn get_tool_signature(&self, call_id: &str) -> Option<GeminiToolCallMeta> {
        self.tool_call_meta
            .read()
            .ok()
            .and_then(|m| m.get(call_id).cloned())
    }

    fn build_request_body(
        &self,
        history: &[Message],
        _cfg: &ProviderConfig,
        tools: Option<&[Tool]>,
    ) -> (
        Vec<GeminiContent>,
        Option<GeminiContent>,
        Option<Vec<GeminiTool>>,
    ) {
        let mut contents = Vec::new();
        let mut system_instruction: Option<GeminiContent> = None;

        for msg in history {
            match msg.role {
                Role::System => {
                    if system_instruction.is_none() {
                        system_instruction = Some(GeminiContent {
                            role: None,
                            parts: vec![GeminiPart {
                                thought: None,
                                thought_signature: None,
                                text: Some(msg.content.clone()),
                                ..Default::default()
                            }],
                        });
                    }
                }
                Role::User => {
                    contents.push(GeminiContent {
                        role: Some("user".to_string()),
                        parts: vec![GeminiPart {
                            thought: None,
                            thought_signature: None,
                            text: Some(msg.content.clone()),
                            ..Default::default()
                        }],
                    });
                }
                Role::Assistant => {
                    let mut parts = Vec::new();
                    if !msg.content.is_empty() {
                        parts.push(GeminiPart {
                            thought: None,
                            thought_signature: None,
                            text: Some(msg.content.clone()),
                            ..Default::default()
                        });
                    }

                    if let Some(tool_calls) = &msg.tool_calls {
                        for call in tool_calls {
                            let signature = self
                                .get_tool_signature(&call.id)
                                .and_then(|meta| meta.signature);

                            parts.push(GeminiPart {
                                thought: None,
                                thought_signature: signature,
                                function_call: Some(GeminiFunctionCall {
                                    name: call.name.clone(),
                                    args: call.arguments.clone(),
                                }),
                                ..Default::default()
                            });
                        }
                    }

                    contents.push(GeminiContent {
                        role: Some("model".to_string()),
                        parts,
                    });
                }
                Role::Tool => {
                    if let Some(call_id) = &msg.tool_call_id {
                        let name = self
                            .get_tool_signature(call_id)
                            .map(|meta| meta.function_name)
                            .unwrap_or_else(|| "tool".to_string());

                        let response_value = Self::parse_tool_response_json(&msg.content);

                        contents.push(GeminiContent {
                            role: Some("user".to_string()),
                            parts: vec![GeminiPart {
                                function_response: Some(GeminiFunctionResponse {
                                    name,
                                    response: response_value,
                                }),
                                ..Default::default()
                            }],
                        });
                    }
                }
            }
        }

        let gemini_tools = tools.map(|t| Self::convert_tools(t));

        (contents, system_instruction, gemini_tools)
    }

    fn convert_tools(tools: &[Tool]) -> Vec<GeminiTool> {
        vec![GeminiTool {
            function_declarations: tools
                .iter()
                .map(|tool| GeminiFunctionDeclaration {
                    name: tool.name.clone(),
                    description: Some(tool.description.clone()),
                    parameters: tool.parameters.clone(),
                })
                .collect(),
        }]
    }

    fn parse_tool_response_json(payload: &str) -> serde_json::Value {
        serde_json::from_str(payload).unwrap_or_else(|_| {
            serde_json::json!({
                "result": payload
            })
        })
    }

    fn update_usage_state(&self, usage: &UsageMetadata) {
        if let Ok(mut state) = self.state.write() {
            state.input_tokens += usage.prompt_token_count.unwrap_or(0) as u64;
            state.output_tokens += usage.candidates_token_count.unwrap_or(0) as u64;
            state.cached_tokens += usage.cached_content_token_count.unwrap_or(0) as u64;
            state.request_count += 1;
            state.last_request_time = Some(std::time::SystemTime::now());
        }
    }

    fn convert_usage(usage: &UsageMetadata) -> TokenUsage {
        TokenUsage {
            input_tokens: usage.prompt_token_count.unwrap_or(0),
            output_tokens: usage.candidates_token_count.unwrap_or(0),
            cached_tokens: usage.cached_content_token_count.unwrap_or(0),
        }
    }

    fn build_generation_config(cfg: &ProviderConfig) -> Option<GeminiGenerationConfig> {
        Some(GeminiGenerationConfig {
            temperature: Some(1.0), // Gemini 3 prefers temperature 1.0
            max_output_tokens: Some(cfg.max_tokens),
            thinking_config: cfg.extra_options.get("thinking_level").and_then(|value| {
                value.as_str().map(|level| GeminiThinkingConfig {
                    thinking_level: level.to_string(),
                })
            }),
        })
    }

    fn parse_candidate_parts(&self, parts: Vec<GeminiPart>) -> (String, Vec<ToolCall>) {
        let mut content = String::new();
        let mut tool_calls = Vec::new();

        for part in parts {
            if let Some(text) = part.text {
                content.push_str(&text);
            }

            if let Some(function_call) = part.function_call {
                let call_id = self.next_call_id();
                let signature = part.thought_signature.clone();

                self.register_tool_call(
                    call_id.clone(),
                    GeminiToolCallMeta {
                        signature: signature.clone(),
                        function_name: function_call.name.clone(),
                    },
                );

                tool_calls.push(ToolCall {
                    id: call_id.clone(),
                    name: function_call.name,
                    arguments: function_call.args,
                });
            }
        }

        (content, tool_calls)
    }

    fn parse_finish_reason(reason: Option<String>) -> FinishReason {
        match reason.as_deref() {
            Some("STOP") => FinishReason::Stop,
            Some("MAX_TOKENS") => FinishReason::Length,
            Some("TOOL_CALLS") => FinishReason::ToolCalls,
            Some("MISSING_THOUGHT_SIGNATURE") => {
                FinishReason::Other("missing_thought_signature".to_string())
            }
            Some(other) => FinishReason::Other(other.to_string()),
            None => FinishReason::Stop,
        }
    }

    fn build_stream_request(
        &self,
        contents: Vec<GeminiContent>,
        system_instruction: Option<GeminiContent>,
        tools: Option<Vec<GeminiTool>>,
        cfg: &ProviderConfig,
    ) -> GenerateContentRequest {
        let tool_config = tools.as_ref().map(|_| GeminiToolConfig {
            function_calling_config: Some(GeminiFunctionCallingConfig {
                mode: "AUTO".to_string(),
            }),
        });

        GenerateContentRequest {
            contents,
            tools,
            system_instruction,
            generation_config: Self::build_generation_config(cfg),
            tool_config,
        }
    }
}

#[async_trait::async_trait]
impl LLMProvider for GeminiProvider {
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

        let (contents, system_instruction, _) = self.build_request_body(&history, &cfg, None);
        let request_body = self.build_stream_request(contents, system_instruction, None, &cfg);

        let response = self
            .client
            .post(format!(
                "{}/models/{}:streamGenerateContent?alt=sse",
                GEMINI_API_BASE, self.model
            ))
            .header("Content-Type", "application/json")
            .header("x-goog-api-key", &self.api_key)
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
                "Gemini HTTP {}: {}",
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
                        if event.data.trim().is_empty() || event.data == "[DONE]" {
                            continue;
                        }

                        match serde_json::from_str::<GenerateContentResponse>(&event.data) {
                            Ok(resp) => {
                                if let Some(candidates) = resp.candidates {
                                    if let Some(candidate) = candidates.into_iter().next() {
                                        if let Some(content) = candidate.content {
                                            for part in content.parts {
                                                if let Some(text) = part.text {
                                                    full_content.push_str(&text);
                                                    yield Ok(StreamChunk::Content(text));
                                                }
                                            }
                                        }

                                        if let Some(usage) = resp.usage_metadata {
                                            if let Ok(mut state) = usage_state.write() {
                                                state.input_tokens +=
                                                    usage.prompt_token_count.unwrap_or(0) as u64;
                                                state.output_tokens +=
                                                    usage.candidates_token_count.unwrap_or(0) as u64;
                                                state.cached_tokens +=
                                                    usage.cached_content_token_count.unwrap_or(0)
                                                        as u64;
                                                state.request_count += 1;
                                                state.last_request_time =
                                                    Some(std::time::SystemTime::now());
                                            }

                                            yield Ok(StreamChunk::Done {
                                                finish_reason: Self::parse_finish_reason(
                                                    candidate.finish_reason,
                                                ),
                                                usage: Self::convert_usage(&usage),
                                                full_content: full_content.clone(),
                                            });
                                        }
                                    }
                                }
                            }
                            Err(err) => {
                                yield Err(ProviderError::ApiError(format!(
                                    "Failed to parse Gemini chunk: {}",
                                    err
                                )));
                                break;
                            }
                        }
                    }
                    Err(err) => {
                        yield Err(ProviderError::ApiError(format!(
                            "Gemini stream error: {}",
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
        let (tool_result_tx, mut tool_result_rx) =
            mpsc::unbounded_channel::<ToolResultSubmission>();

        let client = self.client.clone();
        let api_key = self.api_key.clone();
        let model = self.model.clone();
        let cfg = self.config();
        let history_store = self.history.clone();
        let provider_clone = self.clone();

        tokio::spawn(async move {
            let mut accumulated_usage = TokenUsage::default();
            let mut all_tool_calls = Vec::new();

            let tools_opt = tools;

            loop {
                // Apply pruning if needed
                if let Some(max_turns) = cfg.max_tool_turns {
                    if max_turns > 0 {
                        Self::prune_message_tool_turns(&mut history, max_turns);
                    }
                }

                let (contents, system_instruction, gemini_tools) =
                    provider_clone.build_request_body(&history, &cfg, tools_opt.as_deref());

                let request_body = provider_clone.build_stream_request(
                    contents,
                    system_instruction,
                    gemini_tools,
                    &cfg,
                );

                let response = match client
                    .post(format!(
                        "{}/models/{}:streamGenerateContent?alt=sse",
                        GEMINI_API_BASE, model
                    ))
                    .header("Content-Type", "application/json")
                    .header("x-goog-api-key", &api_key)
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
                        "Gemini HTTP {}: {}",
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
                let mut loop_usage: Option<UsageMetadata> = None;

                while let Some(event_result) = event_stream.next().await {
                    match event_result {
                        Ok(event) => {
                            if event.data.trim().is_empty() || event.data == "[DONE]" {
                                continue;
                            }

                            match serde_json::from_str::<GenerateContentResponse>(&event.data) {
                                Ok(resp) => {
                                    if let Some(usage) = resp.usage_metadata {
                                        loop_usage = Some(usage);
                                    }

                                    if let Some(candidates) = resp.candidates.clone() {
                                        if let Some(candidate) = candidates.into_iter().next() {
                                            finish_reason = candidate.finish_reason.clone();

                                            if let Some(content) = candidate.content {
                                                let (delta_text, new_calls) = provider_clone
                                                    .parse_candidate_parts(content.parts);

                                                if !delta_text.is_empty() {
                                                    content_accumulator.push_str(&delta_text);
                                                    let _ = event_tx
                                                        .send(Ok(LoopStep::Content(delta_text)));
                                                }

                                                if !new_calls.is_empty() {
                                                    pending_tool_calls.extend(new_calls);
                                                }
                                            }
                                        }
                                    }

                                    if !pending_tool_calls.is_empty() {
                                        break;
                                    }
                                }
                                Err(err) => {
                                    let _ = event_tx.send(Err(ProviderError::ApiError(format!(
                                        "Gemini parse error: {}",
                                        err
                                    ))));
                                    return;
                                }
                            }
                        }
                        Err(err) => {
                            let _ = event_tx.send(Err(ProviderError::ApiError(format!(
                                "Gemini stream error: {}",
                                err
                            ))));
                            return;
                        }
                    }
                }

                if let Some(usage) = loop_usage {
                    provider_clone.update_usage_state(&usage);
                    let token_usage = Self::convert_usage(&usage);
                    accumulated_usage.input_tokens += token_usage.input_tokens;
                    accumulated_usage.output_tokens += token_usage.output_tokens;
                    accumulated_usage.cached_tokens += token_usage.cached_tokens;
                }

                if !pending_tool_calls.is_empty() {
                    all_tool_calls.extend(pending_tool_calls.clone());

                    history.push(Message {
                        role: Role::Assistant,
                        content: content_accumulator.clone(),
                        tool_call_id: None,
                        tool_calls: Some(pending_tool_calls.clone()),
                    });

                    let _ = event_tx.send(Ok(LoopStep::ToolCallsRequested {
                        tool_calls: pending_tool_calls.clone(),
                        content: content_accumulator.clone(),
                    }));

                    match tool_result_rx.recv().await {
                        Some(submission) => {
                            let _ = event_tx.send(Ok(LoopStep::ToolResultsReceived {
                                count: submission.results.len(),
                            }));

                            for result in submission.results {
                                history.push(Message {
                                    role: Role::Tool,
                                    content: result.content.clone(),
                                    tool_call_id: Some(result.tool_call_id.clone()),
                                    tool_calls: None,
                                });
                            }

                            continue;
                        }
                        None => {
                            break;
                        }
                    }
                } else {
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
                        all_tool_calls: all_tool_calls.clone(),
                    }));
                    break;
                }
            }

            if let Ok(mut store) = history_store.write() {
                *store = history;
            }
        });

        Ok(ChatLoopHandle::new(event_rx, tool_result_tx))
    }

    fn prompt_cache(&mut self, _cache_prompt: String) -> Result<(), ProviderError> {
        Err(ProviderError::CachingNotSupported)
    }

    async fn compact(&self, _history: Vec<Message>) -> Result<Vec<Message>, ProviderError> {
        Err(ProviderError::ApiError(
            "Gemini compact not implemented".to_string(),
        ))
    }

    fn get_history(&self) -> Vec<Message> {
        self.history.read().map(|h| h.clone()).unwrap_or_default()
    }
}

impl GeminiProvider {
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
struct GenerateContentRequest {
    contents: Vec<GeminiContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<GeminiTool>>,
    #[serde(rename = "systemInstruction", skip_serializing_if = "Option::is_none")]
    system_instruction: Option<GeminiContent>,
    #[serde(rename = "generationConfig", skip_serializing_if = "Option::is_none")]
    generation_config: Option<GeminiGenerationConfig>,
    #[serde(rename = "toolConfig", skip_serializing_if = "Option::is_none")]
    tool_config: Option<GeminiToolConfig>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct GeminiContent {
    #[serde(skip_serializing_if = "Option::is_none")]
    role: Option<String>,
    parts: Vec<GeminiPart>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
struct GeminiPart {
    #[serde(rename = "thought", skip_serializing_if = "Option::is_none")]
    thought: Option<bool>,
    #[serde(rename = "thoughtSignature", skip_serializing_if = "Option::is_none")]
    thought_signature: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,
    #[serde(rename = "functionCall", skip_serializing_if = "Option::is_none")]
    function_call: Option<GeminiFunctionCall>,
    #[serde(rename = "functionResponse", skip_serializing_if = "Option::is_none")]
    function_response: Option<GeminiFunctionResponse>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct GeminiFunctionCall {
    name: String,
    #[serde(default)]
    args: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct GeminiFunctionResponse {
    name: String,
    response: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct GeminiTool {
    #[serde(rename = "functionDeclarations")]
    function_declarations: Vec<GeminiFunctionDeclaration>,
}

#[derive(Debug, Serialize)]
struct GeminiFunctionDeclaration {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    parameters: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct GeminiToolConfig {
    #[serde(
        rename = "functionCallingConfig",
        skip_serializing_if = "Option::is_none"
    )]
    function_calling_config: Option<GeminiFunctionCallingConfig>,
}

#[derive(Debug, Serialize)]
struct GeminiFunctionCallingConfig {
    mode: String,
}

#[derive(Debug, Serialize)]
struct GeminiGenerationConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(rename = "maxOutputTokens", skip_serializing_if = "Option::is_none")]
    max_output_tokens: Option<u32>,
    #[serde(rename = "thinkingConfig", skip_serializing_if = "Option::is_none")]
    thinking_config: Option<GeminiThinkingConfig>,
}

#[derive(Debug, Serialize)]
struct GeminiThinkingConfig {
    #[serde(rename = "thinkingLevel")]
    thinking_level: String,
}

#[derive(Debug, Deserialize)]
struct GenerateContentResponse {
    #[serde(default)]
    candidates: Option<Vec<GeminiCandidate>>,
    #[serde(rename = "usageMetadata")]
    usage_metadata: Option<UsageMetadata>,
}

#[derive(Debug, Deserialize, Clone)]
struct GeminiCandidate {
    content: Option<GeminiContent>,
    #[serde(rename = "finishReason")]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
struct UsageMetadata {
    #[serde(rename = "promptTokenCount")]
    prompt_token_count: Option<u32>,
    #[serde(rename = "cachedContentTokenCount")]
    cached_content_token_count: Option<u32>,
    #[serde(rename = "candidatesTokenCount")]
    candidates_token_count: Option<u32>,
}
