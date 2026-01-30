import { Archive, MoreVertical } from "lucide-react";
import { Button } from "../ui/button";
import { useState } from "react";

interface SessionActionsProps {
  sessionId: string;
  sessionName?: string;
  onArchive: (sessionId: string) => void;
  disabled?: boolean;
}

export function SessionActions({
  sessionId,
  sessionName,
  onArchive,
  disabled = false,
}: SessionActionsProps) {
  const [showMenu, setShowMenu] = useState(false);

  const handleArchive = async () => {
    setShowMenu(false);
    onArchive(sessionId);
  };

  return (
    <div className="relative">
      <Button
        variant="ghost"
        size="sm"
        onClick={() => setShowMenu(!showMenu)}
        disabled={disabled}
        className="h-8 w-8 p-0"
        title="Session actions"
      >
        <MoreVertical className="h-4 w-4" />
      </Button>

      {showMenu && (
        <>
          {/* Backdrop to close menu */}
          <div
            className="fixed inset-0 z-10"
            onClick={() => setShowMenu(false)}
          />

          {/* Dropdown menu */}
          <div className="absolute right-0 top-full mt-1 z-20 min-w-[160px] rounded-md border border-border bg-popover shadow-md">
            <button
              onClick={handleArchive}
              disabled={disabled}
              className="flex w-full items-center gap-2 px-3 py-2 text-sm hover:bg-accent disabled:opacity-50 disabled:cursor-not-allowed rounded-md"
            >
              <Archive className="h-4 w-4" />
              Archive Session
            </button>
          </div>
        </>
      )}
    </div>
  );
}
