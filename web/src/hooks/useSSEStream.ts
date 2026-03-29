// Hook for Server-Sent Events (SSE) streaming with auto-reconnection

import { useEffect, useRef, useState, useCallback } from "react";
import type { AgentEvent } from "../types/backend";

export interface SSEStreamOptions {
  onEvent?: (event: AgentEvent) => void;
  onError?: (error: Error) => void;
  onComplete?: () => void;
  autoConnect?: boolean;
  maxRetries?: number; // Default: 5
  baseRetryDelay?: number; // Default: 1000ms
  maxRetryDelay?: number; // Default: 16000ms
}

export interface SSEStreamState {
  isConnected: boolean;
  error: Error | null;
  isDone: boolean;
  retryCount: number;
  isRetrying: boolean;
}

export function useSSEStream(
  streamUrl: string | null,
  options: SSEStreamOptions = {},
) {
  const {
    onEvent,
    onError,
    onComplete,
    autoConnect = true,
    maxRetries = 5,
    baseRetryDelay = 1000,
    maxRetryDelay = 16000,
  } = options;

  const [state, setState] = useState<SSEStreamState>({
    isConnected: false,
    error: null,
    isDone: false,
    retryCount: 0,
    isRetrying: false,
  });

  const eventSourceRef = useRef<EventSource | null>(null);
  const reconnectTimeoutRef = useRef<number | null>(null);
  const retryCountRef = useRef<number>(0);
  const isDoneRef = useRef<boolean>(false);
  const connectRef = useRef<(() => void) | null>(null);

  // Store callbacks in refs to avoid recreating connect function
  const onEventRef = useRef(onEvent);
  const onErrorRef = useRef(onError);
  const onCompleteRef = useRef(onComplete);

  // Update refs when callbacks change
  useEffect(() => {
    onEventRef.current = onEvent;
  }, [onEvent]);

  useEffect(() => {
    onErrorRef.current = onError;
  }, [onError]);

  useEffect(() => {
    onCompleteRef.current = onComplete;
  }, [onComplete]);

  // Calculate exponential backoff delay
  const getRetryDelay = useCallback(
    (retryCount: number): number => {
      const delay = Math.min(
        baseRetryDelay * Math.pow(2, retryCount),
        maxRetryDelay,
      );
      return delay;
    },
    [baseRetryDelay, maxRetryDelay],
  );

  const disconnect = useCallback(() => {
    if (eventSourceRef.current) {
      eventSourceRef.current.close();
      eventSourceRef.current = null;
    }
    if (reconnectTimeoutRef.current) {
      clearTimeout(reconnectTimeoutRef.current);
      reconnectTimeoutRef.current = null;
    }
    setState((prev) => ({ ...prev, isConnected: false, isRetrying: false }));
  }, []);

  const scheduleReconnect = useCallback(() => {
    // Don't reconnect if stream is done or max retries reached
    if (isDoneRef.current || retryCountRef.current >= maxRetries) {
      console.log(
        "[SSE] Not reconnecting:",
        isDoneRef.current ? "stream done" : "max retries reached",
      );
      setState((prev) => ({
        ...prev,
        isRetrying: false,
        error: new Error(`Failed after ${maxRetries} reconnection attempts`),
      }));
      return;
    }

    const delay = getRetryDelay(retryCountRef.current);
    console.log(
      `[SSE] Scheduling reconnect attempt ${retryCountRef.current + 1}/${maxRetries} in ${delay}ms`,
    );

    setState((prev) => ({
      ...prev,
      isRetrying: true,
      retryCount: retryCountRef.current,
    }));

    reconnectTimeoutRef.current = window.setTimeout(() => {
      retryCountRef.current++;
      connectRef.current?.();
    }, delay);
  }, [maxRetries, getRetryDelay]);

  const connect = useCallback(() => {
    if (!streamUrl) return;

    // Clean up existing connection
    if (eventSourceRef.current) {
      eventSourceRef.current.close();
      eventSourceRef.current = null;
    }

    try {
      console.log(
        `[SSE] Connecting to: ${streamUrl} (attempt ${retryCountRef.current + 1})`,
      );
      const eventSource = new EventSource(streamUrl);
      eventSourceRef.current = eventSource;

      eventSource.onopen = () => {
        console.log("[SSE] Connection opened");
        // Reset retry counter on successful connection
        retryCountRef.current = 0;
        setState((prev) => ({
          ...prev,
          isConnected: true,
          error: null,
          retryCount: 0,
          isRetrying: false,
        }));
      };

      eventSource.onerror = (err) => {
        console.error("[SSE] Error occurred:", err);
        console.error("[SSE] EventSource readyState:", eventSource.readyState);

        const error = new Error("Stream connection failed");
        setState((prev) => ({ ...prev, isConnected: false, error }));
        onErrorRef.current?.(error);

        // Close current connection
        if (eventSourceRef.current) {
          eventSourceRef.current.close();
          eventSourceRef.current = null;
        }

        // Schedule reconnection attempt
        scheduleReconnect();
      };

      // Handle different event types
      eventSource.addEventListener("content", (e) => {
        console.log("[SSE] Received content event:", e.data);
        const data = JSON.parse(e.data);
        onEventRef.current?.({ type: "content", content: data.content });
      });

      eventSource.addEventListener("thinking", (e) => {
        const data = JSON.parse(e.data);
        onEventRef.current?.({ type: "thinking", text: data.text });
      });

      eventSource.addEventListener("tool_calls", (e) => {
        const data = JSON.parse(e.data);
        onEventRef.current?.({
          type: "tool_calls",
          tool_calls: data.tool_calls,
        });
      });

      eventSource.addEventListener("tool_result", (e) => {
        const data = JSON.parse(e.data);
        onEventRef.current?.({
          type: "tool_result",
          tool_call_id: data.tool_call_id,
          tool_name: data.tool_name,
          result: data.result,
          is_error: data.is_error,
        });
      });

      eventSource.addEventListener("loop_detected", (e) => {
        const data = JSON.parse(e.data);
        onEventRef.current?.({
          type: "loop_detected",
          detection: data.detection,
        });
      });

      eventSource.addEventListener("checkpoint", (e) => {
        const data = JSON.parse(e.data);
        onEventRef.current?.({
          type: "checkpoint",
          node_id: data.node_id,
          strategy: data.strategy,
        });
      });

      eventSource.addEventListener("queued_messages", (e) => {
        const data = JSON.parse(e.data);
        onEventRef.current?.({
          type: "queued_messages",
          count: data.count,
        });
      });

      eventSource.addEventListener("followup_processed", (e) => {
        const data = JSON.parse(e.data);
        onEventRef.current?.({
          type: "followup_processed",
          message_index: data.message_index,
          total_queued: data.total_queued,
          source: data.source,
        });
      });

      eventSource.addEventListener("error", (e: MessageEvent) => {
        const data = JSON.parse(e.data);
        onEventRef.current?.({ type: "error", message: data.message });

        // Treat as terminal — stop reconnecting
        isDoneRef.current = true;
        setState((prev) => ({
          ...prev,
          isDone: true,
          error: new Error(data.message),
        }));
        onErrorRef.current?.(new Error(data.message));
        disconnect();
      });

      eventSource.addEventListener("done", (e) => {
        const data = JSON.parse(e.data);
        onEventRef.current?.({
          type: "done",
          total_usage: data.total_usage,
          all_tool_calls: data.all_tool_calls,
          rounds: data.rounds,
          new_node_ids: data.new_node_ids,
          new_nodes: data.new_nodes,
        });

        // Mark as done to prevent reconnection
        isDoneRef.current = true;
        setState((prev) => ({ ...prev, isDone: true }));
        onCompleteRef.current?.();
        disconnect();
      });
    } catch (err) {
      const error =
        err instanceof Error ? err : new Error("Failed to connect to stream");
      setState((prev) => ({ ...prev, error }));
      onErrorRef.current?.(error);

      // Schedule reconnection on connection error
      scheduleReconnect();
    }
  }, [streamUrl, disconnect, scheduleReconnect]);

  // Auto-connect on mount or when URL changes
  useEffect(() => {
    // Store connect function in ref for use in scheduleReconnect
    connectRef.current = connect;
    // Reset done flag when URL changes
    isDoneRef.current = false;
    retryCountRef.current = 0;

    if (autoConnect && streamUrl) {
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
