// Hook for Server-Sent Events (SSE) streaming

import { useEffect, useRef, useState, useCallback } from "react";
import type { AgentEvent } from "../types/backend";

export interface SSEStreamOptions {
  onEvent?: (event: AgentEvent) => void;
  onError?: (error: Error) => void;
  onComplete?: () => void;
  autoConnect?: boolean;
}

export interface SSEStreamState {
  isConnected: boolean;
  error: Error | null;
  isDone: boolean;
}

export function useSSEStream(
  streamUrl: string | null,
  options: SSEStreamOptions = {},
) {
  const { onEvent, onError, onComplete, autoConnect = true } = options;

  const [state, setState] = useState<SSEStreamState>({
    isConnected: false,
    error: null,
    isDone: false,
  });

  const eventSourceRef = useRef<EventSource | null>(null);
  const reconnectTimeoutRef = useRef<number | null>(null);

  const disconnect = useCallback(() => {
    if (eventSourceRef.current) {
      eventSourceRef.current.close();
      eventSourceRef.current = null;
    }
    if (reconnectTimeoutRef.current) {
      clearTimeout(reconnectTimeoutRef.current);
      reconnectTimeoutRef.current = null;
    }
    setState((prev) => ({ ...prev, isConnected: false }));
  }, []);

  const connect = useCallback(() => {
    if (!streamUrl) return;

    // Clean up existing connection
    disconnect();

    try {
      console.log("[SSE] Connecting to:", streamUrl);
      const eventSource = new EventSource(streamUrl);
      eventSourceRef.current = eventSource;

      eventSource.onopen = () => {
        console.log("[SSE] Connection opened");
        setState((prev) => ({ ...prev, isConnected: true, error: null }));
      };

      eventSource.onerror = (err) => {
        console.error("[SSE] Error occurred:", err);
        console.error("[SSE] EventSource readyState:", eventSource.readyState);
        console.error("[SSE] Stream URL:", streamUrl);
        const error = new Error("Stream connection failed");
        setState((prev) => ({ ...prev, isConnected: false, error }));
        onError?.(error);

        // EventSource automatically reconnects, but we'll handle cleanup
        disconnect();
      };

      // Handle different event types
      eventSource.addEventListener("content", (e) => {
        console.log("[SSE] Received content event:", e.data);
        const data = JSON.parse(e.data);
        onEvent?.({ type: "content", content: data.content });
      });

      eventSource.addEventListener("thinking", (e) => {
        const data = JSON.parse(e.data);
        onEvent?.({ type: "thinking", text: data.text });
      });

      eventSource.addEventListener("tool_calls", (e) => {
        const data = JSON.parse(e.data);
        onEvent?.({ type: "tool_calls", tool_calls: data.tool_calls });
      });

      eventSource.addEventListener("tool_result", (e) => {
        const data = JSON.parse(e.data);
        onEvent?.({
          type: "tool_result",
          tool_call_id: data.tool_call_id,
          tool_name: data.tool_name,
          result: data.result,
          is_error: data.is_error,
        });
      });

      eventSource.addEventListener("loop_detected", (e) => {
        const data = JSON.parse(e.data);
        onEvent?.({ type: "loop_detected", detection: data.detection });
      });

      eventSource.addEventListener("checkpoint", (e) => {
        const data = JSON.parse(e.data);
        onEvent?.({
          type: "checkpoint",
          node_id: data.node_id,
          strategy: data.strategy,
        });
      });

      eventSource.addEventListener("done", (e) => {
        const data = JSON.parse(e.data);
        onEvent?.({
          type: "done",
          total_usage: data.total_usage,
          all_tool_calls: data.all_tool_calls,
          rounds: data.rounds,
        });
        setState((prev) => ({ ...prev, isDone: true }));
        onComplete?.();
        disconnect();
      });
    } catch (err) {
      const error =
        err instanceof Error ? err : new Error("Failed to connect to stream");
      setState((prev) => ({ ...prev, error }));
      onError?.(error);
    }
  }, [streamUrl, disconnect, onEvent, onError, onComplete]);

  // Auto-connect on mount or when URL changes
  useEffect(() => {
    if (autoConnect && streamUrl) {
      // eslint-disable-next-line react-hooks/set-state-in-effect
      connect();
    }

    return () => {
      disconnect();
    };
  }, [streamUrl, autoConnect, connect, disconnect]);

  return {
    ...state,
    connect,
    disconnect,
  };
}
