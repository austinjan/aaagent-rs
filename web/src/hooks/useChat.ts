// Hook for managing chat sessions with backend integration
// Now using Zustand store for state management

import React, { useCallback, useMemo, useState } from "react";
import {
  createSession,
  sendChatMessage,
  getSessionPath,
  getStreamUrl,
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
import { Role, NodeKind, type MessageData, type NodeData } from "../types/backend";
import type { TreeNode } from "../components/tree/treeLayout";

// Note: removed messagesToTreeNodes import - now using backendNodesToTreeNodes for full tree

// Convert backend NodeData (from done event) to MessageData for chat display
function convertNodesToMessages(nodes: NodeData[]): MessageData[] {
  const messages: MessageData[] = [];

  for (const node of nodes) {
    // Skip root/system nodes that aren't messages
    if (node.kind === "Root") continue;

    // Case 1: Assistant with tool_calls - create base message + separate tool call messages
    if (node.role === "Assistant" && node.tool_calls && node.tool_calls.length > 0) {
      // Add base assistant message (if it has content)
      if (node.content) {
        messages.push({
          id: node.node_id,
          role: Role.Assistant,
          content: node.content,
          timestamp: new Date(node.created_at * 1000),
          isStreaming: false,
        });
      }

      // Add separate message for each tool call
      for (const toolCall of node.tool_calls) {
        messages.push({
          id: `${node.node_id}-tc-${toolCall.id}`,
          role: Role.Assistant,
          content: "",
          timestamp: new Date(node.created_at * 1000),
          tool_calls: [
            {
              id: toolCall.id,
              name: toolCall.name,
              arguments: toolCall.arguments,
            },
          ],
          isStreaming: false,
        });
      }
    }
    // Case 2: Tool result message
    else if (node.role === "Tool" && node.tool_call_id) {
      messages.push({
        id: node.node_id,
        role: Role.Tool,
        content: node.content,
        timestamp: new Date(node.created_at * 1000),
        tool_call_id: node.tool_call_id,
        is_error: false,
        isStreaming: false,
      });
    }
    // Case 3: Regular message (User, System, or Assistant without tool calls)
    else if (node.role) {
      let role: Role = Role.User;
      if (node.role === "Assistant") role = Role.Assistant;
      else if (node.role === "System") role = Role.System;
      else if (node.role === "Tool") role = Role.Tool;

      messages.push({
        id: node.node_id,
        role,
        content: node.content,
        timestamp: new Date(node.created_at * 1000),
        isStreaming: false,
      });
    }
  }

  return messages;
}

// Convert backend tree nodes to TreeNode format for visualization
function backendNodesToTreeNodes(
  nodes: TreeNodeData[],
  _activeLeafId: string
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
      hasCheckpoint: node.kind === "Checkpoint",
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
          // Mark last Assistant message as complete, create separate messages for each tool call
          const lastMsgTools = currentMessages[currentMessages.length - 1];
          if (lastMsgTools?.isStreaming) {
            store.updateMessage(lastMsgTools.id, { isStreaming: false });
            store.stopStreaming();
          }

          // Add tool calls to streaming state
          if (lastMsgTools) {
            store.addToolCalls(
              lastMsgTools.id,
              event.tool_calls as Array<{
                id: string;
                name: string;
                arguments: Record<string, unknown>;
              }>,
            );
          }

          // Create separate message for each tool call
          const toolCallMessages: MessageData[] = (
            event.tool_calls as Array<{
              id: string;
              name: string;
              arguments: Record<string, unknown>;
            }>
          ).map((tc) => ({
            id: nextMessageId("tool-call", tc.id),
            role: Role.Assistant,
            content: "",
            timestamp: new Date(),
            tool_calls: [
              {
                id: tc.id,
                name: tc.name,
                arguments: tc.arguments,
              },
            ],
            isStreaming: false,
          }));

          toolCallMessages.forEach((msg) => store.addMessage(msg));
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
            const streamingPrefixes = ["user-", "assistant-", "tool-call-", "tool-result-", "error-"];
            const currentMsgs = useChatStore.getState().messages;

            // Find first streaming message index
            let firstStreamingIdx = currentMsgs.findIndex(m =>
              streamingPrefixes.some(prefix => m.id.startsWith(prefix))
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
              const toAdd = treeNodesFromBackend.filter((n) => !existingIds.has(n.id));

              // Recalculate seq numbers
              const merged = [...prev, ...toAdd];
              merged.sort((a, b) => {
                // Sort by finding nodes in newNodes to get created_at
                const aNode = newNodes.find(n => n.node_id === a.id);
                const bNode = newNodes.find(n => n.node_id === b.id);
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

      // Convert nodes to messages - handle tool calls and tool results properly
      const loadedMessages: MessageData[] = [];

      // Build a map of tool_call_id -> tool_name for quick lookup
      const toolCallMap = new Map<string, string>();
      for (const node of pathResponse.nodes) {
        if (node.role === Role.Assistant && node.tool_calls) {
          for (const tc of node.tool_calls) {
            toolCallMap.set(tc.id, tc.name);
          }
        }
      }

      for (const node of pathResponse.nodes) {
        if (node.kind !== NodeKind.Message) continue;

        // Case 1: Assistant message with tool_calls - create base message + separate tool call messages
        if (
          node.role === Role.Assistant &&
          node.tool_calls &&
          node.tool_calls.length > 0
        ) {
          // Add base assistant message (if it has content)
          if (node.content) {
            loadedMessages.push({
              id: node.node_id,
              role: Role.Assistant,
              content: node.content,
              timestamp: new Date(node.created_at * 1000),
              isStreaming: false,
            });
          }

          // Add separate message for each tool call
          for (const toolCall of node.tool_calls) {
            loadedMessages.push({
              id: `${node.node_id}-tc-${toolCall.id}`,
              role: Role.Assistant,
              content: "",
              timestamp: new Date(node.created_at * 1000),
              tool_calls: [
                {
                  id: toolCall.id,
                  name: toolCall.name,
                  arguments: toolCall.arguments,
                },
              ],
              isStreaming: false,
            });
          }
        }
        // Case 2: Tool result message
        else if (node.role === Role.Tool && node.tool_call_id) {
          loadedMessages.push({
            id: node.node_id,
            role: Role.Tool,
            content: node.content,
            timestamp: new Date(node.created_at * 1000),
            tool_call_id: node.tool_call_id,
            is_error: false, // Could check content for error markers
            isStreaming: false,
          });
        }
        // Case 3: Regular message (User, System, or Assistant without tool calls)
        else {
          loadedMessages.push({
            id: node.node_id,
            role: node.role || Role.User,
            content: node.content,
            timestamp: new Date(node.created_at * 1000),
            isStreaming: false,
          });
        }
      }

      store.initializeSession(
        loadSessionId,
        pathResponse.nodes[0]?.node_id || loadSessionId,
      );
      store.setMessages(loadedMessages);
      store.setLoading(false);

      // Load full tree for minimap
      try {
        const treeResponse = await getSessionTree(loadSessionId);
        const treeNodes = backendNodesToTreeNodes(
          treeResponse.nodes,
          treeResponse.active_leaf_id
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
        treeResponse.active_leaf_id
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
    return messages.map((msg, index) => {
      let role: "user" | "assistant" | "system" | "tool" = "system";
      if (msg.role === Role.User) role = "user";
      else if (msg.role === Role.Assistant) role = "assistant";
      else if (msg.role === Role.Tool) role = "tool";

      return {
        id: msg.id,
        parent_id: index > 0 ? messages[index - 1].id : null,
        role,
        content: msg.content,
        seq: index,
        hasCheckpoint: false,
      };
    });
  }, [fullTreeNodes, messages]);

  // Get active leaf ID from tree or fallback to last message
  const activeLeafId = useMemo(() => {
    if (treeActiveLeafId) return treeActiveLeafId;
    if (messages.length === 0) return "";
    return messages[messages.length - 1].id;
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
