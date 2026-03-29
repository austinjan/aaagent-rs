import { useEffect, useState } from "react";
import { getAvailableModels, type ModelInfo } from "@/lib/api";

export function useModels() {
  const [models, setModels] = useState<ModelInfo[]>([]);
  const [loaded, setLoaded] = useState(false);

  useEffect(() => {
    getAvailableModels()
      .then((m) => {
        setModels(m);
        setLoaded(true);
      })
      .catch(() => setLoaded(true));
  }, []);

  const availableModels = models.filter((m) => m.available);

  return { models, availableModels, loaded };
}
