//! Provider Factory
//! Creates LLM providers based on SessionConfig

use crate::config::{ConfigManager, SessionConfig};
use crate::llm::{ActiveProvider, LLMProvider};
use anyhow::{bail, Result};
use secrecy::ExposeSecret;

#[cfg(feature = "openai")]
use crate::llm::OpenAIProvider;

#[cfg(feature = "anthropic")]
use crate::llm::AnthropicProvider;

#[cfg(feature = "gemini")]
use crate::llm::GeminiProvider;

/// Create a provider from resolved config
pub fn create_provider(
    config: &SessionConfig,
    config_manager: &ConfigManager,
) -> Result<ActiveProvider> {
    let model = &config.provider.model;

    // Determine provider type from model name
    if model.starts_with("gpt-") || model.starts_with("o1-") || model.starts_with("o3-") {
        #[cfg(feature = "openai")]
        {
            let api_key = config_manager.get_api_key("openai")?;
            let provider = OpenAIProvider::create(model.clone(), api_key.expose_secret().clone())?;
            return Ok(ActiveProvider::OpenAI(provider));
        }

        #[cfg(not(feature = "openai"))]
        bail!("OpenAI provider not enabled. Enable 'openai' feature.");
    }

    if model.starts_with("claude-") {
        #[cfg(feature = "anthropic")]
        {
            let api_key = config_manager.get_api_key("anthropic")?;
            let provider =
                AnthropicProvider::create(model.clone(), api_key.expose_secret().clone())?;
            return Ok(ActiveProvider::Anthropic(provider));
        }

        #[cfg(not(feature = "anthropic"))]
        bail!("Anthropic provider not enabled. Enable 'anthropic' feature.");
    }

    if model.starts_with("gemini-") {
        #[cfg(feature = "gemini")]
        {
            let api_key = config_manager.get_api_key("google")?;
            let provider = GeminiProvider::create(model.clone(), api_key.expose_secret().clone())?;
            return Ok(ActiveProvider::Gemini(provider));
        }

        #[cfg(not(feature = "gemini"))]
        bail!("Gemini provider not enabled. Enable 'gemini' feature.");
    }

    bail!("Unknown model: {}. Cannot determine provider.", model)
}

/// Create a quick provider for lightweight tasks (summaries, compaction)
/// Uses the "quick" system profile which defaults to a smaller/faster model
pub fn create_quick_provider(
    config_manager: &ConfigManager,
) -> Result<Box<dyn LLMProvider>> {
    let quick_profile = config_manager.get_system_profile("quick");
    let model = &quick_profile.model;

    // Determine provider type from model name
    if model.starts_with("gpt-") || model.starts_with("o1-") || model.starts_with("o3-") {
        #[cfg(feature = "openai")]
        {
            let api_key = config_manager.get_api_key("openai")?;
            let provider = OpenAIProvider::create(model.clone(), api_key.expose_secret().clone())?;
            return Ok(Box::new(provider));
        }

        #[cfg(not(feature = "openai"))]
        bail!("OpenAI provider not enabled. Enable 'openai' feature.");
    }

    if model.starts_with("claude-") {
        #[cfg(feature = "anthropic")]
        {
            let api_key = config_manager.get_api_key("anthropic")?;
            let provider =
                AnthropicProvider::create(model.clone(), api_key.expose_secret().clone())?;
            return Ok(Box::new(provider));
        }

        #[cfg(not(feature = "anthropic"))]
        bail!("Anthropic provider not enabled. Enable 'anthropic' feature.");
    }

    if model.starts_with("gemini-") {
        #[cfg(feature = "gemini")]
        {
            let api_key = config_manager.get_api_key("google")?;
            let provider = GeminiProvider::create(model.clone(), api_key.expose_secret().clone())?;
            return Ok(Box::new(provider));
        }

        #[cfg(not(feature = "gemini"))]
        bail!("Gemini provider not enabled. Enable 'gemini' feature.");
    }

    bail!("Unknown quick model: {}. Cannot determine provider.", model)
}
