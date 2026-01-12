use super::manager::ConfigManager;
use super::presets::{Preset, PresetRegistry};
use super::types::*;
use anyhow::{bail, Context, Result};

pub struct ConfigResolver {
    presets: PresetRegistry,
    config_manager: ConfigManager,
}

impl ConfigResolver {
    pub fn new() -> Result<Self> {
        Ok(Self {
            presets: PresetRegistry::new(),
            config_manager: ConfigManager::new()?,
        })
    }

    /// Get a reference to the config manager
    pub fn config_manager(&self) -> &ConfigManager {
        &self.config_manager
    }

    /// Resolve a chat config into final runtime configuration
    pub fn resolve(&self, config: &ChatConfig) -> Result<ResolvedConfig> {
        // 1. Validate config
        self.validate(config)?;

        // 2. Load preset defaults
        let preset = self
            .presets
            .get(&config.preset)
            .context(format!("Unknown preset: {}", config.preset))?;

        // 3. Build provider config
        let provider = self.build_provider_config(config, preset)?;

        // 4. Build agent config
        let agent = self.build_agent_config(config, preset);

        // 5. Build session config
        let session = self.build_session_config(config, preset);

        Ok(ResolvedConfig {
            provider,
            agent,
            session,
        })
    }

    fn validate(&self, config: &ChatConfig) -> Result<()> {
        // Validate preset exists
        if !self.presets.exists(&config.preset) {
            bail!(
                "Invalid preset '{}'. Available: {:?}",
                config.preset,
                self.presets.list()
            );
        }

        // Validate creativity range
        if config.intent.creativity < 0.0 || config.intent.creativity > 1.0 {
            bail!(
                "creativity must be between 0.0 and 1.0, got {}",
                config.intent.creativity
            );
        }

        // Validate verbosity
        if !matches!(
            config.intent.verbosity.as_str(),
            "short" | "normal" | "long"
        ) {
            bail!(
                "verbosity must be 'short', 'normal', or 'long', got '{}'",
                config.intent.verbosity
            );
        }

        // Validate rounds
        if config.intent.rounds == 0 || config.intent.rounds > 100 {
            bail!(
                "rounds must be between 1 and 100, got {}",
                config.intent.rounds
            );
        }

        // Validate overrides if present
        if let Some(overrides) = &config.overrides {
            if let Some(top_p) = overrides.top_p {
                if !(0.0..=1.0).contains(&top_p) {
                    bail!("top_p must be between 0.0 and 1.0, got {}", top_p);
                }
            }

            if let Some(freq_penalty) = overrides.frequency_penalty {
                if !(-2.0..=2.0).contains(&freq_penalty) {
                    bail!(
                        "frequency_penalty must be between -2.0 and 2.0, got {}",
                        freq_penalty
                    );
                }
            }

            if let Some(pres_penalty) = overrides.presence_penalty {
                if !(-2.0..=2.0).contains(&pres_penalty) {
                    bail!(
                        "presence_penalty must be between -2.0 and 2.0, got {}",
                        pres_penalty
                    );
                }
            }

            // Validate model if specified
            if let Some(model) = &overrides.model {
                let valid_models = [
                    "gpt-5",
                    "gpt-5-mini",
                    "gpt-5-nano",
                    "gpt-5.2",
                    "gemini-3-flash-preview",
                    "gemini-3-pro-preview",
                ];
                if !valid_models.contains(&model.as_str()) {
                    bail!("Invalid model '{}'. Allowed: {:?}", model, valid_models);
                }
            }
        }

        // Validate system prompt length if provided
        if let Some(prompt) = &config.system_prompt {
            if prompt.len() > 10000 {
                bail!(
                    "system_prompt must be at most 10,000 characters, got {}",
                    prompt.len()
                );
            }
        }

        Ok(())
    }

    fn build_provider_config(
        &self,
        config: &ChatConfig,
        preset: &Preset,
    ) -> Result<ProviderConfig> {
        // Determine model (override or preset default)
        let model = config
            .overrides
            .as_ref()
            .and_then(|o| o.model.clone())
            .unwrap_or_else(|| preset.model.clone());

        // Map creativity to temperature
        let temperature = self
            .config_manager
            .map_creativity(&model, config.intent.creativity);

        // Map verbosity to max_tokens
        let max_tokens = match config.intent.verbosity.as_str() {
            "short" => 8192,
            "normal" => 16384,
            "long" => 32768,
            _ => preset.max_tokens, // Fallback (validation should catch this)
        };

        Ok(ProviderConfig {
            model,
            temperature,
            max_tokens,
            top_p: config.overrides.as_ref().and_then(|o| o.top_p),
            frequency_penalty: config.overrides.as_ref().and_then(|o| o.frequency_penalty),
            presence_penalty: config.overrides.as_ref().and_then(|o| o.presence_penalty),
        })
    }

    fn build_agent_config(&self, config: &ChatConfig, _preset: &Preset) -> AgentConfig {
        AgentConfig {
            max_rounds: config.intent.rounds,
            tools_enabled: config.tools_enabled,
        }
    }

    fn build_session_config(&self, config: &ChatConfig, preset: &Preset) -> SessionConfig {
        SessionConfig {
            system_prompt: config
                .system_prompt
                .clone()
                .unwrap_or_else(|| preset.system_prompt.clone()),
            max_context_tokens: preset.max_context_tokens,
        }
    }

    /// Validate that system_prompt is not being changed (for update requests)
    pub fn validate_immutable_fields(
        &self,
        config: &ChatConfig,
        existing: &ResolvedConfig,
    ) -> Result<()> {
        if let Some(new_prompt) = &config.system_prompt {
            if new_prompt != &existing.session.system_prompt {
                bail!(
                    "system_prompt is immutable. Create a new session to use a different prompt."
                );
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_config() -> ChatConfig {
        ChatConfig {
            preset: "general".to_string(),
            system_prompt: None,
            tools_enabled: true,
            intent: ChatIntent {
                creativity: 0.5,
                verbosity: "normal".to_string(),
                rounds: 30,
            },
            overrides: None,
        }
    }

    #[test]
    fn test_resolve_basic_config() {
        let resolver = ConfigResolver::new().unwrap();
        let config = create_test_config();
        let resolved = resolver.resolve(&config).unwrap();

        assert_eq!(resolved.provider.model, "gpt-5-mini");
        assert_eq!(resolved.provider.max_tokens, 16384);
        assert_eq!(resolved.agent.max_rounds, 30);
        assert!(resolved.agent.tools_enabled);
        assert!(!resolved.session.system_prompt.is_empty());
    }

    #[test]
    fn test_custom_system_prompt() {
        let resolver = ConfigResolver::new().unwrap();
        let mut config = create_test_config();
        config.system_prompt = Some("Custom prompt".to_string());

        let resolved = resolver.resolve(&config).unwrap();
        assert_eq!(resolved.session.system_prompt, "Custom prompt");
    }

    #[test]
    fn test_verbosity_mapping() {
        let resolver = ConfigResolver::new().unwrap();

        let mut config = create_test_config();
        config.intent.verbosity = "short".to_string();
        let resolved = resolver.resolve(&config).unwrap();
        assert_eq!(resolved.provider.max_tokens, 8192);

        config.intent.verbosity = "normal".to_string();
        let resolved = resolver.resolve(&config).unwrap();
        assert_eq!(resolved.provider.max_tokens, 16384);

        config.intent.verbosity = "long".to_string();
        let resolved = resolver.resolve(&config).unwrap();
        assert_eq!(resolved.provider.max_tokens, 32768);
    }

    #[test]
    fn test_model_override() {
        let resolver = ConfigResolver::new().unwrap();
        let mut config = create_test_config();
        config.overrides = Some(ChatOverrides {
            model: Some("gpt-5.2".to_string()),
            top_p: None,
            frequency_penalty: None,
            presence_penalty: None,
        });

        let resolved = resolver.resolve(&config).unwrap();
        assert_eq!(resolved.provider.model, "gpt-5.2");
    }

    #[test]
    fn test_temperature_mapping() {
        let resolver = ConfigResolver::new().unwrap();

        // GPT-5 should always use temperature 1.0
        let mut config = create_test_config();
        config.overrides = Some(ChatOverrides {
            model: Some("gpt-5".to_string()),
            top_p: None,
            frequency_penalty: None,
            presence_penalty: None,
        });
        config.intent.creativity = 0.0;
        let resolved = resolver.resolve(&config).unwrap();
        assert_eq!(resolved.provider.temperature, 1.0);

        // GPT-5.2 should map creativity
        config.overrides.as_mut().unwrap().model = Some("gpt-5.2".to_string());
        config.intent.creativity = 0.5;
        let resolved = resolver.resolve(&config).unwrap();
        assert_eq!(resolved.provider.temperature, 0.35);
    }

    #[test]
    fn test_validation_errors() {
        let resolver = ConfigResolver::new().unwrap();

        // Invalid preset
        let mut config = create_test_config();
        config.preset = "invalid".to_string();
        assert!(resolver.resolve(&config).is_err());

        // Invalid creativity
        config = create_test_config();
        config.intent.creativity = 1.5;
        assert!(resolver.resolve(&config).is_err());

        // Invalid verbosity
        config = create_test_config();
        config.intent.verbosity = "invalid".to_string();
        assert!(resolver.resolve(&config).is_err());

        // Invalid rounds
        config = create_test_config();
        config.intent.rounds = 0;
        assert!(resolver.resolve(&config).is_err());

        config = create_test_config();
        config.intent.rounds = 150;
        assert!(resolver.resolve(&config).is_err());
    }

    #[test]
    fn test_immutable_system_prompt() {
        let resolver = ConfigResolver::new().unwrap();
        let existing = ResolvedConfig {
            provider: ProviderConfig {
                model: "gpt-5-mini".to_string(),
                temperature: 1.0,
                max_tokens: 16384,
                top_p: None,
                frequency_penalty: None,
                presence_penalty: None,
            },
            agent: AgentConfig {
                max_rounds: 30,
                tools_enabled: true,
            },
            session: SessionConfig {
                system_prompt: "Original prompt".to_string(),
                max_context_tokens: 200000,
            },
        };

        let mut config = create_test_config();
        config.system_prompt = Some("Different prompt".to_string());

        let result = resolver.validate_immutable_fields(&config, &existing);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("immutable"));
    }

    #[test]
    fn test_different_presets() {
        let resolver = ConfigResolver::new().unwrap();

        // Coding preset
        let mut config = create_test_config();
        config.preset = "coding".to_string();
        let resolved = resolver.resolve(&config).unwrap();
        assert!(resolved.session.system_prompt.contains("software engineer"));

        // Research preset
        config.preset = "research".to_string();
        let resolved = resolver.resolve(&config).unwrap();
        assert!(resolved
            .session
            .system_prompt
            .contains("research assistant"));

        // Quick preset
        config.preset = "quick".to_string();
        let resolved = resolver.resolve(&config).unwrap();
        assert!(resolved.session.system_prompt.contains("concise"));
    }
}
