import { useState } from "react";
import { Copy, Check } from "lucide-react";
import { Button } from "../ui/button";

interface CopySessionIdButtonProps {
  sessionId: string;
  variant?: "ghost" | "outline" | "default";
}

export function CopySessionIdButton({ sessionId, variant = "ghost" }: CopySessionIdButtonProps) {
  const [copied, setCopied] = useState(false);

  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(sessionId);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (err) {
      console.error("Failed to copy session ID:", err);
    }
  };

  return (
    <Button
      variant={variant}
      size="sm"
      onClick={handleCopy}
      className="gap-2"
      title="Copy session ID"
    >
      {copied ? (
        <>
          <Check className="h-3 w-3" />
          <span className="text-xs">Copied!</span>
        </>
      ) : (
        <>
          <Copy className="h-3 w-3" />
          <span className="text-xs font-mono">{sessionId.slice(0, 8)}...</span>
        </>
      )}
    </Button>
  );
}
