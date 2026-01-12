use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Preset {
    pub system_prompt: String,
    pub model: String,
    pub temperature: f32,
    pub max_tokens: u32,
    pub max_rounds: u32,
    pub max_context_tokens: u32,
    pub compression: String,
    pub tools_enabled: bool,
}

pub struct PresetRegistry {
    presets: HashMap<String, Preset>,
}

impl PresetRegistry {
    pub fn new() -> Self {
        let mut presets = HashMap::new();

        // General preset (default)
        presets.insert(
            "general".to_string(),
            Preset {
                system_prompt: "You are a helpful, friendly assistant.".to_string(),
                model: "gpt-5-mini".to_string(),
                temperature: 1.0,
                max_tokens: 16384,
                max_rounds: 30,
                max_context_tokens: 200000,
                compression: "balanced".to_string(),
                tools_enabled: true,
            },
        );

        // Coding preset
        presets.insert(
            "coding".to_string(),
            Preset {
                system_prompt: r#"You are an expert software engineer with deep knowledge of multiple programming languages.
- Write clean, well-documented code following best practices
- Consider performance, security, and maintainability
- Use tools to read/write files when needed
- Explain complex concepts clearly with examples"#
                    .to_string(),
                model: "gpt-5-mini".to_string(),
                temperature: 1.0,
                max_tokens: 32768,
                max_rounds: 40,
                max_context_tokens: 300000,
                compression: "minimal".to_string(),
                tools_enabled: true,
            },
        );

        // Research preset
        presets.insert(
            "research".to_string(),
            Preset {
                system_prompt:
                    r#"You are a thorough research assistant specializing in systematic analysis.
- Break down complex problems into clear components
- Use tools to search and analyze information
- Provide evidence-based reasoning with citations when possible
- Consider multiple perspectives and trade-offs"#
                        .to_string(),
                model: "gpt-5-mini".to_string(),
                temperature: 1.0,
                max_tokens: 32768,
                max_rounds: 50,
                max_context_tokens: 400000,
                compression: "minimal".to_string(),
                tools_enabled: true,
            },
        );

        // Quick preset
        presets.insert(
            "quick".to_string(),
            Preset {
                system_prompt: "You are a concise, efficient assistant focused on quick answers."
                    .to_string(),
                model: "gpt-5-nano".to_string(),
                temperature: 1.0,
                max_tokens: 8192,
                max_rounds: 15,
                max_context_tokens: 150000,
                compression: "aggressive".to_string(),
                tools_enabled: true,
            },
        );

        Self { presets }
    }

    pub fn get(&self, name: &str) -> Option<&Preset> {
        self.presets.get(name)
    }

    pub fn exists(&self, name: &str) -> bool {
        self.presets.contains_key(name)
    }

    pub fn list(&self) -> Vec<&str> {
        self.presets.keys().map(|s| s.as_str()).collect()
    }
}

impl Default for PresetRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_presets_exist() {
        let registry = PresetRegistry::new();
        assert!(registry.exists("general"));
        assert!(registry.exists("coding"));
        assert!(registry.exists("research"));
        assert!(registry.exists("quick"));
    }

    #[test]
    fn test_preset_system_prompts() {
        let registry = PresetRegistry::new();

        let general = registry.get("general").unwrap();
        assert!(!general.system_prompt.is_empty());

        let coding = registry.get("coding").unwrap();
        assert!(coding.system_prompt.contains("software engineer"));

        let research = registry.get("research").unwrap();
        assert!(research.system_prompt.contains("research assistant"));

        let quick = registry.get("quick").unwrap();
        assert!(quick.system_prompt.contains("concise"));
    }

    #[test]
    fn test_preset_parameters() {
        let registry = PresetRegistry::new();

        // General should be balanced
        let general = registry.get("general").unwrap();
        assert_eq!(general.max_rounds, 30);
        assert_eq!(general.max_tokens, 16384);

        // Coding should allow longer responses
        let coding = registry.get("coding").unwrap();
        assert_eq!(coding.max_tokens, 32768);
        assert_eq!(coding.max_rounds, 40);

        // Quick should be minimal
        let quick = registry.get("quick").unwrap();
        assert_eq!(quick.max_rounds, 15);
        assert_eq!(quick.max_tokens, 8192);
        assert_eq!(quick.model, "gpt-5-nano");
    }
}
