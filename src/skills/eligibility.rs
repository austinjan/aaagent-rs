//! Skill eligibility checking

use crate::skills::config::SkillsConfig;
use crate::skills::error::not_eligible_error;
use crate::skills::model::{SkillMetadata, SkillOpenMetadata, SkillRequirements};
use crate::tools::BuiltinBinaries;
use anyhow::Result;
use std::env;

/// Check if a skill is eligible to run in the current environment
pub fn check_eligibility(skill: &SkillMetadata) -> Result<()> {
    check_eligibility_with_builtins(skill, &BuiltinBinaries::empty())
}

/// Check if a skill is eligible, considering built-in binaries
pub fn check_eligibility_with_builtins(
    skill: &SkillMetadata,
    builtins: &BuiltinBinaries,
) -> Result<()> {
    let meta = match &skill.metadata {
        Some(m) => m,
        None => return Ok(()), // No metadata = always eligible
    };

    // Skip checks if always=true
    if meta.always {
        return Ok(());
    }

    // Check OS
    check_os(&skill.name, meta)?;

    // Check requirements
    check_requirements(&skill.name, &meta.requires, builtins)?;

    Ok(())
}

/// Check OS eligibility
fn check_os(name: &str, meta: &SkillOpenMetadata) -> Result<()> {
    if meta.os.is_empty() {
        return Ok(());
    }

    let current_os = env::consts::OS;

    // Normalize OS names
    let matches = meta.os.iter().any(|os| {
        let os_lower = os.to_lowercase();
        match os_lower.as_str() {
            "macos" | "darwin" => current_os == "macos",
            "windows" | "win32" => current_os == "windows",
            "linux" => current_os == "linux",
            _ => os_lower == current_os,
        }
    });

    if !matches {
        return Err(not_eligible_error(
            name,
            &format!("requires OS {:?}, current is {}", meta.os, current_os),
        ));
    }

    Ok(())
}

/// Check requirements (bins, anyBins, env)
fn check_requirements(
    name: &str,
    reqs: &SkillRequirements,
    builtins: &BuiltinBinaries,
) -> Result<()> {
    use crate::tools::is_binary_available;

    // All bins must exist (check built-ins first, then system PATH)
    for bin in &reqs.bins {
        if !is_binary_available(bin, builtins) {
            return Err(not_eligible_error(
                name,
                &format!("required binary '{}' not found", bin),
            ));
        }
    }

    // At least one of anyBins must exist
    if !reqs.any_bins.is_empty() {
        let found = reqs
            .any_bins
            .iter()
            .any(|bin| is_binary_available(bin, builtins));
        if !found {
            return Err(not_eligible_error(
                name,
                &format!("none of required binaries {:?} found", reqs.any_bins),
            ));
        }
    }

    // Required env vars must be set
    for var in &reqs.env {
        if env::var(var).is_err() {
            return Err(not_eligible_error(
                name,
                &format!("required env var '{}' not set", var),
            ));
        }
    }

    // Config checks would require access to config - skip for now
    // (config paths are checked at manager level with config access)

    Ok(())
}

/// Filter skills by eligibility, returning eligible skills and errors
pub fn filter_eligible(skills: Vec<SkillMetadata>) -> (Vec<SkillMetadata>, Vec<String>) {
    filter_eligible_with_config(skills, &SkillsConfig::default())
}

/// Filter skills by eligibility with config-based enabled/disabled support
///
/// Skills are filtered out if:
/// 1. Config has `enabled: false` for the skill
/// 2. Eligibility checks fail (OS, bins, env)
///
/// Skills configured with `enabled: true` still need to pass eligibility checks.
pub fn filter_eligible_with_config(
    skills: Vec<SkillMetadata>,
    config: &SkillsConfig,
) -> (Vec<SkillMetadata>, Vec<String>) {
    filter_eligible_with_config_and_builtins(skills, config, &BuiltinBinaries::empty())
}

/// Filter skills by eligibility with config and built-in binaries support
///
/// Skills are filtered out if:
/// 1. Config has `enabled: false` for the skill
/// 2. Eligibility checks fail (OS, bins, env)
///
/// Built-in binaries are considered available in addition to system PATH binaries.
pub fn filter_eligible_with_config_and_builtins(
    skills: Vec<SkillMetadata>,
    config: &SkillsConfig,
    builtins: &BuiltinBinaries,
) -> (Vec<SkillMetadata>, Vec<String>) {
    let mut eligible = Vec::new();
    let mut errors = Vec::new();

    for skill in skills {
        // Check if explicitly disabled in config
        if config.is_disabled(&skill.name) {
            log::debug!("Skill '{}' disabled in config", skill.name);
            errors.push(format!("Skill '{}' disabled in config", skill.name));
            continue;
        }

        // Check environment/binary requirements (with built-in binaries)
        match check_eligibility_with_builtins(&skill, builtins) {
            Ok(()) => eligible.push(skill),
            Err(e) => {
                log::debug!("Skill '{}' not eligible: {}", skill.name, e);
                errors.push(e.to_string());
            }
        }
    }

    eligible.sort_by(|a, b| a.name.cmp(&b.name));

    (eligible, errors)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skills::model::{SkillInvocation, SkillScope};
    use std::path::PathBuf;

    fn make_skill(name: &str, metadata: Option<SkillOpenMetadata>) -> SkillMetadata {
        SkillMetadata {
            name: name.to_string(),
            description: "Test skill".to_string(),
            path: PathBuf::from("test.md"),
            scope: SkillScope::User,
            invocation: SkillInvocation::default(),
            metadata,
        }
    }

    #[test]
    fn test_no_metadata_is_eligible() {
        let skill = make_skill("test", None);
        assert!(check_eligibility(&skill).is_ok());
    }

    #[test]
    fn test_always_true_skips_checks() {
        let meta = SkillOpenMetadata {
            always: true,
            os: vec!["nonexistent".to_string()],
            ..Default::default()
        };
        let skill = make_skill("test", Some(meta));
        assert!(check_eligibility(&skill).is_ok());
    }

    #[test]
    fn test_current_os_is_eligible() {
        let meta = SkillOpenMetadata {
            os: vec![env::consts::OS.to_string()],
            ..Default::default()
        };
        let skill = make_skill("test", Some(meta));
        assert!(check_eligibility(&skill).is_ok());
    }

    #[test]
    fn test_wrong_os_is_not_eligible() {
        let meta = SkillOpenMetadata {
            os: vec!["nonexistent_os".to_string()],
            ..Default::default()
        };
        let skill = make_skill("test", Some(meta));
        assert!(check_eligibility(&skill).is_err());
    }

    #[test]
    fn test_missing_bin_is_not_eligible() {
        let meta = SkillOpenMetadata {
            requires: SkillRequirements {
                bins: vec!["nonexistent_binary_12345".to_string()],
                ..Default::default()
            },
            ..Default::default()
        };
        let skill = make_skill("test", Some(meta));
        assert!(check_eligibility(&skill).is_err());
    }

    #[test]
    fn test_missing_env_is_not_eligible() {
        let meta = SkillOpenMetadata {
            requires: SkillRequirements {
                env: vec!["NONEXISTENT_ENV_VAR_12345".to_string()],
                ..Default::default()
            },
            ..Default::default()
        };
        let skill = make_skill("test", Some(meta));
        assert!(check_eligibility(&skill).is_err());
    }

    #[test]
    fn test_builtin_binary_makes_skill_eligible() {
        // Skill requires a non-existent system binary
        let meta = SkillOpenMetadata {
            requires: SkillRequirements {
                bins: vec!["my-custom-tool".to_string()],
                ..Default::default()
            },
            ..Default::default()
        };
        let skill = make_skill("test", Some(meta));

        // Without builtins, should fail
        assert!(check_eligibility(&skill).is_err());

        // With builtins registered, should pass
        let builtins = BuiltinBinaries::with_names("/bin", vec!["my-custom-tool".to_string()]);
        assert!(check_eligibility_with_builtins(&skill, &builtins).is_ok());
    }

    #[test]
    fn test_any_bins_with_builtin() {
        let meta = SkillOpenMetadata {
            requires: SkillRequirements {
                any_bins: vec![
                    "nonexistent1".to_string(),
                    "my-custom-tool".to_string(),
                    "nonexistent2".to_string(),
                ],
                ..Default::default()
            },
            ..Default::default()
        };
        let skill = make_skill("test", Some(meta));

        // Without builtins, none found - should fail
        assert!(check_eligibility(&skill).is_err());

        // With builtin for one of them - should pass
        let builtins = BuiltinBinaries::with_names("/bin", vec!["my-custom-tool".to_string()]);
        assert!(check_eligibility_with_builtins(&skill, &builtins).is_ok());
    }
}
