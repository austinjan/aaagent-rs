import { useCallback, useEffect, useState } from "react";
import { Plus } from "lucide-react";
import { ChatContainer } from "../components/chat/ChatContainer";
import { ChatInput } from "../components/chat/ChatInput";
import {
  SessionConfigPanel,
  type SessionConfig,
} from "../components/chat/SessionConfigPanel";
import { TreeNavigationPanel } from "../components/tree/TreeNavigationPanel";
import {
  SessionListSidebar,
  CopySessionIdButton,
  SessionToolbar,
} from "../components/session";
import { Button } from "../components/ui/button";
import { useChat } from "../hooks/useChat";
import { useChatStore, selectSelectedNodeId } from "../store/useChatStore";
import {
  getConfig,
  switchBranch,
  createBranch,
  renameSession,
  archiveSession,
  aiRenameSession,
  getSession,
} from "../services/api";
import type { MessageCardProps } from "../components/chat/MessageCard";
import type { MessageData } from "../types/backend";
import { Role } from "../types/backend";
import { CheckpointCreationModal } from "../components/checkpoint";
import { SummaryPanel } from "../components/summary";

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
    loadTree,
    resetChat,
  } = useChat(DEFAULT_OPTIONS);

  // Get selection from Zustand store
  const selectedMessageId = useChatStore(selectSelectedNodeId);
  const selectNode = useChatStore((state) => state.selectNode);

  // Session list refresh trigger
  const [sessionListRefresh, setSessionListRefresh] = useState(0);

  // View state: chat or tree
  const [currentView, setCurrentView] = useState<"chat" | "tree">("chat");

  // Handle view change with state refresh
  const handleViewChange = useCallback(
    async (view: "chat" | "tree") => {
      setCurrentView(view);
      if (view === "tree" && sessionId) {
        // Refresh tree state from backend when switching to tree view
        try {
          await loadTree(sessionId);
        } catch (err) {
          console.error("Failed to refresh tree state:", err);
        }
      }
    },
    [sessionId, loadTree],
  );

  // Session name
  const [sessionName, setSessionName] = useState<string | null>(null);

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

  // Checkpoint modal state
  const [checkpointModalOpen, setCheckpointModalOpen] = useState(false);
  const [checkpointTargetNodeId, setCheckpointTargetNodeId] = useState<
    string | null
  >(null);

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

  // Load config and session info from backend when session is ready
  useEffect(() => {
    if (sessionId) {
      getConfig(sessionId)
        .then((response) => {
          setSessionConfig(response.session_config);
        })
        .catch((err) => {
          console.error("Failed to load config:", err);
        });

      getSession(sessionId)
        .then((response) => {
          setSessionName(response.name);
        })
        .catch((err) => {
          console.error("Failed to load session info:", err);
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
        // Trigger session list refresh
        setSessionListRefresh((prev) => prev + 1);
      }
      selectNode(null); // Clear selection
    } catch (err) {
      console.error("Failed to create new session:", err);
    }
  };

  const handleSessionSelect = async (selectedSessionId: string) => {
    try {
      if (selectedSessionId !== sessionId) {
        await loadHistory(selectedSessionId);
        persistSession(selectedSessionId);
        selectNode(null); // Clear selection when switching sessions
        // Trigger session list refresh to update timestamps
        setSessionListRefresh((prev) => prev + 1);
      }
    } catch (err) {
      console.error("Failed to load session:", err);
    }
  };

  const handleRenameSession = async (newName: string) => {
    if (!sessionId) return;
    try {
      await renameSession(sessionId, newName);
      // Update local state
      setSessionName(newName);
      // Trigger session list refresh
      setSessionListRefresh((prev) => prev + 1);
    } catch (err) {
      console.error("Failed to rename session:", err);
      alert("Failed to rename session. Please try again.");
    }
  };

  const handleAIRenameSession = async () => {
    if (!sessionId) return;
    try {
      const response = await aiRenameSession(sessionId);
      // Update local state with AI-generated name
      setSessionName(response.name);
      // Trigger session list refresh
      setSessionListRefresh((prev) => prev + 1);
    } catch (err) {
      console.error("Failed to AI rename session:", err);
      alert("Failed to generate AI name. Please try again.");
    }
  };

  const handleArchiveSession = async () => {
    if (!sessionId) return;
    try {
      await archiveSession(sessionId);
      // Create a new session after archiving current one
      await handleNewSession();
    } catch (err) {
      console.error("Failed to archive session:", err);
      alert("Failed to archive session. Please try again.");
    }
  };

  // Branch operations
  const setActiveLeaf = useChatStore((state) => state.setActiveLeaf);

  const handleBranchSwitch = useCallback(
    async (nodeId: string) => {
      if (!sessionId) return;
      try {
        console.log("Switching branch to node:", nodeId);
        const response = await switchBranch(sessionId, nodeId);
        if (response.success) {
          console.log(
            "Branch switch success, new active leaf:",
            response.active_leaf_id,
          );
          setActiveLeaf(response.active_leaf_id);
          // Reload the session to get the updated path
          await loadHistory(sessionId);
          // Also explicitly reload tree to ensure minimap is updated
          await loadTree(sessionId);
        }
      } catch (err) {
        console.error("Failed to switch branch:", err);
      }
    },
    [sessionId, setActiveLeaf, loadHistory, loadTree],
  );

  const handleCreateBranch = useCallback(
    async (fromNodeId: string) => {
      if (!sessionId) return;
      try {
        console.log("Creating branch from node:", fromNodeId);
        const response = await createBranch(sessionId, fromNodeId);
        if (response.success) {
          console.log("Branch created, new leaf:", response.new_leaf_id);
          setActiveLeaf(response.new_leaf_id);
          // Reload to show the new branch point
          await loadHistory(sessionId);
          // Also explicitly reload tree to ensure minimap is updated
          await loadTree(sessionId);
        }
      } catch (err) {
        console.error("Failed to create branch:", err);
      }
    },
    [sessionId, setActiveLeaf, loadHistory, loadTree],
  );

  // Keyboard shortcut: Ctrl+B to create branch from selected message
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key === "b" && selectedMessageId) {
        e.preventDefault();
        handleCreateBranch(selectedMessageId);
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [selectedMessageId, handleCreateBranch]);

  // Checkpoint creation handlers
  const handleOpenCheckpointModal = useCallback((nodeId: string) => {
    setCheckpointTargetNodeId(nodeId);
    setCheckpointModalOpen(true);
  }, []);

  const handleCloseCheckpointModal = useCallback(() => {
    setCheckpointModalOpen(false);
    setCheckpointTargetNodeId(null);
  }, []);

  const handleCheckpointCreated = useCallback(
    async (checkpointId: string) => {
      console.log("Checkpoint created:", checkpointId);
      // Reload the session to show the checkpoint
      if (sessionId) {
        await loadHistory(sessionId);
        await loadTree(sessionId);
      }
    },
    [sessionId, loadHistory, loadTree],
  );

  // Calculate node count for checkpoint modal (count from target to root or last checkpoint)
  const getNodeCountForCheckpoint = useCallback(() => {
    if (!checkpointTargetNodeId) return 0;
    // For now, return a placeholder - the backend calculates this
    return messages.length;
  }, [checkpointTargetNodeId, messages.length]);

  // Estimate tokens for checkpoint (rough estimate)
  const getEstimatedTokensForCheckpoint = useCallback(() => {
    // Rough estimate: 4 chars per token
    const totalChars = messages.reduce(
      (sum, msg) => sum + msg.content.length,
      0,
    );
    return Math.round(totalChars / 4);
  }, [messages]);

  // Convert messages to MessageCardProps with branch and checkpoint callbacks
  const messageCards = messages.map((msg) => ({
    ...toMessageCardProps(msg),
    onCreateBranch: handleCreateBranch,
    canCreateCheckpoint: !isLoading,
    onCreateCheckpoint: handleOpenCheckpointModal,
    // Pass through checkpoint message fields if present
    ...(msg.isCheckpoint && {
      isCheckpoint: msg.isCheckpoint,
      checkpointNodeId: msg.checkpointNodeId,
      checkpointData: msg.checkpointData,
    }),
  }));

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
                <CopySessionIdButton sessionId={sessionId} variant="outline" />
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

      {/* Main Content: Session List + Chat Area */}
      <div className="flex flex-1 overflow-hidden">
        {/* Session List Sidebar (Left) */}
        <div className="w-64 flex-shrink-0">
          <SessionListSidebar
            currentSessionId={sessionId}
            onSessionSelect={handleSessionSelect}
            onNewSession={handleNewSession}
            refreshTrigger={sessionListRefresh}
          />
        </div>

        {/* Chat Area (Right Side) */}
        <div className="flex-1 flex flex-col overflow-hidden">
          {/* Session Toolbar */}
          {sessionId && (
            <div className="border-b border-border bg-background p-3">
              <SessionToolbar
                sessionId={sessionId}
                sessionName={sessionName}
                currentView={currentView}
                config={sessionConfig}
                onRename={handleRenameSession}
                onAIRename={handleAIRenameSession}
                onArchive={handleArchiveSession}
                onViewChange={handleViewChange}
                onConfigChanged={setSessionConfig}
                onCreateBranch={
                  selectedMessageId
                    ? () => handleCreateBranch(selectedMessageId)
                    : undefined
                }
                onCreateCheckpoint={
                  selectedMessageId
                    ? () => handleOpenCheckpointModal(selectedMessageId)
                    : undefined
                }
                disabled={isLoading}
              />
            </div>
          )}

          {/* Main Content Area - Chat or Tree */}
          {currentView === "chat" ? (
            <>
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
            </>
          ) : (
            /* Tree View */
            <TreeNavigationPanel
              nodes={treeNodes}
              activeLeafId={activeLeafId || ""}
              selectedNodeId={selectedMessageId}
              onNodeSelect={handleSelectMessage}
              onBranchSwitch={handleBranchSwitch}
            />
          )}
        </div>
      </div>

      {/* Checkpoint Creation Modal */}
      {sessionId && checkpointTargetNodeId && (
        <CheckpointCreationModal
          isOpen={checkpointModalOpen}
          sessionId={sessionId}
          targetNodeId={checkpointTargetNodeId}
          nodeCount={getNodeCountForCheckpoint()}
          estimatedTokens={getEstimatedTokensForCheckpoint()}
          hasPreviousCheckpoint={false}
          onClose={handleCloseCheckpointModal}
          onCheckpointCreated={handleCheckpointCreated}
        />
      )}
    </div>
  );
}
