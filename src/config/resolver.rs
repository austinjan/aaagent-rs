use super::manager::ConfigManager;
use super::presets::PresetRegistry;
use super::types::{AgentConfig, ProviderConfig, SessionConfig, SessionRuntimeConfig};
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

    /// Build a default session config from a preset
    pub fn default_session_config(&self) -> Result<SessionConfig> {
        self.default_session_config_with_preset("general")
    }

    pub fn default_session_config_with_preset(&self, preset_name: &str) -> Result<SessionConfig> {
        let preset = self
            .presets
            .get(preset_name)
            .context(format!("Unknown preset: {}", preset_name))?;

        let temperature = self.config_manager.map_creativity(&preset.model, 0.5);

        Ok(SessionConfig {
            provider: ProviderConfig {
                model: preset.model.clone(),
                temperature,
                max_tokens: preset.max_tokens,
                top_p: None,
                frequency_penalty: None,
                presence_penalty: None,
                enable_grounding: None,
            },
            agent: AgentConfig {
                max_rounds: 30,
                tools_enabled: true,
            },
            session: SessionRuntimeConfig {
                system_prompt: preset.system_prompt.clone(),
                max_context_tokens: preset.max_context_tokens,
            },
        })
    }

    pub fn validate(&self, config: &SessionConfig) -> Result<()> {
        if config.provider.model.trim().is_empty() {
            bail!("provider.model must not be empty");
        }

        if !(config.provider.model.starts_with("gpt-")
            || config.provider.model.starts_with("o1-")
            || config.provider.model.starts_with("o3-")
            || config.provider.model.starts_with("claude-")
            || config.provider.model.starts_with("gemini-"))
        {
            bail!("provider.model must start with a supported prefix");
        }

        if config.provider.temperature < 0.0 {
            bail!("provider.temperature must be >= 0.0");
        }

        if config.provider.max_tokens == 0 {
            bail!("provider.max_tokens must be > 0");
        }

        if let Some(top_p) = config.provider.top_p {
            if !(0.0..=1.0).contains(&top_p) {
                bail!("provider.top_p must be between 0.0 and 1.0");
            }
        }

        if let Some(freq_penalty) = config.provider.frequency_penalty {
            if !(-2.0..=2.0).contains(&freq_penalty) {
                bail!("provider.frequency_penalty must be between -2.0 and 2.0");
            }
        }

        if let Some(pres_penalty) = config.provider.presence_penalty {
            if !(-2.0..=2.0).contains(&pres_penalty) {
                bail!("provider.presence_penalty must be between -2.0 and 2.0");
            }
        }

        if config.agent.max_rounds == 0 || config.agent.max_rounds > 100 {
            bail!("agent.max_rounds must be between 1 and 100");
        }

        if config.session.system_prompt.len() > 10000 {
            bail!("session.system_prompt must be at most 10,000 characters");
        }

        if config.session.max_context_tokens == 0 {
            bail!("session.max_context_tokens must be > 0");
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_session_config() {
        let resolver = ConfigResolver::new().unwrap();
        let config = resolver.default_session_config().unwrap();

        assert!(!config.provider.model.is_empty());
        assert!(config.provider.max_tokens > 0);
        assert!(config.session.max_context_tokens > 0);
    }

    #[test]
    fn test_validate_rejects_empty_model() {
        let resolver = ConfigResolver::new().unwrap();
        let mut config = resolver.default_session_config().unwrap();
        config.provider.model = "".to_string();

        assert!(resolver.validate(&config).is_err());
    }
}
