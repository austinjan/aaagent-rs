# API Key Management Plan

- Feature name: `api-key-management`
- Status: Draft
- Created: 2026-01-08
- Parent plan: [chat-ui-config.md](./chat-ui-config.md)

## 1) Overview

### Goal
Provide secure, flexible API key management supporting multiple providers (OpenAI, Anthropic, Gemini) with fallback mechanisms and protection against accidental exposure.

### Design Principles

1. **Security first** - Never log or expose keys
2. **Multiple sources** - Support env vars, config file, and runtime
3. **Clear precedence** - Explicit override order
4. **Provider-specific** - Each provider has its own key
5. **Validation** - Check format and presence before use

## 2) API Key Sources (Priority Order)

### ❌ REMOVED: Runtime Override in API Requests

**Why removed**: 
- Keys in HTTP requests can be logged by proxies, load balancers, WAF
- Too easy to accidentally expose in client-side code
- JSON payloads often logged for debugging

### Priority 1: Environment Variables (Recommended)

Standard environment variable names:

```bash
# .env file (never commit this!)
OPENAI_API_KEY=sk-...
ANTHROPIC_API_KEY=sk-ant-...
GOOGLE_API_KEY=...
```

**Use case**: Production deployments, Docker containers, local development

**Security**:
- ✅ Standard 12-factor app practice
- ✅ Supported by all cloud platforms
- ✅ Process isolation prevents accidental exposure
- ✅ Not visible in config files or logs

### Priority 2: Key Reference in config.yaml

**Do NOT store actual keys** - store references to where keys are located:

```yaml
# config.yaml - SAFE, can commit this
api_keys:
  openai:
    env: OPENAI_API_KEY        # Read from environment variable
  anthropic:
    env: ANTHROPIC_API_KEY
  google:
    file: ~/.config/aaagent/keys/google.key  # Read from file
```

**Use case**: Explicit key location management

**Security**:
- ✅ Config file contains NO secrets
- ✅ Safe to commit to git
- ✅ Safe to share in screenshots/logs
- ✅ Clear audit trail of key sources

### Priority 3: secrets.yaml (Dedicated Secrets File)

If file-based keys are needed, use a **separate, clearly-named file**:

```yaml
# secrets.yaml - WARNING: Contains sensitive data!
# This file should ONLY be used for local development
# Production MUST use environment variables

api_keys:
  openai: sk-...
  anthropic: sk-ant-...
```

**Startup warning**:
```
⚠️  WARNING: secrets.yaml detected!
⚠️  This file contains API keys and should ONLY be used locally.
⚠️  Production deployments MUST use environment variables.
⚠️  File location: /path/to/secrets.yaml
⚠️  Press Enter to continue, Ctrl+C to abort...
```

**Security**:
- ⚠️ **Filename clearly indicates sensitive content**
- ⚠️ **Mandatory warning on startup**
- ⚠️ **Blocked in production mode** (only allowed with `--allow-secrets-file` flag)
- ⚠️ **Must be in .gitignore**
- ⚠️ **File permissions checked (must be 600 or 400)**

### Priority 4: Key Files (Individual Files)

Store each key in a separate file:

```bash
~/.config/aaagent/keys/
├── openai.key      # chmod 600
├── anthropic.key   # chmod 600
└── google.key      # chmod 600
```

Referenced in config.yaml:
```yaml
api_keys:
  openai:
    file: ~/.config/aaagent/keys/openai.key
  anthropic:
    file: ~/.config/aaagent/keys/anthropic.key
```

**Security**:
- ✅ Keys separated from config
- ✅ File permissions enforced
- ✅ Easy to rotate individual keys
- ✅ Clear ownership and audit trail

### Priority 5: Default/Error

No key configured - return clear error:

```json
{
  "error": "No API key configured for provider 'openai'. Set OPENAI_API_KEY environment variable, or configure in config.yaml with 'env' or 'file' reference"
}
```

## 3) Configuration Schema

### config.yaml Structure (SAFE - Contains NO Secrets)

```yaml
# API Key References (NOT the actual keys!)
# This file is SAFE to commit to git
api_keys:
  openai:
    env: OPENAI_API_KEY                           # Read from environment variable
  anthropic:
    env: ANTHROPIC_API_KEY
  google:
    file: ~/.config/aaagent/keys/google.key       # Read from file

# Temperature profiles (existing)
temperature_profiles:
  gpt-5.2:
    creativity_map:
      - [0.0, 0.0]
      - [0.5, 0.35]
      - [1.0, 0.7]
  # ... rest of profiles
```

### secrets.yaml Structure (DANGER - Contains Actual Keys)

```yaml
# ⚠️  WARNING: This file contains sensitive API keys!
# ⚠️  ONLY use for local development
# ⚠️  Production MUST use environment variables
# ⚠️  chmod 600 secrets.yaml
# ⚠️  Add to .gitignore

api_keys:
  openai: sk-...
  anthropic: sk-ant-...
  google: ...
```

### Request Schema (API Keys REMOVED)

```typescript
interface ChatConfig {
  preset?: string;
  system_prompt?: string;
  tools_enabled?: boolean;
  intent?: ChatIntent;
  overrides?: ChatOverrides;
  
  // ❌ REMOVED: api_keys field (security risk)
  // API keys are configured server-side only
}
```

## 4) Implementation

### Rust Types

```rust
// In src/config/types.rs

/// API key configuration (runtime override)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeys {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub openai: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anthropic: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub google: Option<String>,
}

// Add to ChatConfig
pub struct ChatConfig {
    pub preset: String,
    pub system_prompt: Option<String>,
    pub tools_enabled: bool,
    pub intent: ChatIntent,
    pub overrides: Option<ChatOverrides>,
    
    /// Runtime API key override (not saved to session)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_keys: Option<ApiKeys>,
}

// In src/config/manager.rs

/// Full configuration file structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigFile {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_keys: Option<ApiKeys>,
    pub temperature_profiles: TemperatureProfiles,
}

pub struct ConfigManager {
    config_path: PathBuf,
    config: ConfigFile,
}

impl ConfigManager {
    /// Get API key for provider with fallback chain
    pub fn get_api_key(&self, provider: &str, runtime_override: Option<&str>) -> Result<String> {
        // Priority 1: Runtime override
        if let Some(key) = runtime_override {
            return Self::validate_api_key(provider, key)?;
        }
        
        // Priority 2: Environment variable
        let env_var = format!("{}_API_KEY", provider.to_uppercase());
        if let Ok(key) = std::env::var(&env_var) {
            return Self::validate_api_key(provider, &key)?;
        }
        
        // Priority 3: config.yaml
        if let Some(api_keys) = &self.config.api_keys {
            let key = match provider {
                "openai" => api_keys.openai.as_ref(),
                "anthropic" => api_keys.anthropic.as_ref(),
                "google" => api_keys.google.as_ref(),
                _ => None,
            };
            
            if let Some(key) = key {
                return Self::validate_api_key(provider, key)?;
            }
        }
        
        // Priority 4: Error
        bail!(
            "No API key configured for provider '{}'. \
            Set {}_API_KEY environment variable or add to config.yaml",
            provider,
            provider.to_uppercase()
        )
    }
    
    fn validate_api_key(provider: &str, key: &str) -> Result<String> {
        let trimmed = key.trim();
        
        // Only check for obvious errors (not strict format validation)
        if trimmed.is_empty() {
            bail!("API key for '{}' is empty or whitespace-only", provider);
        }
        
        if trimmed.len() < 10 {
            bail!("API key for '{}' seems too short (< 10 characters)", provider);
        }
        
        // Check for common mistakes (not security validation)
        if trimmed.contains(' ') {
            log::warn!(
                "API key for '{}' contains spaces - this might be a copy-paste error",
                provider
            );
        }
        
        if trimmed.starts_with("sk-...") || trimmed.starts_with("sk-ant-...") {
            bail!("API key for '{}' looks like a placeholder (sk-...)", provider);
        }
        
        // Soft warnings (not errors) for format hints
        match provider {
            "openai" if !trimmed.starts_with("sk-") => {
                log::warn!(
                    "OpenAI API keys typically start with 'sk-', but this will not be enforced. \
                    The key will be validated when making the first API request."
                );
            },
            "anthropic" if !trimmed.starts_with("sk-ant-") => {
                log::warn!(
                    "Anthropic API keys typically start with 'sk-ant-', but this will not be enforced. \
                    The key will be validated when making the first API request."
                );
            },
            _ => {},
        }
        
        Ok(trimmed.to_string())
    }
    
    /// Verify API key by making a lightweight test call
    pub async fn verify_api_key(&self, provider: &str) -> Result<bool> {
        // This should be called on first use, not at startup
        // Returns true if key is valid, false otherwise
        // Provider implementations should handle this with minimal cost endpoints
        todo!("Implement provider-specific verification")
    }
}
```

### API Integration

```rust
// In src/api/mod.rs

pub async fn chat(
    Path(_session_id): Path<String>,
    State(state): State<AppState>,
    Json(req): Json<ChatRequest>,
) -> Result<Json<ChatResponse>, ApiError> {
    // ... existing validation
    
    // Resolve configuration
    let resolved = state.config_resolver.resolve(&config)
        .map_err(|e| ApiError::BadRequest(e.to_string()))?;
    
    // Get API key for the model's provider
    let provider_name = get_provider_for_model(&resolved.provider.model);
    let runtime_key = config.api_keys.as_ref()
        .and_then(|keys| match provider_name {
            "openai" => keys.openai.as_deref(),
            "anthropic" => keys.anthropic.as_deref(),
            "google" => keys.google.as_deref(),
            _ => None,
        });
    
    let api_key = state.config_manager
        .get_api_key(provider_name, runtime_key)
        .map_err(|e| ApiError::BadRequest(e.to_string()))?;
    
    // Create provider with API key
    // ... rest of implementation
}

fn get_provider_for_model(model: &str) -> &str {
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
```

## 5) Security Measures

### 1. Use `secrecy` Crate for Type Safety

**Dependency**:
```toml
[dependencies]
secrecy = "0.8"
```

**Type-safe secret storage**:
```rust
use secrecy::{Secret, ExposeSecret};

/// API key stored as Secret to prevent accidental exposure
pub type SecretApiKey = Secret<String>;

#[derive(Clone)]
pub struct ApiKeys {
    pub openai: Option<SecretApiKey>,
    pub anthropic: Option<SecretApiKey>,
    pub google: Option<SecretApiKey>,
}

// Debug/Display never expose the secret
impl std::fmt::Debug for ApiKeys {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ApiKeys")
            .field("openai", &self.openai.as_ref().map(|_| "[REDACTED]"))
            .field("anthropic", &self.anthropic.as_ref().map(|_| "[REDACTED]"))
            .field("google", &self.google.as_ref().map(|_| "[REDACTED]"))
            .finish()
    }
}

// Usage: Must explicitly expose secret
impl ConfigManager {
    pub fn get_api_key(&self, provider: &str) -> Result<SecretApiKey> {
        // ... load key from source
        Ok(Secret::new(key_string))
    }
}

// When creating provider
let api_key = config_manager.get_api_key("openai")?;
let provider = OpenAIProvider::create(
    model,
    api_key.expose_secret().clone(), // Explicit exposure required
)?;
```

**Benefits**:
- ✅ Prevents accidental `println!("{:?}", api_keys)`
- ✅ Prevents logging in panic backtraces
- ✅ Prevents exposure via `Display` trait
- ✅ Must explicitly call `.expose_secret()` to use
- ✅ Clear audit trail of where secrets are exposed

### 2. Configure HTTP Client to Never Log Authorization Headers

```rust
use reqwest::Client;
use tracing::Level;

// Configure reqwest client to redact sensitive headers
let client = Client::builder()
    .build()?;

// For tracing/logging middleware
use tower_http::trace::{TraceLayer, DefaultMakeSpan};
use tower_http::sensitive_headers::SetSensitiveHeadersLayer;
use http::header::{AUTHORIZATION, HeaderName};

let sensitive_headers = vec![
    AUTHORIZATION,
    HeaderName::from_static("x-api-key"),
    HeaderName::from_static("anthropic-version"),
];

let app = Router::new()
    .layer(
        SetSensitiveHeadersLayer::new(sensitive_headers)
    )
    .layer(
        TraceLayer::new_for_http()
            .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
    );
```

**Prevents**:
- ❌ HTTP middleware logging `Authorization: Bearer sk-...`
- ❌ Debug logs showing request headers
- ❌ Tracing spans capturing sensitive headers

### 3. File Permissions

```rust
impl ConfigManager {
    pub fn new() -> Result<Self> {
        let config_path = PathBuf::from("config.yaml");
        
        // Check file permissions if exists
        if config_path.exists() {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let metadata = fs::metadata(&config_path)?;
                let mode = metadata.permissions().mode();
                
                // Warn if too permissive (not 600 or 400)
                if mode & 0o077 != 0 {
                    eprintln!(
                        "WARNING: config.yaml has permissive permissions ({}). \
                        Consider: chmod 600 config.yaml",
                        format!("{:o}", mode & 0o777)
                    );
                }
            }
        }
        
        // ... rest of loading
    }
}
```

### 4. Panic Handler Protection

**Problem**: Rust panics can dump entire stack with struct values

```rust
// Bad: Panic could expose key
let api_key = "sk-secret-key";
panic!("Failed with key: {:?}", api_key);  // ❌ Key in panic message
```

**Solution**: Use `Secret<String>` everywhere

```rust
use secrecy::{Secret, ExposeSecret};

let api_key = Secret::new("sk-secret-key".to_string());
panic!("Failed with key: {:?}", api_key);  
// Output: "Failed with key: Secret([REDACTED])"  ✅ Safe
```

### 5. Git Ignore

Update `.gitignore`:

```gitignore
# Sensitive configuration files
secrets.yaml
.env
.env.local
.env.*.local

# Key files directory
keys/
*.key

# DO NOT ignore config.yaml (it only contains references, not actual keys)
```

### 6. Template Files

**config.yaml.example** (Safe to commit):
```yaml
# API Key References (NOT actual keys - safe to commit)
# This file tells the application WHERE to find keys, not the keys themselves

api_keys:
  openai:
    env: OPENAI_API_KEY              # Read from environment variable (recommended)
  anthropic:
    env: ANTHROPIC_API_KEY
  google:
    env: GOOGLE_API_KEY
  
  # Alternative: Read from file
  # openai:
  #   file: ~/.config/aaagent/keys/openai.key

# Temperature profiles for creativity mapping
temperature_profiles:
  gpt-5.2:
    creativity_map:
      - [0.0, 0.0]
      - [0.5, 0.35]
      - [1.0, 0.7]
  gpt-5:
    fixed: 1.0
    ignore_creativity: true
  # ... rest of profiles
```

**secrets.yaml.example** (Template for local dev):
```yaml
# ⚠️  WARNING: This file will contain actual API keys!
# ⚠️  Copy to secrets.yaml (which is in .gitignore)
# ⚠️  chmod 600 secrets.yaml
# ⚠️  NEVER commit secrets.yaml
# ⚠️  Production MUST use environment variables instead

api_keys:
  openai: sk-...
  anthropic: sk-ant-...
  google: ...
```

**.env.example**:
```bash
# Environment Variables for API Keys (recommended method)
# Copy to .env (which is in .gitignore)

OPENAI_API_KEY=sk-...
ANTHROPIC_API_KEY=sk-ant-...
GOOGLE_API_KEY=...
```

## 6) Usage Examples

### Development (Environment Variables)

```bash
# .env file
OPENAI_API_KEY=sk-dev-key-123
ANTHROPIC_API_KEY=sk-ant-dev-456

# Run
cargo run -- serve
```

### Production (Docker)

```dockerfile
# Dockerfile
FROM rust:latest
WORKDIR /app
COPY . .
RUN cargo build --release

# Run with env vars
ENV OPENAI_API_KEY=${OPENAI_API_KEY}
ENV ANTHROPIC_API_KEY=${ANTHROPIC_API_KEY}

CMD ["./target/release/aaagent", "serve"]
```

```bash
# docker-compose.yml
services:
  aaagent:
    build: .
    environment:
      - OPENAI_API_KEY=${OPENAI_API_KEY}
      - ANTHROPIC_API_KEY=${ANTHROPIC_API_KEY}
    ports:
      - "3000:3000"
```

### Testing (Runtime Override)

```bash
curl -X POST http://localhost:3000/api/sessions/test/chat \
  -H "Content-Type: application/json" \
  -d '{
    "message": "Hello",
    "config": {
      "preset": "general",
      "api_keys": {
        "openai": "sk-test-temporary-key"
      }
    }
  }'
```

### Config File (Local Development)

```yaml
# config.yaml
api_keys:
  openai: sk-local-dev-key
  anthropic: sk-ant-local-dev-key

temperature_profiles:
  # ... profiles
```

## 7) Migration Guide

### For Existing Code

**Before:**
```rust
let provider = OpenAIProvider::create(
    "gpt-5-mini".to_string(),
    std::env::var("OPENAI_API_KEY")?,
)?;
```

**After:**
```rust
let api_key = config_manager.get_api_key("openai", None)?;
let provider = OpenAIProvider::create(
    "gpt-5-mini".to_string(),
    api_key,
)?;
```

## 8) Validation Strategy

### Minimal Format Checks (At Load Time)

**Hard errors** (prevent startup):
- Empty or whitespace-only keys
- Keys shorter than 10 characters
- Obvious placeholders (e.g., "sk-...", "your-key-here")

**Soft warnings** (log but continue):
- OpenAI keys not starting with "sk-" (format may change)
- Anthropic keys not starting with "sk-ant-" (format may change)
- Keys containing spaces (likely copy-paste error)

**NOT validated** (too brittle):
- ❌ Specific prefix requirements (formats change)
- ❌ Exact length requirements (varies by provider)
- ❌ Character set restrictions (unknown future formats)

### Real Validation (At First Use)

**The only reliable validation**: Make an actual API call

```rust
// When creating provider for first time
let provider = match OpenAIProvider::create(model, api_key) {
    Ok(p) => p,
    Err(ProviderError::AuthenticationError(msg)) => {
        return Err(ApiError::Unauthorized(format!(
            "Invalid OpenAI API key. Please check your OPENAI_API_KEY. Error: {}",
            msg
        )));
    },
    Err(e) => return Err(e.into()),
};
```

**Provider-specific lightweight verification**:
- OpenAI: `GET /v1/models` (low cost)
- Anthropic: `POST /v1/messages` with minimal request
- Google: `GET /v1/models`

**Caching**: Once verified, cache result per provider

## 9) Error Messages (Never Expose Keys)

**No key configured:**
```json
{
  "error": "No API key configured for provider 'openai'. Set OPENAI_API_KEY environment variable or configure key reference in config.yaml"
}
```

**Key too short:**
```json
{
  "error": "API key for 'openai' seems too short (< 10 characters). Please check your configuration."
}
```

**Authentication failed (at first use):**
```json
{
  "error": "Authentication failed for provider 'openai'. Please verify your API key is correct.",
  "details": "401 Unauthorized: Incorrect API key provided",
  "help": "Check OPENAI_API_KEY at https://platform.openai.com/api-keys"
}
```

**❌ NEVER expose partial keys:**
```json
{
  "error": "API key 'sk-proj-abc123...' is invalid"  // ❌ BAD
}
```

## 10) Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_api_key_priority_runtime() {
        // Runtime override takes precedence
        let manager = ConfigManager::new().unwrap();
        let key = manager.get_api_key("openai", Some("sk-runtime")).unwrap();
        assert_eq!(key, "sk-runtime");
    }
    
    #[test]
    fn test_api_key_priority_env() {
        std::env::set_var("OPENAI_API_KEY", "sk-env");
        let manager = ConfigManager::new().unwrap();
        let key = manager.get_api_key("openai", None).unwrap();
        assert_eq!(key, "sk-env");
    }
    
    #[test]
    fn test_api_key_validation_openai() {
        let result = ConfigManager::validate_api_key("openai", "invalid");
        assert!(result.is_err());
        
        let result = ConfigManager::validate_api_key("openai", "sk-valid-key");
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_api_key_not_logged() {
        let keys = ApiKeys {
            openai: Some("sk-secret".to_string()),
            anthropic: None,
            google: None,
        };
        
        let debug_output = format!("{:?}", keys);
        assert!(!debug_output.contains("sk-secret"));
        assert!(debug_output.contains("***"));
    }
}
```

## 11) Documentation

### README.md Update

```markdown
## Configuration

### API Keys

Provide your API keys using one of these methods (in priority order):

#### 1. Environment Variables (Recommended)

```bash
export OPENAI_API_KEY=sk-...
export ANTHROPIC_API_KEY=sk-ant-...
export GOOGLE_API_KEY=...
```

Or use a `.env` file:

```bash
cp .env.example .env
# Edit .env with your keys
```

#### 2. Configuration File

```bash
cp config.yaml.example config.yaml
# Edit config.yaml with your keys
chmod 600 config.yaml  # Restrict permissions
```

#### 3. Runtime Override (Testing Only)

```json
{
  "config": {
    "api_keys": {
      "openai": "sk-temp-key"
    }
  }
}
```

**Security Notes:**
- Never commit `.env` or `config.yaml` to git
- Use environment variables in production
- Restrict file permissions: `chmod 600 config.yaml`
```

## 12) Security Checklist

### Design Decisions
- ✅ **NO runtime API key override in requests** (prevents logging/proxy exposure)
- ✅ **NO actual keys in config.yaml** (only references: env vars or file paths)
- ✅ **Separate secrets.yaml** with clear warning (filename indicates danger)
- ✅ **Minimal format validation** (avoid brittleness, validate at first use)
- ✅ **Type-safe secrets** using `secrecy` crate (prevents accidental exposure)

### Implementation Requirements
- ✅ Use `Secret<String>` for all API keys (not plain `String`)
- ✅ Configure `SetSensitiveHeadersLayer` to redact Authorization headers
- ✅ Never log/display actual keys (Debug shows "[REDACTED]")
- ✅ Panic-safe (Secret type prevents exposure in backtraces)
- ✅ File permissions check (warn if not 600/400)
- ✅ Startup warning if secrets.yaml detected
- ✅ Block secrets.yaml in production (require `--allow-secrets-file`)

### Common Exposure Vectors (All Prevented)
- ❌ HTTP request logging → No keys in requests
- ❌ Proxy/WAF logs → No keys in requests
- ❌ `println!("{:?}", key)` → Secret type redacts
- ❌ Panic backtraces → Secret type redacts
- ❌ Tracing middleware → SetSensitiveHeadersLayer
- ❌ Git commits → .gitignore + reference-based config
- ❌ Screenshots → No keys in config.yaml
- ❌ Issue reports → Template asks for config.yaml (safe)
- ❌ Log files → Never logged (Secret + middleware)

## 13) Acceptance Criteria

- [ ] ConfigManager loads keys from env vars and file references
- [ ] Priority order enforced (env > file reference > secrets.yaml)
- [ ] All keys use `Secret<String>` type
- [ ] Minimal validation (empty, too short, placeholders)
- [ ] Soft warnings for format hints (not hard failures)
- [ ] Real validation on first API call
- [ ] Debug output shows "[REDACTED]"
- [ ] Panic backtraces never expose keys
- [ ] HTTP middleware redacts Authorization headers
- [ ] File permission warnings on Unix
- [ ] .gitignore includes secrets.yaml, .env, *.key
- [ ] .gitignore does NOT include config.yaml (safe to commit)
- [ ] config.yaml.example uses reference format
- [ ] secrets.yaml.example has clear warnings
- [ ] Startup warning if secrets.yaml detected
- [ ] Production mode blocks secrets.yaml
- [ ] README documents all methods
- [ ] Tests cover all scenarios

## 13) Future Enhancements

- [ ] Key rotation API endpoint
- [ ] Multiple keys per provider (round-robin)
- [ ] Usage tracking per key
- [ ] Encrypted key storage option
- [ ] Key validation via test API call
- [ ] Admin UI for key management

---

**Implementation Status:** Draft  
**Dependencies:** chat-ui-config.md (implemented)  
**Estimated Effort:** 1-2 days
