// GroundingSources component - Display web search citations from Gemini grounding

import { ExternalLink, Globe, ChevronDown } from "lucide-react";
import { useState } from "react";
import { cn } from "@/lib/utils";

export interface GroundingChunk {
  web?: {
    uri: string;
    title?: string;
  };
}

export interface GroundingMetadata {
  web_search_queries?: string[];
  grounding_chunks?: GroundingChunk[];
  search_entry_point?: {
    rendered_content?: string;
  };
}

export interface GroundingSourcesProps {
  metadata: GroundingMetadata;
  className?: string;
}

export function GroundingSources({
  metadata,
  className,
}: GroundingSourcesProps) {
  const [isExpanded, setIsExpanded] = useState(true);

  const sources =
    metadata.grounding_chunks
      ?.map((chunk) => chunk.web)
      .filter((web) => web !== undefined) || [];

  const queries = metadata.web_search_queries || [];

  if (sources.length === 0 && queries.length === 0) {
    return null;
  }

  return (
    <div
      className={cn(
        "mt-3 rounded-lg border border-primary/20 bg-primary/5 overflow-hidden",
        className
      )}
    >
      {/* Header */}
      <button
        onClick={() => setIsExpanded(!isExpanded)}
        className="w-full flex items-center justify-between px-4 py-2.5 hover:bg-primary/10 transition-colors"
      >
        <div className="flex items-center gap-2">
          <Globe className="h-4 w-4 text-primary" />
          <span className="text-sm font-medium text-primary">
            Web Search Results
          </span>
          {sources.length > 0 && (
            <span className="text-xs text-muted-foreground">
              ({sources.length} {sources.length === 1 ? "source" : "sources"})
            </span>
          )}
        </div>
        <ChevronDown
          className={cn(
            "h-4 w-4 text-primary transition-transform",
            isExpanded && "rotate-180"
          )}
        />
      </button>

      {/* Content */}
      {isExpanded && (
        <div className="px-4 pb-3 space-y-3">
          {/* Search Queries */}
          {queries.length > 0 && (
            <div className="space-y-1.5">
              <p className="text-xs font-medium text-muted-foreground">
                Search queries:
              </p>
              <div className="space-y-1">
                {queries.map((query, idx) => (
                  <div
                    key={idx}
                    className="text-xs text-foreground/80 pl-2 border-l-2 border-primary/30"
                  >
                    "{query}"
                  </div>
                ))}
              </div>
            </div>
          )}

          {/* Sources */}
          {sources.length > 0 && (
            <div className="space-y-1.5">
              <p className="text-xs font-medium text-muted-foreground">
                Sources:
              </p>
              <div className="space-y-1.5">
                {sources.map((source, idx) => (
                  <a
                    key={idx}
                    href={source.uri}
                    target="_blank"
                    rel="noopener noreferrer"
                    className="flex items-start gap-2 p-2 rounded-md hover:bg-primary/10 transition-colors group"
                  >
                    <ExternalLink className="h-3.5 w-3.5 text-primary mt-0.5 flex-shrink-0" />
                    <div className="flex-1 min-w-0">
                      <p className="text-sm font-medium text-foreground group-hover:text-primary transition-colors truncate">
                        {source.title || "Untitled"}
                      </p>
                      <p className="text-xs text-muted-foreground truncate">
                        {source.uri}
                      </p>
                    </div>
                  </a>
                ))}
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
