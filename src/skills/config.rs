//! Per-skill configuration
//!
//! Allows users to configure individual skills in config.yaml:
//!
//! ```yaml
//! skills:
//!   entries:
//!     github:
//!       enabled: true
//!       apiKey: ghp_xxxxx
//!     weather:
//!       enabled: true
//!       env:
//!         WEATHER_API_KEY: my-key
//!     spotify:
//!       enabled: false
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Skills system configuration (in config.yaml)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SkillsConfig {
    /// Individual skill configurations (key = skill name or skillKey)
    #[serde(default)]
    pub entries: HashMap<String, SkillConfig>,
}

/// Individual skill configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SkillConfig {
    /// Enable/disable this skill (None = use default eligibility)
    #[serde(default)]
    pub enabled: Option<bool>,

    /// API key (injected to skill's primaryEnv)
    #[serde(default, rename = "apiKey")]
    pub api_key: Option<String>,

    /// Additional environment variables
    #[serde(default)]
    pub env: Option<HashMap<String, String>>,
}

impl SkillsConfig {
    /// Create empty config
    pub fn new() -> Self {
        Self::default()
    }

    /// Get config for a skill by name
    pub fn get(&self, name: &str) -> Option<&SkillConfig> {
        self.entries.get(name)
    }

    /// Check if a skill is explicitly disabled
    pub fn is_disabled(&self, name: &str) -> bool {
        self.entries
            .get(name)
            .and_then(|c| c.enabled)
            .map(|e| !e)
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skills_config_default() {
        let config = SkillsConfig::default();
        assert!(config.entries.is_empty());
    }

    #[test]
    fn test_skill_config_parsing() {
        let yaml = r#"
entries:
  github:
    enabled: true
    apiKey: ghp_test123
  weather:
    enabled: true
    env:
      WEATHER_API_KEY: test-key
      WEATHER_CACHE: /tmp/weather
  disabled-skill:
    enabled: false
"#;
        let config: SkillsConfig = serde_yaml::from_str(yaml).unwrap();

        // Check github config
        let github = config.get("github").unwrap();
        assert_eq!(github.enabled, Some(true));
        assert_eq!(github.api_key, Some("ghp_test123".to_string()));

        // Check weather config
        let weather = config.get("weather").unwrap();
        assert_eq!(weather.enabled, Some(true));
        let env = weather.env.as_ref().unwrap();
        assert_eq!(env.get("WEATHER_API_KEY"), Some(&"test-key".to_string()));

        // Check disabled skill
        assert!(config.is_disabled("disabled-skill"));
        assert!(!config.is_disabled("github"));
        assert!(!config.is_disabled("nonexistent"));
    }

    #[test]
    fn test_skill_config_default_values() {
        let config = SkillConfig::default();
        assert_eq!(config.enabled, None);
        assert_eq!(config.api_key, None);
        assert_eq!(config.env, None);
    }
}
