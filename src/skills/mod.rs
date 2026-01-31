//! Skill System - Dynamic capability loading for LLM agents
//!
//! Skills are defined in SKILL.md files with YAML frontmatter.
//! They are discovered from multiple directories and filtered by eligibility.

pub mod eligibility;
pub mod error;
pub mod loader;
pub mod manager;
pub mod model;

pub use eligibility::{check_eligibility, filter_eligible};
pub use loader::{load_skills_from_roots, skill_roots_for_cwd};
pub use manager::SkillsManager;
pub use model::{SkillLoadOutcome, SkillMetadata, SkillScope, SkillSnapshot};
