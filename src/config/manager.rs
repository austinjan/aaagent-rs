use super::keys::{load_api_key, ApiKeyReferences, SecretApiKey, SecretsFile};
use super::types::TemperatureProfiles;
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Generate a well-documented default config.yaml for LLM readability
fn generate_default_config_yaml() -> String {
    r#"# aaagent-rs Configuration File
# This file configures API keys and model-specific temperature profiles.
# It is designed to be LLM-readable for easy modification without external documentation.

# =============================================================================
# API KEYS CONFIGURATION
# =============================================================================
# This section defines how to load API keys for different LLM providers.
#
# SECURITY BEST PRACTICES:
# - NEVER put actual API keys directly in this file
# - Use environment variables (recommended) or secrets.yaml (local dev only)
# - This file is safe to commit to version control
#
# Three ways to provide API keys (in order of precedence):
# 1. Environment variables: OPENAI_API_KEY, ANTHROPIC_API_KEY, GEMINI_API_KEY
# 2. secrets.yaml file: api_keys.openai, api_keys.anthropic, api_keys.google
# 3. Direct reference (UNSAFE - only for testing): key: "sk-..."
#
# Recommended configuration (uses environment variables):
api_keys:
  openai:
    env: OPENAI_API_KEY
  anthropic:
    env: ANTHROPIC_API_KEY
  google:
    env: GEMINI_API_KEY

# Alternative: Use secrets.yaml (local development only)
# api_keys:
#   openai:
#     file: secrets.yaml
#   anthropic:
#     file: secrets.yaml
#   google:
#     file: secrets.yaml

# =============================================================================
# TEMPERATURE PROFILES
# =============================================================================
# Temperature profiles map creativity intent (0.0-1.0) to model-specific temperatures.
# Different models have different "sweet spots" for temperature ranges.
#
# How it works:
# - User specifies "creativity" as 0.0 (focused) to 1.0 (creative)
# - System maps creativity to appropriate temperature for each model
# - Some models (like GPT-5) have fixed temperatures that ignore creativity
#
# Profile types:
# 1. Fixed temperature (ignore_creativity: true)
#    - Always uses the same temperature regardless of creativity setting
#    - Used for models that work best at a specific temperature
#
# 2. Linear mapping (min/max range)
#    - Maps creativity linearly: creativity 0.0 → min, 1.0 → max
#    - Most common approach for standard models
#
# 3. Exponential mapping (exponent parameter)
#    - Maps creativity exponentially for finer control at low values
#    - formula: min + (max - min) * (creativity ^ exponent)
#    - exponent > 1: More conservative (slower ramp-up)
#    - exponent < 1: More aggressive (faster ramp-up)

temperature_profiles:
  profiles:
    # Default profile for unknown models (fallback)
    # Uses standard 0.0-1.0 linear mapping
    default:
      creativity_map:
        - [0.0, 0.0]
        - [1.0, 1.0]

    # GPT-5 (OpenAI): Fixed temperature
    # Always runs at 1.0 for optimal performance
    gpt-5:
      fixed: 1.0
      ignore_creativity: true

    # GPT-5-mini: Reasoning model with fixed temperature
    gpt-5-mini:
      fixed: 1.0
      ignore_creativity: true

    # GPT-5.2 (OpenAI): Conservative mapping
    # Creativity curve keeps temperature lower for balanced responses
    # - creativity 0.0 → temperature 0.0
    # - creativity 0.5 → temperature 0.35
    # - creativity 1.0 → temperature 0.7
    gpt-5.2:
      creativity_map:
        - [0.0, 0.0]
        - [0.5, 0.35]
        - [1.0, 0.7]

    # Claude models (Anthropic): Standard linear mapping
    # - creativity 0.0 → temperature 0.0
    # - creativity 0.5 → temperature 0.5
    # - creativity 1.0 → temperature 1.0
    claude-3-5-sonnet-20241022:
      creativity_map:
        - [0.0, 0.0]
        - [1.0, 1.0]

    claude-3-5-haiku-20241022:
      creativity_map:
        - [0.0, 0.0]
        - [1.0, 1.0]

    # Gemini models (Google): Linear mapping with cap at 2.0
    # Gemini supports higher temperatures than most models
    gemini-2.0-flash-exp:
      creativity_map:
        - [0.0, 0.0]
        - [1.0, 2.0]

# =============================================================================
# USAGE EXAMPLES FOR LLMs
# =============================================================================
#
# Adding a new model with linear temperature mapping (0.0-1.5 range):
# ```yaml
# temperature_profiles:
#   profiles:
#     my-new-model:
#       creativity_map:
#         - [0.0, 0.0]
#         - [1.0, 1.5]
# ```
#
# Adding a model with custom curve (3 control points):
# ```yaml
# temperature_profiles:
#   profiles:
#     custom-model:
#       creativity_map:
#         - [0.0, 0.0]    # Low creativity → low temperature
#         - [0.5, 0.3]    # Medium creativity → moderate temperature
#         - [1.0, 1.2]    # High creativity → high temperature
# ```
#
# Setting a fixed temperature for a reasoning model:
# ```yaml
# temperature_profiles:
#   profiles:
#     reasoning-model:
#       fixed: 1.0
#       ignore_creativity: true
# ```
#
# Changing default fallback profile:
# ```yaml
# temperature_profiles:
#   profiles:
#     default:
#       creativity_map:
#         - [0.0, 0.2]  # Never go below 0.2
#         - [1.0, 0.9]  # Never go above 0.9
# ```
#
# =============================================================================
# SYSTEM LLM PROFILES (Internal Use)
# =============================================================================
# These profiles are used by the system for automated internal tasks.
# They are separate from user-facing presets to ensure consistent behavior.
#
# Use cases:
# - Auto-compact: Compress long tool results automatically
# - Auto-summary: Summarize conversation segments
# - Background tasks: System maintenance, cleanup
#
# Why separate from presets?
# - User presets may be customized in unpredictable ways
# - System tasks need consistent, reliable behavior
# - Tests need deterministic results regardless of user config
#
# Hardcoded defaults (used when not specified):
# - default: gpt-5-mini (16K tokens) - General system tasks
# - quick: gpt-5-nano (4K tokens) - Lightweight tasks (compact, summary)

system_llm_profiles:
  # Standard profile for general system tasks
  default:
    model: gpt-5-mini
    temperature: 1.0
    max_tokens: 16384

  # Quick profile for lightweight tasks (compact, summary, format)
  quick:
    model: gpt-5-nano
    temperature: 1.0
    max_tokens: 4096

# =============================================================================
# MAINTENANCE CONFIGURATION
# =============================================================================
# Automatic maintenance and cleanup tasks
# - Temp files: Cleans old temporary files created by tool outputs
# - Future tasks: Log rotation, orphaned stream cleanup, etc.

maintenance:
  enabled: true           # Enable/disable maintenance worker
  interval_hours: 6       # Run cleanup every 6 hours

  tasks:
    temp_files:
      enabled: true       # Enable temp file cleanup
      retention_hours: 6  # Delete files older than 6 hours

# =============================================================================
# RELOAD BEHAVIOR
# =============================================================================
# This configuration file can be hot-reloaded at runtime.
# Changes to temperature profiles take effect immediately for new requests.
# API key changes require application restart for security reasons.
"#
    .to_string()
}

/// Full configuration file structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigFile {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_keys: Option<ApiKeyReferences>,
    pub temperature_profiles: TemperatureProfiles,

    /// System LLM profiles for internal tasks (auto-compact, etc.)
    /// Defaults to gpt-5-mini (standard) and gpt-5-nano (quick) if not specified
    #[serde(default)]
    pub system_llm_profiles: super::system_profiles::SystemLLMProfiles,

    /// Maintenance configuration for periodic cleanup tasks
    #[serde(default)]
    pub maintenance: crate::maintenance::MaintenanceConfig,
}

pub struct ConfigManager {
    config_path: PathBuf,
    secrets_path: PathBuf,
    config: ConfigFile,
    secrets: Option<SecretsFile>,
    #[allow(dead_code)]
    allow_secrets_file: bool,
}

impl ConfigManager {
    pub fn new() -> Result<Self> {
        Self::with_paths("config.yaml", "secrets.yaml", false)
    }

    pub fn with_allow_secrets() -> Result<Self> {
        Self::with_paths("config.yaml", "secrets.yaml", true)
    }

    pub fn with_paths(
        config_path: &str,
        secrets_path: &str,
        allow_secrets_file: bool,
    ) -> Result<Self> {
        let config_path = PathBuf::from(config_path);
        let secrets_path = PathBuf::from(secrets_path);

        // Load or create config.yaml
        let config = if config_path.exists() {
            let content = fs::read_to_string(&config_path).context(format!(
                "Failed to read config file: {}",
                config_path.display()
            ))?;
            serde_yaml::from_str(&content).context("Failed to parse config.yaml")?
        } else {
            // Create default config with comprehensive documentation for LLMs
            let yaml = generate_default_config_yaml();

            fs::write(&config_path, yaml).context(format!(
                "Failed to write default config to {}",
                config_path.display()
            ))?;

            println!("Created default config.yaml at {}", config_path.display());

            // Parse the generated YAML to return as ConfigFile
            serde_yaml::from_str(&generate_default_config_yaml())
                .context("Failed to parse generated default config")?
        };

        // Load secrets.yaml if it exists
        let secrets = if secrets_path.exists() {
            if !allow_secrets_file {
                // Check if we're in production mode
                if cfg!(not(debug_assertions)) {
                    bail!(
                        "secrets.yaml detected in production mode! \n\
                        This is a security risk. Use environment variables instead.\n\
                        If you must use secrets.yaml, run with --allow-secrets-file flag."
                    );
                }

                // Warn in development mode
                eprintln!("\n⚠️  WARNING: secrets.yaml detected!");
                eprintln!("⚠️  This file contains API keys and should ONLY be used locally.");
                eprintln!("⚠️  Production deployments MUST use environment variables.");
                eprintln!("⚠️  File location: {}", secrets_path.display());
                eprintln!("⚠️  Press Enter to continue, Ctrl+C to abort...\n");

                let mut input = String::new();
                std::io::stdin().read_line(&mut input).ok();
            }

            // Check file permissions on Unix
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let metadata = fs::metadata(&secrets_path)?;
                let mode = metadata.permissions().mode();

                if mode & 0o077 != 0 {
                    eprintln!(
                        "⚠️  WARNING: secrets.yaml has permissive permissions ({:o}).",
                        mode & 0o777
                    );
                    eprintln!("⚠️  Recommended: chmod 600 secrets.yaml\n");
                }
            }

            let content = fs::read_to_string(&secrets_path).context(format!(
                "Failed to read secrets file: {}",
                secrets_path.display()
            ))?;
            let secrets: SecretsFile =
                serde_yaml::from_str(&content).context("Failed to parse secrets.yaml")?;
            Some(secrets)
        } else {
            None
        };

        Ok(Self {
            config_path,
            secrets_path,
            config,
            secrets,
            allow_secrets_file,
        })
    }

    /// Get API key for a provider with fallback chain
    pub fn get_api_key(&self, provider: &str) -> Result<SecretApiKey> {
        let key_ref = self
            .config
            .api_keys
            .as_ref()
            .and_then(|keys| match provider {
                "openai" => keys.openai.as_ref(),
                "anthropic" => keys.anthropic.as_ref(),
                "google" => keys.google.as_ref(),
                _ => None,
            });

        load_api_key(provider, key_ref, self.secrets.as_ref())
    }

    pub fn map_creativity(&self, model: &str, creativity: f32) -> f32 {
        self.config
            .temperature_profiles
            .get_temperature(model, creativity)
    }

    pub fn reload(&mut self) -> Result<()> {
        if !self.config_path.exists() {
            return Ok(());
        }

        let content =
            fs::read_to_string(&self.config_path).context("Failed to read config file")?;
        let config: ConfigFile =
            serde_yaml::from_str(&content).context("Failed to parse config.yaml")?;

        self.config = config;

        // Reload secrets if it exists
        if self.secrets_path.exists() {
            let content =
                fs::read_to_string(&self.secrets_path).context("Failed to read secrets file")?;
            let secrets: SecretsFile =
                serde_yaml::from_str(&content).context("Failed to parse secrets.yaml")?;
            self.secrets = Some(secrets);
        }

        Ok(())
    }

    pub fn config_path(&self) -> &PathBuf {
        &self.config_path
    }

    /// Get system LLM profile by name
    pub fn get_system_profile(&self, name: &str) -> &super::system_profiles::SystemLLMProfile {
        self.config.system_llm_profiles.get(name)
    }

    /// Get all system LLM profiles
    pub fn system_profiles(&self) -> &super::system_profiles::SystemLLMProfiles {
        &self.config.system_llm_profiles
    }

    /// Get maintenance configuration
    pub fn maintenance_config(&self) -> &crate::maintenance::MaintenanceConfig {
        &self.config.maintenance
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::SystemLLMProfiles;
    use secrecy::ExposeSecret;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_new_creates_default_config() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.yaml");
        let secrets_path = temp_dir.path().join("secrets.yaml");

        let manager = ConfigManager::with_paths(
            config_path.to_str().unwrap(),
            secrets_path.to_str().unwrap(),
            false,
        )
        .unwrap();

        // Config file should be created
        assert!(config_path.exists());

        // Should have default temperature profiles
        assert_eq!(manager.map_creativity("gpt-5", 0.5), 1.0);
        assert_eq!(manager.map_creativity("gpt-5.2", 0.5), 0.35);
    }

    #[test]
    fn test_loads_existing_config() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.yaml");
        let secrets_path = temp_dir.path().join("secrets.yaml");

        // Create a custom config
        let custom_config = ConfigFile {
            api_keys: None,
            temperature_profiles: TemperatureProfiles::default(),
            system_llm_profiles: SystemLLMProfiles::default(),
            maintenance: crate::maintenance::MaintenanceConfig::default(),
        };
        let yaml = serde_yaml::to_string(&custom_config).unwrap();
        fs::write(&config_path, yaml).unwrap();

        let manager = ConfigManager::with_paths(
            config_path.to_str().unwrap(),
            secrets_path.to_str().unwrap(),
            false,
        )
        .unwrap();

        // Should load the config
        assert!(manager.config.api_keys.is_none());
    }

    #[test]
    fn test_api_key_from_env() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.yaml");
        let secrets_path = temp_dir.path().join("secrets.yaml");

        // Set environment variable
        std::env::set_var("OPENAI_API_KEY", "sk-test-key-1234567890");

        let manager = ConfigManager::with_paths(
            config_path.to_str().unwrap(),
            secrets_path.to_str().unwrap(),
            false,
        )
        .unwrap();

        // Should load from environment
        let key = manager.get_api_key("openai").unwrap();
        assert_eq!(key.expose_secret(), "sk-test-key-1234567890");

        // Cleanup
        std::env::remove_var("OPENAI_API_KEY");
    }

    #[test]
    fn test_reload() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.yaml");
        let secrets_path = temp_dir.path().join("secrets.yaml");

        let mut manager = ConfigManager::with_paths(
            config_path.to_str().unwrap(),
            secrets_path.to_str().unwrap(),
            false,
        )
        .unwrap();

        // Initial temperature
        let initial = manager.map_creativity("gpt-5.2", 0.5);
        assert_eq!(initial, 0.35);

        // Reload should work
        manager.reload().unwrap();
        let reloaded = manager.map_creativity("gpt-5.2", 0.5);
        assert_eq!(reloaded, 0.35);
    }
}
