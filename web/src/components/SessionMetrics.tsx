// Component to display real-time session metrics (active subscribers, etc.)

import { useEffect, useState } from "react";
import { getSessionMetrics, type SessionMetrics } from "../services/api";

interface SessionMetricsProps {
  sessionId: string | null;
  className?: string;
}

export function SessionMetricsIndicator({
  sessionId,
  className = "",
}: SessionMetricsProps) {
  const [metrics, setMetrics] = useState<SessionMetrics | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!sessionId) {
      setMetrics(null);
      return;
    }

    // Fetch metrics immediately
    const fetchMetrics = async () => {
      try {
        const data = await getSessionMetrics(sessionId);
        setMetrics(data);
        setError(null);
      } catch (err) {
        console.warn("Failed to fetch session metrics:", err);
        setError(err instanceof Error ? err.message : "Unknown error");
      }
    };

    fetchMetrics();

    // Poll every 5 seconds for updates
    const interval = setInterval(fetchMetrics, 5000);

    return () => clearInterval(interval);
  }, [sessionId]);

  if (!sessionId || error) {
    return null;
  }

  if (!metrics) {
    return null;
  }

  const { active_subscribers } = metrics;

  // Only show if multiple clients are connected
  if (active_subscribers <= 1) {
    return null;
  }

  return (
    <div
      className={`flex items-center gap-2 px-3 py-1.5 rounded-lg bg-yellow-500/10 border border-yellow-500/20 ${className}`}
      title={`${active_subscribers} clients viewing this session`}
    >
      <span className="text-yellow-500 text-lg">👥</span>
      <span className="text-sm font-medium text-yellow-600 dark:text-yellow-400">
        {active_subscribers} clients connected
      </span>
    </div>
  );
}
