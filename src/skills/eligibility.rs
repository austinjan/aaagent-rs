//! Skill eligibility checking

use crate::skills::error::not_eligible_error;
use crate::skills::model::{SkillMetadata, SkillOpenMetadata, SkillRequirements};
use anyhow::Result;
use std::env;

/// Check if a skill is eligible to run in the current environment
pub fn check_eligibility(skill: &SkillMetadata) -> Result<()> {
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
    check_requirements(&skill.name, &meta.requires)?;

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
fn check_requirements(name: &str, reqs: &SkillRequirements) -> Result<()> {
    // All bins must exist
    for bin in &reqs.bins {
        if which::which(bin).is_err() {
            return Err(not_eligible_error(
                name,
                &format!("required binary '{}' not found", bin),
            ));
        }
    }

    // At least one of anyBins must exist
    if !reqs.any_bins.is_empty() {
        let found = reqs.any_bins.iter().any(|bin| which::which(bin).is_ok());
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
    let mut eligible = Vec::new();
    let mut errors = Vec::new();

    for skill in skills {
        match check_eligibility(&skill) {
            Ok(()) => eligible.push(skill),
            Err(e) => {
                log::debug!("Skill '{}' not eligible: {}", skill.name, e);
                errors.push(e.to_string());
            }
        }
    }

    eligible
        .sort_by(|a, b| a.name.cmp(&b.name));

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
}
