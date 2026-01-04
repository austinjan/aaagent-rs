//! Skills system for extending agent capabilities.
//!
//! Skills are markdown files that provide specialized knowledge, workflows,
//! or tool integrations. They are discovered from multiple locations:
//!
//! - **Project skills**: `.aaagent/skills/` in the repository root
//! - **User skills**: `~/.aaagent/skills/`
//! - **System skills**: `~/.aaagent/skills/.system/` (embedded/installed)
//!
//! # Skill File Format
//!
//! Each skill is a directory containing a `SKILL.md` file with YAML frontmatter:
//!
//! ```markdown
//! ---
//! name: my-skill
//! description: What this skill does and when to use it
//! metadata:
//!   short-description: Optional shorter description
//! ---
//!
//! # My Skill
//!
//! Detailed instructions and content...
//! ```
//!
//! # Usage
//!
//! ```rust,ignore
//! use aaagent::skills::{SkillsManager, SkillReference, build_skill_injections};
//!
//! // Create manager
//! let manager = SkillsManager::with_default_home().unwrap();
//!
//! // Load skills for current directory
//! let outcome = manager.skills_for_cwd(&std::env::current_dir().unwrap());
//!
//! // Build injections for specific skills
//! let refs = vec![SkillReference::by_name("code-review")];
//! let injections = build_skill_injections(&refs, Some(&outcome));
//!
//! // Get XML content for model
//! for item in &injections.items {
//!     println!("{}", item.to_xml());
//! }
//! ```

mod injection;
mod loader;
mod manager;
mod model;

// Re-export public types
pub use injection::{
    build_skill_injections, parse_skill_references, render_skills_for_system_prompt,
    render_skills_section, SkillInjections, SkillReference,
};
pub use manager::SkillsManager;
pub use model::{SkillError, SkillInjection, SkillLoadOutcome, SkillMetadata, SkillScope};
