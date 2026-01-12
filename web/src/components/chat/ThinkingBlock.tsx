// Removed unused React import
import { Brain } from "lucide-react";

export interface ThinkingBlockProps {
  content: string;
}

export function ThinkingBlock({ content }: ThinkingBlockProps) {
  return (
    <div className="bg-purple-50 border border-purple-200 rounded-lg p-3 mb-3 shadow-sm">
      <div className="text-xs font-semibold text-purple-600 mb-1 flex items-center gap-1">
        <Brain className="w-3.5 h-3.5" />
        <span>Thinking</span>
      </div>
      <div className="text-sm text-foreground whitespace-pre-wrap leading-relaxed">
        {content}
      </div>
    </div>
  );
}
