//! Skill data models

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Skill scope - defines source and priority (higher = more priority)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum SkillScope {
    Admin,  // Lowest priority (/etc/aaagent/skills/)
    System, // ~/.aaagent/skills/.system/
    User,   // ~/.aaagent/skills/
    Repo,   // Highest priority (.skills/ in project)
}

/// Skill eligibility requirements (from metadata.requires)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SkillRequirements {
    /// All binaries must exist
    #[serde(default)]
    pub bins: Vec<String>,

    /// At least one binary must exist
    #[serde(default, rename = "anyBins")]
    pub any_bins: Vec<String>,

    /// Required environment variables
    #[serde(default)]
    pub env: Vec<String>,

    /// Required config paths (dot-separated)
    #[serde(default)]
    pub config: Vec<String>,
}

/// Skill invocation control
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillInvocation {
    /// User can invoke via slash command (default: true)
    #[serde(default = "default_true")]
    pub user_invocable: bool,

    /// Hidden from model (default: false)
    #[serde(default)]
    pub disable_model_invocation: bool,
}

fn default_true() -> bool {
    true
}

impl Default for SkillInvocation {
    fn default() -> Self {
        Self {
            user_invocable: true,
            disable_model_invocation: false,
        }
    }
}

/// Skill metadata from frontmatter
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SkillOpenMetadata {
    /// Always available (skip eligibility checks)
    #[serde(default)]
    pub always: bool,

    /// Primary env var for API key injection
    #[serde(rename = "primaryEnv")]
    pub primary_env: Option<String>,

    /// Supported OS platforms
    #[serde(default)]
    pub os: Vec<String>,

    /// Eligibility requirements
    #[serde(default)]
    pub requires: SkillRequirements,
}

/// Skill metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillMetadata {
    /// Skill name (unique identifier)
    pub name: String,

    /// Full description
    pub description: String,

    /// Path to SKILL.md file
    pub path: PathBuf,

    /// Skill scope
    pub scope: SkillScope,

    /// Invocation control
    #[serde(default)]
    pub invocation: SkillInvocation,

    /// OpenClaw-style metadata
    #[serde(default)]
    pub metadata: Option<SkillOpenMetadata>,
}

/// SKILL.md frontmatter structure
#[derive(Debug, Deserialize)]
pub struct SkillFrontmatter {
    pub name: String,
    pub description: String,

    /// Metadata as YAML object (can be {"openclaw": {...}} or flat structure)
    pub metadata: Option<serde_json::Value>,

    /// User can invoke (default: true)
    #[serde(default = "default_true", rename = "user-invocable")]
    pub user_invocable: bool,

    /// Disable model invocation (default: false)
    #[serde(default, rename = "disable-model-invocation")]
    pub disable_model_invocation: bool,
}

/// Skill snapshot for agent runtime
#[derive(Debug, Clone)]
pub struct SkillSnapshot {
    /// Formatted XML for system prompt
    pub prompt: String,

    /// Skill entries
    pub skills: Vec<SkillSnapshotEntry>,
}

#[derive(Debug, Clone)]
pub struct SkillSnapshotEntry {
    pub name: String,
    pub path: PathBuf,
    pub primary_env: Option<String>,
}

/// Skill load outcome
#[derive(Debug, Default)]
pub struct SkillLoadOutcome {
    /// Successfully loaded skills
    pub skills: Vec<SkillMetadata>,

    /// Errors during loading (as strings for serialization)
    pub errors: Vec<String>,
}

impl SkillLoadOutcome {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_skill(&mut self, skill: SkillMetadata) {
        self.skills.push(skill);
    }

    pub fn add_error(&mut self, error: anyhow::Error) {
        log::warn!("{:#}", error);
        self.errors.push(error.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skill_scope_ordering() {
        assert!(SkillScope::Repo > SkillScope::User);
        assert!(SkillScope::User > SkillScope::System);
        assert!(SkillScope::System > SkillScope::Admin);
    }

    #[test]
    fn test_skill_invocation_default() {
        let inv = SkillInvocation::default();
        assert!(inv.user_invocable);
        assert!(!inv.disable_model_invocation);
    }
}
