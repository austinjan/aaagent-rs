//! Skill injection into conversations.

use super::model::{SkillInjection, SkillLoadOutcome, SkillMetadata};
use std::collections::HashSet;
use std::fs;

/// Result of building skill injections.
#[derive(Debug, Default)]
pub struct SkillInjections {
    /// Successfully loaded skill contents
    pub items: Vec<SkillInjection>,

    /// Warning messages for skills that failed to load
    pub warnings: Vec<String>,
}

impl SkillInjections {
    /// Check if there are any injections
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Get all skill contents formatted as XML for model context
    pub fn to_xml_messages(&self) -> Vec<String> {
        self.items.iter().map(|item| item.to_xml()).collect()
    }

    /// Get a combined XML string with all skills
    pub fn to_combined_xml(&self) -> String {
        self.items
            .iter()
            .map(|item| item.to_xml())
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}

/// A skill reference in user input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillReference {
    /// Name of the skill
    pub name: String,
    /// Optional path override (for explicit skill file references)
    pub path: Option<String>,
}

impl SkillReference {
    /// Create a new skill reference by name
    pub fn by_name(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            path: None,
        }
    }

    /// Create a new skill reference with explicit path
    pub fn with_path(name: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            path: Some(path.into()),
        }
    }
}

/// Build skill injections from skill references.
///
/// # Arguments
/// * `references` - Skill references from user input
/// * `outcome` - Loaded skills for the current working directory
///
/// # Returns
/// `SkillInjections` containing successfully loaded skill contents and any warnings
pub fn build_skill_injections(
    references: &[SkillReference],
    outcome: Option<&SkillLoadOutcome>,
) -> SkillInjections {
    let mut result = SkillInjections::default();

    if references.is_empty() {
        return result;
    }

    let Some(outcome) = outcome else {
        // No skills loaded, generate warnings for all references
        for reference in references {
            result.warnings.push(format!(
                "Skill '{}' not found: no skills loaded for current directory",
                reference.name
            ));
        }
        return result;
    };

    let mut seen: HashSet<String> = HashSet::new();

    for reference in references {
        // Skip duplicates
        if !seen.insert(reference.name.clone()) {
            continue;
        }

        // Find the skill
        let skill = if let Some(ref path) = reference.path {
            // Match by both name and path
            outcome
                .skills
                .iter()
                .find(|s| s.name == reference.name && s.path.to_string_lossy() == path.as_str())
        } else {
            // Match by name only
            outcome.find_by_name(&reference.name)
        };

        match skill {
            Some(skill) => {
                // Load skill content
                match load_skill_content(skill) {
                    Ok(injection) => result.items.push(injection),
                    Err(err) => result.warnings.push(err),
                }
            }
            None => {
                result.warnings.push(format!(
                    "Skill '{}' not found in available skills",
                    reference.name
                ));
            }
        }
    }

    result
}

/// Load the full content of a skill file.
fn load_skill_content(skill: &SkillMetadata) -> Result<SkillInjection, String> {
    let contents = fs::read_to_string(&skill.path).map_err(|err| {
        format!(
            "Failed to load skill '{}' from {}: {}",
            skill.name,
            skill.path.display(),
            err
        )
    })?;

    Ok(SkillInjection {
        name: skill.name.clone(),
        path: skill.path.to_string_lossy().into_owned(),
        contents,
    })
}

/// Parse skill references from a text input.
///
/// Looks for patterns like:
/// - `/skill:name` - invoke a skill by name
/// - `@skill-name` - alternative syntax
///
/// Returns a list of skill references found in the text.
pub fn parse_skill_references(text: &str) -> Vec<SkillReference> {
    let mut references = Vec::new();
    let mut seen = HashSet::new();

    // Pattern 1: /skill:name or /skill:name
    for word in text.split_whitespace() {
        if let Some(name) = word.strip_prefix("/skill:") {
            let name =
                name.trim_end_matches(|c: char| !c.is_alphanumeric() && c != '-' && c != '_');
            if !name.is_empty() && seen.insert(name.to_string()) {
                references.push(SkillReference::by_name(name));
            }
        }
    }

    references
}

/// Render a skills section for documentation/help.
///
/// Creates a formatted list of available skills.
pub fn render_skills_section(skills: &[SkillMetadata]) -> Option<String> {
    if skills.is_empty() {
        return None;
    }

    let mut lines = Vec::new();
    lines.push("## Available Skills".to_string());
    lines.push(String::new());
    lines.push("Use `/skill:name` to invoke a skill. Available skills:".to_string());
    lines.push(String::new());

    for skill in skills {
        let desc = skill.display_description();
        let scope = skill.scope.to_string();
        lines.push(format!("- **{}** ({}): {}", skill.name, scope, desc));
    }

    Some(lines.join("\n"))
}

/// Render a skills section for system prompt injection.
///
/// This version includes trigger rules that instruct the LLM to automatically
/// select skills based on task description matching.
pub fn render_skills_for_system_prompt(skills: &[SkillMetadata]) -> Option<String> {
    if skills.is_empty() {
        return None;
    }

    let mut lines = Vec::new();

    lines.push("## Skills".to_string());
    lines.push(String::new());
    lines.push(
        "These skills are available to help with specific tasks. \
         Each skill has a name, description, and file path."
            .to_string(),
    );
    lines.push(String::new());

    // List all skills with descriptions
    for skill in skills {
        let desc = &skill.description;
        let path = skill.path.to_string_lossy().replace('\\', "/");
        lines.push(format!("- **{}**: {} (file: {})", skill.name, desc, path));
    }

    lines.push(String::new());
    lines.push("### Skill Usage Guidelines".to_string());
    lines.push(String::new());
    lines.push(
        "- **Trigger rules**: If the user explicitly names a skill (with `$skillname` or `/skill:name`) \
         OR the task clearly matches a skill's description, you SHOULD use that skill."
            .to_string(),
    );
    lines.push(
        "- **Description matching**: The skill's `description` is the primary trigger signal. \
         Use it to decide if a skill is applicable to the current task."
            .to_string(),
    );
    lines.push(
        "- **Invoke skill**: When you decide to use a skill, call the `invoke_skill` tool \
         with the skill name to load its full instructions."
            .to_string(),
    );
    lines.push(
        "- **Multiple skills**: You can invoke multiple skills if the task requires them."
            .to_string(),
    );
    lines.push(
        "- **Progressive loading**: Only the skill names and descriptions are shown here. \
         Full instructions are loaded when you invoke a skill."
            .to_string(),
    );

    Some(lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skills::model::SkillScope;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    fn create_test_skill(dir: &Path, name: &str) -> SkillMetadata {
        let skill_dir = dir.join(name);
        std::fs::create_dir_all(&skill_dir).unwrap();
        let skill_path = skill_dir.join("SKILL.md");
        std::fs::write(
            &skill_path,
            format!(
                r#"---
name: {}
description: Test skill {}
---

# {}

This is the body of the skill.
"#,
                name, name, name
            ),
        )
        .unwrap();

        SkillMetadata {
            name: name.to_string(),
            description: format!("Test skill {}", name),
            short_description: None,
            path: skill_path,
            scope: SkillScope::User,
        }
    }

    #[test]
    fn test_build_skill_injections() {
        let temp = TempDir::new().unwrap();
        let skill = create_test_skill(temp.path(), "test-skill");

        let outcome = SkillLoadOutcome {
            skills: vec![skill],
            errors: vec![],
        };

        let references = vec![SkillReference::by_name("test-skill")];
        let injections = build_skill_injections(&references, Some(&outcome));

        assert_eq!(injections.items.len(), 1);
        assert!(injections.warnings.is_empty());
        assert!(injections.items[0].contents.contains("# test-skill"));
    }

    #[test]
    fn test_build_skill_injections_not_found() {
        let outcome = SkillLoadOutcome {
            skills: vec![],
            errors: vec![],
        };

        let references = vec![SkillReference::by_name("nonexistent")];
        let injections = build_skill_injections(&references, Some(&outcome));

        assert!(injections.items.is_empty());
        assert_eq!(injections.warnings.len(), 1);
        assert!(injections.warnings[0].contains("not found"));
    }

    #[test]
    fn test_build_skill_injections_dedup() {
        let temp = TempDir::new().unwrap();
        let skill = create_test_skill(temp.path(), "my-skill");

        let outcome = SkillLoadOutcome {
            skills: vec![skill],
            errors: vec![],
        };

        // Same skill referenced twice
        let references = vec![
            SkillReference::by_name("my-skill"),
            SkillReference::by_name("my-skill"),
        ];
        let injections = build_skill_injections(&references, Some(&outcome));

        // Should only be injected once
        assert_eq!(injections.items.len(), 1);
    }

    #[test]
    fn test_parse_skill_references() {
        let text = "Please use /skill:code-review to review this code";
        let refs = parse_skill_references(text);

        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].name, "code-review");
    }

    #[test]
    fn test_parse_skill_references_multiple() {
        let text = "/skill:skill-a and /skill:skill-b";
        let refs = parse_skill_references(text);

        assert_eq!(refs.len(), 2);
        assert!(refs.iter().any(|r| r.name == "skill-a"));
        assert!(refs.iter().any(|r| r.name == "skill-b"));
    }

    #[test]
    fn test_render_skills_section() {
        let skills = vec![
            SkillMetadata {
                name: "skill-a".to_string(),
                description: "Description A".to_string(),
                short_description: Some("Short A".to_string()),
                path: PathBuf::from("/path/a"),
                scope: SkillScope::User,
            },
            SkillMetadata {
                name: "skill-b".to_string(),
                description: "Description B".to_string(),
                short_description: None,
                path: PathBuf::from("/path/b"),
                scope: SkillScope::Project,
            },
        ];

        let section = render_skills_section(&skills).unwrap();
        assert!(section.contains("## Available Skills"));
        assert!(section.contains("**skill-a** (user): Short A"));
        assert!(section.contains("**skill-b** (project): Description B"));
    }

    #[test]
    fn test_render_skills_section_empty() {
        let skills: Vec<SkillMetadata> = vec![];
        assert!(render_skills_section(&skills).is_none());
    }

    #[test]
    fn test_skill_injection_to_xml() {
        let injection = SkillInjection {
            name: "test".to_string(),
            path: "/path/to/test".to_string(),
            contents: "# Test\nContent".to_string(),
        };

        let xml = injection.to_xml();
        assert!(xml.starts_with("<skill>"));
        assert!(xml.ends_with("</skill>"));
        assert!(xml.contains("<name>test</name>"));
    }
}
