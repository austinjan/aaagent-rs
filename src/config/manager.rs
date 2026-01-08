use super::keys::{load_api_key, ApiKeyReferences, SecretApiKey, SecretsFile};
use super::types::TemperatureProfiles;
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Full configuration file structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigFile {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_keys: Option<ApiKeyReferences>,
    pub temperature_profiles: TemperatureProfiles,
}

pub struct ConfigManager {
    config_path: PathBuf,
    secrets_path: PathBuf,
    config: ConfigFile,
    secrets: Option<SecretsFile>,
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
            // Create default config (with key references, not actual keys)
            let default_config = ConfigFile {
                api_keys: Some(ApiKeyReferences::default()),
                temperature_profiles: TemperatureProfiles::default(),
            };

            let yaml =
                serde_yaml::to_string(&default_config).context("Failed to serialize default config")?;

            fs::write(&config_path, yaml).context(format!(
                "Failed to write default config to {}",
                config_path.display()
            ))?;

            println!("Created default config.yaml at {}", config_path.display());
            default_config
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
        let key_ref = self.config.api_keys.as_ref().and_then(|keys| match provider {
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
}

#[cfg(test)]
mod tests {
    use super::*;
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
