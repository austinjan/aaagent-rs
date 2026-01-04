//! Skills manager with caching.

use super::loader::{load_skills_from_roots, skill_roots_for_cwd};
use super::model::{SkillLoadOutcome, SkillMetadata};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

/// Manager for loading and caching skills.
///
/// Skills are cached per working directory to avoid repeated filesystem access.
/// The cache can be invalidated by calling `skills_for_cwd_with_options` with
/// `force_reload = true`.
pub struct SkillsManager {
    /// Home directory for user/system skills (~/.aaagent/)
    home: PathBuf,

    /// Cache of loaded skills by working directory
    cache_by_cwd: RwLock<HashMap<PathBuf, SkillLoadOutcome>>,
}

impl SkillsManager {
    /// Create a new SkillsManager with the given home directory.
    ///
    /// # Arguments
    /// * `home` - The home directory for user skills (typically ~/.aaagent/)
    pub fn new(home: PathBuf) -> Self {
        Self {
            home,
            cache_by_cwd: RwLock::new(HashMap::new()),
        }
    }

    /// Create a new SkillsManager using the default home directory.
    ///
    /// Uses `~/.aaagent/` as the home directory.
    pub fn with_default_home() -> Option<Self> {
        let home = dirs::home_dir()?.join(".aaagent");
        Some(Self::new(home))
    }

    /// Get the home directory for this manager.
    pub fn home(&self) -> &Path {
        &self.home
    }

    /// Load skills for the given working directory.
    ///
    /// Uses cached results if available.
    pub fn skills_for_cwd(&self, cwd: &Path) -> SkillLoadOutcome {
        self.skills_for_cwd_with_options(cwd, false)
    }

    /// Load skills for the given working directory with options.
    ///
    /// # Arguments
    /// * `cwd` - The working directory to load skills for
    /// * `force_reload` - If true, bypass the cache and reload from disk
    pub fn skills_for_cwd_with_options(&self, cwd: &Path, force_reload: bool) -> SkillLoadOutcome {
        // Try to get from cache first
        if !force_reload {
            let cache = self.cache_by_cwd.read().unwrap_or_else(|e| e.into_inner());
            if let Some(outcome) = cache.get(cwd) {
                return outcome.clone();
            }
        }

        // Load skills from all roots
        let roots = skill_roots_for_cwd(&self.home, cwd);
        let outcome = load_skills_from_roots(roots);

        // Cache the result
        let mut cache = self.cache_by_cwd.write().unwrap_or_else(|e| e.into_inner());
        cache.insert(cwd.to_path_buf(), outcome.clone());

        outcome
    }

    /// Clear the cache for a specific working directory.
    pub fn invalidate_cache(&self, cwd: &Path) {
        let mut cache = self.cache_by_cwd.write().unwrap_or_else(|e| e.into_inner());
        cache.remove(cwd);
    }

    /// Clear all cached skills.
    pub fn clear_cache(&self) {
        let mut cache = self.cache_by_cwd.write().unwrap_or_else(|e| e.into_inner());
        cache.clear();
    }

    /// Get a list of all cached working directories.
    pub fn cached_cwds(&self) -> Vec<PathBuf> {
        let cache = self.cache_by_cwd.read().unwrap_or_else(|e| e.into_inner());
        cache.keys().cloned().collect()
    }

    /// Find a skill by name across all loaded skills for a cwd.
    pub fn find_skill(&self, cwd: &Path, name: &str) -> Option<SkillMetadata> {
        let outcome = self.skills_for_cwd(cwd);
        outcome.find_by_name(name).cloned()
    }

    /// List all skill names for a cwd.
    pub fn list_skill_names(&self, cwd: &Path) -> Vec<String> {
        let outcome = self.skills_for_cwd(cwd);
        outcome.skills.iter().map(|s| s.name.clone()).collect()
    }
}

impl std::fmt::Debug for SkillsManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SkillsManager")
            .field("home", &self.home)
            .field("cached_cwds", &self.cached_cwds())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_skill(dir: &Path, name: &str, description: &str) {
        let skill_dir = dir.join(name);
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            format!(
                r#"---
name: {}
description: {}
---
Body content
"#,
                name, description
            ),
        )
        .unwrap();
    }

    #[test]
    fn test_skills_manager_caching() {
        let temp_home = TempDir::new().unwrap();
        let temp_cwd = TempDir::new().unwrap();

        // Create user skills directory
        let user_skills = temp_home.path().join("skills");
        fs::create_dir_all(&user_skills).unwrap();
        create_skill(&user_skills, "user-skill", "A user skill");

        let manager = SkillsManager::new(temp_home.path().to_path_buf());

        // First load
        let outcome1 = manager.skills_for_cwd(temp_cwd.path());
        assert_eq!(outcome1.skills.len(), 1);

        // Second load should use cache
        let outcome2 = manager.skills_for_cwd(temp_cwd.path());
        assert_eq!(outcome2.skills.len(), 1);

        // Force reload should bypass cache
        let outcome3 = manager.skills_for_cwd_with_options(temp_cwd.path(), true);
        assert_eq!(outcome3.skills.len(), 1);
    }

    #[test]
    fn test_skills_manager_invalidate() {
        let temp_home = TempDir::new().unwrap();
        let temp_cwd = TempDir::new().unwrap();

        let manager = SkillsManager::new(temp_home.path().to_path_buf());

        // Load skills (will be empty but cached)
        let _ = manager.skills_for_cwd(temp_cwd.path());
        assert_eq!(manager.cached_cwds().len(), 1);

        // Invalidate
        manager.invalidate_cache(temp_cwd.path());
        assert_eq!(manager.cached_cwds().len(), 0);
    }

    #[test]
    fn test_find_skill() {
        let temp_home = TempDir::new().unwrap();
        let temp_cwd = TempDir::new().unwrap();

        let user_skills = temp_home.path().join("skills");
        fs::create_dir_all(&user_skills).unwrap();
        create_skill(&user_skills, "my-skill", "My skill description");

        let manager = SkillsManager::new(temp_home.path().to_path_buf());

        let skill = manager.find_skill(temp_cwd.path(), "my-skill");
        assert!(skill.is_some());
        assert_eq!(skill.unwrap().name, "my-skill");

        let not_found = manager.find_skill(temp_cwd.path(), "nonexistent");
        assert!(not_found.is_none());
    }

    #[test]
    fn test_list_skill_names() {
        let temp_home = TempDir::new().unwrap();
        let temp_cwd = TempDir::new().unwrap();

        let user_skills = temp_home.path().join("skills");
        fs::create_dir_all(&user_skills).unwrap();
        create_skill(&user_skills, "skill-a", "Skill A");
        create_skill(&user_skills, "skill-b", "Skill B");

        let manager = SkillsManager::new(temp_home.path().to_path_buf());

        let names = manager.list_skill_names(temp_cwd.path());
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"skill-a".to_string()));
        assert!(names.contains(&"skill-b".to_string()));
    }
}
