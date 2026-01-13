import { useEffect, useState } from "react";
import { Plus } from "lucide-react";
import { ChatContainer } from "../components/chat/ChatContainer";
import { ChatInput } from "../components/chat/ChatInput";
import {
  SessionConfigPanel,
  type SessionConfig,
} from "../components/chat/SessionConfigPanel";
import {
  TemporaryConfigPanel,
  type TemporaryConfig,
} from "../components/chat/TemporaryConfigPanel";
import { Button } from "../components/ui/button";
import { useChat } from "../hooks/useChat";
import { useChatStore, selectSelectedNodeId } from "../store/useChatStore";
import { getConfig } from "../services/api";
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

const DEFAULT_OPTIONS = {
  preset: "general",
  sessionName: "New Chat",
};
const SESSION_STORAGE_KEY = "aaagent.session_id";

export function Chat() {
  const {
    sessionId,
    messages,
    isLoading,
    initializeSession,
    sendMessage,
    loadHistory,
    resetChat,
  } = useChat(DEFAULT_OPTIONS);

  // Get selection from Zustand store
  const selectedMessageId = useChatStore(selectSelectedNodeId);
  const selectNode = useChatStore((state) => state.selectNode);

  // Session config (persistent)
  const [sessionConfig, setSessionConfig] = useState<SessionConfig>({
    preset: "general",
    toolsEnabled: true,
    creativity: 0.5,
    verbosity: "normal",
    maxRounds: 30,
    overrides: {},
  });

  // Temporary config (per-message)
  const [tempConfig, setTempConfig] = useState<TemporaryConfig>({
    overrides: {},
  });

  const persistSession = (sessionId: string) => {
    localStorage.setItem(SESSION_STORAGE_KEY, sessionId);
    const url = new URL(window.location.href);
    url.searchParams.set("session", sessionId);
    window.history.replaceState({}, "", url.toString());
  };

  // Initialize session on mount (URL > localStorage > new session)
  useEffect(() => {
    let isActive = true;

    const initSession = async () => {
      const params = new URLSearchParams(window.location.search);
      const urlSessionId = params.get("session");
      const storedSessionId = localStorage.getItem(SESSION_STORAGE_KEY);
      const preferredSessionId = urlSessionId || storedSessionId;

      if (preferredSessionId) {
        try {
          await loadHistory(preferredSessionId);
          if (isActive) {
            persistSession(preferredSessionId);
          }
          return;
        } catch (err) {
          console.warn("Failed to load saved session, creating new:", err);
        }
      }

      try {
        const newSessionId = await initializeSession();
        if (newSessionId && isActive) {
          persistSession(newSessionId);
        }
      } catch (err) {
        console.error("Failed to initialize session:", err);
      }
    };

    initSession();

    return () => {
      isActive = false;
    };
  }, [initializeSession, loadHistory]);

  // Load config from backend when session is ready
  useEffect(() => {
    if (sessionId) {
      getConfig(sessionId)
        .then((response) => {
          setSessionConfig({
            preset: response.editable_config.preset,
            toolsEnabled: response.editable_config.tools_enabled,
            creativity: response.editable_config.intent?.creativity ?? 0.5,
            verbosity: response.editable_config.intent?.verbosity ?? "normal",
            maxRounds: response.editable_config.intent?.rounds ?? 30,
            overrides: response.editable_config.overrides || {},
          });
        })
        .catch((err) => {
          console.error("Failed to load config:", err);
        });
    }
  }, [sessionId]);

  const handleSendMessage = async (content: string) => {
    try {
      // Merge session config with temporary overrides
      const config = {
        preset: sessionConfig.preset,
        overrides: {
          ...sessionConfig.overrides,
          ...tempConfig.overrides, // Temporary overrides take precedence
        },
      };

      await sendMessage(content, config);

      // Clear temporary config after sending
      setTempConfig({ overrides: {} });
    } catch (err) {
      console.error("Failed to send message:", err);
    }
  };

  const handleSelectMessage = (id: string) => {
    selectNode(id);
  };

  const handleNewSession = async () => {
    try {
      const newSessionId = await resetChat();
      if (newSessionId) {
        persistSession(newSessionId);
      }
      selectNode(null); // Clear selection
      setTempConfig({ overrides: {} });
    } catch (err) {
      console.error("Failed to create new session:", err);
    }
  };

  // Convert messages to MessageCardProps
  const messageCards = messages.map(toMessageCardProps);

  return (
    <div className="chat-page flex flex-col h-screen bg-background">
      {/* Header */}
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

      {/* Session Config Panel */}
      <SessionConfigPanel
        sessionId={sessionId}
        config={sessionConfig}
        onConfigChanged={setSessionConfig}
        disabled={isLoading}
      />

      {/* Chat Container */}
      <ChatContainer
        messages={messageCards}
        selectedMessageId={selectedMessageId || undefined}
        onSelectMessage={handleSelectMessage}
        isLoading={isLoading}
      />

      {/* Chat Input with Temporary Config */}
      <div className="sticky bottom-0 z-10 border-t border-border bg-background/95 backdrop-blur supports-[backdrop-filter]:bg-background/60">
        <div className="max-w-4xl mx-auto px-4 py-3 space-y-2">
          <TemporaryConfigPanel
            config={tempConfig}
            onChange={setTempConfig}
            disabled={isLoading}
          />
          <ChatInput onSend={handleSendMessage} disabled={isLoading} />
        </div>
      </div>
    </div>
  );
}
