// Removed unused React import

export interface MessageProps {
  role: "user" | "assistant" | "system";
  content: string;
  thinking?: string;
  toolCalls?: Array<{
    id: string;
    name: string;
    input: Record<string, unknown>;
  }>;
  toolResults?: Array<{
    tool_call_id: string;
    tool_name: string;
    result: string;
    is_error: boolean;
  }>;
  isStreaming?: boolean;
}

export function Message({
  role,
  content,
  thinking,
  toolCalls,
  toolResults,
  isStreaming = false,
}: MessageProps) {
  const roleColors = {
    user: "bg-blue-900/30 border-blue-700",
    assistant: "bg-gray-800/50 border-gray-700",
    system: "bg-yellow-900/30 border-yellow-700",
  };

  const roleLabels = {
    user: "You",
    assistant: "Assistant",
    system: "System",
  };

  return (
    <div className={`rounded-lg border p-4 ${roleColors[role]}`}>
      {/* Role header */}
      <div className="mb-2 flex items-center gap-2">
        <span className="text-sm font-semibold text-gray-300">
          {roleLabels[role]}
        </span>
        {isStreaming && (
          <span className="text-xs text-gray-500 animate-pulse">
            ● streaming...
          </span>
        )}
      </div>

      {/* Thinking (if present) */}
      {thinking && (
        <div className="mb-3 rounded bg-purple-900/20 border border-purple-700/30 p-3">
          <div className="text-xs font-semibold text-purple-400 mb-1">
            💭 Thinking
          </div>
          <div className="text-sm text-gray-300 whitespace-pre-wrap">
            {thinking}
          </div>
        </div>
      )}

      {/* Tool calls (if present) */}
      {toolCalls && toolCalls.length > 0 && (
        <div className="mb-3 space-y-2">
          {toolCalls.map((call) => (
            <div
              key={call.id}
              className="rounded bg-cyan-900/20 border border-cyan-700/30 p-3"
            >
              <div className="text-xs font-semibold text-cyan-400 mb-1">
                🔧 {call.name}
              </div>
              <pre className="text-xs text-gray-400 overflow-x-auto">
                {JSON.stringify(call.input, null, 2)}
              </pre>
            </div>
          ))}
        </div>
      )}

      {/* Tool results (if present) */}
      {toolResults && toolResults.length > 0 && (
        <div className="mb-3 space-y-2">
          {toolResults.map((result, idx) => (
            <div
              key={`${result.tool_call_id}-${idx}`}
              className={`rounded border p-3 ${
                result.is_error
                  ? "bg-red-900/20 border-red-700/30"
                  : "bg-green-900/20 border-green-700/30"
              }`}
            >
              <div
                className={`text-xs font-semibold mb-1 ${
                  result.is_error ? "text-red-400" : "text-green-400"
                }`}
              >
                {result.is_error ? "❌" : "✅"} {result.tool_name} result
              </div>
              <pre className="text-xs text-gray-400 overflow-x-auto max-h-40">
                {result.result}
              </pre>
            </div>
          ))}
        </div>
      )}

      {/* Main content */}
      {content && (
        <div className="text-sm text-gray-200 whitespace-pre-wrap">
          {content}
          {isStreaming && (
            <span className="inline-block w-2 h-4 ml-1 bg-gray-400 animate-pulse" />
          )}
        </div>
      )}
    </div>
  );
}
