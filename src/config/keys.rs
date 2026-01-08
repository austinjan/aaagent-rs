use anyhow::{bail, Context, Result};
use secrecy::{ExposeSecret, Secret};
use serde::{Deserialize, Serialize};
use std::fs;

/// Type alias for secret API keys
pub type SecretApiKey = Secret<String>;

/// API key reference - where to find the actual key
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum KeySource {
    /// Read from environment variable
    Env { env: String },
    /// Read from file
    File { file: String },
}

/// API key references in config.yaml (NO actual keys)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ApiKeyReferences {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub openai: Option<KeySource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anthropic: Option<KeySource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub google: Option<KeySource>,
}

/// Actual API keys loaded from secrets.yaml (DANGEROUS - only for local dev)
#[derive(Clone, Serialize, Deserialize)]
pub struct SecretsFile {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub openai: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anthropic: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub google: Option<String>,
}

/// Debug implementation that NEVER exposes actual keys
impl std::fmt::Debug for SecretsFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SecretsFile")
            .field("openai", &self.openai.as_ref().map(|_| "[REDACTED]"))
            .field("anthropic", &self.anthropic.as_ref().map(|_| "[REDACTED]"))
            .field("google", &self.google.as_ref().map(|_| "[REDACTED]"))
            .finish()
    }
}

/// Load API key with fallback priority
pub fn load_api_key(
    provider: &str,
    reference: Option<&KeySource>,
    secrets_file: Option<&SecretsFile>,
) -> Result<SecretApiKey> {
    // Priority 1: Reference from config.yaml (env or file)
    if let Some(source) = reference {
        match source {
            KeySource::Env { env } => {
                if let Ok(key) = std::env::var(env) {
                    return validate_and_wrap(provider, &key);
                }
                // Env var specified but not set - this is an error
                bail!(
                    "Environment variable '{}' specified in config.yaml but not set",
                    env
                );
            }
            KeySource::File { file } => {
                let path = shellexpand::tilde(file).to_string();
                let key = fs::read_to_string(&path).context(format!(
                    "Failed to read API key from file: {}",
                    path
                ))?;
                return validate_and_wrap(provider, key.trim());
            }
        }
    }

    // Priority 2: Default environment variable
    let default_env_var = format!("{}_API_KEY", provider.to_uppercase());
    if let Ok(key) = std::env::var(&default_env_var) {
        return validate_and_wrap(provider, &key);
    }

    // Priority 3: secrets.yaml (if allowed)
    if let Some(secrets) = secrets_file {
        let key_opt = match provider {
            "openai" => secrets.openai.as_deref(),
            "anthropic" => secrets.anthropic.as_deref(),
            "google" => secrets.google.as_deref(),
            _ => None,
        };

        if let Some(key) = key_opt {
            return validate_and_wrap(provider, key);
        }
    }

    // Priority 4: Error - no key found
    bail!(
        "No API key configured for provider '{}'. \
        Set {}_API_KEY environment variable or configure in config.yaml with 'env' or 'file' reference",
        provider,
        provider.to_uppercase()
    )
}

/// Validate API key and wrap in Secret
fn validate_and_wrap(provider: &str, key: &str) -> Result<SecretApiKey> {
    let trimmed = key.trim();

    // Only check for obvious errors (not strict format validation)
    if trimmed.is_empty() {
        bail!("API key for '{}' is empty or whitespace-only", provider);
    }

    if trimmed.len() < 10 {
        bail!(
            "API key for '{}' seems too short (< 10 characters)",
            provider
        );
    }

    // Check for common mistakes
    if trimmed.contains(' ') {
        log::warn!(
            "API key for '{}' contains spaces - this might be a copy-paste error",
            provider
        );
    }

    if trimmed.starts_with("sk-...") || trimmed.starts_with("sk-ant-...") {
        bail!(
            "API key for '{}' looks like a placeholder (sk-...)",
            provider
        );
    }

    // Soft warnings for format hints (not errors)
    match provider {
        "openai" if !trimmed.starts_with("sk-") => {
            log::warn!(
                "OpenAI API keys typically start with 'sk-', but this will not be enforced. \
                The key will be validated when making the first API request."
            );
        }
        "anthropic" if !trimmed.starts_with("sk-ant-") => {
            log::warn!(
                "Anthropic API keys typically start with 'sk-ant-', but this will not be enforced. \
                The key will be validated when making the first API request."
            );
        }
        _ => {}
    }

    Ok(Secret::new(trimmed.to_string()))
}

/// Map model name to provider name
pub fn get_provider_for_model(model: &str) -> &str {
    if model.starts_with("gpt-") {
        "openai"
    } else if model.starts_with("claude-") {
        "anthropic"
    } else if model.starts_with("gemini-") {
        "google"
    } else {
        "openai" // default
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_empty_key() {
        let result = validate_and_wrap("openai", "");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("empty"));
    }

    #[test]
    fn test_validate_too_short() {
        let result = validate_and_wrap("openai", "sk-123");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("too short"));
    }

    #[test]
    fn test_validate_placeholder() {
        let result = validate_and_wrap("openai", "sk-...1234567890");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("placeholder"));
    }

    #[test]
    fn test_validate_valid_key() {
        let result = validate_and_wrap("openai", "sk-proj-1234567890abcdef");
        assert!(result.is_ok());

        let secret = result.unwrap();
        assert_eq!(secret.expose_secret(), "sk-proj-1234567890abcdef");
    }

    #[test]
    fn test_secret_not_exposed_in_debug() {
        let secret = Secret::new("sk-secret-key-12345".to_string());
        let debug_output = format!("{:?}", secret);
        assert!(!debug_output.contains("sk-secret"));
        assert!(debug_output.contains("Secret"));
    }

    #[test]
    fn test_secrets_file_debug() {
        let secrets = SecretsFile {
            openai: Some("sk-secret-123".to_string()),
            anthropic: Some("sk-ant-secret-456".to_string()),
            google: None,
        };

        let debug_output = format!("{:?}", secrets);
        assert!(!debug_output.contains("sk-secret"));
        assert!(!debug_output.contains("sk-ant-secret"));
        assert!(debug_output.contains("[REDACTED]"));
    }

    #[test]
    fn test_get_provider_for_model() {
        assert_eq!(get_provider_for_model("gpt-5-mini"), "openai");
        assert_eq!(get_provider_for_model("gpt-4o"), "openai");
        assert_eq!(get_provider_for_model("claude-3-5-sonnet"), "anthropic");
        assert_eq!(get_provider_for_model("gemini-3-flash"), "google");
        assert_eq!(get_provider_for_model("unknown-model"), "openai");
    }
}
