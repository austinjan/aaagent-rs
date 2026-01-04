//! Skill loading from filesystem.

use super::model::{SkillError, SkillFrontmatter, SkillLoadOutcome, SkillMetadata, SkillScope};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

/// Filename for skill definition files
const SKILL_FILENAME: &str = "SKILL.md";

/// Directory name for skills
const SKILLS_DIR_NAME: &str = "skills";

/// Config directory name in repo root
const CONFIG_DIR_NAME: &str = ".aaagent";

/// Maximum length for skill name
const MAX_NAME_LEN: usize = 64;

/// Maximum length for skill description
const MAX_DESCRIPTION_LEN: usize = 1024;

/// A root directory to search for skills
#[derive(Debug, Clone)]
pub(crate) struct SkillRoot {
    pub path: PathBuf,
    pub scope: SkillScope,
}

/// Get the user skills root directory (~/.aaagent/skills/)
pub(crate) fn user_skills_root(home: &Path) -> SkillRoot {
    SkillRoot {
        path: home.join(SKILLS_DIR_NAME),
        scope: SkillScope::User,
    }
}

/// Get the system skills root directory (~/.aaagent/skills/.system/)
pub(crate) fn system_skills_root(home: &Path) -> SkillRoot {
    SkillRoot {
        path: home.join(SKILLS_DIR_NAME).join(".system"),
        scope: SkillScope::System,
    }
}

/// Get the project skills root directory (.aaagent/skills/ in repo)
///
/// Searches from cwd upward to find the git repo root, then looks for
/// .aaagent/skills/ directory.
pub(crate) fn project_skills_root(cwd: &Path) -> Option<SkillRoot> {
    let base = if cwd.is_dir() { cwd } else { cwd.parent()? };

    // Try to find git repo root
    let repo_root = find_git_root(base);

    // Search from cwd up to repo root (or filesystem root)
    let stop_at = repo_root.as_deref();

    for dir in base.ancestors() {
        let skills_root = dir.join(CONFIG_DIR_NAME).join(SKILLS_DIR_NAME);
        if skills_root.is_dir() {
            return Some(SkillRoot {
                path: skills_root,
                scope: SkillScope::Project,
            });
        }

        // Stop at repo root if found
        if let Some(root) = stop_at {
            if dir == root {
                break;
            }
        }
    }

    None
}

/// Find the git repository root from a starting directory
fn find_git_root(start: &Path) -> Option<PathBuf> {
    for dir in start.ancestors() {
        if dir.join(".git").exists() {
            return Some(dir.to_path_buf());
        }
    }
    None
}

/// Get all skill roots for a given cwd, in priority order
///
/// Order: Project > User > System
pub(crate) fn skill_roots_for_cwd(home: &Path, cwd: &Path) -> Vec<SkillRoot> {
    let mut roots = Vec::with_capacity(3);

    // Project skills have highest priority
    if let Some(project_root) = project_skills_root(cwd) {
        roots.push(project_root);
    }

    // User skills
    roots.push(user_skills_root(home));

    // System skills have lowest priority
    roots.push(system_skills_root(home));

    roots
}

/// Load all skills from multiple roots, deduplicating by name
pub(crate) fn load_skills_from_roots(roots: Vec<SkillRoot>) -> SkillLoadOutcome {
    let mut outcome = SkillLoadOutcome::new();
    let mut seen_names: HashSet<String> = HashSet::new();

    for root in roots {
        if !root.path.exists() || !root.path.is_dir() {
            continue;
        }

        let root_outcome = load_skills_from_root(&root);

        // Add skills, deduplicating by name (first one wins)
        for skill in root_outcome.skills {
            if seen_names.insert(skill.name.clone()) {
                outcome.skills.push(skill);
            }
        }

        // Collect all errors
        outcome.errors.extend(root_outcome.errors);
    }

    outcome
}

/// Load all skills from a single root directory
fn load_skills_from_root(root: &SkillRoot) -> SkillLoadOutcome {
    let mut outcome = SkillLoadOutcome::new();

    // List immediate subdirectories (each is a skill)
    let entries = match fs::read_dir(&root.path) {
        Ok(entries) => entries,
        Err(err) => {
            outcome.errors.push(SkillError {
                path: root.path.clone(),
                message: format!("Failed to read directory: {}", err),
            });
            return outcome;
        }
    };

    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();

        // Skip non-directories and hidden directories (except .system for user root)
        if !path.is_dir() {
            continue;
        }

        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n,
            None => continue,
        };

        // Skip hidden directories (like .system) at project/user level
        if name.starts_with('.') && root.scope != SkillScope::System {
            continue;
        }

        // Look for SKILL.md in the directory
        let skill_file = path.join(SKILL_FILENAME);
        if !skill_file.exists() {
            continue;
        }

        match parse_skill_file(&skill_file, root.scope) {
            Ok(skill) => outcome.skills.push(skill),
            Err(err) => outcome.errors.push(err),
        }
    }

    outcome
}

/// Parse a SKILL.md file into SkillMetadata
fn parse_skill_file(path: &Path, scope: SkillScope) -> Result<SkillMetadata, SkillError> {
    // Read file contents
    let contents = fs::read_to_string(path).map_err(|err| SkillError {
        path: path.to_path_buf(),
        message: format!("Failed to read file: {}", err),
    })?;

    // Extract YAML frontmatter
    let frontmatter = extract_frontmatter(&contents).ok_or_else(|| SkillError {
        path: path.to_path_buf(),
        message: "Missing or invalid YAML frontmatter (must start with ---)".to_string(),
    })?;

    // Parse YAML
    let parsed: SkillFrontmatter =
        serde_yaml::from_str(&frontmatter).map_err(|err| SkillError {
            path: path.to_path_buf(),
            message: format!("Invalid YAML frontmatter: {}", err),
        })?;

    // Validate and sanitize fields
    let name = sanitize_single_line(&parsed.name);
    let description = sanitize_single_line(&parsed.description);
    let short_description = parsed
        .metadata
        .short_description
        .as_deref()
        .map(sanitize_single_line)
        .filter(|s| !s.is_empty());

    // Validate field lengths
    validate_field(&name, MAX_NAME_LEN, "name", path)?;
    validate_field(&description, MAX_DESCRIPTION_LEN, "description", path)?;
    if let Some(ref short_desc) = short_description {
        validate_field(short_desc, MAX_DESCRIPTION_LEN, "short_description", path)?;
    }

    // Validate name format (alphanumeric, hyphens, underscores)
    if !is_valid_skill_name(&name) {
        return Err(SkillError {
            path: path.to_path_buf(),
            message:
                "Skill name must contain only alphanumeric characters, hyphens, and underscores"
                    .to_string(),
        });
    }

    Ok(SkillMetadata {
        name,
        description,
        short_description,
        path: path.to_path_buf(),
        scope,
    })
}

/// Extract YAML frontmatter from file contents
///
/// Frontmatter must be enclosed in `---` delimiters at the start of the file.
fn extract_frontmatter(contents: &str) -> Option<String> {
    let mut lines = contents.lines();

    // First line must be ---
    if !matches!(lines.next(), Some(line) if line.trim() == "---") {
        return None;
    }

    let mut frontmatter_lines: Vec<&str> = Vec::new();
    let mut found_closing = false;

    for line in lines {
        if line.trim() == "---" {
            found_closing = true;
            break;
        }
        frontmatter_lines.push(line);
    }

    if frontmatter_lines.is_empty() || !found_closing {
        return None;
    }

    Some(frontmatter_lines.join("\n"))
}

/// Sanitize a string to a single line (replace newlines with spaces)
fn sanitize_single_line(s: &str) -> String {
    s.lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Validate field length
fn validate_field(value: &str, max_len: usize, field: &str, path: &Path) -> Result<(), SkillError> {
    if value.is_empty() {
        return Err(SkillError {
            path: path.to_path_buf(),
            message: format!("Field '{}' cannot be empty", field),
        });
    }
    if value.len() > max_len {
        return Err(SkillError {
            path: path.to_path_buf(),
            message: format!(
                "Field '{}' exceeds maximum length of {} characters",
                field, max_len
            ),
        });
    }
    Ok(())
}

/// Check if a skill name is valid
fn is_valid_skill_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_skill_file(dir: &Path, name: &str, content: &str) -> PathBuf {
        let skill_dir = dir.join(name);
        fs::create_dir_all(&skill_dir).unwrap();
        let skill_file = skill_dir.join("SKILL.md");
        fs::write(&skill_file, content).unwrap();
        skill_file
    }

    #[test]
    fn test_extract_frontmatter() {
        let content = r#"---
name: test-skill
description: A test skill
---

# Body content
"#;
        let fm = extract_frontmatter(content).unwrap();
        assert!(fm.contains("name: test-skill"));
        assert!(fm.contains("description: A test skill"));
    }

    #[test]
    fn test_extract_frontmatter_missing() {
        let content = "# No frontmatter";
        assert!(extract_frontmatter(content).is_none());
    }

    #[test]
    fn test_sanitize_single_line() {
        let multiline = "line 1\n  line 2  \n\nline 3";
        assert_eq!(sanitize_single_line(multiline), "line 1 line 2 line 3");
    }

    #[test]
    fn test_is_valid_skill_name() {
        assert!(is_valid_skill_name("my-skill"));
        assert!(is_valid_skill_name("my_skill"));
        assert!(is_valid_skill_name("MySkill123"));
        assert!(!is_valid_skill_name("my skill"));
        assert!(!is_valid_skill_name("my.skill"));
        assert!(!is_valid_skill_name(""));
    }

    #[test]
    fn test_parse_skill_file() {
        let temp = TempDir::new().unwrap();
        let content = r#"---
name: test-skill
description: A test skill for testing
metadata:
  short-description: Short desc
---

# Test Skill

This is the body.
"#;
        let path = create_skill_file(temp.path(), "test-skill", content);

        let skill = parse_skill_file(&path, SkillScope::User).unwrap();
        assert_eq!(skill.name, "test-skill");
        assert_eq!(skill.description, "A test skill for testing");
        assert_eq!(skill.short_description, Some("Short desc".to_string()));
        assert_eq!(skill.scope, SkillScope::User);
    }

    #[test]
    fn test_load_skills_from_root() {
        let temp = TempDir::new().unwrap();

        // Create two valid skills
        create_skill_file(
            temp.path(),
            "skill-a",
            r#"---
name: skill-a
description: Skill A
---
Body A
"#,
        );

        create_skill_file(
            temp.path(),
            "skill-b",
            r#"---
name: skill-b
description: Skill B
---
Body B
"#,
        );

        let root = SkillRoot {
            path: temp.path().to_path_buf(),
            scope: SkillScope::User,
        };

        let outcome = load_skills_from_root(&root);
        assert_eq!(outcome.skills.len(), 2);
        assert!(outcome.errors.is_empty());
    }

    #[test]
    fn test_deduplication_by_name() {
        let temp1 = TempDir::new().unwrap();
        let temp2 = TempDir::new().unwrap();

        // Same skill name in both roots
        create_skill_file(
            temp1.path(),
            "my-skill",
            r#"---
name: my-skill
description: From root 1 (project)
---
"#,
        );

        create_skill_file(
            temp2.path(),
            "my-skill",
            r#"---
name: my-skill
description: From root 2 (user)
---
"#,
        );

        let roots = vec![
            SkillRoot {
                path: temp1.path().to_path_buf(),
                scope: SkillScope::Project,
            },
            SkillRoot {
                path: temp2.path().to_path_buf(),
                scope: SkillScope::User,
            },
        ];

        let outcome = load_skills_from_roots(roots);

        // Only one skill should be loaded (first one wins)
        assert_eq!(outcome.skills.len(), 1);
        assert_eq!(outcome.skills[0].description, "From root 1 (project)");
        assert_eq!(outcome.skills[0].scope, SkillScope::Project);
    }
}
