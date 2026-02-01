//! Environment variable injection for skills
//!
//! Injects API keys and custom environment variables from skill config
//! into the process environment, with automatic restoration.
//!
//! # Example
//!
//! ```ignore
//! let guard = apply_env_overrides(&skills, &skills_config);
//! // ... run agent with skills ...
//! guard.restore(); // or let it drop to auto-restore
//! ```

use crate::skills::config::{SkillConfig, SkillsConfig};
use crate::skills::model::SkillMetadata;
use std::env;

/// Record of an environment variable change
#[derive(Debug)]
struct EnvUpdate {
    key: String,
    previous: Option<String>,
}

/// Guard that restores environment variables when dropped
#[derive(Debug)]
pub struct EnvRestoreGuard {
    updates: Vec<EnvUpdate>,
}

impl EnvRestoreGuard {
    /// Manually restore environment variables
    pub fn restore(self) {
        // Dropping self will trigger Drop::drop
    }

    /// Get count of overridden variables
    pub fn count(&self) -> usize {
        self.updates.len()
    }
}

impl Drop for EnvRestoreGuard {
    fn drop(&mut self) {
        for update in &self.updates {
            match &update.previous {
                Some(prev) => env::set_var(&update.key, prev),
                None => env::remove_var(&update.key),
            }
        }
    }
}

/// Apply environment variable overrides for skills
///
/// This function:
/// 1. Reads skill configs from SkillsConfig
/// 2. Injects `apiKey` into skill's `primaryEnv` (if not already set)
/// 3. Injects custom `env` variables (if not already set)
///
/// Returns a guard that will restore original values when dropped.
///
/// # Arguments
///
/// * `skills` - List of eligible skills
/// * `skills_config` - Per-skill configuration from config.yaml
///
/// # Example
///
/// ```ignore
/// // Skill has primaryEnv = "GITHUB_TOKEN"
/// // Config has apiKey = "ghp_xxx"
/// // Result: GITHUB_TOKEN=ghp_xxx is set in environment
/// ```
pub fn apply_env_overrides(
    skills: &[SkillMetadata],
    skills_config: &SkillsConfig,
) -> EnvRestoreGuard {
    let mut updates = Vec::new();

    for skill in skills {
        // Get skill config by name
        let skill_config = match skills_config.get(&skill.name) {
            Some(cfg) => cfg,
            None => continue,
        };

        // Inject custom env variables
        if let Some(env_map) = &skill_config.env {
            for (env_key, env_value) in env_map {
                // Don't override existing environment variables
                if env::var(env_key).is_ok() {
                    continue;
                }

                updates.push(EnvUpdate {
                    key: env_key.clone(),
                    previous: None, // Was not set
                });
                env::set_var(env_key, env_value);
            }
        }

        // Inject API key into primaryEnv
        inject_api_key(skill, skill_config, &mut updates);
    }

    EnvRestoreGuard { updates }
}

/// Inject API key into skill's primaryEnv
fn inject_api_key(
    skill: &SkillMetadata,
    skill_config: &SkillConfig,
    updates: &mut Vec<EnvUpdate>,
) {
    // Get primaryEnv from skill metadata
    let primary_env = match skill
        .metadata
        .as_ref()
        .and_then(|m| m.primary_env.as_ref())
    {
        Some(env) => env,
        None => return, // No primaryEnv defined
    };

    // Get API key from config
    let api_key = match &skill_config.api_key {
        Some(key) => key,
        None => return, // No API key configured
    };

    // Don't override existing environment variable
    if env::var(primary_env).is_ok() {
        return;
    }

    updates.push(EnvUpdate {
        key: primary_env.clone(),
        previous: None,
    });
    env::set_var(primary_env, api_key);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skills::model::{SkillInvocation, SkillOpenMetadata, SkillScope};
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn create_test_skill(name: &str, primary_env: Option<&str>) -> SkillMetadata {
        SkillMetadata {
            name: name.to_string(),
            description: format!("Test skill {}", name),
            path: PathBuf::from(format!("/test/{}/SKILL.md", name)),
            scope: SkillScope::User,
            invocation: SkillInvocation::default(),
            metadata: primary_env.map(|env| SkillOpenMetadata {
                primary_env: Some(env.to_string()),
                ..Default::default()
            }),
        }
    }

    #[test]
    fn test_apply_env_overrides_api_key() {
        // Use unique env var names to avoid test interference
        let env_key = "TEST_SKILL_API_KEY_12345";
        env::remove_var(env_key);

        let skills = vec![create_test_skill("test-skill", Some(env_key))];

        let mut config = SkillsConfig::default();
        config.entries.insert(
            "test-skill".to_string(),
            SkillConfig {
                enabled: Some(true),
                api_key: Some("test-api-key".to_string()),
                env: None,
            },
        );

        // Apply overrides
        let guard = apply_env_overrides(&skills, &config);
        assert_eq!(guard.count(), 1);
        assert_eq!(env::var(env_key).unwrap(), "test-api-key");

        // Restore
        drop(guard);
        assert!(env::var(env_key).is_err());
    }

    #[test]
    fn test_apply_env_overrides_custom_env() {
        let env_key = "TEST_CUSTOM_ENV_67890";
        env::remove_var(env_key);

        let skills = vec![create_test_skill("weather", None)];

        let mut env_map = HashMap::new();
        env_map.insert(env_key.to_string(), "custom-value".to_string());

        let mut config = SkillsConfig::default();
        config.entries.insert(
            "weather".to_string(),
            SkillConfig {
                enabled: Some(true),
                api_key: None,
                env: Some(env_map),
            },
        );

        let guard = apply_env_overrides(&skills, &config);
        assert_eq!(env::var(env_key).unwrap(), "custom-value");

        drop(guard);
        assert!(env::var(env_key).is_err());
    }

    #[test]
    fn test_no_override_existing_env() {
        let env_key = "TEST_EXISTING_ENV_11111";
        env::set_var(env_key, "original-value");

        let skills = vec![create_test_skill("test", Some(env_key))];

        let mut config = SkillsConfig::default();
        config.entries.insert(
            "test".to_string(),
            SkillConfig {
                enabled: Some(true),
                api_key: Some("new-value".to_string()),
                env: None,
            },
        );

        let guard = apply_env_overrides(&skills, &config);
        // Should not override existing value
        assert_eq!(env::var(env_key).unwrap(), "original-value");
        assert_eq!(guard.count(), 0);

        drop(guard);
        // Cleanup
        env::remove_var(env_key);
    }

    #[test]
    fn test_guard_auto_restore_on_drop() {
        let env_key = "TEST_AUTO_RESTORE_22222";
        env::remove_var(env_key);

        let skills = vec![create_test_skill("auto", Some(env_key))];

        let mut config = SkillsConfig::default();
        config.entries.insert(
            "auto".to_string(),
            SkillConfig {
                api_key: Some("temp-value".to_string()),
                ..Default::default()
            },
        );

        {
            let _guard = apply_env_overrides(&skills, &config);
            assert_eq!(env::var(env_key).unwrap(), "temp-value");
            // _guard drops here
        }

        // Should be restored (removed)
        assert!(env::var(env_key).is_err());
    }
}
