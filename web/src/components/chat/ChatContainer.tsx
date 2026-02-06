import { useEffect, useMemo, useRef } from "react";
import { MessageCard } from "./MessageCard";
import { CheckpointMessageCard } from "./CheckpointMessageCard";
import { EyeOff, FoldVertical, UnfoldVertical } from "lucide-react";
import type { MessageCardProps } from "./MessageCard";
import { useChatStore, selectCollapsedNodes } from "../../store/useChatStore";

export interface ChatContainerProps {
  messages: MessageCardProps[];
  selectedMessageId?: string;
  onSelectMessage?: (id: string) => void;
  onToggleCollapse?: (id: string) => void;
  onExpandGroup?: (nodeIds: string[]) => void;
  onCollapseAllTools?: () => void;
  onExpandAll?: () => void;
  hasCollapsedNodes?: boolean;
  autoScroll?: boolean;
  isLoading?: boolean;
}

// A display item is either a normal message or a collapsed group summary
type DisplayItem =
  | { kind: "message"; message: MessageCardProps }
  | { kind: "checkpoint"; message: MessageCardProps }
  | { kind: "collapsed-group"; nodeIds: string[]; roles: Record<string, number> };

// Format role counts for collapsed group summary
function formatRoleCounts(roles: Record<string, number>): string {
  return Object.entries(roles)
    .map(([role, count]) => `${count} ${role}`)
    .join(", ");
}

export function ChatContainer({
  messages,
  selectedMessageId,
  onSelectMessage,
  onToggleCollapse,
  onExpandGroup,
  onCollapseAllTools,
  onExpandAll,
  hasCollapsedNodes = false,
  autoScroll = true,
  isLoading = false,
}: ChatContainerProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const collapsedNodes = useChatStore(selectCollapsedNodes);

  // Auto-scroll to bottom when new messages arrive (only if not manually selecting)
  useEffect(() => {
    if (autoScroll && !selectedMessageId && messagesEndRef.current) {
      messagesEndRef.current.scrollIntoView({ behavior: "smooth" });
    }
  }, [messages.length, autoScroll, selectedMessageId]);

  const handleSelectMessage = (id: string) => {
    if (onSelectMessage) {
      onSelectMessage(id);
    }
  };

  // Check if there are any tool messages (to show collapse button)
  const hasToolMessages = useMemo(
    () => messages.some((m) => m.role === "tool" || (m.role === "assistant" && m.tool_calls && m.tool_calls.length > 0)),
    [messages],
  );

  // Group consecutive collapsed nodes into summary items
  const displayItems = useMemo((): DisplayItem[] => {
    const items: DisplayItem[] = [];
    let currentGroup: { nodeIds: string[]; roles: Record<string, number> } | null = null;

    const flushGroup = () => {
      if (currentGroup) {
        items.push({ kind: "collapsed-group", ...currentGroup });
        currentGroup = null;
      }
    };

    for (const message of messages) {
      // Checkpoint messages are always shown
      if (message.isCheckpoint && message.checkpointData && message.checkpointNodeId) {
        flushGroup();
        items.push({ kind: "checkpoint", message });
        continue;
      }

      const isCollapsed = collapsedNodes.has(message.id);

      if (isCollapsed) {
        // Accumulate into current group
        if (!currentGroup) {
          currentGroup = { nodeIds: [], roles: {} };
        }
        currentGroup.nodeIds.push(message.id);
        currentGroup.roles[message.role] = (currentGroup.roles[message.role] || 0) + 1;
      } else {
        flushGroup();
        items.push({ kind: "message", message });
      }
    }

    flushGroup();
    return items;
  }, [messages, collapsedNodes]);

  // Expand all nodes in a collapsed group via batch handler
  const handleExpandGroup = (nodeIds: string[]) => {
    if (onExpandGroup) {
      onExpandGroup(nodeIds);
    }
  };

  return (
    <div
      ref={containerRef}
      className="chat-container flex-1 overflow-y-auto p-4 bg-background"
    >
      <div className="max-w-4xl mx-auto space-y-4">
        {/* Collapse/Expand toolbar - shown when there are tool messages */}
        {hasToolMessages && messages.length > 0 && (
          <div className="flex items-center gap-2 justify-end">
            {hasCollapsedNodes ? (
              <button
                className="flex items-center gap-1.5 px-2.5 py-1 text-xs text-muted-foreground hover:text-foreground border border-border rounded-md hover:bg-accent/50 transition-colors"
                onClick={onExpandAll}
                title="Expand all collapsed messages"
              >
                <UnfoldVertical className="h-3 w-3" />
                <span>Expand All</span>
              </button>
            ) : (
              <button
                className="flex items-center gap-1.5 px-2.5 py-1 text-xs text-muted-foreground hover:text-foreground border border-border rounded-md hover:bg-accent/50 transition-colors"
                onClick={onCollapseAllTools}
                title="Collapse all tool call groups"
              >
                <FoldVertical className="h-3 w-3" />
                <span>Collapse Tools</span>
              </button>
            )}
          </div>
        )}

        {displayItems.length === 0 ? (
          <div className="flex items-center justify-center h-full text-muted-foreground text-sm">
            <div className="text-center">
              <p className="mb-2">No messages yet</p>
              <p className="text-xs">Start a conversation below</p>
            </div>
          </div>
        ) : (
          <>
            {displayItems.map((item) => {
              if (item.kind === "checkpoint") {
                const msg = item.message;
                return (
                  <CheckpointMessageCard
                    key={msg.id}
                    id={msg.id}
                    checkpointNodeId={msg.checkpointNodeId!}
                    checkpointData={msg.checkpointData!}
                    timestamp={msg.timestamp || new Date()}
                  />
                );
              }

              if (item.kind === "collapsed-group") {
                const count = item.nodeIds.length;
                return (
                  <div
                    key={`collapsed-${item.nodeIds[0]}`}
                    className="flex items-center gap-2 py-1.5 px-4 opacity-40 hover:opacity-70 cursor-pointer transition-opacity"
                    onClick={() => handleExpandGroup(item.nodeIds)}
                    title="Click to expand"
                  >
                    <EyeOff className="h-3.5 w-3.5 text-muted-foreground" />
                    <span className="text-xs text-muted-foreground">
                      {count} {count === 1 ? "message" : "messages"} collapsed ({formatRoleCounts(item.roles)})
                    </span>
                  </div>
                );
              }

              // Normal message
              const msg = item.message;
              return (
                <MessageCard
                  key={msg.id}
                  {...msg}
                  isSelected={msg.id === selectedMessageId}
                  isCollapsed={false}
                  onSelect={handleSelectMessage}
                  onToggleCollapse={onToggleCollapse}
                />
              );
            })}
            {isLoading && (
              <div className="flex items-center gap-2 text-sm text-muted-foreground p-4">
                <div className="flex gap-1">
                  <span
                    className="w-2 h-2 bg-primary rounded-full animate-bounce"
                    style={{ animationDelay: "0ms" }}
                  />
                  <span
                    className="w-2 h-2 bg-primary rounded-full animate-bounce"
                    style={{ animationDelay: "150ms" }}
                  />
                  <span
                    className="w-2 h-2 bg-primary rounded-full animate-bounce"
                    style={{ animationDelay: "300ms" }}
                  />
                </div>
                <span>Assistant is thinking...</span>
              </div>
            )}
          </>
        )}
        <div ref={messagesEndRef} />
      </div>
    </div>
  );
}
