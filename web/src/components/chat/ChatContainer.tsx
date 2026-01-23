import { useEffect, useRef } from "react";
import { MessageCard } from "./MessageCard";
import { CheckpointMessageCard } from "./CheckpointMessageCard";
import type { MessageCardProps } from "./MessageCard";

export interface ChatContainerProps {
  messages: MessageCardProps[];
  selectedMessageId?: string;
  onSelectMessage?: (id: string) => void;
  autoScroll?: boolean;
  isLoading?: boolean;
}

export function ChatContainer({
  messages,
  selectedMessageId,
  onSelectMessage,
  autoScroll = true,
  isLoading = false,
}: ChatContainerProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const messagesEndRef = useRef<HTMLDivElement>(null);

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

  return (
    <div
      ref={containerRef}
      className="chat-container flex-1 overflow-y-auto p-4 bg-background"
    >
      <div className="max-w-4xl mx-auto space-y-3">
        {messages.length === 0 ? (
          <div className="flex items-center justify-center h-full text-muted-foreground text-sm">
            <div className="text-center">
              <p className="mb-2">No messages yet</p>
              <p className="text-xs">Start a conversation below</p>
            </div>
          </div>
        ) : (
          <>
            {messages.map((message) => {
              // Render checkpoint message card for synthetic checkpoint messages
              if (
                message.isCheckpoint &&
                message.checkpointData &&
                message.checkpointNodeId
              ) {
                return (
                  <CheckpointMessageCard
                    key={message.id}
                    id={message.id}
                    checkpointNodeId={message.checkpointNodeId}
                    checkpointData={message.checkpointData}
                    timestamp={message.timestamp || new Date()}
                  />
                );
              }

              // Render normal message card
              return (
                <MessageCard
                  key={message.id}
                  {...message}
                  isSelected={message.id === selectedMessageId}
                  onSelect={handleSelectMessage}
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
