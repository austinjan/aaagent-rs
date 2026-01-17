// MessageCard component - matches backend Node structure exactly

import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { ThinkingBlock } from "./ThinkingBlock";
import { ToolCallCard } from "./ToolCallCard";
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "../ui/collapsible";
import { ChevronDown, GitBranch } from "lucide-react";
import { useChatStore } from "../../store/useChatStore";
import { Button } from "../ui/button";
import {
  CheckpointBadge,
  type CheckpointStats,
} from "../branch/CheckpointBadge";
import { cn } from "@/lib/utils";

export interface ToolCall {
  id: string;
  name: string;
  arguments: Record<string, unknown>;
}

export interface CheckpointData {
  summary: string;
  stats?: CheckpointStats;
}

export interface MessageCardProps {
  id: string;
  role: "user" | "assistant" | "system" | "tool";
  content: string;
  thinking?: string;

  // For Assistant messages with tool calls
  tool_calls?: ToolCall[];

  // For Tool messages (tool results)
  tool_call_id?: string;
  is_error?: boolean;

  timestamp?: Date;
  isStreaming?: boolean;
  isSelected?: boolean;
  onSelect?: (id: string) => void;

  // Branch creation
  onCreateBranch?: (id: string) => void;

  // Checkpoint support
  hasCheckpoint?: boolean;
  checkpoint?: CheckpointData;
  checkpointExpanded?: boolean;
  onCheckpointToggle?: (expanded: boolean) => void;
}

export function MessageCard({
  id,
  role,
  content,
  thinking,
  tool_calls,
  tool_call_id,
  is_error,
  timestamp,
  isStreaming = false,
  isSelected = false,
  onSelect,
  onCreateBranch,
  hasCheckpoint = false,
  checkpoint,
  checkpointExpanded,
  onCheckpointToggle,
}: MessageCardProps) {
  const toggleToolCalls = useChatStore((state) => state.toggleToolCalls);
  const expandedToolCalls = useChatStore((state) => state.ui.expandedToolCalls);
  const toolCallsExpanded = expandedToolCalls.has(id);

  // Check if this is an error message
  const isErrorMessage = content.startsWith("❌") || is_error;

  const roleStyles = {
    user: "bg-[hsl(var(--role-user)/0.08)] border-[hsl(var(--role-user)/0.25)] hover:border-[hsl(var(--role-user)/0.4)]",
    assistant: isErrorMessage
      ? "bg-red-500/10 border-red-500/30 hover:border-red-500/50"
      : "bg-[hsl(var(--role-assistant)/0.08)] border-[hsl(var(--role-assistant)/0.25)] hover:border-[hsl(var(--role-assistant)/0.4)]",
    system:
      "bg-[hsl(var(--role-system)/0.08)] border-[hsl(var(--role-system)/0.25)] hover:border-[hsl(var(--role-system)/0.4)]",
    tool: isErrorMessage
      ? "bg-red-500/10 border-red-500/30 hover:border-red-500/50"
      : "bg-[hsl(var(--role-tool)/0.08)] border-[hsl(var(--role-tool)/0.25)] hover:border-[hsl(var(--role-tool)/0.4)]",
  };

  const roleLabelColors = {
    user: "text-[hsl(var(--role-user))]",
    assistant: isErrorMessage
      ? "text-red-500"
      : "text-[hsl(var(--role-assistant))]",
    system: "text-[hsl(var(--role-system))]",
    tool: isErrorMessage ? "text-red-500" : "text-[hsl(var(--role-tool))]",
  };

  const roleLabels = {
    user: "You",
    assistant: isErrorMessage ? "Error" : "Assistant",
    system: "System",
    tool: isErrorMessage ? "Tool Error" : "Tool Result",
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
      className={cn(
        "group message-card rounded-lg border-2 p-4 transition-all cursor-pointer relative",
        roleStyles[role],
        isSelected ? "shadow-lg scale-[1.02] bg-accent/10" : "shadow-sm"
      )}
      onClick={handleClick}
    >
      {/* Left indicator bar */}
      {isSelected && (
        <div
          className="absolute left-0 top-0 bottom-0 w-1 rounded-l-lg"
          style={{ backgroundColor: `hsl(var(--role-${role}))` }}
        />
      )}

      {/* Header */}
      <div
        className={cn(
          "message-header flex items-center justify-between mb-3",
          isSelected && "ml-2"
        )}
      >
        <div className="flex items-center gap-2">
          <span
            className={cn(
              "role-label text-xs font-bold uppercase",
              roleLabelColors[role]
            )}
          >
            {roleLabels[role]}
          </span>
          {/* Checkpoint badge */}
          {hasCheckpoint && checkpoint && (
            <CheckpointBadge
              summary={checkpoint.summary}
              stats={checkpoint.stats}
              isExpanded={checkpointExpanded}
              onToggle={onCheckpointToggle}
            />
          )}
          {tool_call_id && (
            <span className="text-xs text-muted-foreground font-mono">
              {tool_call_id}
            </span>
          )}
          {isStreaming && (
            <span className="text-xs text-muted-foreground flex items-center gap-1">
              <span className="inline-block w-2 h-2 bg-green-500 rounded-full animate-pulse" />
              streaming...
            </span>
          )}
        </div>
        <div className="flex items-center gap-2">
          {timestamp && (
            <span className="text-xs text-muted-foreground">
              {formatTime(timestamp)}
            </span>
          )}
          {/* Branch button - only for user/assistant messages, hidden during streaming */}
          {onCreateBranch && role !== "tool" && role !== "system" && !isStreaming && (
            <Button
              variant="ghost"
              size="icon"
              className="h-6 w-6 opacity-0 group-hover:opacity-100 transition-opacity"
              onClick={(e) => {
                e.stopPropagation();
                onCreateBranch(id);
              }}
              title="Create branch (Ctrl+B)"
            >
              <GitBranch className="h-3.5 w-3.5" />
            </Button>
          )}
        </div>
      </div>

      {/* Thinking block */}
      {thinking && <ThinkingBlock content={thinking} />}

      {/* Tool calls (for Assistant messages) */}
      {tool_calls && tool_calls.length > 0 && (
        <Collapsible
          open={toolCallsExpanded}
          onOpenChange={() => toggleToolCalls(id)}
        >
          <CollapsibleTrigger className="flex items-center gap-2 w-full mb-2 text-sm text-muted-foreground hover:text-foreground transition-colors">
            <ChevronDown
              className={cn(
                "h-4 w-4 transition-transform",
                !toolCallsExpanded && "-rotate-90"
              )}
            />
            <span className="font-medium">
              Tool Calls ({tool_calls.length})
            </span>
          </CollapsibleTrigger>
          <CollapsibleContent className="space-y-2 mb-3">
            {tool_calls.map((tc) => (
              <ToolCallCard
                key={tc.id}
                id={tc.id}
                name={tc.name}
                input={tc.arguments}
                result={undefined}
              />
            ))}
          </CollapsibleContent>
        </Collapsible>
      )}

      {/* Message content */}
      {content && (
        <div className="message-body prose prose-sm dark:prose-invert max-w-none">
          {role === "tool" ? (
            // Tool results: display as raw text with same prose styling as assistant
            <div className="whitespace-pre-wrap break-words">{content}</div>
          ) : (
            // Other messages: render as markdown
            <ReactMarkdown remarkPlugins={[remarkGfm]}>{content}</ReactMarkdown>
          )}
        </div>
      )}
    </div>
  );
}
