import {
  useSubAgentStore,
  type SubAgentInfo,
} from "../../stores/subAgentStore";
import { useEffect } from "react";

/**
 * Side panel showing detailed information about active and completed sub-agents
 */
export const SubAgentDetailPanel = () => {
  const isPanelOpen = useSubAgentStore((state) => state.isPanelOpen);
  const activeRuns = useSubAgentStore((state) => state.activeRuns);
  const completedRuns = useSubAgentStore((state) => state.completedRuns);
  const selectedRunId = useSubAgentStore((state) => state.selectedRunId);
  const closePanel = useSubAgentStore((state) => state.closePanel);
  const selectRun = useSubAgentStore((state) => state.selectRun);
  const pruneOldRuns = useSubAgentStore((state) => state.pruneOldRuns);

  // Prune old completed runs periodically
  useEffect(() => {
    const interval = setInterval(() => {
      pruneOldRuns();
    }, 60000); // Every minute
    return () => clearInterval(interval);
  }, [pruneOldRuns]);

  if (!isPanelOpen) return null;

  const allRuns = [
    ...Array.from(activeRuns.values()),
    ...Array.from(completedRuns.values()),
  ].sort((a, b) => b.startTime - a.startTime);

  const selectedRun = selectedRunId
    ? activeRuns.get(selectedRunId) || completedRuns.get(selectedRunId)
    : null;

  return (
    <div className="fixed top-16 right-0 h-[calc(100vh-4rem)] w-96 bg-base-100 border-l border-base-300 shadow-xl z-40 flex flex-col backdrop-blur-sm">
      {/* Header */}
      <div className="flex items-center justify-between p-4 border-b border-base-300">
        <h3 className="text-lg font-semibold">Sub-Agent Activity</h3>
        <button
          onClick={closePanel}
          className="btn btn-ghost btn-sm btn-circle"
          title="Close panel"
        >
          <svg
            xmlns="http://www.w3.org/2000/svg"
            className="h-5 w-5"
            fill="none"
            viewBox="0 0 24 24"
            stroke="currentColor"
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={2}
              d="M6 18L18 6M6 6l12 12"
            />
          </svg>
        </button>
      </div>

      {/* Content */}
      <div className="flex-1 overflow-y-auto">
        {allRuns.length === 0 ? (
          <div className="flex flex-col items-center justify-center h-full text-base-content/50 p-4">
            <svg
              xmlns="http://www.w3.org/2000/svg"
              className="h-12 w-12 mb-2"
              fill="none"
              viewBox="0 0 24 24"
              stroke="currentColor"
            >
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth={2}
                d="M9.75 17L9 20l-1 1h8l-1-1-.75-3M3 13h18M5 17h14a2 2 0 002-2V9a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2z"
              />
            </svg>
            <p className="text-sm">No sub-agent activity</p>
          </div>
        ) : selectedRun ? (
          // Detail view of selected run
          <SubAgentDetailView
            run={selectedRun}
            onBack={() => selectRun(null)}
          />
        ) : (
          // List view of all runs
          <div className="divide-y divide-base-300">
            {allRuns.map((run) => (
              <SubAgentListItem
                key={run.runId}
                run={run}
                onClick={() => selectRun(run.runId)}
              />
            ))}
          </div>
        )}
      </div>
    </div>
  );
};

/**
 * Single item in the sub-agent list
 */
const SubAgentListItem = ({
  run,
  onClick,
}: {
  run: SubAgentInfo;
  onClick: () => void;
}) => {
  const phaseColors = {
    spawning: "badge-info",
    running: "badge-warning",
    completed: "badge-success",
    error: "badge-error",
  };

  const phaseIcons = {
    spawning: "🚀",
    running: "⚙️",
    completed: "✅",
    error: "❌",
  };

  const duration = run.endTime
    ? Math.round((run.endTime - run.startTime) / 1000)
    : Math.round((Date.now() - run.startTime) / 1000);

  return (
    <button
      onClick={onClick}
      className="w-full p-4 hover:bg-base-300 transition-colors text-left"
    >
      <div className="flex items-start justify-between gap-2 mb-2">
        <div className="flex-1 min-w-0">
          <p className="font-medium truncate">{run.taskLabel}</p>
          <p className="text-xs text-base-content/60">
            {duration}s {run.endTime ? "ago" : "elapsed"}
          </p>
        </div>
        <span className={`badge badge-sm ${phaseColors[run.phase]}`}>
          {phaseIcons[run.phase]} {run.phase}
        </span>
      </div>

      {run.toolCalls.length > 0 && (
        <div className="flex items-center gap-1 text-xs text-base-content/60">
          <svg
            xmlns="http://www.w3.org/2000/svg"
            className="h-3 w-3"
            fill="none"
            viewBox="0 0 24 24"
            stroke="currentColor"
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={2}
              d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z"
            />
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={2}
              d="M15 12a3 3 0 11-6 0 3 3 0 016 0z"
            />
          </svg>
          <span>{run.toolCalls.length} tool calls</span>
        </div>
      )}

      {run.errors.length > 0 && (
        <div className="text-xs text-error mt-1">
          {run.errors.length} error{run.errors.length > 1 ? "s" : ""}
        </div>
      )}
    </button>
  );
};

/**
 * Detailed view of a single sub-agent run
 */
const SubAgentDetailView = ({
  run,
  onBack,
}: {
  run: SubAgentInfo;
  onBack: () => void;
}) => {
  const duration = run.endTime
    ? Math.round((run.endTime - run.startTime) / 1000)
    : Math.round((Date.now() - run.startTime) / 1000);

  return (
    <div className="p-4">
      {/* Back button */}
      <button onClick={onBack} className="btn btn-ghost btn-sm gap-2 mb-4">
        <svg
          xmlns="http://www.w3.org/2000/svg"
          className="h-4 w-4"
          fill="none"
          viewBox="0 0 24 24"
          stroke="currentColor"
        >
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth={2}
            d="M15 19l-7-7 7-7"
          />
        </svg>
        Back
      </button>

      {/* Task info */}
      <div className="mb-4">
        <h4 className="font-semibold mb-2">{run.taskLabel}</h4>
        <div className="text-sm text-base-content/60 space-y-1">
          <p>Duration: {duration}s</p>
          <p>Started: {new Date(run.startTime).toLocaleTimeString()}</p>
          {run.endTime && (
            <p>Ended: {new Date(run.endTime).toLocaleTimeString()}</p>
          )}
        </div>
      </div>

      {/* Tool calls */}
      {run.toolCalls.length > 0 && (
        <div className="mb-4">
          <h5 className="font-medium mb-2">Tool Calls</h5>
          <div className="space-y-2">
            {run.toolCalls.map((toolCall, index) => (
              <div key={index} className="bg-base-300 rounded-lg p-3 text-sm">
                <div className="flex items-center justify-between mb-1">
                  <span className="font-mono font-medium">
                    {toolCall.toolName}
                  </span>
                  <span
                    className={`text-xs ${
                      toolCall.status === "success"
                        ? "text-success"
                        : toolCall.status === "error"
                          ? "text-error"
                          : "text-warning"
                    }`}
                  >
                    {toolCall.status}
                  </span>
                </div>
                <p className="text-xs text-base-content/60">
                  {new Date(toolCall.timestamp).toLocaleTimeString()}
                </p>
                {toolCall.result && (
                  <p className="text-xs mt-2 text-base-content/80 font-mono">
                    {toolCall.result.length > 100
                      ? toolCall.result.substring(0, 100) + "..."
                      : toolCall.result}
                  </p>
                )}
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Errors */}
      {run.errors.length > 0 && (
        <div>
          <h5 className="font-medium mb-2 text-error">Errors</h5>
          <div className="space-y-2">
            {run.errors.map((error, index) => (
              <div
                key={index}
                className="bg-error/10 border border-error/20 rounded-lg p-3 text-sm text-error"
              >
                {error}
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Progress bar (if available) */}
      {run.progress && (
        <div className="mt-4">
          <div className="flex items-center justify-between text-sm mb-1">
            <span>Progress</span>
            <span className="text-base-content/60">
              {run.progress.current} / {run.progress.total}
            </span>
          </div>
          <progress
            className="progress progress-warning w-full"
            value={run.progress.current}
            max={run.progress.total}
          ></progress>
        </div>
      )}
    </div>
  );
};
