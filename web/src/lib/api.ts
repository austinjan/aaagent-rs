// API client for backend endpoints

const API_BASE = import.meta.env.DEV ? 'http://localhost:3000/api' : '/api';

export interface ChatConfig {
  preset: string;
  system_prompt?: string;
  tools_enabled: boolean;
  intent: {
    creativity: number;
    verbosity: string;
    rounds: number;
  };
  overrides?: {
    model?: string;
    top_p?: number;
    frequency_penalty?: number;
    presence_penalty?: number;
  };
}

export interface ResolvedConfig {
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
  resolved_config: ResolvedConfig;
  editable_config: ChatConfig;
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
 * Note: system_prompt cannot be changed after session creation
 */
export async function updateSessionConfig(
  sessionId: string,
  config: ChatConfig
): Promise<ResolvedConfig> {
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
  message: string,
  config?: ChatConfig,
  temporaryConfig?: ChatConfig
): Promise<{ stream_id: string; resolved_config: ResolvedConfig }> {
  const response = await fetch(`${API_BASE}/sessions/${sessionId}/chat`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      message,
      config,
      temporary_config: temporaryConfig,
    }),
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
