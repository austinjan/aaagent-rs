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
  ChatConfig,
  ResolvedConfig,
  HealthResponse,
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
  config: ChatConfig,
): Promise<ResolvedConfig> {
  const response = await fetch(`${API_BASE}/sessions/${sessionId}/config`, {
    method: "PATCH",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(config),
  });
  return handleResponse<ResolvedConfig>(response);
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
