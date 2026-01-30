use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Unified session configuration (stored in session metadata)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    pub provider: ProviderConfig,
    pub agent: AgentConfig,
    pub session: SessionRuntimeConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub model: String,
    pub temperature: f32,
    pub max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_grounding: Option<bool>, // Web search grounding (Gemini only)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub max_rounds: u32,
    pub tools_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRuntimeConfig {
    pub system_prompt: String,
    pub max_context_tokens: u32,
}

/// Temperature profiles for model-specific behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemperatureProfiles {
    pub profiles: HashMap<String, TemperatureProfile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TemperatureProfile {
    Fixed {
        fixed: f32,
        #[serde(default)]
        ignore_creativity: bool,
    },
    Mapped {
        creativity_map: Vec<(f32, f32)>,
    },
}

impl TemperatureProfiles {
    pub fn get_temperature(&self, model: &str, creativity: f32) -> f32 {
        let profile = self
            .profiles
            .get(model)
            .or_else(|| self.profiles.get("default"))
            .expect("Default temperature profile must exist");

        match profile {
            TemperatureProfile::Fixed { fixed, .. } => *fixed,
            TemperatureProfile::Mapped { creativity_map } => {
                Self::interpolate_temperature(creativity_map, creativity)
            }
        }
    }

    fn interpolate_temperature(map: &[(f32, f32)], creativity: f32) -> f32 {
        if map.is_empty() {
            return 1.0;
        }

        // Clamp creativity to [0, 1]
        let creativity = creativity.clamp(0.0, 1.0);

        // Find surrounding points
        let mut lower = map[0];
        let mut upper = map[map.len() - 1];

        for i in 0..map.len() - 1 {
            if creativity >= map[i].0 && creativity <= map[i + 1].0 {
                lower = map[i];
                upper = map[i + 1];
                break;
            }
        }

        // Linear interpolation
        if (upper.0 - lower.0).abs() < 0.001 {
            return lower.1;
        }

        let ratio = (creativity - lower.0) / (upper.0 - lower.0);
        lower.1 + ratio * (upper.1 - lower.1)
    }
}

impl Default for TemperatureProfiles {
    fn default() -> Self {
        let mut profiles = HashMap::new();

        // GPT-5.2 - configurable temperature
        profiles.insert(
            "gpt-5.2".to_string(),
            TemperatureProfile::Mapped {
                creativity_map: vec![(0.0, 0.0), (0.5, 0.35), (1.0, 0.7)],
            },
        );

        // GPT-5 variants - fixed at 1.0 (reasoning models)
        for model in ["gpt-5", "gpt-5-mini", "gpt-5-nano"] {
            profiles.insert(
                model.to_string(),
                TemperatureProfile::Fixed {
                    fixed: 1.0,
                    ignore_creativity: true,
                },
            );
        }

        // Gemini-3 variants - fixed at 1.0
        for model in ["gemini-3-flash-preview", "gemini-3-pro-preview"] {
            profiles.insert(
                model.to_string(),
                TemperatureProfile::Fixed {
                    fixed: 1.0,
                    ignore_creativity: true,
                },
            );
        }

        // Default fallback - conservative mapping
        profiles.insert(
            "default".to_string(),
            TemperatureProfile::Mapped {
                creativity_map: vec![(0.0, 0.0), (1.0, 1.0)],
            },
        );

        Self { profiles }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_temperature_interpolation() {
        let profiles = TemperatureProfiles::default();

        // Test GPT-5.2 mapping
        assert_eq!(profiles.get_temperature("gpt-5.2", 0.0), 0.0);
        assert_eq!(profiles.get_temperature("gpt-5.2", 0.5), 0.35);
        assert_eq!(profiles.get_temperature("gpt-5.2", 1.0), 0.7);

        // Test interpolation at 0.25 (halfway between 0.0 and 0.5)
        let temp = profiles.get_temperature("gpt-5.2", 0.25);
        assert!((temp - 0.175).abs() < 0.01);
    }

    #[test]
    fn test_fixed_temperature() {
        let profiles = TemperatureProfiles::default();

        // Test reasoning models ignore creativity
        assert_eq!(profiles.get_temperature("gpt-5", 0.0), 1.0);
        assert_eq!(profiles.get_temperature("gpt-5", 0.5), 1.0);
        assert_eq!(profiles.get_temperature("gpt-5", 1.0), 1.0);
    }

    #[test]
    fn test_default_fallback() {
        let profiles = TemperatureProfiles::default();

        // Unknown model should use default profile
        assert_eq!(profiles.get_temperature("unknown-model", 0.0), 0.0);
        assert_eq!(profiles.get_temperature("unknown-model", 0.5), 0.5);
        assert_eq!(profiles.get_temperature("unknown-model", 1.0), 1.0);
    }
}
