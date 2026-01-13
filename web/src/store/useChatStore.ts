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
  expandedCheckpoints: Set<string>;
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

// ============================================================================
// Zustand Store Implementation
// ============================================================================

export const useChatStore = create<ChatStore>()(
  devtools(
    (set) => ({
      ...initialState,

      // ===== OPTIMISTIC ACTIONS =====

      selectNode: (nodeId) =>
        set(
          (state) => ({
            ui: { ...state.ui, selectedNodeId: nodeId },
          }),
          false,
          "selectNode",
        ),

      toggleToolPair: (toolPairId) =>
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
        ),

      toggleCheckpoint: (checkpointId) =>
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
        ),

      updateScrollPosition: (offset) =>
        set(
          (state) => ({
            ui: { ...state.ui, scrollPosition: offset },
          }),
          false,
          "updateScrollPosition",
        ),

      // ===== SERVER-AUTHORITATIVE ACTIONS =====

      initializeSession: (sessionId, rootNodeId) =>
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
        ),

      addMessage: (message) =>
        set(
          (state) => ({
            messages: [...state.messages, message],
          }),
          false,
          "addMessage",
        ),

      updateMessage: (messageId, updates) =>
        set(
          (state) => {
            const messages = state.messages.map((msg) =>
              msg.id === messageId ? { ...msg, ...updates } : msg,
            );
            return { messages };
          },
          false,
          "updateMessage",
        ),

      setMessages: (messages) => set({ messages }, false, "setMessages"),

      addCheckpoint: (checkpoint) =>
        set(
          (state) => {
            const checkpoints = new Map(state.checkpoints);
            checkpoints.set(checkpoint.id, checkpoint);
            return { checkpoints };
          },
          false,
          "addCheckpoint",
        ),

      // ===== STREAMING ACTIONS =====

      startStreaming: (messageId) =>
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
        ),

      stopStreaming: () =>
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
        ),

      addToolCalls: (messageId, toolCalls) =>
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
            return {
              streaming: { ...state.streaming, toolPairGroups: groups },
            };
          },
          false,
          "addToolCalls",
        ),

      addToolResult: (toolCallId, result) =>
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
        ),

      updateToolPairState: (toolCallId, state, elapsedMs = 0) =>
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
        ),

      // ===== UTILITY ACTIONS =====

      setError: (error) => set({ error }, false, "setError"),

      setLoading: (isLoading) => set({ isLoading }, false, "setLoading"),

      resetStore: () =>
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
        ),
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
export const selectIsStreaming = (state: ChatStore) =>
  state.streaming.isStreaming;
export const selectToolPairGroups = (state: ChatStore) =>
  state.streaming.toolPairGroups;
export const selectCheckpoints = (state: ChatStore) => state.checkpoints;
export const selectError = (state: ChatStore) => state.error;
export const selectIsLoading = (state: ChatStore) => state.isLoading;
