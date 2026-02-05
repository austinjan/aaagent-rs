import { create } from "zustand";

/**
 * Represents a single sub-agent run with its lifecycle information
 */
export interface SubAgentInfo {
  runId: string;
  taskLabel: string;
  startTime: number; // Unix timestamp
  endTime?: number; // Unix timestamp
  phase: "spawning" | "running" | "completed" | "error";
  toolCalls: Array<{
    toolName: string;
    timestamp: number;
    status: "running" | "success" | "error";
    result?: string;
  }>;
  errors: string[];
  progress?: {
    current: number;
    total: number;
  };
}

interface SubAgentStore {
  // Active sub-agents (currently running)
  activeRuns: Map<string, SubAgentInfo>;

  // Recently completed sub-agents (for history)
  completedRuns: Map<string, SubAgentInfo>;

  // Panel state
  isPanelOpen: boolean;
  selectedRunId: string | null;

  // Actions
  addRun: (runId: string, taskLabel: string) => void;
  updateRun: (runId: string, updates: Partial<SubAgentInfo>) => void;
  completeRun: (runId: string, success: boolean, error?: string) => void;
  addToolCall: (
    runId: string,
    toolName: string,
    status: "running" | "success" | "error",
    result?: string,
  ) => void;
  pruneOldRuns: () => void;

  // Panel actions
  togglePanel: () => void;
  openPanel: () => void;
  closePanel: () => void;
  selectRun: (runId: string | null) => void;
}

const MAX_COMPLETED_RUNS = 20;
const PRUNE_AGE_MS = 5 * 60 * 1000; // 5 minutes

export const useSubAgentStore = create<SubAgentStore>((set) => ({
  activeRuns: new Map(),
  completedRuns: new Map(),
  isPanelOpen: false,
  selectedRunId: null,

  addRun: (runId: string, taskLabel: string) => {
    set((state) => {
      const newActiveRuns = new Map(state.activeRuns);
      newActiveRuns.set(runId, {
        runId,
        taskLabel,
        startTime: Date.now(),
        phase: "spawning",
        toolCalls: [],
        errors: [],
      });
      return { activeRuns: newActiveRuns };
    });
  },

  updateRun: (runId: string, updates: Partial<SubAgentInfo>) => {
    set((state) => {
      const newActiveRuns = new Map(state.activeRuns);
      const existing = newActiveRuns.get(runId);
      if (existing) {
        newActiveRuns.set(runId, { ...existing, ...updates });
      }
      return { activeRuns: newActiveRuns };
    });
  },

  completeRun: (runId: string, success: boolean, error?: string) => {
    set((state) => {
      const newActiveRuns = new Map(state.activeRuns);
      const run = newActiveRuns.get(runId);

      if (run) {
        const completedRun: SubAgentInfo = {
          ...run,
          endTime: Date.now(),
          phase: success ? "completed" : "error",
          errors: error ? [...run.errors, error] : run.errors,
        };

        newActiveRuns.delete(runId);

        const newCompletedRuns = new Map(state.completedRuns);
        newCompletedRuns.set(runId, completedRun);

        // Keep only MAX_COMPLETED_RUNS most recent
        if (newCompletedRuns.size > MAX_COMPLETED_RUNS) {
          const sorted = Array.from(newCompletedRuns.values()).sort(
            (a, b) => (b.endTime || 0) - (a.endTime || 0),
          );
          const toKeep = sorted.slice(0, MAX_COMPLETED_RUNS);
          newCompletedRuns.clear();
          toKeep.forEach((run) => newCompletedRuns.set(run.runId, run));
        }

        return {
          activeRuns: newActiveRuns,
          completedRuns: newCompletedRuns,
        };
      }

      return state;
    });
  },

  addToolCall: (
    runId: string,
    toolName: string,
    status: "running" | "success" | "error",
    result?: string,
  ) => {
    set((state) => {
      const newActiveRuns = new Map(state.activeRuns);
      const run = newActiveRuns.get(runId);

      if (run) {
        const updatedRun: SubAgentInfo = {
          ...run,
          toolCalls: [
            ...run.toolCalls,
            {
              toolName,
              timestamp: Date.now(),
              status,
              result,
            },
          ],
        };
        newActiveRuns.set(runId, updatedRun);
        return { activeRuns: newActiveRuns };
      }

      return state;
    });
  },

  pruneOldRuns: () => {
    set((state) => {
      const now = Date.now();
      const newCompletedRuns = new Map(state.completedRuns);

      // Remove runs older than PRUNE_AGE_MS
      for (const [runId, run] of newCompletedRuns.entries()) {
        if (run.endTime && now - run.endTime > PRUNE_AGE_MS) {
          newCompletedRuns.delete(runId);
        }
      }

      return { completedRuns: newCompletedRuns };
    });
  },

  togglePanel: () => {
    set((state) => ({ isPanelOpen: !state.isPanelOpen }));
  },

  openPanel: () => {
    set({ isPanelOpen: true });
  },

  closePanel: () => {
    set({ isPanelOpen: false });
  },

  selectRun: (runId: string | null) => {
    set({ selectedRunId: runId });
  },
}));
