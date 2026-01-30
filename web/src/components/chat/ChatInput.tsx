import { useState, useRef, useEffect, type KeyboardEvent } from "react";
import { Send, Loader2 } from "lucide-react";
import { Button } from "../ui/button";
import { cn } from "@/lib/utils";

export interface ChatInputProps {
  onSend: (message: string) => void;
  disabled?: boolean;
  placeholder?: string;
}

export function ChatInput({
  onSend,
  disabled = false,
  placeholder = "Type your message... (Enter to send, Shift+Enter for new line)",
}: ChatInputProps) {
  const [message, setMessage] = useState("");
  const [isFocused, setIsFocused] = useState(false);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  // Auto-resize textarea based on content
  useEffect(() => {
    const textarea = textareaRef.current;
    if (textarea) {
      textarea.style.height = "auto";
      textarea.style.height = `${textarea.scrollHeight}px`;
    }
  }, [message]);

  // Auto-focus on mount
  useEffect(() => {
    textareaRef.current?.focus();
  }, []);

  const handleSend = () => {
    const trimmed = message.trim();
    if (trimmed && !disabled) {
      onSend(trimmed);
      setMessage("");
      // Reset textarea height
      if (textareaRef.current) {
        textareaRef.current.style.height = "auto";
        textareaRef.current.focus();
      }
    }
  };

  const handleKeyDown = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    // Enter alone to send (Shift+Enter for new line)
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  };

  const canSend = message.trim().length > 0 && !disabled;

  return (
    <div className="sticky bottom-0 z-10 border-t border-border bg-background/95 backdrop-blur supports-[backdrop-filter]:bg-background/60">
      <div className="max-w-4xl mx-auto px-4 py-4">
        <div
          className={cn(
            "flex gap-3 rounded-lg border p-2 transition-all",
            isFocused
              ? "border-ring ring-2 ring-ring/20 shadow-sm"
              : "border-input shadow-sm",
            disabled && "opacity-50",
          )}
        >
          <textarea
            ref={textareaRef}
            value={message}
            onChange={(e) => setMessage(e.target.value)}
            onKeyDown={handleKeyDown}
            onFocus={() => setIsFocused(true)}
            onBlur={() => setIsFocused(false)}
            disabled={disabled}
            placeholder={placeholder}
            rows={1}
            className="flex-1 resize-none bg-transparent px-2 py-1 text-sm placeholder:text-muted-foreground focus-visible:outline-none disabled:cursor-not-allowed min-h-[36px] max-h-[200px] overflow-y-auto"
          />
          <div className="flex items-end gap-2">
            {disabled && (
              <div className="flex items-center text-xs text-muted-foreground mb-1">
                <Loader2 className="mr-1 h-3 w-3 animate-spin" />
                Sending...
              </div>
            )}
            <Button
              size="icon"
              onClick={handleSend}
              disabled={!canSend}
              title={
                canSend
                  ? "Send message (Enter)"
                  : disabled
                    ? "Waiting for response..."
                    : "Type a message to send"
              }
              className="shrink-0"
            >
              <Send className="h-4 w-4" />
            </Button>
          </div>
        </div>
        {/* Keyboard hint */}
        <div className="mt-2 flex items-center justify-between text-xs text-muted-foreground">
          <span>Press Enter to send, Shift+Enter for new line</span>
          <span className="text-right">
            {message.length > 0 && `${message.length} characters`}
          </span>
        </div>
      </div>
    </div>
  );
}
