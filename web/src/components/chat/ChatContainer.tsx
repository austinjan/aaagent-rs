import { useEffect, useRef } from "react";
import { MessageCard } from "./MessageCard";
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

  // Auto-scroll to bottom when new messages arrive
  useEffect(() => {
    if (autoScroll && messagesEndRef.current) {
      messagesEndRef.current.scrollIntoView({ behavior: "smooth" });
    }
  }, [messages, autoScroll]);

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
            {messages.map((message) => (
              <MessageCard
                key={message.id}
                {...message}
                isSelected={message.id === selectedMessageId}
                onSelect={handleSelectMessage}
              />
            ))}
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
