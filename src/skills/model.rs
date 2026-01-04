//! Skill data structures and types.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Scope/source of a skill, determining its priority in deduplication.
///
/// Loading order (priority): Project > User > System
/// When skills have the same name, the first one found wins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SkillScope {
    /// Project-level skills from `.aaagent/skills/` in the repo
    Project,
    /// User-level skills from `~/.aaagent/skills/`
    User,
    /// System/embedded skills from `~/.aaagent/skills/.system/`
    System,
}

impl std::fmt::Display for SkillScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SkillScope::Project => write!(f, "project"),
            SkillScope::User => write!(f, "user"),
            SkillScope::System => write!(f, "system"),
        }
    }
}

/// Metadata for a discovered skill.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillMetadata {
    /// Unique name of the skill (used for invocation)
    pub name: String,

    /// Full description of when to use this skill
    pub description: String,

    /// Optional shorter description for listing
    pub short_description: Option<String>,

    /// Path to the SKILL.md file
    pub path: PathBuf,

    /// Scope/source of this skill
    pub scope: SkillScope,
}

impl SkillMetadata {
    /// Get the display description (short if available, otherwise full)
    pub fn display_description(&self) -> &str {
        self.short_description
            .as_deref()
            .unwrap_or(&self.description)
    }
}

/// Error encountered while loading a skill.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillError {
    /// Path to the skill file that failed to load
    pub path: PathBuf,

    /// Error message describing what went wrong
    pub message: String,
}

impl std::fmt::Display for SkillError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.path.display(), self.message)
    }
}

impl std::error::Error for SkillError {}

/// Outcome of loading skills from one or more roots.
#[derive(Debug, Clone, Default)]
pub struct SkillLoadOutcome {
    /// Successfully loaded skills
    pub skills: Vec<SkillMetadata>,

    /// Errors encountered during loading
    pub errors: Vec<SkillError>,
}

impl SkillLoadOutcome {
    /// Create an empty outcome
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if any skills were loaded
    pub fn has_skills(&self) -> bool {
        !self.skills.is_empty()
    }

    /// Check if any errors occurred
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    /// Merge another outcome into this one
    pub fn merge(&mut self, other: SkillLoadOutcome) {
        self.skills.extend(other.skills);
        self.errors.extend(other.errors);
    }

    /// Find a skill by name
    pub fn find_by_name(&self, name: &str) -> Option<&SkillMetadata> {
        self.skills.iter().find(|s| s.name == name)
    }
}

/// YAML frontmatter structure in SKILL.md files.
#[derive(Debug, Deserialize)]
pub(crate) struct SkillFrontmatter {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub metadata: SkillFrontmatterMetadata,
}

/// Optional metadata section in skill frontmatter.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct SkillFrontmatterMetadata {
    pub short_description: Option<String>,
}

/// Skill content ready for injection into a conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillInjection {
    /// Skill name
    pub name: String,

    /// Path to the skill file
    pub path: String,

    /// Full contents of the skill file
    pub contents: String,
}

impl SkillInjection {
    /// Format as XML for injection into model context
    pub fn to_xml(&self) -> String {
        format!(
            "<skill>\n<name>{}</name>\n<path>{}</path>\n{}\n</skill>",
            self.name, self.path, self.contents
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skill_scope_display() {
        assert_eq!(SkillScope::Project.to_string(), "project");
        assert_eq!(SkillScope::User.to_string(), "user");
        assert_eq!(SkillScope::System.to_string(), "system");
    }

    #[test]
    fn test_skill_injection_to_xml() {
        let injection = SkillInjection {
            name: "test-skill".to_string(),
            path: "/path/to/skill".to_string(),
            contents: "# Test\nSome content".to_string(),
        };

        let xml = injection.to_xml();
        assert!(xml.contains("<name>test-skill</name>"));
        assert!(xml.contains("<path>/path/to/skill</path>"));
        assert!(xml.contains("# Test\nSome content"));
    }

    #[test]
    fn test_skill_load_outcome_merge() {
        let mut outcome1 = SkillLoadOutcome {
            skills: vec![SkillMetadata {
                name: "skill1".to_string(),
                description: "desc1".to_string(),
                short_description: None,
                path: PathBuf::from("/path1"),
                scope: SkillScope::User,
            }],
            errors: vec![],
        };

        let outcome2 = SkillLoadOutcome {
            skills: vec![SkillMetadata {
                name: "skill2".to_string(),
                description: "desc2".to_string(),
                short_description: None,
                path: PathBuf::from("/path2"),
                scope: SkillScope::Project,
            }],
            errors: vec![SkillError {
                path: PathBuf::from("/error"),
                message: "test error".to_string(),
            }],
        };

        outcome1.merge(outcome2);
        assert_eq!(outcome1.skills.len(), 2);
        assert_eq!(outcome1.errors.len(), 1);
    }
}
