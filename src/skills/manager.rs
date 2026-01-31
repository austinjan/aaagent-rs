//! Skill Manager - Main entry point for skill system

use crate::skills::eligibility::filter_eligible;
use crate::skills::loader::{load_skills_from_roots, skill_roots_for_cwd};
use crate::skills::model::{SkillMetadata, SkillSnapshot, SkillSnapshotEntry};
use std::path::{Path, PathBuf};

/// Skill Manager handles discovery, loading, and eligibility filtering
#[derive(Debug)]
pub struct SkillsManager {
    /// App home directory (~/.aaagent)
    app_home: PathBuf,
    /// Current working directory
    cwd: PathBuf,
    /// Eligible skills after filtering
    skills: Vec<SkillMetadata>,
    /// Errors encountered during loading
    errors: Vec<String>,
}

impl SkillsManager {
    /// Create a new SkillsManager and load skills
    pub fn new(app_home: &Path, cwd: &Path) -> Self {
        let roots = skill_roots_for_cwd(app_home, cwd);
        let outcome = load_skills_from_roots(&roots);
        let (skills, eligibility_errors) = filter_eligible(outcome.skills);

        let mut errors = outcome.errors;
        errors.extend(eligibility_errors);

        Self {
            app_home: app_home.to_path_buf(),
            cwd: cwd.to_path_buf(),
            skills,
            errors,
        }
    }

    /// Get all eligible skills
    pub fn skills(&self) -> &[SkillMetadata] {
        &self.skills
    }

    /// Get errors from loading/eligibility
    pub fn errors(&self) -> &[String] {
        &self.errors
    }

    /// Get user-invocable skills (for slash commands)
    pub fn user_invocable(&self) -> Vec<&SkillMetadata> {
        self.skills
            .iter()
            .filter(|s| s.invocation.user_invocable)
            .collect()
    }

    /// Get model-invocable skills (for system prompt)
    pub fn model_invocable(&self) -> Vec<&SkillMetadata> {
        self.skills
            .iter()
            .filter(|s| !s.invocation.disable_model_invocation)
            .collect()
    }

    /// Find a skill by name
    pub fn find(&self, name: &str) -> Option<&SkillMetadata> {
        self.skills.iter().find(|s| s.name == name)
    }

    /// Create a snapshot for agent runtime
    pub fn snapshot(&self) -> SkillSnapshot {
        let model_skills = self.model_invocable();
        let prompt = render_skills_xml(&model_skills);
        let entries = model_skills
            .into_iter()
            .map(|s| SkillSnapshotEntry {
                name: s.name.clone(),
                path: s.path.clone(),
                primary_env: s.metadata.as_ref().and_then(|m| m.primary_env.clone()),
            })
            .collect();

        SkillSnapshot {
            prompt,
            skills: entries,
        }
    }

    /// Reload skills (useful after config changes)
    pub fn reload(&mut self) {
        let roots = skill_roots_for_cwd(&self.app_home, &self.cwd);
        let outcome = load_skills_from_roots(&roots);
        let (skills, eligibility_errors) = filter_eligible(outcome.skills);

        self.skills = skills;
        self.errors = outcome.errors;
        self.errors.extend(eligibility_errors);
    }
}

/// Render skills as XML for system prompt
fn render_skills_xml(skills: &[&SkillMetadata]) -> String {
    if skills.is_empty() {
        return String::new();
    }

    let mut xml = String::from("<available-skills>\n");
    xml.push_str("Use the Read tool to get full skill instructions when needed.\n\n");

    for skill in skills {
        xml.push_str(&format!(
            "  <skill name=\"{}\" path=\"{}\">{}</skill>\n",
            skill.name,
            skill.path.display(),
            skill.description
        ));
    }

    xml.push_str("</available-skills>");
    xml
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skills::model::{SkillInvocation, SkillScope};
    use tempfile::TempDir;

    fn create_skill_file(dir: &Path, name: &str, user_invocable: bool, disable_model: bool) {
        let skill_dir = dir.join(name);
        std::fs::create_dir_all(&skill_dir).unwrap();

        let content = format!(
            r#"---
name: {}
description: Test skill {}
user-invocable: {}
disable-model-invocation: {}
---

# {}
"#,
            name, name, user_invocable, disable_model, name
        );
        std::fs::write(skill_dir.join("SKILL.md"), content).unwrap();
    }

    #[test]
    fn test_manager_loads_skills() {
        let temp = TempDir::new().unwrap();
        let skills_dir = temp.path().join("skills");
        std::fs::create_dir_all(&skills_dir).unwrap();

        create_skill_file(&skills_dir, "test-skill", true, false);

        let manager = SkillsManager::new(temp.path(), temp.path());

        assert_eq!(manager.skills().len(), 1);
        assert_eq!(manager.skills()[0].name, "test-skill");
    }

    #[test]
    fn test_user_invocable_filter() {
        let temp = TempDir::new().unwrap();
        let skills_dir = temp.path().join("skills");
        std::fs::create_dir_all(&skills_dir).unwrap();

        create_skill_file(&skills_dir, "user-skill", true, false);
        create_skill_file(&skills_dir, "hidden-skill", false, false);

        let manager = SkillsManager::new(temp.path(), temp.path());

        let user_skills = manager.user_invocable();
        assert_eq!(user_skills.len(), 1);
        assert_eq!(user_skills[0].name, "user-skill");
    }

    #[test]
    fn test_model_invocable_filter() {
        let temp = TempDir::new().unwrap();
        let skills_dir = temp.path().join("skills");
        std::fs::create_dir_all(&skills_dir).unwrap();

        create_skill_file(&skills_dir, "model-skill", true, false);
        create_skill_file(&skills_dir, "hidden-from-model", true, true);

        let manager = SkillsManager::new(temp.path(), temp.path());

        let model_skills = manager.model_invocable();
        assert_eq!(model_skills.len(), 1);
        assert_eq!(model_skills[0].name, "model-skill");
    }

    #[test]
    fn test_snapshot_xml() {
        let temp = TempDir::new().unwrap();
        let skills_dir = temp.path().join("skills");
        std::fs::create_dir_all(&skills_dir).unwrap();

        create_skill_file(&skills_dir, "my-skill", true, false);

        let manager = SkillsManager::new(temp.path(), temp.path());
        let snapshot = manager.snapshot();

        assert!(snapshot.prompt.contains("<available-skills>"));
        assert!(snapshot.prompt.contains("my-skill"));
        assert_eq!(snapshot.skills.len(), 1);
    }

    #[test]
    fn test_render_skills_xml() {
        let skill = SkillMetadata {
            name: "test".to_string(),
            description: "A test skill".to_string(),
            path: PathBuf::from("/path/to/test.md"),
            scope: SkillScope::User,
            invocation: SkillInvocation::default(),
            metadata: None,
        };

        let xml = render_skills_xml(&[&skill]);
        assert!(xml.contains("name=\"test\""));
        assert!(xml.contains("path=\"/path/to/test.md\""));
        assert!(xml.contains("A test skill"));
        assert!(xml.contains("Use the Read tool"));
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_load_real_project_skills() {
        // Use actual project directory
        let cwd = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let app_home = PathBuf::from("/tmp/test-aaagent");
        
        let manager = SkillsManager::new(&app_home, &cwd);
        
        println!("\n=== Loaded {} skills ===", manager.skills().len());
        for skill in manager.skills() {
            println!("  ✓ {}", skill.name);
        }
        
        println!("\n=== {} errors ===", manager.errors().len());
        for err in manager.errors().iter().take(3) {
            println!("  ✗ {}", err);
        }
        
        // Should load at least managing-feature-plans from .codex/skills
        assert!(manager.skills().len() >= 1, "Should load at least 1 skill");
    }
}
