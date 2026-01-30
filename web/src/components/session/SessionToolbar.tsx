import { useState } from "react";
import {
  Edit2,
  Archive,
  MessageSquare,
  GitBranch,
  Check,
  X,
  Sparkles,
  Settings2,
} from "lucide-react";
import { Button } from "../ui/button";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "../ui/select";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "../ui/dialog";
import { SUPPORTED_MODELS, CUSTOM_MODEL_VALUE } from "@/lib/constants";
import { SessionConfigPanel } from "../chat/SessionConfigPanel";
import type { SessionConfig } from "../chat/SessionConfigPanel";

interface SessionToolbarProps {
  sessionId: string;
  sessionName: string | null;
  currentView: "chat" | "tree";
  config: SessionConfig;
  onRename: (newName: string) => void;
  onAIRename: () => void;
  onArchive: () => void;
  onViewChange: (view: "chat" | "tree") => void;
  onConfigChanged: (config: SessionConfig) => void;
  // Tree actions
  onCreateBranch?: () => void;
  onCreateCheckpoint?: () => void;
  disabled?: boolean;
}

export function SessionToolbar({
  sessionId,
  sessionName,
  currentView,
  config,
  onRename,
  onAIRename,
  onArchive,
  onViewChange,
  onConfigChanged,
  onCreateBranch,
  onCreateCheckpoint,
  disabled = false,
}: SessionToolbarProps) {
  const [isEditing, setIsEditing] = useState(false);
  const [editName, setEditName] = useState(sessionName || "");
  const [showRenameMenu, setShowRenameMenu] = useState(false);
  const [showSettingsModal, setShowSettingsModal] = useState(false);

  const handleStartEdit = () => {
    setEditName(sessionName || "");
    setIsEditing(true);
    setShowRenameMenu(false);
  };

  const handleAIRename = () => {
    setShowRenameMenu(false);
    onAIRename();
  };

  const handleSaveEdit = () => {
    const trimmed = editName.trim();
    if (trimmed && trimmed !== sessionName) {
      onRename(trimmed);
    }
    setIsEditing(false);
  };

  const handleCancelEdit = () => {
    setEditName(sessionName || "");
    setIsEditing(false);
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter") {
      handleSaveEdit();
    } else if (e.key === "Escape") {
      handleCancelEdit();
    }
  };

  return (
    <div className="flex items-center gap-2">
      {/* Session Name / Rename */}
      {isEditing ? (
        <div className="flex items-center gap-1">
          <input
            type="text"
            value={editName}
            onChange={(e) => setEditName(e.target.value)}
            onKeyDown={handleKeyDown}
            className="px-2 py-1 text-sm border border-border rounded bg-background focus:outline-none focus:ring-2 focus:ring-ring"
            autoFocus
            disabled={disabled}
          />
          <Button
            variant="ghost"
            size="sm"
            onClick={handleSaveEdit}
            disabled={disabled}
            className="h-7 w-7 p-0"
            title="Save"
          >
            <Check className="h-4 w-4 text-green-600" />
          </Button>
          <Button
            variant="ghost"
            size="sm"
            onClick={handleCancelEdit}
            disabled={disabled}
            className="h-7 w-7 p-0"
            title="Cancel"
          >
            <X className="h-4 w-4 text-muted-foreground" />
          </Button>
        </div>
      ) : (
        <div className="relative">
          <Button
            variant="ghost"
            size="sm"
            onClick={() => setShowRenameMenu(!showRenameMenu)}
            disabled={disabled}
            className="gap-2"
            title="Rename session"
          >
            <Edit2 className="h-4 w-4" />
            Rename
          </Button>

          {showRenameMenu && (
            <>
              {/* Backdrop */}
              <div
                className="fixed inset-0 z-10"
                onClick={() => setShowRenameMenu(false)}
              />
              {/* Dropdown menu */}
              <div className="absolute left-0 top-full mt-1 z-20 min-w-[180px] rounded-md border border-border bg-popover shadow-md">
                <button
                  onClick={handleStartEdit}
                  disabled={disabled}
                  className="flex w-full items-center gap-2 px-3 py-2 text-sm hover:bg-accent disabled:opacity-50 disabled:cursor-not-allowed rounded-t-md"
                >
                  <Edit2 className="h-4 w-4" />
                  Manual Rename
                </button>
                <button
                  onClick={handleAIRename}
                  disabled={disabled}
                  className="flex w-full items-center gap-2 px-3 py-2 text-sm hover:bg-accent disabled:opacity-50 disabled:cursor-not-allowed rounded-b-md"
                >
                  <Sparkles className="h-4 w-4" />
                  AI Generate Name
                </button>
              </div>
            </>
          )}
        </div>
      )}

      {/* Divider */}
      <div className="h-6 w-px bg-border" />

      {/* View Toggle */}
      <div className="flex items-center gap-1 bg-muted rounded-md p-1">
        <Button
          variant={currentView === "chat" ? "default" : "ghost"}
          size="sm"
          onClick={() => onViewChange("chat")}
          disabled={disabled}
          className="h-7 gap-2"
          title="Show chat"
        >
          <MessageSquare className="h-4 w-4" />
          Chat
        </Button>
        <Button
          variant={currentView === "tree" ? "default" : "ghost"}
          size="sm"
          onClick={() => onViewChange("tree")}
          disabled={disabled}
          className="h-7 gap-2"
          title="Show tree history"
        >
          <GitBranch className="h-4 w-4" />
          Tree
        </Button>
      </div>

      {/* Spacer */}
      <div className="flex-1" />

      {/* View-specific Actions */}
      {currentView === "chat" ? (
        <>
          {/* Model Selector for Chat view */}
          <Select
            value={config.provider.model}
            onValueChange={(model) =>
              onConfigChanged({
                ...config,
                provider: { ...config.provider, model },
              })
            }
            disabled={disabled}
          >
            <SelectTrigger className="h-8 w-[180px]">
              <SelectValue placeholder="Select a model..." />
            </SelectTrigger>
            <SelectContent>
              {SUPPORTED_MODELS.map((model) => (
                <SelectItem key={model.value} value={model.value}>
                  {model.label}
                </SelectItem>
              ))}
              <SelectItem value={CUSTOM_MODEL_VALUE}>
                Custom Model...
              </SelectItem>
            </SelectContent>
          </Select>

          {/* Settings Button */}
          <Button
            variant="ghost"
            size="sm"
            onClick={() => setShowSettingsModal(true)}
            disabled={disabled}
            className="gap-2"
            title="Session settings"
          >
            <Settings2 className="h-4 w-4" />
            Settings
          </Button>
        </>
      ) : (
        <>
          {/* Tree Actions */}
          <Button
            variant="ghost"
            size="sm"
            onClick={onCreateBranch}
            disabled={disabled}
            className="gap-2"
            title="Create branch from selected"
          >
            <GitBranch className="h-4 w-4" />
            Create Branch
          </Button>
          <Button
            variant="ghost"
            size="sm"
            onClick={onCreateCheckpoint}
            disabled={disabled}
            className="gap-2"
            title="Create checkpoint"
          >
            <Archive className="h-4 w-4" />
            Checkpoint
          </Button>
        </>
      )}

      {/* Divider */}
      <div className="h-6 w-px bg-border" />

      {/* Archive */}
      <Button
        variant="ghost"
        size="sm"
        onClick={onArchive}
        disabled={disabled}
        className="gap-2"
        title="Archive session"
      >
        <Archive className="h-4 w-4" />
        Archive
      </Button>

      {/* Settings Modal */}
      <Dialog open={showSettingsModal} onOpenChange={setShowSettingsModal}>
        <DialogContent className="max-w-4xl max-h-[90vh] overflow-y-auto">
          <DialogHeader>
            <DialogTitle>Session Settings</DialogTitle>
          </DialogHeader>
          <SessionConfigPanel
            sessionId={sessionId}
            config={config}
            onConfigChanged={(newConfig) => {
              onConfigChanged(newConfig);
              setShowSettingsModal(false);
            }}
            disabled={disabled}
          />
        </DialogContent>
      </Dialog>
    </div>
  );
}
