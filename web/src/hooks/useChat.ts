// Hook for managing chat sessions with backend integration
// Now using Zustand store for state management

import React, { useCallback, useMemo, useState } from "react";
import {
  createSession,
  sendChatMessage,
  getSessionPath,
  getStreamUrl,
  getSessionEventsUrl,
  getSessionTree,
  type TreeNodeData,
} from "../services/api";
import { useSSEStream } from "./useSSEStream";
import {
  useChatStore,
  selectIsLoading,
  selectError,
} from "../store/useChatStore";
import { useShallow } from "zustand/react/shallow";
import {
  Role,
  NodeKind,
  type MessageData,
  type NodeData,
} from "../types/backend";
import type { TreeNode } from "../components/tree/treeLayout";

// Note: removed messagesToTreeNodes import - now using backendNodesToTreeNodes for full tree

// Convert backend NodeData (from done event) to MessageData for chat display
function convertNodesToMessages(nodes: NodeData[]): MessageData[] {
  const messages: MessageData[] = [];

  for (const node of nodes) {
    // Skip root/system nodes that aren't messages
    if (node.kind === "Root") continue;

    if (!node.role) continue;

    // Map role from backend to frontend
    let role: Role = Role.User;
    if (node.role === "Assistant") role = Role.Assistant;
    else if (node.role === "System") role = Role.System;
    else if (node.role === "Tool") role = Role.Tool;

    // Create one message per node - keep tool_calls array intact
    messages.push({
      id: node.node_id,
      role,
      content: node.content,
      tool_calls: node.tool_calls, // Pass entire array, don't split
      tool_call_id: node.tool_call_id,
      is_error: false,
      timestamp: new Date(node.created_at * 1000),
      isStreaming: false,
      token_usage: node.token_usage, // Include token usage if present
    });
  }

  return messages;
}

// Convert backend tree nodes to TreeNode format for visualization
function backendNodesToTreeNodes(
  nodes: TreeNodeData[],
  _activeLeafId: string,
): TreeNode[] {
  // Sort nodes by created_at to get sequence numbers
  const sortedNodes = [...nodes].sort((a, b) => a.created_at - b.created_at);
  const seqMap = new Map<string, number>();
  sortedNodes.forEach((node, index) => {
    seqMap.set(node.node_id, index);
  });

  // Convert to TreeNode format
  return nodes.map((node) => {
    // Map role from backend format to frontend format
    let role: "user" | "assistant" | "system" | "tool" = "system";
    if (node.role === "User") role = "user";
    else if (node.role === "Assistant") role = "assistant";
    else if (node.role === "Tool") role = "tool";

    return {
      id: node.node_id,
      parent_id: node.parent_id,
      role,
      content: node.content || "",
      seq: seqMap.get(node.node_id) || 0,
      hasCheckpoint: node.has_checkpoint === true,
      inContext: node.in_context === true,
      checkpointSummary: node.checkpoint?.summary,
      tool_calls: node.tool_calls,
      tool_call_id: node.tool_call_id,
      token_usage: node.token_usage,
    };
  });
}

export interface UseChatOptions {
  sessionName?: string;
}

export function useChat(options: UseChatOptions = {}) {
  // Get store state and actions
  const sessionId = useChatStore((state) => state.session.sessionId);
  const messages = useChatStore((state) => state.messages);
  const checkpoints = useChatStore(
    useShallow((state) => Array.from(state.checkpoints.values())),
  );
  const isLoading = useChatStore(selectIsLoading);
  const error = useChatStore(selectError);

  // Full tree state for minimap (all nodes, not just active path)
  const [fullTreeNodes, setFullTreeNodes] = useState<TreeNode[]>([]);
  const [treeActiveLeafId, setTreeActiveLeafId] = useState<string>("");

  // SSE stream URL (stored in local state since it changes per message)
  const [streamUrl, setStreamUrl] = React.useState<string | null>(null);
  const messageSequence = React.useRef(0);
  const nextMessageId = useCallback((prefix: string, baseId?: string) => {
    messageSequence.current += 1;
    const base = baseId ? `${baseId}-` : "";
    return `${prefix}-${base}${Date.now()}-${messageSequence.current}`;
  }, []);

  // Handle SSE events
  const handleSSEEvent = useCallback(
    (event: Record<string, unknown>) => {
      const store = useChatStore.getState();
      const currentMessages = store.messages;

      switch (event.type) {
        case "content": {
          // Create new Assistant message on first content, append to last Assistant message on subsequent events
          const lastMsg = currentMessages[currentMessages.length - 1];

          // If last message is streaming Assistant, append to it
          if (lastMsg?.role === Role.Assistant && lastMsg.isStreaming) {
            store.updateMessage(lastMsg.id, {
              content: lastMsg.content + event.content,
            });
          } else {
            // Otherwise create new Assistant message
            const newMsg: MessageData = {
              id: `assistant-${Date.now()}`,
              role: Role.Assistant,
              content: event.content as string,
              timestamp: new Date(),
              isStreaming: true,
            };
            store.addMessage(newMsg);
            store.startStreaming(newMsg.id);
          }
          break;
        }

        case "thinking": {
          // Append to last Assistant message's thinking field
          const lastMsgThinking = currentMessages[currentMessages.length - 1];
          if (lastMsgThinking?.role === Role.Assistant) {
            store.updateMessage(lastMsgThinking.id, {
              thinking: (lastMsgThinking.thinking || "") + event.text,
            });
          }
          break;
        }

        case "tool_calls": {
          // Get content from event (may be empty if LLM didn't send content before tool calls)
          const toolCallsContent = (event.content as string) || "";
          const toolCalls = event.tool_calls as Array<{
            id: string;
            name: string;
            arguments: Record<string, unknown>;
          }>;

          // Try to find last streaming Assistant message
          const lastMsgTools = currentMessages[currentMessages.length - 1];

          if (
            lastMsgTools?.isStreaming &&
            lastMsgTools.role === Role.Assistant
          ) {
            // Update existing streaming Assistant message
            store.updateMessage(lastMsgTools.id, {
              content: lastMsgTools.content + toolCallsContent,
              isStreaming: false,
              tool_calls: toolCalls,
            });
            store.stopStreaming();

            // Add tool calls to streaming state
            store.addToolCalls(lastMsgTools.id, toolCalls);
          } else {
            // No streaming Assistant message found - create a new one
            // This happens when LLM directly calls tools without sending content first
            const newAssistantMsg: MessageData = {
              id: `assistant-${Date.now()}`,
              role: Role.Assistant,
              content: toolCallsContent,
              timestamp: new Date(),
              isStreaming: false,
              tool_calls: toolCalls,
            };

            store.addMessage(newAssistantMsg);
            store.addToolCalls(newAssistantMsg.id, toolCalls);
          }

          // Don't create separate messages - tool_calls are now part of the assistant message
          break;
        }

        case "tool_result": {
          // Create separate message for tool result
          const toolResultData = {
            tool_call_id: event.tool_call_id as string,
            tool_name: event.tool_name as string,
            result: event.result as string,
            is_error: event.is_error as boolean,
          };

          const toolResultMessage: MessageData = {
            id: nextMessageId("tool-result", event.tool_call_id as string),
            role: Role.Tool,
            content: event.result as string,
            timestamp: new Date(),
            tool_call_id: event.tool_call_id as string,
            is_error: event.is_error as boolean,
            isStreaming: false,
          };

          store.addMessage(toolResultMessage);
          store.addToolResult(event.tool_call_id as string, toolResultData);
          break;
        }

        case "checkpoint": {
          // Add checkpoint to the store
          store.addCheckpoint({
            id: event.node_id as string,
            summary: "Checkpoint created",
            timestamp: new Date(),
          });
          break;
        }

        case "done": {
          // Mark last message as complete, stop loading
          const currentMessagesForDone = useChatStore.getState().messages;
          const lastMsgDone =
            currentMessagesForDone[currentMessagesForDone.length - 1];
          if (lastMsgDone?.isStreaming) {
            store.updateMessage(lastMsgDone.id, { isStreaming: false });
          }
          store.stopStreaming();
          store.setLoading(false);

          // Convert new_nodes to messages and tree nodes, replacing streaming messages
          const newNodes = event.new_nodes as NodeData[] | undefined;
          if (newNodes && newNodes.length > 0) {
            // Convert backend nodes to messages with proper IDs
            const backendMessages = convertNodesToMessages(newNodes);

            // Find where streaming messages start (after existing backend messages)
            // Streaming messages have IDs like "user-xxx", "assistant-xxx", "tool-call-xxx", "tool-result-xxx"
            const streamingPrefixes = [
              "user-",
              "assistant-",
              "tool-call-",
              "tool-result-",
              "error-",
            ];
            const currentMsgs = useChatStore.getState().messages;

            // Find first streaming message index
            let firstStreamingIdx = currentMsgs.findIndex((m) =>
              streamingPrefixes.some((prefix) => m.id.startsWith(prefix)),
            );

            if (firstStreamingIdx === -1) {
              // No streaming messages found, append backend messages
              firstStreamingIdx = currentMsgs.length;
            }

            // Keep messages before streaming, replace with backend messages
            const preservedMessages = currentMsgs.slice(0, firstStreamingIdx);
            const newMessages = [...preservedMessages, ...backendMessages];
            store.setMessages(newMessages);

            // Update tree nodes
            const treeNodesFromBackend = newNodes.map((node) => {
              let role: "user" | "assistant" | "system" | "tool" = "system";
              if (node.role === "User") role = "user";
              else if (node.role === "Assistant") role = "assistant";
              else if (node.role === "Tool") role = "tool";

              return {
                id: node.node_id,
                parent_id: node.parent_id,
                role,
                content: node.content || "",
                seq: 0, // Will be recalculated
                hasCheckpoint: node.kind === "Checkpoint",
              };
            });

            // Merge new tree nodes with existing ones
            setFullTreeNodes((prev) => {
              const existingIds = new Set(prev.map((n) => n.id));
              const toAdd = treeNodesFromBackend.filter(
                (n) => !existingIds.has(n.id),
              );

              // Recalculate seq numbers
              const merged = [...prev, ...toAdd];
              merged.sort((a, b) => {
                // Sort by finding nodes in newNodes to get created_at
                const aNode = newNodes.find((n) => n.node_id === a.id);
                const bNode = newNodes.find((n) => n.node_id === b.id);
                const aTime = aNode?.created_at || 0;
                const bTime = bNode?.created_at || 0;
                return aTime - bTime;
              });

              return merged.map((node, idx) => ({ ...node, seq: idx }));
            });

            // Update active leaf to the last new node
            const lastNode = newNodes[newNodes.length - 1];
            if (lastNode) {
              setTreeActiveLeafId(lastNode.node_id);
            }
          }
          break;
        }

        case "queued_messages": {
          // Show toast notification for queued messages
          console.log(`[Queue] Processing ${event.count} queued message(s)`);
          // TODO: Show toast notification UI
          break;
        }

        case "followup_processed": {
          // Show progress for followup message processing
          console.log(
            `[Queue] Processing followup ${event.message_index}/${event.total_queued} from ${event.source}`,
          );
          // TODO: Show progress indicator UI
          break;
        }

        default:
          console.warn("Unknown SSE event type:", event.type);
      }
    },
    [], // No dependencies - we access store directly
  );

  // Handle SSE errors - add error message to chat
  const handleSSEError = useCallback((err: Error) => {
    const store = useChatStore.getState();
    const currentMessages = store.messages;

    // Check if we already have an error message in the last message (from Agent)
    const lastMessage = currentMessages[currentMessages.length - 1];
    const hasErrorContent = lastMessage?.content.startsWith("❌");

    // If Agent already sent an error, don't add another one
    if (!hasErrorContent) {
      const errorMessage: MessageData = {
        id: `error-${Date.now()}`,
        role: Role.Assistant,
        content: `❌ Error: ${err.message}\n\nPlease check the server logs for more details.`,
        timestamp: new Date(),
        isStreaming: false,
      };
      store.addMessage(errorMessage);
    }

    store.setError(err);
    store.setLoading(false);
    store.stopStreaming();
  }, []);

  // Initialize SSE stream (auto-connects when streamUrl changes)
  useSSEStream(streamUrl, {
    onEvent: handleSSEEvent,
    onError: handleSSEError,
    onComplete: () => {
      const store = useChatStore.getState();
      store.setLoading(false);
      store.stopStreaming();
      // Tree reload is now triggered from the done event handler
      // only when there are new nodes (more efficient)
    },
    autoConnect: true, // Auto-connect when streamUrl is set
  });

  // Initialize a new session
  const handleInitializeSession = useCallback(async () => {
    try {
      const store = useChatStore.getState();
      store.resetStore();
      store.setLoading(true);
      store.setError(null);

      const response = await createSession({
        name: options.sessionName || "New Chat",
      });

      store.initializeSession(response.session_id, response.session_id);
      store.setLoading(false);

      return response.session_id;
    } catch (err) {
      const store = useChatStore.getState();
      const errorObj =
        err instanceof Error ? err : new Error("Failed to create session");

      // Add error message to chat history
      const errorMessage: MessageData = {
        id: `error-${Date.now()}`,
        role: Role.Assistant,
        content: `❌ Failed to create session: ${errorObj.message}\n\nPlease check:\n1. Backend server is running\n2. API keys are configured in secrets.yaml\n3. data/sessions directory exists`,
        timestamp: new Date(),
        isStreaming: false,
      };

      store.addMessage(errorMessage);
      store.setError(errorObj);
      store.setLoading(false);

      throw errorObj;
    }
  }, [options.sessionName]);

  // Send a message
  const sendMessage = useCallback(async (content: string) => {
    const store = useChatStore.getState();
    const currentSessionId = store.session.sessionId;

    if (!currentSessionId) {
      throw new Error("No active session");
    }

    try {
      store.setLoading(true);
      store.setError(null);

      // Add user message immediately (optimistic)
      const userMessage: MessageData = {
        id: `user-${Date.now()}`,
        role: Role.User,
        content,
        timestamp: new Date(),
        isStreaming: false,
      };

      store.addMessage(userMessage);

      // Send to backend
      const response = await sendChatMessage(currentSessionId, {
        message: content,
      });

      // Connect to SSE stream (first content event will create Assistant message)
      setStreamUrl(getStreamUrl(currentSessionId, response.stream_id));
    } catch (err) {
      const store = useChatStore.getState();
      const errorObj =
        err instanceof Error ? err : new Error("Failed to send message");

      // Add error message to chat history
      const errorMessage: MessageData = {
        id: `error-${Date.now()}`,
        role: Role.Assistant,
        content: `❌ Error: ${errorObj.message}`,
        timestamp: new Date(),
        isStreaming: false,
      };

      store.addMessage(errorMessage);
      store.setError(errorObj);
      store.setLoading(false);

      throw errorObj;
    }
  }, []);

  // Load session history
  const loadHistory = useCallback(async (loadSessionId: string) => {
    try {
      const store = useChatStore.getState();
      store.resetStore();
      store.setLoading(true);
      store.setError(null);

      const pathResponse = await getSessionPath(loadSessionId);

      // Convert nodes to messages - one message per node
      const loadedMessages: MessageData[] = [];

      for (const node of pathResponse.nodes) {
        if (node.kind !== NodeKind.Message) continue;

        // Extract checkpoint data if present (for displaying badge on the original message)
        const checkpointInfo =
          node.has_checkpoint && node.checkpoint
            ? {
                hasCheckpoint: true,
                checkpoint: {
                  summary: node.checkpoint.summary,
                  stats: node.checkpoint.stats
                    ? {
                        nodesCovered: node.checkpoint.stats.nodes_covered,
                        originalTokens: node.checkpoint.stats.total_tokens,
                        compressedTokens: node.checkpoint.stats.summary_tokens,
                        compressionRatio:
                          node.checkpoint.stats.compression_ratio,
                      }
                    : undefined,
                },
              }
            : {};

        // Flag to track if we should add a checkpoint message after this node
        const shouldAddCheckpointMessage =
          node.has_checkpoint && node.checkpoint;

        // Create one message per node - keep tool_calls array intact
        loadedMessages.push({
          id: node.node_id,
          role: node.role || Role.User,
          content: node.content,
          tool_calls: node.tool_calls, // Pass entire array
          tool_call_id: node.tool_call_id,
          is_error: false,
          timestamp: new Date(node.created_at * 1000),
          isStreaming: false,
          token_usage: node.token_usage, // Include token usage if present
          ...checkpointInfo,
        });

        // Add checkpoint message card if this node has a checkpoint
        if (shouldAddCheckpointMessage && node.checkpoint) {
          loadedMessages.push({
            id: `checkpoint-${node.node_id}`,
            role: Role.System, // Placeholder role
            content: "",
            timestamp: new Date(node.checkpoint.created_at * 1000),
            isStreaming: false,
            // Checkpoint-specific fields
            isCheckpoint: true,
            checkpointNodeId: node.node_id,
            checkpointData: node.checkpoint,
          });
        }
      }

      store.initializeSession(
        loadSessionId,
        pathResponse.nodes[0]?.node_id || loadSessionId,
      );
      store.setMessages(loadedMessages);
      store.setLoading(false);

      // Establish SSE connection to receive real-time events for this session
      setStreamUrl(getSessionEventsUrl(loadSessionId));

      // Load full tree for minimap
      try {
        const treeResponse = await getSessionTree(loadSessionId);
        const treeNodes = backendNodesToTreeNodes(
          treeResponse.nodes,
          treeResponse.active_leaf_id,
        );
        setFullTreeNodes(treeNodes);
        setTreeActiveLeafId(treeResponse.active_leaf_id);
      } catch (treeErr) {
        console.warn("Failed to load full tree:", treeErr);
        // Don't fail the whole load, tree is optional
      }
    } catch (err) {
      const store = useChatStore.getState();
      const errorObj =
        err instanceof Error ? err : new Error("Failed to load history");
      store.setError(errorObj);
      store.setLoading(false);
      throw errorObj;
    }
  }, []);

  // Reset chat (create new session)
  const resetChat = useCallback(async () => {
    const store = useChatStore.getState();
    store.resetStore();
    setFullTreeNodes([]);
    setTreeActiveLeafId("");
    return handleInitializeSession();
  }, [handleInitializeSession]);

  // Load full tree (call after branch operations)
  const loadTree = useCallback(async (treeSessionId: string) => {
    try {
      const treeResponse = await getSessionTree(treeSessionId);
      const nodes = backendNodesToTreeNodes(
        treeResponse.nodes,
        treeResponse.active_leaf_id,
      );
      setFullTreeNodes(nodes);
      setTreeActiveLeafId(treeResponse.active_leaf_id);
    } catch (err) {
      console.warn("Failed to load tree:", err);
    }
  }, []);

  // Use full tree nodes if available, otherwise fall back to messages-based tree
  const treeNodes = useMemo<TreeNode[]>(() => {
    if (fullTreeNodes.length > 0) {
      return fullTreeNodes;
    }

    // Fallback: convert messages to linear tree (no branches visible)
    // IMPORTANT: Filter out synthetic checkpoint messages (they're not real tree nodes)
    const realMessages = messages.filter((msg) => !msg.isCheckpoint);

    return realMessages.map((msg, index) => {
      let role: "user" | "assistant" | "system" | "tool" = "system";
      if (msg.role === Role.User) role = "user";
      else if (msg.role === Role.Assistant) role = "assistant";
      else if (msg.role === Role.Tool) role = "tool";

      return {
        id: msg.id,
        parent_id: index > 0 ? realMessages[index - 1].id : null,
        role,
        content: msg.content,
        seq: index,
        hasCheckpoint: msg.hasCheckpoint || false,
      };
    });
  }, [fullTreeNodes, messages]);

  // Get active leaf ID from tree or fallback to last message
  // IMPORTANT: Skip synthetic checkpoint messages (they don't exist in backend)
  const activeLeafId = useMemo(() => {
    if (treeActiveLeafId) return treeActiveLeafId;
    if (messages.length === 0) return "";

    // Find the last non-checkpoint message (checkpoint IDs start with "checkpoint-")
    for (let i = messages.length - 1; i >= 0; i--) {
      if (!messages[i].id.startsWith("checkpoint-")) {
        return messages[i].id;
      }
    }

    return "";
  }, [treeActiveLeafId, messages]);

  return React.useMemo(
    () => ({
      sessionId,
      messages,
      checkpoints,
      isLoading,
      error,
      treeNodes,
      activeLeafId,
      initializeSession: handleInitializeSession,
      sendMessage,
      loadHistory,
      loadTree,
      resetChat,
    }),
    [
      sessionId,
      messages,
      checkpoints,
      isLoading,
      error,
      treeNodes,
      activeLeafId,
      handleInitializeSession,
      sendMessage,
      loadHistory,
      loadTree,
      resetChat,
    ],
  );
}
