import type { SubAgentInfo } from "../../stores/subAgentStore";

interface SubAgentStatusCardProps {
  run: SubAgentInfo;
  onViewDetails?: () => void;
}

/**
 * Inline status card displayed in the chat flow
 * Shows sub-agent lifecycle with collapsible tool call details
 */
export const SubAgentStatusCard = ({
  run,
  onViewDetails,
}: SubAgentStatusCardProps) => {
  const phaseColors = {
    spawning: "border-info bg-info/10",
    running: "border-warning bg-warning/10",
    completed: "border-success bg-success/10",
    error: "border-error bg-error/10",
  };

  const phaseIcons = {
    spawning: "🚀",
    running: "⚙️",
    completed: "✅",
    error: "❌",
  };

  const phaseLabels = {
    spawning: "Spawning",
    running: "Running",
    completed: "Completed",
    error: "Failed",
  };

  const duration = run.endTime
    ? Math.round((run.endTime - run.startTime) / 1000)
    : Math.round((Date.now() - run.startTime) / 1000);

  return (
    <div
      className={`border-l-4 ${phaseColors[run.phase]} rounded-lg p-4 my-2 max-w-2xl`}
    >
      {/* Header */}
      <div className="flex items-start justify-between gap-2 mb-2">
        <div className="flex items-center gap-2">
          <span className="text-lg">{phaseIcons[run.phase]}</span>
          <div>
            <p className="font-semibold text-sm">
              Sub-Agent: {phaseLabels[run.phase]}
            </p>
            <p className="text-xs text-base-content/60">{run.taskLabel}</p>
          </div>
        </div>

        {/* Duration badge */}
        <span className="badge badge-sm badge-ghost">{duration}s</span>
      </div>

      {/* Progress indicator for running state */}
      {run.phase === "running" && (
        <div className="mb-2">
          {run.progress ? (
            <div>
              <div className="flex items-center justify-between text-xs mb-1">
                <span className="text-base-content/60">Progress</span>
                <span className="text-base-content/60">
                  {run.progress.current} / {run.progress.total}
                </span>
              </div>
              <progress
                className="progress progress-warning w-full h-2"
                value={run.progress.current}
                max={run.progress.total}
              ></progress>
            </div>
          ) : (
            <div className="flex items-center gap-2">
              <span className="loading loading-spinner loading-xs text-warning"></span>
              <span className="text-xs text-base-content/60">
                In progress...
              </span>
            </div>
          )}
        </div>
      )}

      {/* Tool calls summary */}
      {run.toolCalls.length > 0 && (
        <div className="space-y-1">
          <details className="collapse collapse-arrow bg-base-100/50 rounded-md">
            <summary className="collapse-title text-xs font-medium py-2 px-3 cursor-pointer">
              Tool Calls ({run.toolCalls.length})
            </summary>
            <div className="collapse-content px-3 pb-2">
              <div className="space-y-1">
                {run.toolCalls.map((toolCall, index) => (
                  <div
                    key={index}
                    className="flex items-center justify-between text-xs py-1"
                  >
                    <span className="font-mono flex items-center gap-1">
                      {toolCall.status === "running" && "⏳"}
                      {toolCall.status === "success" && "✓"}
                      {toolCall.status === "error" && "✗"}
                      <span className="text-base-content/80">
                        {toolCall.toolName}
                      </span>
                    </span>
                    <span className="text-base-content/60">
                      {new Date(toolCall.timestamp).toLocaleTimeString()}
                    </span>
                  </div>
                ))}
              </div>
            </div>
          </details>
        </div>
      )}

      {/* Errors */}
      {run.errors.length > 0 && (
        <div className="mt-2 space-y-1">
          {run.errors.map((error, index) => (
            <div
              key={index}
              className="text-xs text-error bg-error/10 rounded px-2 py-1"
            >
              {error}
            </div>
          ))}
        </div>
      )}

      {/* View details button */}
      {onViewDetails && (
        <button
          onClick={onViewDetails}
          className="btn btn-ghost btn-xs mt-2 gap-1"
        >
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
              d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"
            />
          </svg>
          View Details
        </button>
      )}
    </div>
  );
};
