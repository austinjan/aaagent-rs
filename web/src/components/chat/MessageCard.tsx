// Removed unused React import
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { ThinkingBlock } from "./ThinkingBlock";
import { ToolCallCard } from "./ToolCallCard";

export interface ToolCallData {
  id: string;
  name: string;
  arguments: Record<string, unknown>;
}

export interface ToolResultData {
  tool_call_id: string;
  tool_name: string;
  result: string;
  is_error: boolean;
}

export interface MessageCardProps {
  id: string;
  role: "user" | "assistant" | "system" | "tool";
  content: string;
  thinking?: string;
  toolCall?: ToolCallData; // Single tool call (for tool call messages)
  toolResult?: ToolResultData; // Single tool result (for tool result messages)
  timestamp?: Date;
  isStreaming?: boolean;
  isSelected?: boolean;
  onSelect?: (id: string) => void;
}

export function MessageCard({
  id,
  role,
  content,
  thinking,
  toolCall,
  toolResult,
  timestamp,
  isStreaming = false,
  isSelected = false,
  onSelect,
}: MessageCardProps) {
  // Check if this is an error message
  const isError = content.startsWith("❌") || toolResult?.is_error;

  // Special handling for tool call/result messages
  if (toolCall) {
    return (
      <div
        id={`message-${id}`}
        className="message-card rounded-lg border p-3 transition-all bg-[hsl(var(--role-assistant)/0.05)] border-[hsl(var(--role-assistant)/0.2)]"
      >
        <ToolCallCard
          id={toolCall.id}
          name={toolCall.name}
          input={toolCall.arguments}
          result={undefined}
        />
      </div>
    );
  }

  if (toolResult) {
    return (
      <div
        id={`message-${id}`}
        className={`message-card rounded-lg border p-3 transition-all ${
          toolResult.is_error
            ? "bg-red-500/5 border-red-500/20"
            : "bg-[hsl(var(--role-system)/0.05)] border-[hsl(var(--role-system)/0.2)]"
        }`}
      >
        <ToolCallCard
          id={toolResult.tool_call_id}
          name={toolResult.tool_name}
          input={undefined}
          result={{
            tool_call_id: toolResult.tool_call_id,
            tool_name: toolResult.tool_name,
            result: toolResult.result,
            is_error: toolResult.is_error,
          }}
        />
      </div>
    );
  }

  const roleStyles = {
    user: "bg-[hsl(var(--role-user)/0.08)] border-[hsl(var(--role-user)/0.25)] hover:border-[hsl(var(--role-user)/0.4)]",
    assistant: isError
      ? "bg-red-500/10 border-red-500/30 hover:border-red-500/50"
      : "bg-[hsl(var(--role-assistant)/0.08)] border-[hsl(var(--role-assistant)/0.25)] hover:border-[hsl(var(--role-assistant)/0.4)]",
    system:
      "bg-[hsl(var(--role-system)/0.08)] border-[hsl(var(--role-system)/0.25)] hover:border-[hsl(var(--role-system)/0.4)]",
    tool: "bg-[hsl(var(--role-system)/0.08)] border-[hsl(var(--role-system)/0.25)] hover:border-[hsl(var(--role-system)/0.4)]",
  };

  const roleLabelColors = {
    user: "text-[hsl(var(--role-user))]",
    assistant: isError ? "text-red-500" : "text-[hsl(var(--role-assistant))]",
    system: "text-[hsl(var(--role-system))]",
    tool: "text-[hsl(var(--role-system))]",
  };

  const roleLabels = {
    user: "You",
    assistant: isError ? "Error" : "Assistant",
    system: "System",
    tool: "Tool",
  };

  const handleClick = () => {
    if (onSelect) {
      onSelect(id);
    }
  };

  const formatTime = (date: Date) => {
    return date.toLocaleTimeString("en-US", {
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
      hour12: false,
    });
  };

  return (
    <div
      id={`message-${id}`}
      className={`
        message-card rounded-lg border p-4 transition-all cursor-pointer shadow-sm
        ${roleStyles[role]}
        ${isSelected ? "ring-2 ring-ring ring-offset-2 ring-offset-background" : ""}
      `}
      onClick={handleClick}
    >
      {/* Header */}
      <div className="message-header flex items-center justify-between mb-3">
        <div className="flex items-center gap-2">
          <span
            className={`role-label text-xs font-bold uppercase ${roleLabelColors[role]}`}
          >
            {roleLabels[role]}
          </span>
          {isStreaming && (
            <span className="text-xs text-muted-foreground flex items-center gap-1">
              <span className="inline-block w-2 h-2 bg-green-500 rounded-full animate-pulse" />
              streaming...
            </span>
          )}
        </div>
        {timestamp && (
          <span className="text-xs text-muted-foreground">
            {formatTime(timestamp)}
          </span>
        )}
      </div>

      {/* Thinking block */}
      {thinking && <ThinkingBlock content={thinking} />}

      {/* Main content */}
      {content && (
        <div className="message-content text-sm text-foreground">
          {role === "assistant" ? (
            <div className="prose prose-sm max-w-none dark:prose-invert">
              <ReactMarkdown remarkPlugins={[remarkGfm]}>
                {content}
              </ReactMarkdown>
              {isStreaming && (
                <span className="inline-block w-0.5 h-4 ml-1 bg-muted-foreground animate-pulse" />
              )}
            </div>
          ) : (
            <div className="whitespace-pre-wrap">
              {content}
              {isStreaming && (
                <span className="inline-block w-0.5 h-4 ml-1 bg-muted-foreground animate-pulse" />
              )}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
