// Hook for fetching and caching available skills

import { useState, useEffect, useCallback } from "react";
import { getSkills } from "../services/api";
import type { Skill, SkillError } from "../types/backend";

export interface UseSkillsReturn {
  skills: Skill[];
  errors: SkillError[];
  loading: boolean;
  error: Error | null;
  refresh: () => Promise<void>;
}

export function useSkills(): UseSkillsReturn {
  const [skills, setSkills] = useState<Skill[]>([]);
  const [errors, setErrors] = useState<SkillError[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<Error | null>(null);

  const fetchSkills = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const data = await getSkills();
      // Filter to only user-invocable skills
      const userSkills = data.loaded.skills.filter(
        (skill) => skill.user_invocable,
      );
      setSkills(userSkills);
      setErrors(data.errors.details);
    } catch (err) {
      setError(
        err instanceof Error ? err : new Error("Failed to fetch skills"),
      );
      setSkills([]);
      setErrors([]);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchSkills();
  }, [fetchSkills]);

  return {
    skills,
    errors,
    loading,
    error,
    refresh: fetchSkills,
  };
}
