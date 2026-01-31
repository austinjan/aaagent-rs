//! Skill error helpers - uses anyhow for consistency with project

use anyhow::{anyhow, Context, Result};
use std::path::Path;

/// Create file read error with path context
pub fn file_read_error(path: &Path, e: std::io::Error) -> anyhow::Error {
    anyhow!("Cannot read skill '{}': {}", path.display(), e)
}

/// Create YAML parse error with path context
pub fn yaml_parse_error(path: &Path, e: serde_yaml::Error) -> anyhow::Error {
    anyhow!("Invalid YAML in '{}': {}", path.display(), e)
}

/// Create missing frontmatter error
pub fn missing_frontmatter_error(path: &Path) -> anyhow::Error {
    anyhow!("Missing frontmatter in '{}'", path.display())
}

/// Create invalid name error
pub fn invalid_name_error(name: &str, path: &Path, reason: &str) -> anyhow::Error {
    anyhow!(
        "Invalid skill name '{}' in '{}': {}",
        name,
        path.display(),
        reason
    )
}

/// Create not eligible error
pub fn not_eligible_error(name: &str, reason: &str) -> anyhow::Error {
    anyhow!("Skill '{}' not eligible: {}", name, reason)
}

/// Extension trait for adding skill context to Results
pub trait SkillResultExt<T> {
    fn with_skill_context(self, path: &Path) -> Result<T>;
}

impl<T, E: std::error::Error + Send + Sync + 'static> SkillResultExt<T>
    for std::result::Result<T, E>
{
    fn with_skill_context(self, path: &Path) -> Result<T> {
        self.with_context(|| format!("in skill '{}'", path.display()))
    }
}
