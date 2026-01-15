//! System LLM Profiles
//! 
//! These profiles are used internally by the system for automated tasks.
//! They are separate from user-facing presets to ensure consistent behavior.
//! 
//! Design principles:
//! 1. Hardcoded defaults ensure tests are deterministic
//! 2. config.yaml can override for production customization
//! 3. Tests can inject custom profiles for specific scenarios

use serde::{Deserialize, Serialize};

/// System LLM profile for internal tasks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemLLMProfile {
    pub model: String,
    pub temperature: f32,
    pub max_tokens: u32,
}

impl SystemLLMProfile {
    /// Hardcoded default profile (gpt-5-mini)
    /// Used when no config.yaml exists (e.g., in tests)
    pub fn default_standard() -> Self {
        Self {
            model: "gpt-5-mini".to_string(),
            temperature: 1.0,
            max_tokens: 16384,
        }
    }

    /// Hardcoded quick profile (gpt-5-nano)
    /// Used for lightweight tasks (compact, summary, etc.)
    pub fn default_quick() -> Self {
        Self {
            model: "gpt-5-nano".to_string(),
            temperature: 1.0,
            max_tokens: 4096,
        }
    }
}

/// System LLM profiles configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemLLMProfiles {
    /// Standard profile for general system tasks
    #[serde(default = "SystemLLMProfile::default_standard")]
    pub default: SystemLLMProfile,

    /// Quick profile for lightweight tasks (compact, summary)
    #[serde(default = "SystemLLMProfile::default_quick")]
    pub quick: SystemLLMProfile,
}

impl Default for SystemLLMProfiles {
    fn default() -> Self {
        Self {
            default: SystemLLMProfile::default_standard(),
            quick: SystemLLMProfile::default_quick(),
        }
    }
}

impl SystemLLMProfiles {
    /// Get profile by name
    pub fn get(&self, name: &str) -> &SystemLLMProfile {
        match name {
            "quick" => &self.quick,
            _ => &self.default,
        }
    }

    /// Create profiles for testing with custom models
    #[cfg(test)]
    pub fn for_testing(default_model: &str, quick_model: &str) -> Self {
        Self {
            default: SystemLLMProfile {
                model: default_model.to_string(),
                temperature: 1.0,
                max_tokens: 16384,
            },
            quick: SystemLLMProfile {
                model: quick_model.to_string(),
                temperature: 1.0,
                max_tokens: 4096,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_profiles() {
        let profiles = SystemLLMProfiles::default();
        
        assert_eq!(profiles.default.model, "gpt-5-mini");
        assert_eq!(profiles.default.max_tokens, 16384);
        
        assert_eq!(profiles.quick.model, "gpt-5-nano");
        assert_eq!(profiles.quick.max_tokens, 4096);
    }

    #[test]
    fn test_get_profile() {
        let profiles = SystemLLMProfiles::default();
        
        assert_eq!(profiles.get("default").model, "gpt-5-mini");
        assert_eq!(profiles.get("quick").model, "gpt-5-nano");
        assert_eq!(profiles.get("unknown").model, "gpt-5-mini"); // Falls back to default
    }

    #[test]
    fn test_for_testing() {
        let profiles = SystemLLMProfiles::for_testing("custom-model", "custom-quick");
        
        assert_eq!(profiles.default.model, "custom-model");
        assert_eq!(profiles.quick.model, "custom-quick");
    }

    #[test]
    fn test_serialization() {
        let profiles = SystemLLMProfiles::default();
        let yaml = serde_yaml::to_string(&profiles).unwrap();
        
        assert!(yaml.contains("gpt-5-mini"));
        assert!(yaml.contains("gpt-5-nano"));
        
        // Deserialize back
        let deserialized: SystemLLMProfiles = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(deserialized.default.model, "gpt-5-mini");
    }
}
