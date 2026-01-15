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

export interface ToolResultData {
  tool_call_id: string;
  tool_name: string;
  result: string;
  is_error: boolean;
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

export interface SessionRuntimeConfig {
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
  model: string;
  temperature: number;
  max_tokens: number;
  top_p?: number;
  frequency_penalty?: number;
  presence_penalty?: number;
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

export interface SessionConfig {
  provider: ProviderConfig;
  agent: AgentConfig;
  session: SessionRuntimeConfig;
}

// ============================================================================
// API Request/Response Types
// ============================================================================

export interface ChatRequest {
  message: string;
}

export interface ChatResponse {
  stream_id: string;
}

export interface ConfigResponse {
  session_config: SessionConfig;
}

export interface CreateSessionRequest {
  name?: string;
}

export interface CreateSessionResponse {
  session_id: SessionId;
  name: string | null;
  created_at: number;
  updated_at: number;
  session_config: SessionConfig;
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

// Convert Node to MessageCard format - matches backend structure exactly
export interface MessageData {
  id: NodeId;
  role: Role;
  content: string;
  timestamp: Date;
  thinking?: string;

  // For Assistant messages with tool calls
  tool_calls?: ToolCall[];

  // For Tool messages (tool results)
  tool_call_id?: string;
  is_error?: boolean;

  isStreaming?: boolean;
}

// Convert CheckpointData to CheckpointCard format
export interface CheckpointMessage {
  id: NodeId;
  summary: string;
  timestamp: Date;
  stats?: CheckpointStats;
  isExpanded?: boolean;
}
