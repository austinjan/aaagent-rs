import { useEffect, useRef } from "react";
import { useSubAgentStore } from "../stores/subAgentStore";

/**
 * Hook to connect to SSE stream and update sub-agent store
 * Listens to /api/sessions/:sessionId/stream endpoint
 */
export function useSubAgentSSE(sessionId: string | null) {
  const addRun = useSubAgentStore((state) => state.addRun);
  const updateRun = useSubAgentStore((state) => state.updateRun);
  const completeRun = useSubAgentStore((state) => state.completeRun);
  const addToolCall = useSubAgentStore((state) => state.addToolCall);
  const eventSourceRef = useRef<EventSource | null>(null);

  useEffect(() => {
    if (!sessionId) return;

    const streamUrl = `/api/sessions/${sessionId}/events`;
    console.log("[SubAgentSSE] Connecting to:", streamUrl);

    const eventSource = new EventSource(streamUrl);
    eventSourceRef.current = eventSource;

    eventSource.onopen = () => {
      console.log("[SubAgentSSE] Connection opened");
    };

    eventSource.onerror = (err) => {
      console.error("[SubAgentSSE] Error:", err);
    };

    // Listen for agent events wrapped in envelopes
    eventSource.addEventListener("agent_event", (e) => {
      try {
        const envelope = JSON.parse(e.data);
        const { run_id, session_id: eventSessionId, event } = envelope;

        // Only process events for this session
        if (eventSessionId !== sessionId) return;

        // Check if this is a sub-agent event (run_id !== session_id means it's a sub-agent)
        const isSubAgent = run_id !== eventSessionId;
        if (!isSubAgent) return; // Ignore main agent events

        console.log("[SubAgentSSE] Sub-agent event:", {
          run_id,
          eventType: event.type,
        });

        // Handle different event types
        switch (event.type) {
          case "content":
            // First content event means sub-agent has started
            if (!useSubAgentStore.getState().activeRuns.has(run_id)) {
              addRun(run_id, `Sub-Agent ${run_id.slice(0, 8)}`);
            }
            updateRun(run_id, { phase: "running" });
            break;

          case "tool_calls":
            // Sub-agent is making tool calls
            event.tool_calls?.forEach((toolCall: any) => {
              addToolCall(run_id, toolCall.name, "running");
            });
            break;

          case "tool_result":
            // Tool call completed
            addToolCall(
              run_id,
              event.tool_name || "unknown",
              event.is_error ? "error" : "success",
              event.result,
            );
            break;

          case "done":
            // Sub-agent completed
            completeRun(run_id, true);
            break;

          case "loop_detected":
            // Sub-agent hit a loop
            updateRun(run_id, {
              errors: [
                ...(useSubAgentStore.getState().activeRuns.get(run_id)
                  ?.errors || []),
                event.detection,
              ],
            });
            break;

          default:
            // Ignore other event types
            break;
        }
      } catch (err) {
        console.error("[SubAgentSSE] Failed to parse event:", err);
      }
    });

    // Listen for sub-agent spawn announcements (custom event type from backend)
    eventSource.addEventListener("subagent_spawned", (e) => {
      try {
        const data = JSON.parse(e.data);
        const { run_id, task_label } = data;
        console.log("[SubAgentSSE] Sub-agent spawned:", { run_id, task_label });
        addRun(run_id, task_label || `Sub-Agent ${run_id.slice(0, 8)}`);
      } catch (err) {
        console.error(
          "[SubAgentSSE] Failed to parse subagent_spawned event:",
          err,
        );
      }
    });

    // Listen for sub-agent completion announcements
    eventSource.addEventListener("subagent_completed", (e) => {
      try {
        const data = JSON.parse(e.data);
        const { run_id, success, error } = data;
        console.log("[SubAgentSSE] Sub-agent completed:", { run_id, success });
        completeRun(run_id, success, error);
      } catch (err) {
        console.error(
          "[SubAgentSSE] Failed to parse subagent_completed event:",
          err,
        );
      }
    });

    return () => {
      console.log("[SubAgentSSE] Disconnecting");
      eventSource.close();
      eventSourceRef.current = null;
    };
  }, [sessionId, addRun, updateRun, completeRun, addToolCall]);
}
