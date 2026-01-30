import { useEffect, useState } from "react";
import { MessageSquare, Plus, Loader2, X } from "lucide-react";
import {
  listSessions,
  archiveSession as archiveSessionApi,
  type SessionFilters,
} from "../../services/api";
import type { SessionInfo } from "../../types/backend";
import { SessionActions } from "./SessionActions";
import { Button } from "../ui/button";
import { Input } from "../ui/input";

interface SessionListSidebarProps {
  currentSessionId: string | null;
  onSessionSelect: (sessionId: string) => void;
  onNewSession: () => void;
  refreshTrigger?: number; // Optional trigger to force refresh
}

export function SessionListSidebar({
  currentSessionId,
  onSessionSelect,
  onNewSession,
  refreshTrigger,
}: SessionListSidebarProps) {
  const [sessions, setSessions] = useState<SessionInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [filterInput, setFilterInput] = useState("");
  const [activeFilters, setActiveFilters] = useState<SessionFilters>({});

  const loadSessions = async (filters?: SessionFilters) => {
    try {
      setLoading(true);
      setError(null);
      const response = await listSessions(filters);
      // Sort by updated_at (most recent first)
      const sorted = [...response.sessions].sort(
        (a, b) => b.updated_at - a.updated_at,
      );
      setSessions(sorted);
    } catch (err) {
      console.error("Failed to load sessions:", err);
      setError("Failed to load sessions");
    } finally {
      setLoading(false);
    }
  };

  const handleFilterInputKeyDown = (
    e: React.KeyboardEvent<HTMLInputElement>,
  ) => {
    if (e.key === "Enter" && filterInput.trim()) {
      const trimmed = filterInput.trim();
      // Parse input: if it contains spaces, treat as name search, otherwise as tag
      const isTag = !trimmed.includes(" ");

      if (isTag) {
        // Add to tags filter
        const newTags = [...(activeFilters.tags || []), trimmed];
        const newFilters = { ...activeFilters, tags: newTags };
        setActiveFilters(newFilters);
        loadSessions(newFilters);
      } else {
        // Set name filter
        const newFilters = { ...activeFilters, name: trimmed };
        setActiveFilters(newFilters);
        loadSessions(newFilters);
      }
      setFilterInput("");
    }
  };

  const handleRemoveTagFilter = (tag: string) => {
    const newTags = (activeFilters.tags || []).filter((t) => t !== tag);
    const newFilters = {
      ...activeFilters,
      tags: newTags.length > 0 ? newTags : undefined,
    };
    setActiveFilters(newFilters);
    loadSessions(newFilters);
  };

  const handleClearNameFilter = () => {
    const newFilters = { ...activeFilters, name: undefined };
    setActiveFilters(newFilters);
    loadSessions(newFilters);
  };

  const handleClearAllFilters = () => {
    setActiveFilters({});
    loadSessions();
  };

  useEffect(() => {
    loadSessions(activeFilters);
  }, [refreshTrigger]); // Reload when refreshTrigger changes

  const handleArchiveSession = async (sessionId: string) => {
    try {
      await archiveSessionApi(sessionId);

      // If we archived the current session, trigger new session creation
      // (which will also trigger refresh)
      if (sessionId === currentSessionId) {
        onNewSession();
      } else {
        // For non-current sessions, reload to ensure consistency
        await loadSessions(activeFilters);
      }
    } catch (err) {
      console.error("Failed to archive session:", err);
      alert("Failed to archive session. Please try again.");
    }
  };

  const formatTimestamp = (timestamp: number) => {
    const date = new Date(timestamp * 1000);
    const now = new Date();
    const diffMs = now.getTime() - date.getTime();
    const diffMins = Math.floor(diffMs / 60000);
    const diffHours = Math.floor(diffMins / 60);
    const diffDays = Math.floor(diffHours / 24);

    if (diffMins < 1) return "Just now";
    if (diffMins < 60) return `${diffMins}m ago`;
    if (diffHours < 24) return `${diffHours}h ago`;
    if (diffDays < 7) return `${diffDays}d ago`;
    return date.toLocaleDateString();
  };

  return (
    <div className="flex flex-col h-full bg-background border-r border-border">
      {/* Header */}
      <div className="p-4 border-b border-border">
        <div className="flex items-center justify-between mb-3">
          <h2 className="text-lg font-semibold text-foreground">Sessions</h2>
          <Button
            variant="default"
            size="sm"
            onClick={onNewSession}
            className="gap-2"
          >
            <Plus className="h-4 w-4" />
            New
          </Button>
        </div>

        {/* Filter Input */}
        <div className="mb-2">
          <Input
            type="text"
            placeholder="Filter by tag or name..."
            value={filterInput}
            onChange={(e) => setFilterInput(e.target.value)}
            onKeyDown={handleFilterInputKeyDown}
            className="h-8 text-sm"
          />
          <p className="text-xs text-muted-foreground mt-1">
            Press Enter to add tag (single word) or search name (with spaces)
          </p>
        </div>

        {/* Show Archived Toggle */}
        <div className="mb-2">
          <label className="flex items-center gap-2 text-sm cursor-pointer">
            <input
              type="checkbox"
              checked={activeFilters.include_archived || false}
              onChange={(e) => {
                const newFilters = {
                  ...activeFilters,
                  include_archived: e.target.checked,
                };
                setActiveFilters(newFilters);
                loadSessions(newFilters);
              }}
              className="checkbox checkbox-sm checkbox-primary"
            />
            <span className="text-muted-foreground">
              Show archived sessions
            </span>
          </label>
        </div>

        {/* Active Filters */}
        {(activeFilters.tags || activeFilters.name) && (
          <div className="flex flex-wrap gap-1 mb-2">
            {activeFilters.tags?.map((tag) => (
              <span
                key={tag}
                className="inline-flex items-center gap-1 px-2 py-0.5 rounded text-xs font-medium bg-primary/20 text-primary"
              >
                #{tag}
                <button
                  onClick={() => handleRemoveTagFilter(tag)}
                  className="hover:text-primary/70"
                >
                  <X className="h-3 w-3" />
                </button>
              </span>
            ))}
            {activeFilters.name && (
              <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded text-xs font-medium bg-secondary text-secondary-foreground">
                name: {activeFilters.name}
                <button
                  onClick={handleClearNameFilter}
                  className="hover:opacity-70"
                >
                  <X className="h-3 w-3" />
                </button>
              </span>
            )}
            <button
              onClick={handleClearAllFilters}
              className="text-xs text-muted-foreground hover:text-foreground underline"
            >
              Clear all
            </button>
          </div>
        )}

        <p className="text-xs text-muted-foreground">
          {sessions.length} {sessions.length === 1 ? "session" : "sessions"}
        </p>
      </div>

      {/* Session List */}
      <div className="flex-1 overflow-y-auto">
        {loading && (
          <div className="flex items-center justify-center py-8">
            <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
          </div>
        )}

        {error && (
          <div className="p-4 text-sm text-destructive">
            {error}
            <button
              onClick={() => loadSessions()}
              className="block mt-2 text-primary hover:underline"
            >
              Retry
            </button>
          </div>
        )}

        {!loading && !error && sessions.length === 0 && (
          <div className="p-6 text-center">
            <MessageSquare className="h-12 w-12 mx-auto mb-3 text-muted-foreground opacity-50" />
            <p className="text-sm text-muted-foreground mb-4">
              No sessions yet
            </p>
            <Button
              variant="outline"
              size="sm"
              onClick={onNewSession}
              className="gap-2"
            >
              <Plus className="h-4 w-4" />
              Create your first session
            </Button>
          </div>
        )}

        {!loading && !error && sessions.length > 0 && (
          <div className="divide-y divide-border">
            {sessions.map((session) => (
              <div
                key={session.session_id}
                className={`
                  group relative p-4 cursor-pointer transition-colors
                  hover:bg-accent/50
                  ${session.session_id === currentSessionId ? "bg-accent" : ""}
                `}
                onClick={() => onSessionSelect(session.session_id)}
              >
                {/* Active indicator */}
                {session.session_id === currentSessionId && (
                  <div className="absolute left-0 top-0 bottom-0 w-1 bg-primary" />
                )}

                <div className="flex items-start justify-between gap-2">
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-2">
                      <h3 className="text-sm font-medium text-foreground truncate">
                        {session.name || "Untitled Session"}
                      </h3>
                      {session.archived && (
                        <span className="inline-flex items-center px-1.5 py-0.5 rounded text-xs font-medium bg-muted text-muted-foreground border border-border">
                          Archived
                        </span>
                      )}
                    </div>
                    <div className="flex items-center gap-2 mt-1">
                      <span className="text-xs text-muted-foreground">
                        {session.message_count} messages
                      </span>
                      <span className="text-xs text-muted-foreground">•</span>
                      <span className="text-xs text-muted-foreground">
                        {formatTimestamp(session.updated_at)}
                      </span>
                    </div>
                    {/* Tags */}
                    {session.tags && session.tags.length > 0 && (
                      <div className="flex flex-wrap gap-1 mt-2">
                        {session.tags.map((tag) => (
                          <span
                            key={tag}
                            className="inline-flex items-center px-2 py-0.5 rounded text-xs font-medium bg-primary/10 text-primary"
                          >
                            {tag}
                          </span>
                        ))}
                      </div>
                    )}
                  </div>

                  {/* Actions */}
                  <div className="flex items-center gap-1">
                    <SessionActions
                      sessionId={session.session_id}
                      sessionName={session.name || undefined}
                      onArchive={handleArchiveSession}
                    />
                  </div>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
