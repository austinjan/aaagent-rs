import { useEffect, useRef, useState, useCallback } from "react";

const API_BASE = import.meta.env.DEV ? "http://localhost:3000/api" : "/api";

export interface SSEEvent {
  type:
    | "content"
    | "thinking"
    | "tool_calls"
    | "tool_result"
    | "loop_detected"
    | "checkpoint"
    | "done";
  data: Record<string, unknown>;
}

export interface UseSSEOptions {
  onEvent?: (event: SSEEvent) => void;
  onError?: (error: Error) => void;
  onComplete?: () => void;
}

export interface UseSSEReturn {
  isConnected: boolean;
  error: Error | null;
  close: () => void;
}

/**
 * React hook for consuming Server-Sent Events (SSE) from the chat API
 *
 * @param sessionId - The session ID
 * @param streamId - The stream ID returned from chat endpoint
 * @param options - Callback options for handling events
 * @returns Connection state and control functions
 */
export function useSSE(
  sessionId: string | null,
  streamId: string | null,
  options: UseSSEOptions = {},
): UseSSEReturn {
  const [isConnected, setIsConnected] = useState(false);
  const [error, setError] = useState<Error | null>(null);
  const eventSourceRef = useRef<EventSource | null>(null);
  const { onEvent, onError, onComplete } = options;

  const close = useCallback(() => {
    if (eventSourceRef.current) {
      eventSourceRef.current.close();
      eventSourceRef.current = null;
      setIsConnected(false);
    }
  }, []);

  useEffect(() => {
    // Don't connect if session or stream ID is missing
    if (!sessionId || !streamId) {
      return;
    }

    // Close any existing connection
    close();

    const url = `${API_BASE}/sessions/${sessionId}/stream/${streamId}`;
    const eventSource = new EventSource(url);
    eventSourceRef.current = eventSource;

    eventSource.onopen = () => {
      setIsConnected(true);
      setError(null);
    };

    eventSource.onerror = () => {
      const error = new Error("SSE connection error");
      setError(error);
      setIsConnected(false);

      if (onError) {
        onError(error);
      }

      // EventSource will auto-reconnect unless we close it
      // For our use case, close on error (stream is one-time use)
      close();
    };

    // Content event
    eventSource.addEventListener("content", (e: MessageEvent) => {
      try {
        const data = JSON.parse(e.data);
        if (onEvent) {
          onEvent({ type: "content", data });
        }
      } catch (err) {
        console.error("Failed to parse content event:", err);
      }
    });

    // Thinking event
    eventSource.addEventListener("thinking", (e: MessageEvent) => {
      try {
        const data = JSON.parse(e.data);
        if (onEvent) {
          onEvent({ type: "thinking", data });
        }
      } catch (err) {
        console.error("Failed to parse thinking event:", err);
      }
    });

    // Tool calls event
    eventSource.addEventListener("tool_calls", (e: MessageEvent) => {
      try {
        const data = JSON.parse(e.data);
        if (onEvent) {
          onEvent({ type: "tool_calls", data });
        }
      } catch (err) {
        console.error("Failed to parse tool_calls event:", err);
      }
    });

    // Tool result event
    eventSource.addEventListener("tool_result", (e: MessageEvent) => {
      try {
        const data = JSON.parse(e.data);
        if (onEvent) {
          onEvent({ type: "tool_result", data });
        }
      } catch (err) {
        console.error("Failed to parse tool_result event:", err);
      }
    });

    // Loop detected event
    eventSource.addEventListener("loop_detected", (e: MessageEvent) => {
      try {
        const data = JSON.parse(e.data);
        if (onEvent) {
          onEvent({ type: "loop_detected", data });
        }
      } catch (err) {
        console.error("Failed to parse loop_detected event:", err);
      }
    });

    // Checkpoint event
    eventSource.addEventListener("checkpoint", (e: MessageEvent) => {
      try {
        const data = JSON.parse(e.data);
        if (onEvent) {
          onEvent({ type: "checkpoint", data });
        }
      } catch (err) {
        console.error("Failed to parse checkpoint event:", err);
      }
    });

    // Done event (final)
    eventSource.addEventListener("done", (e: MessageEvent) => {
      try {
        const data = JSON.parse(e.data);
        if (onEvent) {
          onEvent({ type: "done", data });
        }
        if (onComplete) {
          onComplete();
        }
      } catch (err) {
        console.error("Failed to parse done event:", err);
      } finally {
        // Stream is complete, close connection
        close();
      }
    });

    // Cleanup on unmount or when dependencies change
    return () => {
      close();
    };
  }, [sessionId, streamId, onEvent, onError, onComplete, close]);

  return {
    isConnected,
    error,
    close,
  };
}
