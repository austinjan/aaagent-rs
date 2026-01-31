//! Skill discovery and loading

use crate::skills::error::{
    file_read_error, invalid_name_error, missing_frontmatter_error, yaml_parse_error,
};
use crate::skills::model::{
    SkillFrontmatter, SkillInvocation, SkillLoadOutcome, SkillMetadata, SkillOpenMetadata,
    SkillScope,
};
use anyhow::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Repo skill directory paths to check
const REPO_SKILL_PATHS: &[&str] = &[".skills", ".agent/skills", "skills"];

/// Get skill roots for a working directory
pub fn skill_roots_for_cwd(app_home: &Path, cwd: &Path) -> Vec<(PathBuf, SkillScope)> {
    let mut roots = Vec::new();

    // 1. Repo scope: check cwd and parents for project root
    if let Some(repo_root) = find_project_root(cwd) {
        for sub_path in REPO_SKILL_PATHS {
            let repo_skills = repo_root.join(sub_path);
            if repo_skills.exists() && repo_skills.is_dir() {
                roots.push((repo_skills, SkillScope::Repo));
            }
        }
    }

    // 2. User scope: ~/.aaagent/skills/
    let user_skills = app_home.join("skills");
    if user_skills.exists() {
        roots.push((user_skills, SkillScope::User));
    }

    // 3. System scope: ~/.aaagent/skills/.system/
    let system_skills = app_home.join("skills").join(".system");
    if system_skills.exists() {
        roots.push((system_skills, SkillScope::System));
    }

    // 4. Admin scope: /etc/aaagent/skills/ (Unix only)
    #[cfg(unix)]
    {
        let admin_skills = PathBuf::from("/etc/aaagent/skills");
        if admin_skills.exists() {
            roots.push((admin_skills, SkillScope::Admin));
        }
    }

    roots
}

/// Load skills from multiple roots with deduplication
pub fn load_skills_from_roots(roots: &[(PathBuf, SkillScope)]) -> SkillLoadOutcome {
    let mut outcome = SkillLoadOutcome::new();
    let mut skills_by_name: HashMap<String, SkillMetadata> = HashMap::new();

    for (root, scope) in roots {
        log::debug!("Scanning skills in: {}", root.display());

        // Walk directory looking for SKILL.md files
        for entry in jwalk::WalkDir::new(root)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();

            if path.file_name() != Some(std::ffi::OsStr::new("SKILL.md")) {
                continue;
            }

            match parse_skill(&path, *scope) {
                Ok(skill) => {
                    log::info!("Loaded skill: {} from {}", skill.name, path.display());

                    // Deduplicate by priority (higher scope wins)
                    if let Some(existing) = skills_by_name.get(&skill.name) {
                        if skill.scope > existing.scope {
                            skills_by_name.insert(skill.name.clone(), skill);
                        }
                    } else {
                        skills_by_name.insert(skill.name.clone(), skill);
                    }
                }
                Err(e) => {
                    outcome.add_error(e);
                }
            }
        }
    }

    // Add deduplicated skills to outcome
    for skill in skills_by_name.into_values() {
        outcome.add_skill(skill);
    }

    log::info!(
        "Skill loading complete: {} skills, {} errors",
        outcome.skills.len(),
        outcome.errors.len()
    );

    outcome
}

/// Parse a single skill file
fn parse_skill(path: &Path, scope: SkillScope) -> Result<SkillMetadata> {
    let content = std::fs::read_to_string(path).map_err(|e| file_read_error(path, e))?;

    let frontmatter = extract_frontmatter(&content, path)?;

    validate_skill_name(&frontmatter.name, path)?;

    // Parse metadata if present (handles both flat and openclaw wrapper formats)
    let metadata = if let Some(ref meta_value) = frontmatter.metadata {
        // Check if it's wrapped in "openclaw" key
        let inner = if let Some(openclaw) = meta_value.get("openclaw") {
            openclaw.clone()
        } else {
            meta_value.clone()
        };

        Some(
            serde_json::from_value::<SkillOpenMetadata>(inner)
                .map_err(|e| anyhow::anyhow!("Invalid metadata in '{}': {}", path.display(), e))?,
        )
    } else {
        None
    };

    Ok(SkillMetadata {
        name: frontmatter.name,
        description: frontmatter.description,
        path: path.to_path_buf(),
        scope,
        invocation: SkillInvocation {
            user_invocable: frontmatter.user_invocable,
            disable_model_invocation: frontmatter.disable_model_invocation,
        },
        metadata,
    })
}

/// Extract YAML frontmatter from content
fn extract_frontmatter(content: &str, path: &Path) -> Result<SkillFrontmatter> {
    if !content.starts_with("---\n") && !content.starts_with("---\r\n") {
        return Err(missing_frontmatter_error(path));
    }

    // Find the closing ---
    let rest = &content[4..];
    let end = rest
        .find("\n---")
        .ok_or_else(|| missing_frontmatter_error(path))?;

    let yaml_str = &rest[..end];

    serde_yaml::from_str(yaml_str).map_err(|e| yaml_parse_error(path, e))
}

/// Validate skill name
fn validate_skill_name(name: &str, path: &Path) -> Result<()> {
    if name.is_empty() {
        return Err(invalid_name_error(name, path, "name cannot be empty"));
    }

    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(invalid_name_error(
            name,
            path,
            "name can only contain a-z, 0-9, -, _",
        ));
    }

    Ok(())
}

/// Find project root by looking for markers
fn find_project_root(start: &Path) -> Option<PathBuf> {
    let mut current = start;
    loop {
        if current.join(".git").exists()
            || current.join(".skills").exists()
            || current.join(".agent").exists()
        {
            return Some(current.to_path_buf());
        }
        current = current.parent()?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_validate_skill_name() {
        let path = Path::new("test.md");

        assert!(validate_skill_name("my-skill", path).is_ok());
        assert!(validate_skill_name("skill_123", path).is_ok());
        assert!(validate_skill_name("", path).is_err());
        assert!(validate_skill_name("my skill", path).is_err());
        assert!(validate_skill_name("my@skill", path).is_err());
    }

    #[test]
    fn test_extract_frontmatter() {
        let content = r#"---
name: test-skill
description: A test skill
---

# Body content
"#;
        let path = Path::new("test.md");
        let fm = extract_frontmatter(content, path).unwrap();

        assert_eq!(fm.name, "test-skill");
        assert_eq!(fm.description, "A test skill");
    }

    #[test]
    fn test_load_skills_from_temp_dir() {
        let temp = TempDir::new().unwrap();
        let skill_dir = temp.path().join("my-skill");
        std::fs::create_dir_all(&skill_dir).unwrap();

        let skill_content = r#"---
name: my-skill
description: Test skill
---

# My Skill
"#;
        std::fs::write(skill_dir.join("SKILL.md"), skill_content).unwrap();

        let roots = vec![(temp.path().to_path_buf(), SkillScope::User)];
        let outcome = load_skills_from_roots(&roots);

        assert_eq!(outcome.skills.len(), 1);
        assert_eq!(outcome.skills[0].name, "my-skill");
        assert!(outcome.errors.is_empty());
    }
}
