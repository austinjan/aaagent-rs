import { useEffect, useState } from "react";
import { getProvidersStatus, type ProvidersStatus } from "@/lib/api";

const DEFAULT_STATUS: ProvidersStatus = {
  openai: false,
  anthropic: false,
  google: false,
};

export function useProvidersStatus() {
  const [status, setStatus] = useState<ProvidersStatus>(DEFAULT_STATUS);
  const [loaded, setLoaded] = useState(false);

  useEffect(() => {
    getProvidersStatus()
      .then((s) => {
        setStatus(s);
        setLoaded(true);
      })
      .catch(() => setLoaded(true));
  }, []);

  return { status, loaded };
}
