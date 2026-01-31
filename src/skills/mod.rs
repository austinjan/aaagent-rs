//! Skill System - Dynamic capability loading for LLM agents
//!
//! Skills are defined in SKILL.md files with YAML frontmatter.
//! They are discovered from multiple directories and filtered by eligibility.
//!
//! # Configuration
//!
//! Skills can be configured in config.yaml:
//!
//! ```yaml
//! skills:
//!   entries:
//!     github:
//!       enabled: true
//!       apiKey: ghp_xxxxx
//!     weather:
//!       env:
//!         WEATHER_API_KEY: my-key
//! ```

pub mod config;
pub mod eligibility;
pub mod env_override;
pub mod error;
pub mod loader;
pub mod manager;
pub mod model;

pub use config::{SkillConfig, SkillsConfig};
pub use eligibility::{check_eligibility, filter_eligible, filter_eligible_with_config};
pub use env_override::{apply_env_overrides, EnvRestoreGuard};
pub use loader::{load_skills_from_roots, skill_roots_for_cwd};
pub use manager::SkillsManager;
pub use model::{SkillLoadOutcome, SkillMetadata, SkillScope, SkillSnapshot};
