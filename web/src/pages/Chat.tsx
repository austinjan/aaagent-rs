import { useEffect, useState } from "react";
import { Plus } from "lucide-react";
import { ChatContainer } from "../components/chat/ChatContainer";
import { ChatInput } from "../components/chat/ChatInput";
import { Button } from "../components/ui/button";
import { useChat } from "../hooks/useChat";
import type { MessageCardProps } from "../components/chat/MessageCard";
import type { MessageData } from "../types/backend";
import { Role } from "../types/backend";

// Convert MessageData to MessageCardProps
function toMessageCardProps(msg: MessageData): MessageCardProps {
  return {
    id: msg.id,
    role:
      msg.role === Role.User
        ? "user"
        : msg.role === Role.Assistant
          ? "assistant"
          : msg.role === Role.Tool
            ? "tool"
            : "system",
    content: msg.content,
    thinking: msg.thinking,
    toolCall: msg.toolCall,
    toolResult: msg.toolResult,
    timestamp: msg.timestamp,
    isStreaming: msg.isStreaming,
  };
}

export function Chat() {
  const {
    sessionId,
    messages,
    isLoading,
    initializeSession,
    sendMessage,
    resetChat,
  } = useChat({
    preset: "general",
    sessionName: "New Chat",
  });

  const [selectedMessageId, setSelectedMessageId] = useState<
    string | undefined
  >();

  // Initialize session on mount
  useEffect(() => {
    initializeSession().catch((err) => {
      console.error("Failed to initialize session:", err);
    });
  }, [initializeSession]);

  const handleSendMessage = async (content: string) => {
    try {
      await sendMessage(content);
    } catch (err) {
      console.error("Failed to send message:", err);
    }
  };

  const handleSelectMessage = (id: string) => {
    setSelectedMessageId(id);
  };

  const handleNewSession = async () => {
    try {
      await resetChat();
      setSelectedMessageId(undefined);
    } catch (err) {
      console.error("Failed to create new session:", err);
    }
  };

  // Convert messages to MessageCardProps
  const messageCards = messages.map(toMessageCardProps);

  return (
    <div className="chat-page flex flex-col h-screen bg-background">
      {/* Header - Sticky with improved visual hierarchy */}
      <header className="sticky top-0 z-10 border-b border-border bg-background/95 backdrop-blur supports-[backdrop-filter]:bg-background/60">
        <div className="max-w-4xl mx-auto px-4 py-4">
          <div className="flex items-center justify-between">
            <div className="space-y-1">
              <h1 className="text-2xl font-bold tracking-tight text-foreground">
                Chat
              </h1>
              <p className="text-sm text-muted-foreground">
                Conversational AI with tree-based history
              </p>
            </div>
            <div className="flex items-center gap-3">
              {sessionId && (
                <span className="text-xs text-muted-foreground font-mono">
                  Session: {sessionId.slice(0, 8)}...
                </span>
              )}
              <Button
                variant="outline"
                size="sm"
                onClick={handleNewSession}
                disabled={isLoading}
              >
                <Plus className="mr-2 h-4 w-4" />
                New Session
              </Button>
            </div>
          </div>
        </div>
      </header>

      {/* Chat Container */}
      <ChatContainer
        messages={messageCards}
        selectedMessageId={selectedMessageId}
        onSelectMessage={handleSelectMessage}
        isLoading={isLoading}
      />

      {/* Chat Input */}
      <ChatInput onSend={handleSendMessage} disabled={isLoading} />
    </div>
  );
}
