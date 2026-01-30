// API service for backend communication

import type {
  ChatRequest,
  ChatResponse,
  CreateSessionRequest,
  CreateSessionResponse,
  ListSessionsResponse,
  SessionInfo,
  SessionPath,
  ConfigResponse,
  SessionConfig,
  HealthResponse,
  CreateCheckpointRequest,
  CreateCheckpointResponse,
  PreviewCheckpointRequest,
  PreviewCheckpointResponse,
  ContextStringStrategy,
  ContextStringResponse,
} from "../types/backend";

const API_BASE = "/api";

// ============================================================================
// Error Handling
// ============================================================================

export class ApiError extends Error {
  status: number;
  details?: unknown;

  constructor(status: number, message: string, details?: unknown) {
    super(message);
    this.name = "ApiError";
    this.status = status;
    this.details = details;
  }
}

async function handleResponse<T>(response: Response): Promise<T> {
  if (!response.ok) {
    let errorMessage = `HTTP ${response.status}: ${response.statusText}`;
    let errorDetails = undefined;

    try {
      const errorData = await response.json();
      if (errorData.error) {
        errorMessage = errorData.error;
      }
      errorDetails = errorData;
    } catch {
      // If error response is not JSON, try to get text
      try {
        const text = await response.text();
        if (text) {
          errorMessage = `${errorMessage} - ${text}`;
        }
      } catch {
        // Use default message
      }
    }
    throw new ApiError(response.status, errorMessage, errorDetails);
  }

  return response.json();
}

// ============================================================================
// Health Check
// ============================================================================

export async function checkHealth(): Promise<HealthResponse> {
  const response = await fetch(`${API_BASE}/health`);
  return handleResponse<HealthResponse>(response);
}

// ============================================================================
// Session Management
// ============================================================================

export async function listSessions(): Promise<ListSessionsResponse> {
  const response = await fetch(`${API_BASE}/sessions`);
  return handleResponse<ListSessionsResponse>(response);
}

export async function createSession(
  req: CreateSessionRequest,
): Promise<CreateSessionResponse> {
  const response = await fetch(`${API_BASE}/sessions`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(req),
  });
  return handleResponse<CreateSessionResponse>(response);
}

export async function getSession(sessionId: string): Promise<SessionInfo> {
  const response = await fetch(`${API_BASE}/sessions/${sessionId}`);
  return handleResponse<SessionInfo>(response);
}

export interface ArchiveSessionResponse {
  success: boolean;
  session_id: string;
  archived: boolean;
}

export async function archiveSession(
  sessionId: string,
): Promise<ArchiveSessionResponse> {
  const response = await fetch(`${API_BASE}/sessions/${sessionId}/archive`, {
    method: "PATCH",
  });
  return handleResponse<ArchiveSessionResponse>(response);
}

export interface RenameSessionRequest {
  name: string;
}

export interface RenameSessionResponse {
  success: boolean;
  session_id: string;
  name: string;
}

export async function renameSession(
  sessionId: string,
  name: string,
): Promise<RenameSessionResponse> {
  const response = await fetch(`${API_BASE}/sessions/${sessionId}/rename`, {
    method: "PATCH",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ name }),
  });
  return handleResponse<RenameSessionResponse>(response);
}

export async function aiRenameSession(
  sessionId: string,
): Promise<RenameSessionResponse> {
  const response = await fetch(`${API_BASE}/sessions/${sessionId}/ai-rename`, {
    method: "POST",
  });
  return handleResponse<RenameSessionResponse>(response);
}

export async function getSessionPath(sessionId: string): Promise<SessionPath> {
  const response = await fetch(`${API_BASE}/sessions/${sessionId}/path`);
  return handleResponse<SessionPath>(response);
}

// ============================================================================
// Chat
// ============================================================================

export async function sendChatMessage(
  sessionId: string,
  req: ChatRequest,
): Promise<ChatResponse> {
  const response = await fetch(`${API_BASE}/sessions/${sessionId}/chat`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(req),
  });
  return handleResponse<ChatResponse>(response);
}

// ============================================================================
// Configuration
// ============================================================================

export async function getConfig(sessionId: string): Promise<ConfigResponse> {
  const response = await fetch(`${API_BASE}/sessions/${sessionId}/config`);
  return handleResponse<ConfigResponse>(response);
}

export async function updateConfig(
  sessionId: string,
  config: SessionConfig,
): Promise<SessionConfig> {
  const response = await fetch(`${API_BASE}/sessions/${sessionId}/config`, {
    method: "PATCH",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(config),
  });
  return handleResponse<SessionConfig>(response);
}

// ============================================================================
// Branch Operations
// ============================================================================

export interface SwitchBranchRequest {
  node_id: string;
}

export interface SwitchBranchResponse {
  success: boolean;
  active_leaf_id: string;
  path: string[];
  error?: string;
}

export interface CreateBranchRequest {
  from_node_id: string;
}

export interface CreateBranchResponse {
  success: boolean;
  new_leaf_id: string;
  branch_point_id: string;
  error?: string;
}

export async function switchBranch(
  sessionId: string,
  nodeId: string,
): Promise<SwitchBranchResponse> {
  const response = await fetch(
    `${API_BASE}/sessions/${sessionId}/switch-branch`,
    {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ node_id: nodeId } as SwitchBranchRequest),
    },
  );
  return handleResponse<SwitchBranchResponse>(response);
}

export async function createBranch(
  sessionId: string,
  fromNodeId: string,
): Promise<CreateBranchResponse> {
  const response = await fetch(
    `${API_BASE}/sessions/${sessionId}/branch-from`,
    {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ from_node_id: fromNodeId } as CreateBranchRequest),
    },
  );
  return handleResponse<CreateBranchResponse>(response);
}

// ============================================================================
// Tree Visualization API
// ============================================================================

export interface TreeNodeCheckpointData {
  summary: string;
  strategy?: string;
  stats?: {
    nodes_covered: number;
    total_tokens: number;
    summary_tokens: number;
    compression_ratio: number;
  };
}

export interface TreeNodeData {
  node_id: string;
  parent_id: string | null;
  session_id: string;
  kind: string;
  role: string | null;
  content: string | null;
  tool_calls?: Array<{
    id: string;
    name: string;
    arguments: Record<string, unknown>;
  }>;
  tool_call_id?: string;
  created_at: number;
  children_count: number;
  has_checkpoint?: boolean;
  in_context?: boolean;
  checkpoint?: TreeNodeCheckpointData;
}

export interface SessionTreeResponse {
  session_id: string;
  root_node_id: string;
  active_leaf_id: string;
  context_boundary_id: string | null;
  context_node_ids: string[];
  nodes: TreeNodeData[];
  total_nodes: number;
}

export async function getSessionTree(
  sessionId: string,
): Promise<SessionTreeResponse> {
  const response = await fetch(`${API_BASE}/sessions/${sessionId}/tree`);
  return handleResponse<SessionTreeResponse>(response);
}

// ============================================================================
// SSE Stream URL Builder
// ============================================================================

export function getStreamUrl(sessionId: string, streamId: string): string {
  // EventSource requires absolute URLs in development mode (Vite proxy doesn't work with EventSource)
  const isDev = import.meta.env.DEV;
  const baseUrl = isDev ? "http://localhost:3000" : "";
  return `${baseUrl}/api/sessions/${sessionId}/stream/${streamId}`;
}

// ============================================================================
// Checkpoint Operations
// ============================================================================

export async function createCheckpoint(
  sessionId: string,
  req: CreateCheckpointRequest,
): Promise<CreateCheckpointResponse> {
  const response = await fetch(
    `${API_BASE}/sessions/${sessionId}/checkpoints`,
    {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(req),
    },
  );
  return handleResponse<CreateCheckpointResponse>(response);
}

export async function previewCheckpoint(
  sessionId: string,
  req: PreviewCheckpointRequest,
): Promise<PreviewCheckpointResponse> {
  const response = await fetch(
    `${API_BASE}/sessions/${sessionId}/checkpoints/preview`,
    {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(req),
    },
  );
  return handleResponse<PreviewCheckpointResponse>(response);
}

// ============================================================================
// Context String (Summary)
// ============================================================================

export interface GetContextStringParams {
  leafId?: string;
  strategy?: ContextStringStrategy;
}

export async function getContextString(
  sessionId: string,
  params?: GetContextStringParams,
): Promise<ContextStringResponse> {
  const queryParams = new URLSearchParams();
  if (params?.leafId) {
    queryParams.set("leaf_id", params.leafId);
  }
  if (params?.strategy) {
    queryParams.set("strategy", params.strategy);
  }
  const queryString = queryParams.toString();
  const url = `${API_BASE}/sessions/${sessionId}/context-string${queryString ? `?${queryString}` : ""}`;
  const response = await fetch(url);
  return handleResponse<ContextStringResponse>(response);
}
