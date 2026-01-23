import { useState, useCallback } from "react";
import type { ContextStringStrategy, ContextStringResponse } from "@/types/backend";
import { getContextString, type GetContextStringParams } from "@/services/api";

export interface UseContextStringOptions {
  sessionId: string;
  leafId?: string;
  strategy?: ContextStringStrategy;
}

export interface UseContextStringReturn {
  content: string | null;
  isLoading: boolean;
  error: string | null;
  fetch: (options?: Partial<UseContextStringOptions>) => Promise<ContextStringResponse | null>;
  reset: () => void;
}

/**
 * Hook for fetching conversation context as a formatted string.
 *
 * Strategies:
 * - "full": All messages from leaf to root
 * - "default": Messages from leaf to nearest checkpoint or root
 * - "aggressive": Skip tool-related nodes, to checkpoint or root
 *
 * @example
 * ```tsx
 * const { content, isLoading, fetch } = useContextString({ sessionId });
 *
 * // Fetch with default strategy
 * await fetch();
 *
 * // Fetch with specific strategy
 * await fetch({ strategy: "aggressive" });
 *
 * // Display the summary
 * if (content) {
 *   console.log(content);
 * }
 * ```
 */
export function useContextString(options: UseContextStringOptions): UseContextStringReturn {
  const { sessionId, leafId, strategy } = options;

  const [content, setContent] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const fetch = useCallback(
    async (overrideOptions?: Partial<UseContextStringOptions>): Promise<ContextStringResponse | null> => {
      const effectiveLeafId = overrideOptions?.leafId ?? leafId;
      const effectiveStrategy = overrideOptions?.strategy ?? strategy;

      setIsLoading(true);
      setError(null);

      try {
        const params: GetContextStringParams = {};
        if (effectiveLeafId) {
          params.leafId = effectiveLeafId;
        }
        if (effectiveStrategy) {
          params.strategy = effectiveStrategy;
        }

        const response = await getContextString(sessionId, params);
        setContent(response.content);
        return response;
      } catch (err) {
        const message = err instanceof Error ? err.message : "Failed to fetch context string";
        setError(message);
        return null;
      } finally {
        setIsLoading(false);
      }
    },
    [sessionId, leafId, strategy]
  );

  const reset = useCallback(() => {
    setContent(null);
    setError(null);
    setIsLoading(false);
  }, []);

  return {
    content,
    isLoading,
    error,
    fetch,
    reset,
  };
}

export default useContextString;
