import { useChatStore } from "./useChatStore";
import { Role, type MessageData, type ToolCall, type ToolResultData } from "../types/backend";

export interface SyncTestResult {
  name: string;
  passed: boolean;
  details?: string;
}

const assertResult = (condition: boolean, name: string, details?: string): SyncTestResult => ({
  name,
  passed: condition,
  details: condition ? undefined : details,
});

export const runChatStoreSyncTests = (): SyncTestResult[] => {
  const store = useChatStore.getState();
  const results: SyncTestResult[] = [];

  store.resetStore();

  const userMessage: MessageData = {
    id: "user-1",
    role: Role.User,
    content: "Hello",
    timestamp: new Date(),
    isStreaming: false,
  };

  store.addMessage(userMessage);
  store.selectNode(userMessage.id);

  let state = useChatStore.getState();
  results.push(
    assertResult(
      state.ui.selectedNodeId === userMessage.id,
      "selectNode syncs selectedNodeId",
      `expected ${userMessage.id}, got ${state.ui.selectedNodeId ?? "null"}`,
    ),
  );
  results.push(
    assertResult(
      state.messages.some((msg) => msg.id === state.ui.selectedNodeId),
      "selectedNodeId exists in messages",
    ),
  );

  const assistantMessage: MessageData = {
    id: "assistant-1",
    role: Role.Assistant,
    content: "Streaming...",
    timestamp: new Date(),
    isStreaming: true,
  };

  store.addMessage(assistantMessage);
  store.startStreaming(assistantMessage.id);

  state = useChatStore.getState();
  results.push(
    assertResult(state.streaming.isStreaming, "startStreaming sets isStreaming"),
  );
  results.push(
    assertResult(
      state.streaming.currentMessageId === assistantMessage.id,
      "startStreaming sets currentMessageId",
    ),
  );

  store.stopStreaming();
  state = useChatStore.getState();
  results.push(
    assertResult(!state.streaming.isStreaming, "stopStreaming clears isStreaming"),
  );
  results.push(
    assertResult(
      state.streaming.currentMessageId === null,
      "stopStreaming clears currentMessageId",
    ),
  );

  const toolCalls: ToolCall[] = [
    { id: "tool-1", name: "search", arguments: { query: "zustand" } },
    { id: "tool-2", name: "calc", arguments: { value: 42 } },
  ];

  store.addToolCalls(assistantMessage.id, toolCalls);
  state = useChatStore.getState();
  const group = state.streaming.toolPairGroups.get(assistantMessage.id);

  results.push(assertResult(Boolean(group), "addToolCalls creates group"));
  if (group) {
    results.push(
      assertResult(group.pairs.length === 2, "addToolCalls adds tool pairs"),
    );
    results.push(
      assertResult(
        group.completionSummary.pending === 2 &&
          group.completionSummary.complete === 0 &&
          group.completionSummary.errors === 0,
        "completion summary initialized",
      ),
    );
  }

  const toolResult: ToolResultData = {
    tool_call_id: "tool-1",
    tool_name: "search",
    result: "result payload",
    is_error: false,
  };

  store.addToolResult(toolResult.tool_call_id, toolResult);
  state = useChatStore.getState();
  const updatedGroup = state.streaming.toolPairGroups.get(assistantMessage.id);

  if (updatedGroup) {
    results.push(
      assertResult(
        updatedGroup.completionSummary.complete === 1 &&
          updatedGroup.completionSummary.pending === 1,
        "completion summary updates after tool result",
      ),
    );
  }

  store.addCheckpoint({
    id: "checkpoint-1",
    summary: "Checkpoint created",
    timestamp: new Date(),
  });

  state = useChatStore.getState();
  results.push(
    assertResult(
      state.checkpoints.has("checkpoint-1"),
      "addCheckpoint stores checkpoint",
    ),
  );

  return results;
};
