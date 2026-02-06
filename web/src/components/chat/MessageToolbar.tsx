// MessageToolbar - Reusable toolbar for checkpoint and branch operations

import {
  DropdownMenu,
  DropdownMenuTrigger,
  DropdownMenuContent,
  DropdownMenuItem,
} from "../ui/dropdown-menu";
import { ChevronDown, GitBranch, Bookmark, ArrowRight } from "lucide-react";
import { Button } from "../ui/button";

export interface MessageToolbarProps {
  nodeId: string;
  role: "user" | "assistant" | "system" | "tool";
  isStreaming?: boolean;

  // Checkpoint creation
  canCreateCheckpoint?: boolean;
  onCreateCheckpoint?: (nodeId: string) => void;

  // Branch creation
  onBranchAfter?: (nodeId: string) => void;
  onBranchAlternative?: (nodeId: string) => void;
}

export function MessageToolbar({
  nodeId,
  role,
  isStreaming = false,
  canCreateCheckpoint = false,
  onCreateCheckpoint,
  onBranchAfter,
  onBranchAlternative,
}: MessageToolbarProps) {
  // Don't render anything during streaming
  if (isStreaming) {
    return null;
  }

  const showCheckpoint = canCreateCheckpoint && onCreateCheckpoint && role === "assistant";
  const showBranch = (onBranchAfter || onBranchAlternative) && role !== "tool" && role !== "system";

  // Don't render if nothing to show
  if (!showCheckpoint && !showBranch) {
    return null;
  }

  return (
    <div className="flex items-center gap-2">
      {/* Checkpoint button */}
      {showCheckpoint && (
        <Button
          variant="ghost"
          size="sm"
          className="h-6 w-6 p-0 opacity-50 hover:opacity-100 transition-opacity text-[hsl(var(--role-checkpoint))]"
          onClick={(e) => {
            e.stopPropagation();
            onCreateCheckpoint(nodeId);
          }}
          title="Create checkpoint"
        >
          <Bookmark className="h-3.5 w-3.5" />
        </Button>
      )}

      {/* Branch dropdown */}
      {showBranch && (
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button
              variant="ghost"
              size="sm"
              className="h-6 px-1.5 opacity-50 hover:opacity-100 transition-opacity"
              onClick={(e) => {
                e.stopPropagation();
              }}
              title="Branch options (Ctrl+B / Ctrl+Shift+B)"
            >
              <GitBranch className="h-3.5 w-3.5" />
              <ChevronDown className="h-2.5 w-2.5 ml-0.5" />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end" onClick={(e) => e.stopPropagation()}>
            {onBranchAfter && (
              <DropdownMenuItem
                onClick={(e) => {
                  e.stopPropagation();
                  console.log("Branch After clicked, nodeId:", nodeId);
                  onBranchAfter(nodeId);
                }}
              >
                <ArrowRight className="h-3.5 w-3.5 mr-2" />
                <span>Continue from here</span>
                <span className="ml-auto pl-4 text-xs text-muted-foreground">
                  Ctrl+B
                </span>
              </DropdownMenuItem>
            )}
            {onBranchAlternative && (
              <DropdownMenuItem
                onClick={(e) => {
                  e.stopPropagation();
                  console.log("Branch Alternative clicked, nodeId:", nodeId);
                  onBranchAlternative(nodeId);
                }}
              >
                <GitBranch className="h-3.5 w-3.5 mr-2" />
                <span>Create alternative</span>
                <span className="ml-auto pl-4 text-xs text-muted-foreground">
                  Ctrl+Shift+B
                </span>
              </DropdownMenuItem>
            )}
          </DropdownMenuContent>
        </DropdownMenu>
      )}
    </div>
  );
}
