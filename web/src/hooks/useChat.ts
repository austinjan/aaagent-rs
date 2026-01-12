// Hook for managing chat sessions with backend integration

import { useState, useCallback } from "react";
import {
  createSession,
  sendChatMessage,
  getSessionPath,
  getStreamUrl,
} from "../services/api";
import { useSSEStream } from "./useSSEStream";
import {
  Role,
  NodeKind,
  type MessageData,
  type CheckpointMessage,
  type Node,
} from "../types/backend";

export interface UseChatOptions {
  preset?: string;
  sessionName?: string;
}

export interface ChatState {
  sessionId: string | null;
  messages: MessageData[];
  checkpoints: CheckpointMessage[];
  isLoading: boolean;
  error: Error | null;
}

export function useChat(options: UseChatOptions = {}) {
  const [state, setState] = useState<ChatState>({
    sessionId: null,
    messages: [],
    checkpoints: [],
    isLoading: false,
    error: null,
  });

  const [streamUrl, setStreamUrl] = useState<string | null>(null);

  // Handle SSE events
  const handleSSEEvent = useCallback((event: Record<string, unknown>) => {
    switch (event.type) {
      case "content":
        // Create new Assistant message on first content, append to last Assistant message on subsequent events
        setState((prev) => {
          const lastMsg = prev.messages[prev.messages.length - 1];

          // If last message is streaming Assistant, append to it
          if (lastMsg?.role === Role.Assistant && lastMsg.isStreaming) {
            return {
              ...prev,
              messages: [
                ...prev.messages.slice(0, -1),
                { ...lastMsg, content: lastMsg.content + event.content },
              ],
            };
          }

          // Otherwise create new Assistant message
          const newMsg: MessageData = {
            id: `assistant-${Date.now()}`,
            role: Role.Assistant,
            content: event.content as string,
            timestamp: new Date(),
            isStreaming: true,
          };

          return {
            ...prev,
            messages: [...prev.messages, newMsg],
          };
        });
        break;

      case "thinking":
        // Append to last Assistant message's thinking field
        setState((prev) => {
          const lastMsg = prev.messages[prev.messages.length - 1];

          if (lastMsg?.role === Role.Assistant) {
            return {
              ...prev,
              messages: [
                ...prev.messages.slice(0, -1),
                { ...lastMsg, thinking: (lastMsg.thinking || "") + event.text },
              ],
            };
          }

          return prev; // Ignore if no Assistant message
        });
        break;

      case "tool_calls":
        // Mark last Assistant message as complete, create separate messages for each tool call
        setState((prev) => {
          // Mark last Assistant message as complete
          const lastMsg = prev.messages[prev.messages.length - 1];
          const updatedMessages = lastMsg?.isStreaming
            ? [
                ...prev.messages.slice(0, -1),
                { ...lastMsg, isStreaming: false },
              ]
            : prev.messages;

          // Create separate message for each tool call
          const toolCallMessages: MessageData[] = (
            event.tool_calls as Array<{
              id: string;
              name: string;
              arguments: Record<string, unknown>;
            }>
          ).map((tc) => ({
            id: tc.id,
            role: Role.Assistant,
            content: "",
            timestamp: new Date(),
            toolCall: {
              id: tc.id,
              name: tc.name,
              arguments: tc.arguments,
            },
            isStreaming: false,
          }));

          return {
            ...prev,
            messages: [...updatedMessages, ...toolCallMessages],
          };
        });
        break;

      case "tool_result": {
        // Create separate message for tool result
        const toolResultMessage: MessageData = {
          id: `result-${event.tool_call_id}`,
          role: Role.Tool,
          content: event.result as string,
          timestamp: new Date(),
          toolResult: {
            tool_call_id: event.tool_call_id as string,
            tool_name: event.tool_name as string,
            result: event.result as string,
            is_error: event.is_error as boolean,
          },
          isStreaming: false,
        };

        setState((prev) => ({
          ...prev,
          messages: [...prev.messages, toolResultMessage],
        }));
        break;
      }

      case "checkpoint": {
        // Add checkpoint to the list
        const checkpoint: CheckpointMessage = {
          id: event.node_id as string,
          summary: "Checkpoint created",
          timestamp: new Date(),
        };

        setState((prev) => ({
          ...prev,
          checkpoints: [...prev.checkpoints, checkpoint],
        }));
        break;
      }

      case "done":
        // Mark last message as complete, stop loading
        setState((prev) => {
          const lastMsg = prev.messages[prev.messages.length - 1];
          const updatedMessages = lastMsg?.isStreaming
            ? [
                ...prev.messages.slice(0, -1),
                { ...lastMsg, isStreaming: false },
              ]
            : prev.messages;

          return {
            ...prev,
            isLoading: false,
            messages: updatedMessages,
          };
        });
        break;

      default:
        console.warn("Unknown SSE event type:", event.type);
    }
  }, []);

  // Handle SSE errors - add error message to chat
  const handleSSEError = useCallback((error: Error) => {
    setState((prev) => {
      // Check if we already have an error message in the last message (from Agent)
      const lastMessage = prev.messages[prev.messages.length - 1];
      const hasErrorContent = lastMessage?.content.startsWith("❌");

      // If Agent already sent an error, don't add another one
      if (hasErrorContent) {
        return {
          ...prev,
          error,
          isLoading: false,
        };
      }

      // Otherwise, add generic SSE error
      const errorMessage: MessageData = {
        id: `error-${Date.now()}`,
        role: Role.Assistant,
        content: `❌ Error: ${error.message}\n\nPlease check the server logs for more details.`,
        timestamp: new Date(),
        isStreaming: false,
      };

      return {
        ...prev,
        error,
        isLoading: false,
        messages: [...prev.messages, errorMessage],
      };
    });
  }, []);

  // Initialize SSE stream (auto-connects when streamUrl changes)
  useSSEStream(streamUrl, {
    onEvent: handleSSEEvent,
    onError: handleSSEError,
    onComplete: () => {
      setState((prev) => ({
        ...prev,
        isLoading: false,
      }));
    },
    autoConnect: true, // Auto-connect when streamUrl is set
  });

  // Initialize a new session
  const initializeSession = useCallback(async () => {
    try {
      setState((prev) => ({ ...prev, isLoading: true, error: null }));

      const response = await createSession({
        name: options.sessionName || "New Chat",
        preset: options.preset || "general",
      });

      setState((prev) => ({
        ...prev,
        sessionId: response.session_id,
        isLoading: false,
      }));

      return response.session_id;
    } catch (err) {
      const error =
        err instanceof Error ? err : new Error("Failed to create session");

      // Add error message to chat history
      const errorMessage: MessageData = {
        id: `error-${Date.now()}`,
        role: Role.Assistant,
        content: `❌ Failed to create session: ${error.message}\n\nPlease check:\n1. Backend server is running\n2. API keys are configured in secrets.yaml\n3. data/sessions directory exists`,
        timestamp: new Date(),
        isStreaming: false,
      };

      setState((prev) => ({
        ...prev,
        error,
        isLoading: false,
        messages: [errorMessage],
      }));

      throw error;
    }
  }, [options.preset, options.sessionName]);

  // Send a message
  const sendMessage = useCallback(
    async (content: string) => {
      if (!state.sessionId) {
        throw new Error("No active session");
      }

      try {
        setState((prev) => ({ ...prev, isLoading: true, error: null }));

        // Add user message immediately
        const userMessage: MessageData = {
          id: `user-${Date.now()}`,
          role: Role.User,
          content,
          timestamp: new Date(),
          isStreaming: false,
        };

        setState((prev) => ({
          ...prev,
          messages: [...prev.messages, userMessage],
        }));

        // Send to backend
        const response = await sendChatMessage(state.sessionId, {
          message: content,
        });

        // Connect to SSE stream (first content event will create Assistant message)
        setStreamUrl(getStreamUrl(state.sessionId, response.stream_id));
      } catch (err) {
        const error =
          err instanceof Error ? err : new Error("Failed to send message");

        // Add error message to chat history
        const errorMessage: MessageData = {
          id: `error-${Date.now()}`,
          role: Role.Assistant,
          content: `❌ Error: ${error.message}`,
          timestamp: new Date(),
          isStreaming: false,
        };

        setState((prev) => ({
          ...prev,
          error,
          isLoading: false,
          messages: [...prev.messages, errorMessage],
        }));

        throw error;
      }
    },
    [state.sessionId],
  );

  // Load session history
  const loadHistory = useCallback(async (sessionId: string) => {
    try {
      setState((prev) => ({ ...prev, isLoading: true, error: null }));

      const pathResponse = await getSessionPath(sessionId);

      // Convert nodes to messages
      const messages: MessageData[] = pathResponse.nodes
        .filter((node: Node) => node.kind === NodeKind.Message)
        .map((node: Node) => ({
          id: node.node_id,
          role: node.role || Role.User,
          content: node.content,
          timestamp: new Date(node.created_at * 1000),
          toolCalls: node.tool_calls,
          isStreaming: false,
        }));

      setState((prev) => ({
        ...prev,
        sessionId,
        messages,
        isLoading: false,
      }));
    } catch (err) {
      const error =
        err instanceof Error ? err : new Error("Failed to load history");
      setState((prev) => ({ ...prev, error, isLoading: false }));
      throw error;
    }
  }, []);

  // Reset chat (create new session)
  const resetChat = useCallback(async () => {
    setState({
      sessionId: null,
      messages: [],
      checkpoints: [],
      isLoading: false,
      error: null,
    });

    return initializeSession();
  }, [initializeSession]);

  return {
    ...state,
    initializeSession,
    sendMessage,
    loadHistory,
    resetChat,
  };
}
