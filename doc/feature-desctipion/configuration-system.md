# Chat UI Configuration System

**Status:** ✅ Implemented  
**Version:** 0.1.0  
**Date:** 2026-01-08

## Overview

The Chat UI Configuration System provides an intent-first API for configuring LLM behavior. Instead of technical parameters like `temperature` and `max_tokens`, users specify their goals through high-level intents like `creativity`, `verbosity`, and `rounds`.

## Key Principles

1. **Intent over parameters** - Users specify what they want, not how to achieve it
2. **Presets for common cases** - Ready-to-use configurations for typical scenarios
3. **Immutable system prompts** - Ensures consistent session behavior
4. **Model-specific intelligence** - Automatically adapts to different LLM capabilities
5. **Secure API key management** - Production-grade security with multiple configuration methods

## Quick Start

### Step 1: Configure API Keys

Choose one of these secure methods:

**Method 1: Environment Variables (Recommended)**
```bash
# .env file
OPENAI_API_KEY=sk-...
ANTHROPIC_API_KEY=sk-ant-...
GOOGLE_API_KEY=...
```

**Method 2: Key References in config.yaml (Safe to commit)**
```yaml
# config.yaml - contains NO actual keys
api_keys:
  openai:
    env: OPENAI_API_KEY              # Read from environment
  anthropic:
    file: ~/.config/aaagent/keys/anthropic.key  # Read from file
```

**Method 3: secrets.yaml (Local dev only, with warnings)**
```yaml
# secrets.yaml - WARNING: Contains actual keys
api_keys:
  openai: sk-...
  anthropic: sk-ant-...
```

### Step 2: Start the Server

```bash
# Using environment variables
export OPENAI_API_KEY=sk-...
cargo run -- serve

# Or with .env file
cargo run -- serve  # Loads from .env automatically
```

### Step 3: Send Chat Requests

```bash
# Send a chat request
curl -X POST http://localhost:3000/api/sessions/my-session/chat \
  -H "Content-Type: application/json" \
  -d '{
    "message": "Explain quantum computing",
    "config": {
      "preset": "general",
      "intent": {
        "creativity": 0.5,
        "verbosity": "normal",
        "rounds": 30
      }
    }
  }'
```

### Using Presets

```json
POST /api/sessions/my-session/chat
{
  "message": "Write a Python function to sort a list",
  "config": {
    "preset": "coding",
    "intent": {
      "verbosity": "long"
    }
  }
}
```

### Power User Overrides

```json
{
  "message": "Analyze this dataset",
  "config": {
    "preset": "research",
    "intent": {
      "creativity": 0.3,
      "verbosity": "long",
      "rounds": 50
    },
    "overrides": {
      "model": "gpt-5.2",
      "top_p": 0.9
    }
  }
}
```

## Configuration Options

### Presets

| Preset | Use Case | Model | Tokens | Rounds | Compression |
|--------|----------|-------|--------|--------|-------------|
| **general** | Everyday conversation, Q&A | gpt-5-mini | 16K | 30 | balanced |
| **coding** | Code generation, debugging | gpt-5-mini | 32K | 40 | minimal |
| **research** | Analysis, complex problems | gpt-5-mini | 32K | 50 | minimal |
| **quick** | Fast answers, low cost | gpt-5-nano | 8K | 15 | aggressive |

Each preset includes an optimized system prompt tailored to its use case.

### Intent Fields

#### `creativity` (0.0 - 1.0)

Maps to temperature based on model capabilities:

- **0.0** - Deterministic, factual responses
- **0.5** - Balanced (default)
- **1.0** - Creative, exploratory responses

**Model-specific behavior:**
- GPT-5.2: Linear mapping (0.0→0.0, 0.5→0.35, 1.0→0.7)
- GPT-5/5-mini/5-nano: Fixed at 1.0 (reasoning models)
- Gemini-3: Fixed at 1.0 (recommended by Google)

#### `verbosity` (string)

Controls output length:

- `"short"` - 8K tokens max (concise answers)
- `"normal"` - 16K tokens max (default)
- `"long"` - 32K tokens max (detailed explanations)

#### `rounds` (number)

Maximum agent execution rounds:

- **Low (10-15)** - Quick tasks
- **Standard (30)** - Default, handles most tasks
- **High (50+)** - Complex multi-step problems

### Top-Level Fields

#### `preset` (string)

Which preset to use: `"general"` | `"coding"` | `"research"` | `"quick"`

Default: `"general"`

#### `system_prompt` (string, optional)

Custom system prompt to override preset default.

**Important:** 
- Max 10,000 characters
- **Immutable** - can ONLY be set during session creation
- Attempting to update returns 400 error

```json
{
  "preset": "general",
  "system_prompt": "You are a friendly math tutor who explains concepts with analogies."
}
```

#### `tools_enabled` (boolean)

Enable or disable tool calling:

- `true` - Agent can use tools (default)
- `false` - Single response without tool execution

### Overrides (Power Users)

**Allowed overrides:**

| Parameter | Type | Range | Description |
|-----------|------|-------|-------------|
| `model` | string | whitelist | Switch LLM provider |
| `top_p` | number | 0.0-1.0 | Nucleus sampling |
| `frequency_penalty` | number | -2.0 to 2.0 | Reduce repetition |
| `presence_penalty` | number | -2.0 to 2.0 | Topic diversity |

**Whitelist of models:**
- `gpt-5`, `gpt-5-mini`, `gpt-5-nano`, `gpt-5.2`
- `gemini-3-flash-preview`, `gemini-3-pro-preview`

**Not allowed:**
- Direct `temperature` (use `creativity` intent)
- Direct `max_tokens` (use `verbosity` intent)
- Direct `max_rounds` (use `rounds` intent)

## API Reference

### Endpoint

```
POST /api/sessions/{session_id}/chat
```

### Request Schema

```typescript
interface ChatRequest {
  message: string;                    // Required
  config?: ChatConfig;                // Optional (persistent)
  temporary_config?: ChatConfig;      // Optional (one-time)
}

interface ChatConfig {
  preset?: string;                    // Default: "general"
  system_prompt?: string;             // Max 10K chars, immutable
  tools_enabled?: boolean;            // Default: true
  intent?: ChatIntent;
  overrides?: ChatOverrides;
}

interface ChatIntent {
  creativity?: number;                // 0.0-1.0, default: 0.5
  verbosity?: string;                 // "short"|"normal"|"long", default: "normal"
  rounds?: number;                    // 1-100, default: 30
}

interface ChatOverrides {
  model?: string;
  top_p?: number;
  frequency_penalty?: number;
  presence_penalty?: number;
}
```

### Response Schema

```typescript
interface ChatResponse {
  stream_id: string;
  resolved_config: ResolvedConfig;
}

interface ResolvedConfig {
  provider: {
    model: string;
    temperature: number;
    max_tokens: number;
    top_p?: number;
    frequency_penalty?: number;
    presence_penalty?: number;
  };
  agent: {
    max_rounds: number;
    tools_enabled: boolean;
  };
  session: {
    system_prompt: string;
    max_context_tokens: number;
  };
}
```

### Error Responses

**400 Bad Request:**
```json
{
  "error": "creativity must be between 0.0 and 1.0, got 1.5"
}
```

**400 Immutable Field:**
```json
{
  "error": "system_prompt is immutable. Create a new session to use a different prompt."
}
```

## Configuration Flow

### New Session

```
1. Load preset defaults (e.g., "coding")
   ├─ system_prompt: "You are an expert software engineer..."
   ├─ model: gpt-5-mini
   ├─ max_tokens: 32768
   └─ max_rounds: 40

2. Apply custom system_prompt (if provided)
   ⚠️  This is the ONLY time system_prompt can be set

3. Apply top-level config
   └─ tools_enabled: true/false

4. Apply intent mappings
   ├─ creativity → temperature (model-specific)
   ├─ verbosity → max_tokens
   └─ rounds → max_rounds

5. Apply overrides
   └─ model, top_p, penalties

6. Resolve to final config

7. Save to session.metadata["resolved_config"]
   🔒 system_prompt is now LOCKED
```

### Existing Session

```
1. Load resolved_config from session.metadata
2. Use as-is (system_prompt is immutable)
3. Other parameters can be updated
```

### Temporary Override

```json
{
  "message": "Explain in detail",
  "temporary_config": {
    "intent": {
      "verbosity": "long"
    }
  }
}
```

- Config change applies to this request only
- NOT saved to session metadata
- **Cannot include `system_prompt`** (immutable)

## Temperature Profiles

### config.yaml

Located in working directory, auto-created with defaults:

```yaml
profiles:
  gpt-5.2:
    creativity_map:
      - [0.0, 0.0]
      - [0.5, 0.35]
      - [1.0, 0.7]
  
  gpt-5:
    fixed: 1.0
    ignore_creativity: true
  
  gpt-5-mini:
    fixed: 1.0
    ignore_creativity: true
  
  gpt-5-nano:
    fixed: 1.0
    ignore_creativity: true
  
  gemini-3-flash-preview:
    fixed: 1.0
    ignore_creativity: true
  
  gemini-3-pro-preview:
    fixed: 1.0
    ignore_creativity: true
  
  default:
    creativity_map:
      - [0.0, 0.0]
      - [1.0, 1.0]
```

### Customizing Profiles

1. Edit `config.yaml` in your working directory
2. Restart server or call `ConfigManager::reload()`
3. New mappings take effect immediately

### Linear Interpolation

For models with `creativity_map`, temperature is interpolated:

```
creativity = 0.25 (halfway between 0.0 and 0.5)
GPT-5.2: temp = 0.175 (halfway between 0.0 and 0.35)
```

## Session Metadata Integration

### Saving Resolved Config

```rust
use aaagent::config::{ConfigResolver, ChatConfig};

let resolver = ConfigResolver::new()?;
let resolved = resolver.resolve(&config)?;

// Save to session metadata
session.set_metadata("resolved_config", &resolved)?;
```

### Loading Resolved Config

```rust
use aaagent::config::ResolvedConfig;

// Load from session metadata
let config: ResolvedConfig = session
    .get_metadata("resolved_config")
    .ok_or_else(|| anyhow!("No config found"))?;
```

### Checking Immutability

```rust
// Validate that system_prompt hasn't changed
resolver.validate_immutable_fields(&new_config, &existing_config)?;
// Returns error if system_prompt differs
```

## Use Cases

### 1. Creative Writing Session

```json
{
  "preset": "general",
  "system_prompt": "You are a creative writing assistant who helps craft compelling stories.",
  "intent": {
    "creativity": 0.9,
    "verbosity": "long",
    "rounds": 40
  }
}
```

### 2. Code Review Session

```json
{
  "preset": "coding",
  "intent": {
    "creativity": 0.2,
    "verbosity": "normal",
    "rounds": 30
  }
}
```

### 3. Quick Reference Lookup

```json
{
  "preset": "quick",
  "intent": {
    "creativity": 0.0,
    "verbosity": "short",
    "rounds": 10
  }
}
```

### 4. Deep Research Analysis

```json
{
  "preset": "research",
  "intent": {
    "creativity": 0.4,
    "verbosity": "long",
    "rounds": 50
  },
  "overrides": {
    "model": "gpt-5"
  }
}
```

## Advanced Topics

### Model Selection Strategy

**Default model per preset:**
- general, coding, research → `gpt-5-mini` (balanced cost/performance)
- quick → `gpt-5-nano` (fastest, cheapest)

**Override when:**
- Need advanced reasoning → `gpt-5`
- Want configurable temperature → `gpt-5.2`
- Google ecosystem → `gemini-3-flash-preview` or `gemini-3-pro-preview`

### Compression Strategies

Presets include compression settings (future feature):

- **balanced** - Compress tool results over 5K chars
- **minimal** - Preserve more context (coding, research)
- **aggressive** - Compress aggressively (quick)

### Context Budget

Each preset defines `max_context_tokens`:

- general: 200K tokens
- coding: 300K tokens (needs more context)
- research: 400K tokens (maximum context)
- quick: 150K tokens (cost-sensitive)

## Validation Reference

### Errors You Might Encounter

| Error | Cause | Solution |
|-------|-------|----------|
| `"Invalid preset 'xyz'"` | Unknown preset name | Use: general, coding, research, quick |
| `"creativity must be between 0.0 and 1.0"` | Out of range | Use value in valid range |
| `"verbosity must be 'short', 'normal', or 'long'"` | Invalid enum | Use one of the three options |
| `"rounds must be between 1 and 100"` | Out of range | Reduce rounds count |
| `"Invalid model 'xyz'"` | Model not in whitelist | Use supported model |
| `"system_prompt is immutable"` | Trying to update | Create new session instead |
| `"system_prompt must be at most 10,000 characters"` | Too long | Shorten prompt |

## Best Practices

### 1. Choose the Right Preset

- Use `coding` for anything code-related
- Use `research` for analysis and complex reasoning
- Use `quick` for simple lookups
- Use `general` for everything else

### 2. Start with Defaults

Don't override unless you have a specific reason:
- Presets are optimized for their use case
- Default creativity (0.5) works well for most tasks

### 3. System Prompt Guidelines

- Be specific about role and behavior
- Include examples if needed
- Keep under 10K characters
- Remember: **You can't change it later**

### 4. Creativity Settings

- **0.0-0.3** - Factual, deterministic (documentation, code)
- **0.4-0.6** - Balanced (general conversation)
- **0.7-1.0** - Creative, exploratory (brainstorming, writing)

### 5. Rounds Budget

- **10-15** - Single-turn or simple tasks
- **20-30** - Standard conversations
- **40-50** - Multi-step reasoning or tool-heavy tasks

## Migration Guide

### From Direct Parameters

**Before:**
```json
{
  "temperature": 0.7,
  "max_tokens": 32768,
  "max_rounds": 40
}
```

**After:**
```json
{
  "preset": "coding",
  "intent": {
    "creativity": 1.0,
    "verbosity": "long",
    "rounds": 40
  }
}
```

### Benefits of Intent-First API

1. **Model-agnostic** - Creativity maps correctly across models
2. **Semantic** - Intent is clearer than technical parameters
3. **Safe defaults** - Presets prevent misconfiguration
4. **Future-proof** - New models auto-adapt

## Troubleshooting

### Config Not Loading

**Problem:** Custom config.yaml not being used

**Solution:**
```bash
# Check file exists in working directory
ls config.yaml

# Verify YAML syntax
cat config.yaml

# Restart server to reload
cargo run -- serve
```

### Temperature Not Changing

**Problem:** Creativity changes have no effect

**Cause:** Using a reasoning model (GPT-5, Gemini-3) with fixed temperature

**Solution:** 
- Use GPT-5.2 for configurable temperature
- Or accept that reasoning models work best at temp=1.0

### Session Prompt Won't Update

**Problem:** `"system_prompt is immutable"` error

**Solution:** This is intentional! Create a new session:

```bash
# New session ID with different prompt
curl -X POST http://localhost:3000/api/sessions/new-session-id/chat \
  -d '{"config": {"system_prompt": "New prompt..."}}'
```

## API Key Management

### Security Features

The API key management system implements production-grade security:

1. **Type-safe secrets** using `secrecy` crate
   - All API keys use `Secret<String>`
   - Prevents accidental exposure in logs/panics/debug output
   - Must explicitly `.expose_secret()` to use

2. **HTTP middleware protection**
   - `SetSensitiveRequestHeadersLayer` redacts Authorization headers
   - Prevents keys from appearing in request logs

3. **Reference-based configuration**
   - `config.yaml` contains NO actual keys (safe to commit)
   - Stores only references (env vars or file paths)

4. **Separated secrets file**
   - `secrets.yaml` clearly indicates sensitive content
   - Startup warning in development mode
   - Blocked in production mode

5. **Gentle validation**
   - Only checks obvious errors (empty, too short, placeholders)
   - Soft warnings for format hints (not hard failures)
   - Real validation happens on first API call

### API Key Loading Priority

```
1. Key Reference from config.yaml
   ├─ env: OPENAI_API_KEY      → Read from environment variable
   └─ file: ~/.keys/openai.key → Read from file

2. Default Environment Variable
   └─ OPENAI_API_KEY (if not specified in config.yaml)

3. secrets.yaml (if allowed and exists)
   └─ api_keys.openai

4. Error with helpful message
```

### Protected Exposure Vectors

The system prevents key exposure through:

- ✅ HTTP request logging (no keys in requests)
- ✅ Proxy/WAF logs (no keys in requests)
- ✅ Debug output (`Secret` type redacts)
- ✅ Panic backtraces (`Secret` type redacts)
- ✅ Tracing middleware (sensitive headers layer)
- ✅ Git commits (.gitignore + reference-based config)
- ✅ Screenshots (no keys in config.yaml)
- ✅ Issue reports (template asks for safe config.yaml)
- ✅ Log files (never logged)

### Configuration Files

**config.yaml** - Safe to commit
```yaml
# Contains only references, NOT actual keys
api_keys:
  openai:
    env: OPENAI_API_KEY
  anthropic:
    file: ~/.config/aaagent/keys/anthropic.key
```

**secrets.yaml** - In .gitignore, with warnings
```yaml
# ⚠️  WARNING: Contains actual keys!
# Only for local development
api_keys:
  openai: sk-...
```

**.env** - In .gitignore
```bash
OPENAI_API_KEY=sk-...
ANTHROPIC_API_KEY=sk-ant-...
```

## Implementation Details

### File Structure

```
src/config/
├── mod.rs           # Module exports
├── types.rs         # Config types, temperature profiles
├── presets.rs       # Preset definitions
├── manager.rs       # ConfigManager (loads config.yaml, secrets.yaml)
├── resolver.rs      # ConfigResolver (validation, mapping)
└── keys.rs          # API key types and loading logic (NEW)
```

### Dependencies

```toml
[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
serde_yaml = "0.9"
anyhow = "1.0"
secrecy = "0.8"          # Type-safe secrets (NEW)
shellexpand = "3.1"      # Expand ~ in file paths (NEW)
tower-http = { version = "0.5", features = ["sensitive-headers"] }  # (NEW)
```

### Testing

```bash
# Run config tests
cargo test --lib config

# Run all tests
cargo test --lib

# Expected: 102+ tests passing (including API key tests)
```

## Related Documentation

### Planning Documents
- [Chat UI Plan](../plan/chat-ui-plan.md) - Overall chat UI architecture
- [Configuration Plan](../plan/chat-ui-config.md) - Detailed design decisions
- [API Key Management Plan](../plan/api-key-management.md) - Security design

### Implementation Notes
- [Configuration Implementation](../implementation/chat-ui-config-implementation.md) - Technical details
- [API Key Implementation](../implementation/api-key-management-implementation.md) - Security implementation

## Future Enhancements

### Configuration System
- [ ] PATCH endpoint to update mutable config fields
- [ ] GET endpoint to retrieve current session config
- [ ] Config versioning for backward compatibility
- [ ] Per-user default presets
- [ ] Custom preset creation API
- [ ] Frontend React components

### API Key Management
- [ ] Key rotation API endpoint
- [ ] Multiple keys per provider (round-robin)
- [ ] Usage tracking per key
- [ ] Encrypted key storage option
- [ ] Key validation via test API call
- [ ] Admin UI for key management

---

**Last Updated:** 2026-01-08  
**Status:** Configuration System ✅ Implemented | API Key Management ✅ Implemented  
**Maintainer:** aaagent-rs team
