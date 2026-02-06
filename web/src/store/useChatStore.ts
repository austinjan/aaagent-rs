// Zustand store for centralized chat state management
// Following the architecture from chat-ui-state-management.md

import { create } from "zustand";
import { devtools } from "zustand/middleware";
import type {
  NodeId,
  SessionId,
  Node,
  MessageData,
  CheckpointMessage,
  ToolCall,
  ToolResultData,
  CollapsedState,
} from "../types/backend";

// ============================================================================
// Tool Pair Grouping (from plan)
// ============================================================================

export type ToolPairState =
  | "pending"
  | "slow"
  | "complete"
  | "error"
  | "orphaned";

export interface ToolPair {
  toolCall: ToolCall;
  result?: ToolResultData;
  state: ToolPairState;
  elapsedMs: number;
}

export interface ToolPairGroup {
  assistantMessageId: NodeId;
  pairs: ToolPair[];
  isCollapsed: boolean;
  completionSummary: {
    total: number;
    complete: number;
    pending: number;
    errors: number;
  };
}

// ============================================================================
// State Structure (from plan)
// ============================================================================

export interface SessionState {
  sessionId: SessionId | null;
  totalNodes: number;
  activeLeafId: NodeId | null;
  rootNodeId: NodeId | null;
}

export interface UIState {
  selectedNodeId: NodeId | null;
  expandedToolPairs: Set<string>;
  expandedToolCalls: Set<string>;
  expandedCheckpoints: Set<string>;

  // Collapse state (synced with backend)
  collapsedNodes: Set<NodeId>;
  collapsedToolGroups: Map<NodeId, NodeId[]>;

  scrollPosition: number;
  visibleRange: { start: number; end: number };
  loadedChunks: Set<string>;
}

export interface StreamingState {
  isStreaming: boolean;
  currentMessageId: NodeId | null;
  toolPairGroups: Map<string, ToolPairGroup>;
  pendingEvents: unknown[];
}

export interface PerformanceMetrics {
  renderTime: number;
  memoryUsage: number;
  fps: number;
}

// ============================================================================
// Chat Store Interface
// ============================================================================

export interface ChatStore {
  // ===== SERVER-AUTHORITATIVE DATA =====
  session: SessionState;
  nodes: Map<NodeId, Node>;
  messages: MessageData[]; // Linear view for rendering
  activePath: NodeId[];
  checkpoints: Map<NodeId, CheckpointMessage>;

  // ===== CLIENT-AUTHORITATIVE UI STATE =====
  ui: UIState;

  // ===== TRANSIENT STREAMING STATE =====
  streaming: StreamingState;

  // ===== PERFORMANCE METRICS =====
  metrics: PerformanceMetrics;

  // ===== ERROR STATE =====
  error: Error | null;
  isLoading: boolean;

  // ===== OPTIMISTIC ACTIONS (instant UI update) =====
  selectNode: (nodeId: NodeId | null) => void;
  toggleToolPair: (toolPairId: string) => void;
  toggleCheckpoint: (checkpointId: string) => void;
  toggleToolCalls: (messageId: string) => void;
  updateScrollPosition: (offset: number) => void;

  // ===== SERVER-AUTHORITATIVE ACTIONS (wait for SSE) =====
  initializeSession: (sessionId: SessionId, rootNodeId: NodeId) => void;
  addMessage: (message: MessageData) => void;
  updateMessage: (messageId: NodeId, updates: Partial<MessageData>) => void;
  setMessages: (messages: MessageData[]) => void;
  addCheckpoint: (checkpoint: CheckpointMessage) => void;

  // ===== STREAMING ACTIONS =====
  startStreaming: (messageId: NodeId) => void;
  stopStreaming: () => void;
  addToolCalls: (messageId: NodeId, toolCalls: ToolCall[]) => void;
  addToolResult: (toolCallId: string, result: ToolResultData) => void;
  updateToolPairState: (
    toolCallId: string,
    state: ToolPairState,
    elapsedMs?: number,
  ) => void;

  // ===== BRANCH OPERATIONS =====
  setActiveLeaf: (nodeId: NodeId) => void;

  // ===== COLLAPSE ACTIONS =====
  setCollapsedState: (state: CollapsedState) => void;
  toggleNodeCollapse: (nodeId: NodeId) => void;
  isNodeCollapsed: (nodeId: NodeId) => boolean;

  // ===== UTILITY ACTIONS =====
  setError: (error: Error | null) => void;
  setLoading: (isLoading: boolean) => void;
  resetStore: () => void;
}

// ============================================================================
// Initial State
// ============================================================================

const initialState = {
  session: {
    sessionId: null,
    totalNodes: 0,
    activeLeafId: null,
    rootNodeId: null,
  },
  nodes: new Map(),
  messages: [],
  activePath: [],
  checkpoints: new Map(),
  ui: {
    selectedNodeId: null,
    expandedToolPairs: new Set(),
    expandedCheckpoints: new Set(),
    expandedToolCalls: new Set(),
    collapsedNodes: new Set(),
    collapsedToolGroups: new Map(),
    scrollPosition: 0,
    visibleRange: { start: 0, end: 50 },
    loadedChunks: new Set(),
  },
  streaming: {
    isStreaming: false,
    currentMessageId: null,
    toolPairGroups: new Map(),
    pendingEvents: [],
  },
  metrics: {
    renderTime: 0,
    memoryUsage: 0,
    fps: 60,
  },
  error: null,
  isLoading: false,
};

const isDev = import.meta.env?.DEV ?? false;

const validateState = (state: ChatStore, context: string) => {
  if (!isDev) {
    return;
  }

  const messageIds = new Set(state.messages.map((msg) => msg.id));

  if (state.ui.selectedNodeId && !messageIds.has(state.ui.selectedNodeId)) {
    console.warn(
      "[ChatStore] selectedNodeId does not exist in messages:",
      context,
      state.ui.selectedNodeId,
    );
  }

  if (state.streaming.isStreaming && state.streaming.currentMessageId) {
    const currentMessage = state.messages.find(
      (msg) => msg.id === state.streaming.currentMessageId,
    );
    if (!currentMessage) {
      console.warn(
        "[ChatStore] currentMessageId not found while streaming:",
        context,
        state.streaming.currentMessageId,
      );
    } else if (!currentMessage.isStreaming) {
      console.warn(
        "[ChatStore] currentMessageId is not marked streaming:",
        context,
        state.streaming.currentMessageId,
      );
    }
  }

  for (const [groupId, group] of state.streaming.toolPairGroups) {
    for (const pair of group.pairs) {
      if (!pair.toolCall?.id) {
        console.warn(
          "[ChatStore] tool pair missing toolCall:",
          context,
          groupId,
        );
      }
      if (pair.state === "complete" && !pair.result) {
        console.warn(
          "[ChatStore] tool pair complete without result:",
          context,
          groupId,
          pair.toolCall?.id,
        );
      }
    }

    const computed = {
      total: group.pairs.length,
      complete: group.pairs.filter((p) => p.state === "complete").length,
      pending: group.pairs.filter(
        (p) => p.state === "pending" || p.state === "slow",
      ).length,
      errors: group.pairs.filter(
        (p) => p.state === "error" || p.state === "orphaned",
      ).length,
    };

    if (
      computed.total !== group.completionSummary.total ||
      computed.complete !== group.completionSummary.complete ||
      computed.pending !== group.completionSummary.pending ||
      computed.errors !== group.completionSummary.errors
    ) {
      console.warn(
        "[ChatStore] tool pair summary mismatch:",
        context,
        groupId,
        { expected: computed, actual: group.completionSummary },
      );
    }
  }
};

const validateAfter = (get: () => ChatStore, context: string) => {
  validateState(get(), context);
};

// ============================================================================
// Zustand Store Implementation
// ============================================================================

export const useChatStore = create<ChatStore>()(
  devtools(
    (set, get) => ({
      ...initialState,

      // ===== OPTIMISTIC ACTIONS =====

      selectNode: (nodeId) => {
        set(
          (state) => ({
            ui: { ...state.ui, selectedNodeId: nodeId },
          }),
          false,
          "selectNode",
        );
        validateAfter(get, "selectNode");
      },

      toggleToolPair: (toolPairId) => {
        set(
          (state) => {
            const expanded = new Set(state.ui.expandedToolPairs);
            if (expanded.has(toolPairId)) {
              expanded.delete(toolPairId);
            } else {
              expanded.add(toolPairId);
            }
            return { ui: { ...state.ui, expandedToolPairs: expanded } };
          },
          false,
          "toggleToolPair",
        );
        validateAfter(get, "toggleToolPair");
      },

      toggleCheckpoint: (checkpointId) => {
        set(
          (state) => {
            const expanded = new Set(state.ui.expandedCheckpoints);
            if (expanded.has(checkpointId)) {
              expanded.delete(checkpointId);
            } else {
              expanded.add(checkpointId);
            }
            return { ui: { ...state.ui, expandedCheckpoints: expanded } };
          },
          false,
          "toggleCheckpoint",
        );
        validateAfter(get, "toggleCheckpoint");
      },

      toggleToolCalls: (messageId) => {
        set(
          (state) => {
            const expanded = new Set(state.ui.expandedToolCalls);
            if (expanded.has(messageId)) {
              expanded.delete(messageId);
            } else {
              expanded.add(messageId);
            }
            return { ui: { ...state.ui, expandedToolCalls: expanded } };
          },
          false,
          "toggleToolCalls",
        );
        validateAfter(get, "toggleToolCalls");
      },

      updateScrollPosition: (offset) => {
        set(
          (state) => ({
            ui: { ...state.ui, scrollPosition: offset },
          }),
          false,
          "updateScrollPosition",
        );
        validateAfter(get, "updateScrollPosition");
      },

      // ===== SERVER-AUTHORITATIVE ACTIONS =====

      initializeSession: (sessionId, rootNodeId) => {
        set(
          (state) => ({
            session: {
              ...state.session,
              sessionId,
              rootNodeId,
              activeLeafId: rootNodeId,
            },
          }),
          false,
          "initializeSession",
        );
        validateAfter(get, "initializeSession");
      },

      addMessage: (message) => {
        set(
          (state) => ({
            messages: [...state.messages, message],
          }),
          false,
          "addMessage",
        );
        validateAfter(get, "addMessage");
      },

      updateMessage: (messageId, updates) => {
        set(
          (state) => {
            const messages = state.messages.map((msg) =>
              msg.id === messageId ? { ...msg, ...updates } : msg,
            );
            return { messages };
          },
          false,
          "updateMessage",
        );
        validateAfter(get, "updateMessage");
      },

      setMessages: (messages) => {
        set({ messages }, false, "setMessages");
        validateAfter(get, "setMessages");
      },

      addCheckpoint: (checkpoint) => {
        set(
          (state) => {
            const checkpoints = new Map(state.checkpoints);
            checkpoints.set(checkpoint.id, checkpoint);
            return { checkpoints };
          },
          false,
          "addCheckpoint",
        );
        validateAfter(get, "addCheckpoint");
      },

      // ===== STREAMING ACTIONS =====

      startStreaming: (messageId) => {
        set(
          (state) => ({
            streaming: {
              ...state.streaming,
              isStreaming: true,
              currentMessageId: messageId,
            },
          }),
          false,
          "startStreaming",
        );
        validateAfter(get, "startStreaming");
      },

      stopStreaming: () => {
        set(
          (state) => ({
            streaming: {
              ...state.streaming,
              isStreaming: false,
              currentMessageId: null,
            },
          }),
          false,
          "stopStreaming",
        );
        validateAfter(get, "stopStreaming");
      },

      addToolCalls: (messageId, toolCalls) => {
        set(
          (state) => {
            const groups = new Map(state.streaming.toolPairGroups);
            groups.set(messageId, {
              assistantMessageId: messageId,
              pairs: toolCalls.map((tc) => ({
                toolCall: tc,
                result: undefined,
                state: "pending" as ToolPairState,
                elapsedMs: 0,
              })),
              isCollapsed: true,
              completionSummary: {
                total: toolCalls.length,
                complete: 0,
                pending: toolCalls.length,
                errors: 0,
              },
            });
            // Auto-expand tool calls section when added
            const expanded = new Set(state.ui.expandedToolCalls);
            expanded.add(messageId);
            return {
              streaming: { ...state.streaming, toolPairGroups: groups },
              ui: { ...state.ui, expandedToolCalls: expanded },
            };
          },
          false,
          "addToolCalls",
        );
        validateAfter(get, "addToolCalls");
      },

      addToolResult: (toolCallId, result) => {
        set(
          (state) => {
            const groups = new Map(state.streaming.toolPairGroups);

            // Find the pair
            for (const [nodeId, group] of groups) {
              const pair = group.pairs.find(
                (p) => p.toolCall.id === toolCallId,
              );
              if (pair) {
                pair.result = result;
                pair.state = result.is_error ? "error" : "complete";

                // Update summary
                group.completionSummary = {
                  total: group.pairs.length,
                  complete: group.pairs.filter((p) => p.state === "complete")
                    .length,
                  pending: group.pairs.filter(
                    (p) => p.state === "pending" || p.state === "slow",
                  ).length,
                  errors: group.pairs.filter(
                    (p) => p.state === "error" || p.state === "orphaned",
                  ).length,
                };

                groups.set(nodeId, group);
                break;
              }
            }

            return {
              streaming: { ...state.streaming, toolPairGroups: groups },
            };
          },
          false,
          "addToolResult",
        );
        validateAfter(get, "addToolResult");
      },

      updateToolPairState: (toolCallId, state, elapsedMs = 0) => {
        set(
          (prevState) => {
            const groups = new Map(prevState.streaming.toolPairGroups);

            for (const [nodeId, group] of groups) {
              const pair = group.pairs.find(
                (p) => p.toolCall.id === toolCallId,
              );
              if (pair) {
                pair.state = state;
                pair.elapsedMs = elapsedMs;
                groups.set(nodeId, group);
                break;
              }
            }

            return {
              streaming: { ...prevState.streaming, toolPairGroups: groups },
            };
          },
          false,
          "updateToolPairState",
        );
        validateAfter(get, "updateToolPairState");
      },

      // ===== BRANCH OPERATIONS =====

      setActiveLeaf: (nodeId) => {
        set(
          (state) => ({
            session: {
              ...state.session,
              activeLeafId: nodeId,
            },
          }),
          false,
          "setActiveLeaf",
        );
        validateAfter(get, "setActiveLeaf");
      },

      // ===== COLLAPSE ACTIONS =====

      setCollapsedState: (collapseState) => {
        set(
          (state) => ({
            ui: {
              ...state.ui,
              collapsedNodes: new Set(collapseState.collapsed_nodes),
              collapsedToolGroups: new Map(
                Object.entries(collapseState.collapsed_tool_groups),
              ),
            },
          }),
          false,
          "setCollapsedState",
        );
        validateAfter(get, "setCollapsedState");
      },

      toggleNodeCollapse: (nodeId) => {
        set(
          (state) => {
            const collapsed = new Set(state.ui.collapsedNodes);
            if (collapsed.has(nodeId)) {
              collapsed.delete(nodeId);
            } else {
              collapsed.add(nodeId);
            }
            return { ui: { ...state.ui, collapsedNodes: collapsed } };
          },
          false,
          "toggleNodeCollapse",
        );
        validateAfter(get, "toggleNodeCollapse");
      },

      isNodeCollapsed: (nodeId) => {
        return get().ui.collapsedNodes.has(nodeId);
      },

      // ===== UTILITY ACTIONS =====

      setError: (error) => set({ error }, false, "setError"),

      setLoading: (isLoading) => set({ isLoading }, false, "setLoading"),

      resetStore: () => {
        set(
          {
            session: initialState.session,
            nodes: new Map(),
            messages: [],
            activePath: [],
            checkpoints: new Map(),
            ui: {
              selectedNodeId: null,
              expandedToolPairs: new Set<string>(),
              expandedCheckpoints: new Set<string>(),
              expandedToolCalls: new Set<string>(),
              collapsedNodes: new Set<string>(),
              collapsedToolGroups: new Map<string, string[]>(),
              scrollPosition: 0,
              visibleRange: { start: 0, end: 50 },
              loadedChunks: new Set<string>(),
            },
            streaming: {
              isStreaming: false,
              currentMessageId: null,
              toolPairGroups: new Map(),
              pendingEvents: [],
            },
            metrics: initialState.metrics,
            error: null,
            isLoading: false,
          },
          false,
          "resetStore",
        );
        validateAfter(get, "resetStore");
      },
    }),
    { name: "ChatStore" },
  ),
);

// ============================================================================
// Selectors (for performance optimization)
// ============================================================================

export const selectSession = (state: ChatStore) => state.session;
export const selectMessages = (state: ChatStore) => state.messages;
export const selectSelectedNodeId = (state: ChatStore) =>
  state.ui.selectedNodeId;
export const selectActiveLeafId = (state: ChatStore) =>
  state.session.activeLeafId;
export const selectIsStreaming = (state: ChatStore) =>
  state.streaming.isStreaming;
export const selectToolPairGroups = (state: ChatStore) =>
  state.streaming.toolPairGroups;
export const selectCheckpoints = (state: ChatStore) => state.checkpoints;
export const selectCollapsedNodes = (state: ChatStore) =>
  state.ui.collapsedNodes;
export const selectCollapsedToolGroups = (state: ChatStore) =>
  state.ui.collapsedToolGroups;
export const selectError = (state: ChatStore) => state.error;
export const selectIsLoading = (state: ChatStore) => state.isLoading;
