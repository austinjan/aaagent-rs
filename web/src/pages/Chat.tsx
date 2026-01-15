import { useEffect, useState } from "react";
import { Plus } from "lucide-react";
import { ChatContainer } from "../components/chat/ChatContainer";
import { ChatInput } from "../components/chat/ChatInput";
import {
  SessionConfigPanel,
  type SessionConfig,
} from "../components/chat/SessionConfigPanel";
import { TreeNavigationPanel } from "../components/tree/TreeNavigationPanel";
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
    tool_calls: msg.tool_calls,
    tool_call_id: msg.tool_call_id,
    is_error: msg.is_error,
    timestamp: msg.timestamp,
    isStreaming: msg.isStreaming,
  };
}

const DEFAULT_OPTIONS = {
  sessionName: "New Chat",
};
const SESSION_STORAGE_KEY = "aaagent.session_id";

export function Chat() {
  const {
    sessionId,
    messages,
    isLoading,
    treeNodes,
    activeLeafId,
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
    provider: {
      model: "gpt-5-mini",
      temperature: 1.0,
      max_tokens: 16384,
      top_p: undefined,
      frequency_penalty: undefined,
      presence_penalty: undefined,
    },
    agent: {
      max_rounds: 30,
      tools_enabled: true,
    },
    session: {
      system_prompt: "You are a helpful, friendly assistant.",
      max_context_tokens: 200000,
    },
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
          setSessionConfig(response.session_config);
        })
        .catch((err) => {
          console.error("Failed to load config:", err);
        });
    }
  }, [sessionId]);

  const handleSendMessage = async (content: string) => {
    try {
      await sendMessage(content);
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
        <div className="max-w-7xl mx-auto px-4 py-4">
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

      {/* Main Content: Tree Panel + Chat */}
      <div className="flex flex-1 overflow-hidden">
        {/* Tree Navigation Panel (Left Sidebar) */}
        <div className="w-80 border-r border-border flex-shrink-0">
          <TreeNavigationPanel
            nodes={treeNodes}
            activeLeafId={activeLeafId}
            selectedNodeId={selectedMessageId}
            onNodeSelect={handleSelectMessage}
          />
        </div>

        {/* Chat Area (Right Side) */}
        <div className="flex-1 flex flex-col overflow-hidden">
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

          {/* Chat Input */}
          <div className="sticky bottom-0 z-10 border-t border-border bg-background/95 backdrop-blur supports-[backdrop-filter]:bg-background/60">
            <div className="max-w-4xl mx-auto px-4 py-3 space-y-2">
              <ChatInput onSend={handleSendMessage} disabled={isLoading} />
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
