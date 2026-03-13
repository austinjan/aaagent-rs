// API client for backend endpoints

const API_BASE = import.meta.env.DEV ? 'http://localhost:3000/api' : '/api';

export interface SessionConfig {
  provider: {
    model: string;
    temperature: number;
    max_tokens: number;
    top_p?: number;
    frequency_penalty?: number;
    presence_penalty?: number;
  };
  agent: {
    max_rounds: number;
    tools_enabled: boolean;
  };
  session: {
    system_prompt: string;
    max_context_tokens: number;
  };
}

export interface ConfigResponse {
  session_config: SessionConfig;
}

export interface ApiError {
  error: string;
}

/**
 * Fetch session configuration
 */
export async function getSessionConfig(sessionId: string): Promise<ConfigResponse> {
  const response = await fetch(`${API_BASE}/sessions/${sessionId}/config`);

  if (!response.ok) {
    const error: ApiError = await response.json();
    throw new Error(error.error || 'Failed to fetch config');
  }

  return response.json();
}

/**
 * Update session configuration
 * Update session configuration
 */
export async function updateSessionConfig(
  sessionId: string,
  config: SessionConfig
): Promise<SessionConfig> {
  const response = await fetch(`${API_BASE}/sessions/${sessionId}/config`, {
    method: 'PATCH',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify(config),
  });

  if (!response.ok) {
    const error: ApiError = await response.json();
    throw new Error(error.error || 'Failed to update config');
  }

  return response.json();
}

/**
 * Send chat message
 */
export async function sendChatMessage(
  sessionId: string,
  message: string
): Promise<{ stream_id: string }> {
  const response = await fetch(`${API_BASE}/sessions/${sessionId}/chat`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({ message }),
  });

  if (!response.ok) {
    const error: ApiError = await response.json();
    throw new Error(error.error || 'Failed to send message');
  }

  return response.json();
}

/**
 * Health check
 */
export async function healthCheck(): Promise<{ status: string; message: string; version: string }> {
  const response = await fetch(`${API_BASE}/health`);
  return response.json();
}

/**
 * Provider availability status - which providers have API keys configured
 */
export interface ProvidersStatus {
  openai: boolean;
  anthropic: boolean;
  google: boolean;
}

export async function getProvidersStatus(): Promise<ProvidersStatus> {
  const response = await fetch(`${API_BASE}/providers/status`);
  return response.json();
}
