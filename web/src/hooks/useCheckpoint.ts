import { useState, useCallback } from "react";
import type {
  CreateCheckpointRequest,
  CreateCheckpointResponse,
  PreviewCheckpointResponse,
} from "@/types/backend";
import { createCheckpoint, previewCheckpoint } from "@/services/api";

export interface UseCheckpointOptions {
  sessionId: string;
  onCheckpointCreated?: (response: CreateCheckpointResponse) => void;
}

export interface UseCheckpointReturn {
  // State
  isCreating: boolean;
  isPreviewing: boolean;
  error: string | null;
  lastCreated: CreateCheckpointResponse | null;
  lastPreview: PreviewCheckpointResponse | null;

  // Actions
  create: (request: CreateCheckpointRequest) => Promise<CreateCheckpointResponse | null>;
  preview: (request: CreateCheckpointRequest) => Promise<PreviewCheckpointResponse | null>;
  reset: () => void;
}

/**
 * Hook for managing checkpoint operations (create and preview).
 *
 * @example
 * ```tsx
 * const { create, preview, isCreating, lastPreview } = useCheckpoint({
 *   sessionId,
 *   onCheckpointCreated: (response) => {
 *     console.log("Created checkpoint:", response.checkpoint_id);
 *   },
 * });
 *
 * // Preview a checkpoint
 * const previewResult = await preview({
 *   target_node_id: nodeId,
 *   strategy: "balanced",
 * });
 *
 * // Create a checkpoint
 * const createResult = await create({
 *   target_node_id: nodeId,
 *   strategy: "balanced",
 * });
 * ```
 */
export function useCheckpoint(options: UseCheckpointOptions): UseCheckpointReturn {
  const { sessionId, onCheckpointCreated } = options;

  const [isCreating, setIsCreating] = useState(false);
  const [isPreviewing, setIsPreviewing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [lastCreated, setLastCreated] = useState<CreateCheckpointResponse | null>(null);
  const [lastPreview, setLastPreview] = useState<PreviewCheckpointResponse | null>(null);

  const create = useCallback(
    async (request: CreateCheckpointRequest): Promise<CreateCheckpointResponse | null> => {
      setIsCreating(true);
      setError(null);

      try {
        const response = await createCheckpoint(sessionId, request);
        setLastCreated(response);
        onCheckpointCreated?.(response);
        return response;
      } catch (err) {
        const message = err instanceof Error ? err.message : "Failed to create checkpoint";
        setError(message);
        return null;
      } finally {
        setIsCreating(false);
      }
    },
    [sessionId, onCheckpointCreated]
  );

  const preview = useCallback(
    async (request: CreateCheckpointRequest): Promise<PreviewCheckpointResponse | null> => {
      setIsPreviewing(true);
      setError(null);

      try {
        const response = await previewCheckpoint(sessionId, request);
        setLastPreview(response);
        return response;
      } catch (err) {
        const message = err instanceof Error ? err.message : "Failed to preview checkpoint";
        setError(message);
        return null;
      } finally {
        setIsPreviewing(false);
      }
    },
    [sessionId]
  );

  const reset = useCallback(() => {
    setIsCreating(false);
    setIsPreviewing(false);
    setError(null);
    setLastCreated(null);
    setLastPreview(null);
  }, []);

  return {
    isCreating,
    isPreviewing,
    error,
    lastCreated,
    lastPreview,
    create,
    preview,
    reset,
  };
}

export default useCheckpoint;
