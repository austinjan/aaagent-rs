import { useState, useRef, useEffect } from "react";
import { ChevronLeft, ChevronRight, GitBranch } from "lucide-react";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import type { NodeId } from "@/types/backend";

export interface BranchInfo {
  leafId: NodeId;
  depth: number;
  preview: string;
  lastRole: "user" | "assistant" | "system" | "tool";
  lastUpdated: Date;
  isActive: boolean;
  branchPointId: NodeId | null;
}

export interface BranchIndicatorProps {
  currentIndex: number;
  totalBranches: number;
  branches: BranchInfo[];
  disabled?: boolean;
  onPrev: () => void;
  onNext: () => void;
  onSelectBranch: (leafId: NodeId) => void;
}

export function BranchIndicator({
  currentIndex,
  totalBranches,
  branches,
  disabled = false,
  onPrev,
  onNext,
  onSelectBranch,
}: BranchIndicatorProps) {
  const [isOpen, setIsOpen] = useState(false);
  const dropdownRef = useRef<HTMLDivElement>(null);

  // Close dropdown when clicking outside
  useEffect(() => {
    function handleClickOutside(event: MouseEvent) {
      if (
        dropdownRef.current &&
        !dropdownRef.current.contains(event.target as Node)
      ) {
        setIsOpen(false);
      }
    }

    if (isOpen) {
      document.addEventListener("mousedown", handleClickOutside);
      return () =>
        document.removeEventListener("mousedown", handleClickOutside);
    }
  }, [isOpen]);

  // Close on Escape key
  useEffect(() => {
    function handleEscape(event: KeyboardEvent) {
      if (event.key === "Escape") {
        setIsOpen(false);
      }
    }

    if (isOpen) {
      document.addEventListener("keydown", handleEscape);
      return () => document.removeEventListener("keydown", handleEscape);
    }
  }, [isOpen]);

  if (totalBranches <= 1) {
    return null;
  }

  const canGoPrev = currentIndex > 1;
  const canGoNext = currentIndex < totalBranches;

  return (
    <div className="relative inline-flex items-center" ref={dropdownRef}>
      <div
        className={cn(
          "inline-flex items-center gap-0.5 rounded-md",
          "bg-[hsl(var(--muted))] border border-[hsl(var(--border))]",
          "transition-all",
          disabled && "opacity-50 pointer-events-none"
        )}
      >
        {/* Previous button */}
        <Button
          variant="ghost"
          size="icon"
          className="h-6 w-6 rounded-l-md rounded-r-none"
          disabled={!canGoPrev || disabled}
          onClick={(e) => {
            e.stopPropagation();
            onPrev();
          }}
          aria-label="Previous branch"
        >
          <ChevronLeft className="h-3.5 w-3.5" />
        </Button>

        {/* Branch count - clickable to open dropdown */}
        <button
          type="button"
          className={cn(
            "px-1.5 py-0.5 text-xs font-medium",
            "text-[hsl(var(--muted-foreground))]",
            "hover:bg-[hsl(var(--accent))] hover:text-[hsl(var(--accent-foreground))]",
            "transition-colors cursor-pointer",
            "flex items-center gap-1"
          )}
          onClick={(e) => {
            e.stopPropagation();
            setIsOpen(!isOpen);
          }}
          aria-expanded={isOpen}
          aria-haspopup="listbox"
        >
          <GitBranch className="h-3 w-3" />
          <span>
            {currentIndex}/{totalBranches}
          </span>
        </button>

        {/* Next button */}
        <Button
          variant="ghost"
          size="icon"
          className="h-6 w-6 rounded-r-md rounded-l-none"
          disabled={!canGoNext || disabled}
          onClick={(e) => {
            e.stopPropagation();
            onNext();
          }}
          aria-label="Next branch"
        >
          <ChevronRight className="h-3.5 w-3.5" />
        </Button>
      </div>

      {/* Dropdown */}
      {isOpen && (
        <div
          className={cn(
            "absolute top-full left-0 mt-1 z-50",
            "min-w-[200px] max-w-[300px]",
            "rounded-md border border-[hsl(var(--border))]",
            "bg-[hsl(var(--popover))] text-[hsl(var(--popover-foreground))]",
            "shadow-lg",
            "py-1"
          )}
          role="listbox"
          aria-label="Select branch"
        >
          {branches.map((branch, index) => {
            const branchNumber = index + 1;
            const isCurrentBranch = branchNumber === currentIndex;

            return (
              <button
                key={branch.leafId}
                type="button"
                role="option"
                aria-selected={isCurrentBranch}
                className={cn(
                  "w-full px-3 py-2 text-left",
                  "flex flex-col gap-0.5",
                  "transition-colors",
                  isCurrentBranch
                    ? "bg-[hsl(var(--accent))] text-[hsl(var(--accent-foreground))]"
                    : "hover:bg-[hsl(var(--muted))]"
                )}
                onClick={() => {
                  onSelectBranch(branch.leafId);
                  setIsOpen(false);
                }}
              >
                <div className="flex items-center justify-between">
                  <span className="text-sm font-medium">
                    Branch {branchNumber}
                  </span>
                  <RoleBadge role={branch.lastRole} />
                </div>
                <p className="text-xs text-[hsl(var(--muted-foreground))] truncate">
                  {branch.preview}
                </p>
                <p className="text-xs text-[hsl(var(--muted-foreground))]">
                  {formatRelativeTime(branch.lastUpdated)}
                </p>
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
}

function RoleBadge({ role }: { role: BranchInfo["lastRole"] }) {
  const roleConfig = {
    user: {
      label: "User",
      className: "bg-[hsl(var(--role-user)/0.15)] text-[hsl(var(--role-user))]",
    },
    assistant: {
      label: "AI",
      className:
        "bg-[hsl(var(--role-assistant)/0.15)] text-[hsl(var(--role-assistant))]",
    },
    system: {
      label: "Sys",
      className:
        "bg-[hsl(var(--role-system)/0.15)] text-[hsl(var(--role-system))]",
    },
    tool: {
      label: "Tool",
      className: "bg-[hsl(var(--role-tool)/0.15)] text-[hsl(var(--role-tool))]",
    },
  };

  const config = roleConfig[role];

  return (
    <span
      className={cn(
        "px-1.5 py-0.5 rounded text-xs font-medium",
        config.className
      )}
    >
      {config.label}
    </span>
  );
}

function formatRelativeTime(date: Date): string {
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffSec = Math.floor(diffMs / 1000);
  const diffMin = Math.floor(diffSec / 60);
  const diffHour = Math.floor(diffMin / 60);
  const diffDay = Math.floor(diffHour / 24);

  if (diffSec < 60) return "just now";
  if (diffMin < 60) return `${diffMin}m ago`;
  if (diffHour < 24) return `${diffHour}h ago`;
  if (diffDay < 7) return `${diffDay}d ago`;

  return date.toLocaleDateString();
}

export default BranchIndicator;
