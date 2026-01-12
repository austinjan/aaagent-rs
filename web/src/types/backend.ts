// Backend data types matching Rust structures

// ============================================================================
// Message and Node Types
// ============================================================================

export type NodeId = string; // ULID
export type SessionId = string; // ULID

export const Role = {
  System: "System",
  User: "User",
  Assistant: "Assistant",
  Tool: "Tool",
} as const;

export type Role = (typeof Role)[keyof typeof Role];

export const NodeKind = {
  Root: "Root",
  Message: "Message",
  Tool: "Tool",
} as const;

export type NodeKind = (typeof NodeKind)[keyof typeof NodeKind];

export const ContentType = {
  Text: "Text",
  Json: "Json",
  Markdown: "Markdown",
  Base64: "Base64",
} as const;

export type ContentType = (typeof ContentType)[keyof typeof ContentType];

export interface NodeFlags {
  important: boolean;
  ephemeral: boolean;
  hidden: boolean;
}

export interface ToolCall {
  id: string;
  name: string;
  arguments: Record<string, unknown>; // JSON object
}

export interface Node {
  node_id: NodeId;
  session_id: SessionId;
  parent_id: NodeId | null;
  kind: NodeKind;
  role: Role | null;
  content_type: ContentType;
  content: string;
  created_at: number; // Unix timestamp (seconds)
  seq: number;
  flags: NodeFlags;
  tool_call_id?: string;
  tool_calls?: ToolCall[];
  pruned_at?: number;
  metadata?: Record<string, unknown>;
}

export interface CheckpointStats {
  nodes_covered: number;
  total_tokens: number;
  summary_tokens: number;
  compression_ratio: number;
  covered_time_range: [number, number];
}

export interface CheckpointData {
  summary: string;
  created_at: number;
  strategy?: string;
  stats?: CheckpointStats;
  extensions?: Record<string, unknown>;
}

// ============================================================================
// Agent Events (SSE streaming)
// ============================================================================

export type AgentEvent =
  | { type: "content"; content: string }
  | { type: "thinking"; text: string }
  | { type: "tool_calls"; tool_calls: ToolCall[] }
  | {
      type: "tool_result";
      tool_call_id: string;
      tool_name: string;
      result: string;
      is_error: boolean;
    }
  | { type: "loop_detected"; detection: string }
  | { type: "checkpoint"; node_id: string; strategy: string }
  | {
      type: "done";
      total_usage: TokenUsage;
      all_tool_calls: ToolCall[];
      rounds: number;
    };

export interface TokenUsage {
  input_tokens: number;
  output_tokens: number;
  total_tokens: number;
}

// ============================================================================
// Session Types
// ============================================================================

export interface SessionInfo {
  session_id: SessionId;
  name: string | null;
  created_at: number;
  updated_at: number;
  preset: string;
  message_count: number;
  root_node_id: NodeId;
  active_leaf_id: NodeId;
}

export interface SessionPath {
  nodes: Node[];
}

export interface SessionMetadata {
  total_nodes: number;
}

// ============================================================================
// Config Types
// ============================================================================

export interface ChatIntent {
  creativity: number; // 0.0-1.0
  verbosity: "short" | "normal" | "long";
  rounds: number;
}

export interface ChatConfig {
  preset: string;
  system_prompt?: string;
  tools_enabled: boolean;
  intent: ChatIntent;
  overrides?: Record<string, unknown>;
}

export interface SessionConfig {
  system_prompt: string;
  max_context_tokens: number;
  max_history_length?: number;
  auto_checkpoint?: {
    every_n_turns?: number;
    every_n_tokens?: number;
  };
  compression?: {
    enabled: boolean;
    max_chars: number;
  };
}

export interface ProviderConfig {
  provider: "openai" | "anthropic" | "gemini";
  model: string;
  temperature: number;
  max_tokens: number;
  top_p?: number;
  top_k?: number;
  enable_reasoning: boolean;
}

export interface AgentConfig {
  max_rounds: number;
  tools_enabled: boolean;
  loop_detection?: {
    max_identical_calls: number;
    max_similar_patterns: number;
    similarity_threshold: number;
  };
}

export interface ResolvedConfig {
  session: SessionConfig;
  provider: ProviderConfig;
  agent: AgentConfig;
}

export interface ConfigResponse {
  resolved_config: ResolvedConfig;
  editable_config: ChatConfig;
}

// ============================================================================
// API Request/Response Types
// ============================================================================

export interface ChatRequest {
  message: string;
  config?: ChatConfig;
  temporary_config?: ChatConfig;
}

export interface ChatResponse {
  stream_id: string;
  resolved_config: ResolvedConfig;
}

export interface CreateSessionRequest {
  name?: string;
  preset?: string;
  system_prompt?: string;
}

export interface CreateSessionResponse {
  session_id: SessionId;
  name: string | null;
  created_at: number;
  updated_at: number;
  resolved_config: ResolvedConfig;
}

export interface ListSessionsResponse {
  sessions: SessionInfo[];
  total: number;
}

export interface HealthResponse {
  status: "ok" | "error";
  message: string;
  version: string;
}

// ============================================================================
// Helper Types for Frontend
// ============================================================================

// Convert Node to MessageCard format
export interface MessageData {
  id: NodeId;
  role: Role;
  content: string;
  timestamp: Date;
  thinking?: string;
  toolCall?: ToolCallData; // Single tool call for ToolCall role
  toolResult?: ToolResultData; // Single tool result for Tool role
  isStreaming?: boolean;
}

export interface ToolCallData {
  id: string;
  name: string;
  arguments: Record<string, unknown>;
}

export interface ToolResultData {
  tool_call_id: string;
  tool_name: string;
  result: string;
  is_error: boolean;
}

// Convert CheckpointData to CheckpointCard format
export interface CheckpointMessage {
  id: NodeId;
  summary: string;
  timestamp: Date;
  stats?: CheckpointStats;
  isExpanded?: boolean;
}
