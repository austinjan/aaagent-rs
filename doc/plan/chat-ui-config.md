# Chat UI Configuration Plan (Intent-First Design)

- Feature name: `chat-ui-config`
- Status: Draft
- Created: 2026-01-07
- Updated: 2026-01-07
- Parent plan: [chat-ui-plan.md](./chat-ui-plan.md)

## 1) Overview

### Goal
Provide an **intent-first** configuration API that exposes user intentions (style, creativity, verbosity, budget) rather than low-level parameters (temperature, max_tokens, etc.).

### Design Principles

1. **API exposes intent, not parameters** - Users specify what they want, not how to achieve it
2. **Resolved config is single source of truth** - Stored in session metadata, never recalculated
3. **File config defines presets and policies** - Not user-editable runtime config
4. **Request allows intent + minimal overrides** - Whitelist only (model, system_prompt)
5. **System prompt belongs to session** - Not provider config

### Problem with Old Design

The original design exposed too many low-level parameters:
- Users had to understand `temperature`, `max_tokens`, `top_p`, `compression.full_context_turns`, etc.
- No clear mapping between user goals and technical parameters
- Configuration could be inconsistent across requests
- API was too complex for 99% of use cases

### New Design Benefits

- **Simpler API**: 5 intent fields instead of 20+ parameters
- **Consistent behavior**: Resolved config stored once in metadata
- **Power user escape hatch**: Whitelisted overrides for advanced users
- **Maintainable**: Intent → runtime mapping defined in one place

## 2) API Design (External Interface)

### 2.1) Request Payload

```json
POST /api/sessions/{session_id}/chat
{
  "message": "Hello, how are you?",
  
  // Persistent config (for new sessions or permanent changes)
  "config": {
    "preset": "balanced",
    "intent": {
      "style": "coding",
      "creativity": 0.3,
      "verbosity": "short",
      "tooling": "auto",
      "budget": "normal"
    },
    "overrides": {
      "provider": { 
        "model": "gpt-5-mini",
        "top_p": 0.9,
        "frequency_penalty": 0.5
      },
      "session": { 
        "system_prompt": "You are a Rust coding assistant." 
      }
    }
  },
  
  // Temporary config (only affects THIS request, not saved to metadata)
  "temporary_config": {
    "intent": {
      "creativity": 0.1,  // Lower creativity just for this question
      "verbosity": "long"  // Need detailed answer this time
    }
  }
}
```

### 2.2) Field Definitions

#### `preset` (string)

Predefined configuration profiles:
- `"balanced"` - Default, good for general chat/assistance
- `"agent"` - **Optimized for autonomous agents** (high limits, minimal restrictions)
- `"aggressive"` - Heavy optimization for long chat sessions
- `"minimal"` - Minimal optimization, maximum context preservation

#### `intent` (object)

**`style`** - Conversation style
- `"general"` - General purpose assistant
- `"coding"` - Code generation and debugging
- `"analysis"` - Data analysis and reasoning
- `"support"` - Help desk / customer support

**`creativity`** - Creative freedom (maps to temperature)
- Range: `0.0` - `1.0`
- `0.0` = Deterministic, factual
- `0.5` = Balanced
- `1.0` = Creative, exploratory

**`verbosity`** - Response length preference (controls `max_tokens` = max OUTPUT tokens per agent turn)
- `"short"` - Concise responses (8192 tokens max output)
- `"normal"` - Standard agent output (16384 tokens max output)
- `"long"` - Extensive analysis/generation (32768 tokens max output)

**Note**: `max_tokens` controls the **maximum output length per turn**, not input. For agents, we need generous limits since they may generate tool calls, reasoning chains, and detailed responses. Total context window is controlled by `max_context_tokens` in session config. With 2026 models having 400K context windows and agents running multiple rounds, we prioritize capability over token savings.

**`tooling`** - Tool calling behavior
- `"off"` - Disable tools entirely (tools_enabled=false, single-round response)
- `"auto"` - Standard tool policy (12 rounds)
- `"max"` - Maximum tool usage (25 rounds, higher retry limits)

**`budget`** - Agent execution budget and optimization strategy
- `"low"` - Quick tasks (15 rounds, aggressive compression, frequent checkpoints)
- `"normal"` - Standard agent workload (25 rounds, balanced compression)
- `"high"` - Complex long-running tasks (50 rounds, minimal compression, preserve context)

#### `overrides` (object) - **Whitelist Only**

Only these fields can be overridden:

```typescript
{
  "provider": {
    // Model selection
    "model": "gpt-5" | "gpt-5-mini" | "gpt-5-nano" | "gpt-5.2" | 
             "gemini-3-flash-preview" | "gemini-3-pro-preview",
    
    // Sampling parameters (for power users)
    "top_p": number,              // 0.0-1.0, nucleus sampling
    "frequency_penalty": number,  // -2.0 to 2.0, reduce repetition
    "presence_penalty": number    // -2.0 to 2.0, encourage diversity
  },
  "session": {
    "system_prompt": "Custom system prompt..."
  }
}
```

**Validation rules:**
- `top_p`: Must be between 0.0 and 1.0
- `frequency_penalty`: Must be between -2.0 and 2.0
- `presence_penalty`: Must be between -2.0 and 2.0
- `model`: Must be one of supported models (see section 13)

**No deep merging allowed.** Any other fields are rejected with 400 Bad Request.

**Why these are whitelisted:**
- `model`: Common need to switch between providers
- `top_p`: Alternative to temperature, useful for advanced prompt engineering
- `frequency_penalty`: Improves quality by preventing repetition
- `presence_penalty`: Encourages topic variety in creative tasks
- `system_prompt`: Custom personas for specific use cases

**Not whitelisted (use intent instead):**
- `temperature`: Use `creativity` intent
- `max_tokens`: Use `verbosity` intent
- `stop_sequences`: Can break agent loop logic

### 2.3) Response

```json
{
  "stream_id": "stream-abc123",
  "config_preview": {
    "provider": {
      "model": "gpt-4o-mini",
      "temperature": 0.3,
      "max_tokens": 2048
    },
    "agent": {
      "max_rounds": 12
    },
    "session": {
      "system_prompt": "You are a Rust coding assistant.",
      "max_context_tokens": 200000
    }
  }
}
```

The `config_preview` shows the resolved configuration for debugging.

## 3) Configuration Precedence & Update Strategy

### 3.1) Initial Configuration (New Sessions)

Configuration is resolved during session creation:

```
1. Built-in defaults (from code)
   ↓
2. Global config.yaml (preset definitions + policies)
   ↓
3. Request config (intent + overrides)
   ↓
4. Resolve → ChatConfigResolved
   ↓
5. Save to session.metadata.resolved_config
   ↓
6. Use metadata as base for all future requests
```

**No session-specific config files** - Session config is ONLY stored in metadata, not in separate YAML files.

### 3.2) Configuration Updates (Existing Sessions)

#### Temporary Overrides (Per-Request)

Use `temporary_config` for experimental adjustments without modifying metadata:

```json
POST /api/sessions/abc-123/chat
{
  "message": "Explain quantum physics",
  "temporary_config": {
    "intent": {
      "creativity": 0.1,  // Just for this question
      "verbosity": "long"
    }
  }
}
```

**Behavior:**
- Loads base config from metadata
- Applies temporary overrides for THIS request only
- Does NOT save changes back to metadata
- Next request uses original metadata config

#### Permanent Updates (Via PATCH API)

Use `PATCH /api/sessions/{id}/config` for committed changes:

```json
PATCH /api/sessions/abc-123/config
{
  "intent": {
    "budget": "high"  // Permanently switch to high budget
  },
  "overrides": {
    "provider": {
      "model": "gemini-3-flash-preview"  // Switch model permanently
    }
  }
}
```

**Behavior:**
- Resolves new config from scratch
- Validates update safety (see policy below)
- Updates metadata with new resolved config
- All future requests use new config

#### Update Safety Policy

| Parameter | Update Allowed | Via temporary_config | Via PATCH | Notes |
|-----------|----------------|---------------------|-----------|-------|
| `creativity` | ✅ Always | ✅ | ✅ | Safe to change anytime |
| `verbosity` | ✅ Always | ✅ | ✅ | Safe to change anytime |
| `tooling` | ✅ Always | ✅ | ✅ | Safe to change anytime |
| `budget` | ✅ With care | ✅ | ✅ | May affect optimization |
| `model` | ✅ Allowed | ❌ | ✅ | Only via PATCH (explicit action) |
| `style` | ⚠️ Careful | ✅ | ✅ | Changes system_prompt |
| `system_prompt` | ❌ Immutable | ❌ | ❌ | Cannot change mid-session |
| `max_context_tokens` | ❌ Immutable | ❌ | ❌ | Cannot change mid-session |

**Rationale:**
- **Freely updatable** (creativity, verbosity, tooling): Don't affect conversation consistency
- **Explicit update only** (model, budget): Require user confirmation via PATCH
- **Immutable** (system_prompt, max_context_tokens): Changing mid-session causes inconsistency

## 4) Intent → Runtime Mapping

### 4.1) Verbosity Mapping

| Verbosity | max_tokens | Description |
|-----------|------------|-------------|
| `short`   | 8192       | Concise agent responses |
| `normal`  | 16384      | Standard agent output (tool calls + reasoning) |
| `long`    | 32768      | Extensive analysis/generation |

### 4.2) Budget Mapping (Agent-Optimized)

| Budget   | max_rounds | Compression     | Checkpoint Frequency | Use Case |
|----------|------------|-----------------|----------------------|----------|
| `low`    | 15         | aggressive      | every 10 turns       | Quick tasks, cost-sensitive |
| `normal` | 25         | balanced        | every 30 turns       | Standard agent workflows |
| `high`   | 50         | minimal         | every 100 turns      | Complex research, code generation |

**Note**: Agent rounds are expensive (multiple LLM calls, tool executions). These limits are designed for real agent workloads, not simple chat.

### 4.3) Tooling Mapping

| Tooling | tools_enabled | max_rounds | Description |
|---------|---------------|------------|-------------|
| `off`   | false         | 1          | Single response, no tool calls |
| `auto`  | true          | 25         | Standard agent tool usage |
| `max`   | true          | 50         | Complex tasks requiring many tool interactions |

**Important:** `tooling: off` sets `tools_enabled = false`, NOT `max_rounds = 0`. Setting max_rounds to 0 would break the entire agent loop.

**Note**: For agent systems, `auto` is already generous (25 rounds = potentially 50+ LLM calls with tool use). `max` is for exceptionally complex tasks like "analyze entire codebase and refactor".

### 4.4) Creativity Mapping (Model-Specific)

**Creativity is mapped to temperature using model-specific profiles:**

#### GPT-5.2 (Only GPT-5 variant with configurable temperature)

| Creativity | Temperature | Notes |
|------------|-------------|-------|
| 0.0        | 0.0         | Reproducible, deterministic |
| 0.5        | 0.35        | Balanced (enterprise default) |
| 1.0        | 0.7         | Creative (enterprise max) |

Linear interpolation between points. Can go up to 1.0+ for creative tasks.

#### GPT-5, GPT-5-mini, GPT-5-nano (Reasoning models)

| Creativity | Temperature | Notes |
|------------|-------------|-------|
| **Any value** | **1.0** | **Fixed by OpenAI, creativity ignored** |

These are reasoning models with internal multi-step processes. Temperature is locked at 1.0 and cannot be customized.

#### Gemini 3 Flash/Pro (Reasoning models)

| Creativity | Temperature | Notes |
|------------|-------------|-------|
| **Any value** | **1.0** | **Fixed, Google strongly recommends** |

Google docs warn: lowering temperature may cause **looping or degraded chain-of-thought performance**.

#### Unknown Models (Default fallback)

| Creativity | Temperature | Notes |
|------------|-------------|-------|
| 0.0        | 0.0         | Conservative 1:1 mapping |
| 0.5        | 0.5         | |
| 1.0        | 1.0         | |

**Implementation:** See `temperature_profiles` in config.yaml (section 6).

### 4.5) Style Mapping (with System Prompt Templates)

| Style      | Provider Adjustments | Session Adjustments | Description |
|------------|---------------------|---------------------|-------------|
| `general`  | None | None | General purpose assistant |
| `coding`   | enable_reasoning=true, max_tokens=8192 | "You are an expert software engineer..." | Code generation and debugging |
| `analysis` | enable_reasoning=true, max_tokens=16384 | "You are an analytical reasoning expert..." | Structured problem-solving |
| `support`  | temperature=0.3, max_tokens=2048 | "You are a helpful, empathetic customer support..." | Customer service tone |

**System Prompt Templates (examples):**

```yaml
coding:
  session:
    system_prompt_template: |
      You are an expert software engineer with deep knowledge of multiple programming languages.
      - Provide clean, well-documented code following best practices
      - Explain complex concepts clearly with examples
      - Consider performance, security, and maintainability

analysis:
  session:
    system_prompt_template: |
      You are an analytical reasoning expert specializing in structured problem-solving.
      - Break down complex problems into clear components
      - Provide evidence-based reasoning with citations when possible
      - Consider multiple perspectives and trade-offs

support:
  session:
    system_prompt_template: |
      You are a helpful, empathetic customer support assistant.
      - Use friendly, approachable language
      - Be patient and understanding of user frustrations
      - Provide clear, step-by-step guidance
```

**Precedence:** User's explicit `overrides.session.system_prompt` > Style template > Preset default > None

## 5) Configuration Schema

### 5.1) ChatConfigInput (API Request)

```rust
// src/api/config/input.rs

use serde::{Deserialize, Serialize};

/// External API configuration (intent-first)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatConfigInput {
    /// Preset to use as base
    #[serde(default = "default_preset")]
    pub preset: String,
    
    /// User intent
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intent: Option<ChatIntent>,
    
    /// Whitelisted overrides
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overrides: Option<ChatOverrides>,
}

fn default_preset() -> String {
    "balanced".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatIntent {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,  // "general" | "coding" | "analysis" | "support"
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creativity: Option<f32>,  // 0.0 - 1.0
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verbosity: Option<String>,  // "short" | "normal" | "long"
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tooling: Option<String>,  // "off" | "auto" | "max"
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub budget: Option<String>,  // "low" | "normal" | "high"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatOverrides {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<ProviderOverride>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session: Option<SessionOverride>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderOverride {
    /// Whitelisted: model selection
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    
    /// Whitelisted: nucleus sampling (alternative to temperature)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    
    /// Whitelisted: reduce repetition (-2.0 to 2.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f32>,
    
    /// Whitelisted: encourage topic diversity (-2.0 to 2.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionOverride {
    /// Whitelisted: custom system prompt
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
}
```

### 5.2) ChatConfigResolved (Runtime, Stored in Metadata)

```rust
// src/api/config/resolved.rs

use serde::{Deserialize, Serialize};
use crate::llm::{ProviderConfig, LoopDetectorConfig};
use crate::history::{SessionConfig, CompressionConfig, CheckpointConfig};
use crate::agent::AgentConfig;

/// Fully resolved configuration (stored in session metadata)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatConfigResolved {
    /// Provider configuration
    pub provider: ProviderConfig,
    
    /// Agent configuration
    pub agent: AgentConfig,
    
    /// Session configuration
    pub session: SessionConfig,
    
    /// Original intent (for display/debugging)
    pub original_intent: Option<ChatIntent>,
}

impl ChatConfigResolved {
    /// Extract provider config
    pub fn provider_config(&self) -> &ProviderConfig {
        &self.provider
    }
    
    /// Extract agent config
    pub fn agent_config(&self) -> &AgentConfig {
        &self.agent
    }
    
    /// Extract session config
    pub fn session_config(&self) -> &SessionConfig {
        &self.session
    }
}
```

### 5.3) ChatConfigPolicy (Preset Definitions)

```rust
// src/api/config/policy.rs

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Policy file structure (loaded from config.yaml)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatConfigPolicy {
    pub presets: HashMap<String, PresetDefinition>,
    
    /// NEW: Temperature profiles for model-specific creativity mapping
    pub temperature_profiles: HashMap<String, TemperatureProfile>,
}

/// NEW: Model-specific temperature profile
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemperatureProfile {
    /// Fixed temperature (for reasoning models)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fixed_temperature: Option<f32>,
    
    /// Whether to ignore creativity parameter
    #[serde(default)]
    pub ignore_creativity: bool,
    
    /// Creativity → temperature mapping points
    /// Format: [[creativity, temperature], ...]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creativity_map: Option<Vec<[f32; 2]>>,
}

impl ChatConfigPolicy {
    /// Map creativity to temperature for a specific model
    pub fn map_creativity_to_temperature(&self, model: &str, creativity: f32) -> f32 {
        let profile = self.temperature_profiles
            .get(model)
            .or_else(|| self.temperature_profiles.get("default"))
            .expect("Default temperature profile must exist");
        
        // Check if this model ignores creativity (e.g., gpt-5, gemini-3)
        if profile.ignore_creativity {
            return profile.fixed_temperature.unwrap_or(1.0);
        }
        
        // Linear interpolation between points
        if let Some(ref map) = profile.creativity_map {
            let mut prev = &map[0];
            for point in map.iter() {
                if creativity <= point[0] {
                    // Interpolate between prev and point
                    let t = (creativity - prev[0]) / (point[0] - prev[0]);
                    return prev[1] + t * (point[1] - prev[1]);
                }
                prev = point;
            }
            // Beyond last point - use last value
            return prev[1];
        }
        
        // Fallback
        creativity
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresetDefinition {
    /// Default values for this preset
    pub defaults: PresetDefaults,
    
    /// Intent mapping rules
    pub mapping: IntentMapping,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresetDefaults {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<ProviderDefaults>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<AgentDefaults>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session: Option<SessionDefaults>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderDefaults {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentDefaults {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_rounds: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionDefaults {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_context_tokens: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentMapping {
    pub verbosity: HashMap<String, VerbosityRule>,
    pub budget: HashMap<String, BudgetRule>,
    pub tooling: HashMap<String, ToolingRule>,
    pub style: HashMap<String, StyleRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbosityRule {
    pub provider: ProviderAdjustments,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetRule {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<AgentAdjustments>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub optimization: Option<OptimizationAdjustments>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolingRule {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<AgentAdjustments>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StyleRule {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<ProviderAdjustments>,
    
    /// NEW: Session adjustments (system prompt template)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session: Option<SessionAdjustments>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderAdjustments {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_reasoning: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentAdjustments {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_rounds: Option<usize>,
    
    /// NEW: Explicit tool enable/disable
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools_enabled: Option<bool>,
}

/// NEW: Session-level adjustments
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionAdjustments {
    /// System prompt template for this style
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt_template: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationAdjustments {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compression: Option<String>,  // "aggressive" | "balanced" | "minimal"
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checkpoint_every_n_turns: Option<u32>,
}
```

## 6) Global Config File (config.yaml)

### 6.1) File Format

```yaml
# config.yaml - Global policy and preset definitions (2026)

# Temperature profiles for model-specific creativity mapping
temperature_profiles:
  # GPT-5.2 - Only GPT-5 variant with configurable temperature
  gpt-5.2:
    creativity_map:
      - [0.0, 0.0]    # creativity 0.0 → temp 0.0 (reproducible)
      - [0.5, 0.35]   # creativity 0.5 → temp 0.35 (balanced)
      - [1.0, 0.7]    # creativity 1.0 → temp 0.7 (creative, enterprise max)
    notes: "Enterprise default: 0.0-0.7. Can go up to 1.0+ for creative tasks."
  
  # GPT-5 reasoning models - IGNORE creativity (fixed at 1.0)
  gpt-5:
    fixed_temperature: 1.0
    ignore_creativity: true
    notes: "Reasoning model - temperature is fixed and cannot be customized."
  
  gpt-5-mini:
    fixed_temperature: 1.0
    ignore_creativity: true
    notes: "Smaller reasoning model - temperature fixed at 1.0."
  
  gpt-5-nano:
    fixed_temperature: 1.0
    ignore_creativity: true
    notes: "Fastest reasoning model - temperature fixed at 1.0."
  
  # Gemini 3 models - Google strongly recommends fixed 1.0
  gemini-3-flash-preview:
    fixed_temperature: 1.0
    ignore_creativity: true
    notes: "Google docs warn: lower temperature may cause looping/degraded performance."
  
  gemini-3-pro-preview:
    fixed_temperature: 1.0
    ignore_creativity: true
    notes: "Optimized for temperature 1.0 - changing may degrade chain-of-thought."
  
  # Default fallback (conservative)
  default:
    creativity_map:
      - [0.0, 0.0]
      - [0.5, 0.5]
      - [1.0, 1.0]
    notes: "Conservative fallback for unknown models."

presets:
  # Balanced preset - Good for general chat and simple assistance
  balanced:
    defaults:
      provider:
        model: gpt-5-mini
        temperature: 1.0  # Fixed for reasoning models
        max_tokens: 8192
      agent:
        max_rounds: 15
        tools_enabled: true
      session:
        max_context_tokens: 200000
  
  # Agent preset - Optimized for autonomous agent workflows
  agent:
    defaults:
      provider:
        model: gpt-5-mini
        temperature: 1.0
        max_tokens: 32768  # Allow long outputs (tool calls + reasoning + response)
      agent:
        max_rounds: 50  # High limit for complex multi-step tasks
        tools_enabled: true
      session:
        max_context_tokens: 400000  # Use full context window
    
    mapping:
      verbosity:
        short:
          provider:
            max_tokens: 8192
        normal:
          provider:
            max_tokens: 16384
        long:
          provider:
            max_tokens: 32768
      
      budget:
        low:
          agent:
            max_rounds: 15
          optimization:
            compression: aggressive
            checkpoint_every_n_turns: 10
        normal:
          agent:
            max_rounds: 25
          optimization:
            compression: balanced
            checkpoint_every_n_turns: 30
        high:
          agent:
            max_rounds: 50
          optimization:
            compression: minimal
            checkpoint_every_n_turns: 100
      
      tooling:
        off:
          agent:
            tools_enabled: false
            max_rounds: 1  # Single response only
        auto:
          agent:
            tools_enabled: true
            max_rounds: 25
        max:
          agent:
            tools_enabled: true
            max_rounds: 50
      
      style:
        general: {}  # No adjustments
        
        coding:
          provider:
            enable_reasoning: true
            max_tokens: 8192
          session:
            system_prompt_template: |
              You are an expert software engineer with deep knowledge of multiple programming languages.
              - Provide clean, well-documented code following best practices
              - Explain complex concepts clearly with examples
              - Consider performance, security, and maintainability
              - When debugging, systematically analyze root causes
        
        analysis:
          provider:
            enable_reasoning: true
            max_tokens: 16384
          session:
            system_prompt_template: |
              You are an analytical reasoning expert specializing in structured problem-solving.
              - Break down complex problems into clear components
              - Provide evidence-based reasoning with citations when possible
              - Consider multiple perspectives and trade-offs
              - Present findings in a logical, well-organized manner
        
        support:
          provider:
            temperature: 0.3
            max_tokens: 2048
          session:
            system_prompt_template: |
              You are a helpful, empathetic customer support assistant.
              - Use friendly, approachable language
              - Be patient and understanding of user frustrations
              - Provide clear, step-by-step guidance
              - Acknowledge emotions and show genuine care for resolving issues

  # Aggressive preset - GPT-5-nano for speed, aggressive optimization
  aggressive:
    defaults:
      provider:
        model: gpt-5-nano
        temperature: 1.0  # Fixed for reasoning models
        max_tokens: 4096
      agent:
        max_rounds: 10
        tools_enabled: true
      session:
        max_context_tokens: 150000
    
    mapping:
      # Same structure as balanced, but more aggressive defaults
      budget:
        low:
          agent:
            max_rounds: 5
          optimization:
            compression: aggressive
            checkpoint_every_n_turns: 10
        # ...

  # Minimal preset - Gemini 3 Flash for large context, minimal optimization
  minimal:
    defaults:
      provider:
        model: gemini-3-flash-preview
        temperature: 1.0  # Fixed for Gemini 3
        max_tokens: 8192
      agent:
        max_rounds: 20
        tools_enabled: true
      session:
        max_context_tokens: 300000
    
    mapping:
      # Minimal optimization - keep everything
      budget:
        low:
          agent:
            max_rounds: 15
          optimization:
            compression: minimal
            checkpoint_every_n_turns: 200
        # ...
```

## 7) Configuration Resolver

### 7.1) Resolver Implementation

```rust
// src/api/config/resolver.rs

use anyhow::{Result, Context, bail};
use std::path::PathBuf;

pub struct ConfigResolver {
    policy: ChatConfigPolicy,
}

impl ConfigResolver {
    /// Load from config.yaml
    pub fn new() -> Result<Self> {
        let config_path = PathBuf::from("config.yaml");
        
        let policy = if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)
                .context("Failed to read config.yaml")?;
            serde_yaml::from_str(&content)
                .context("Failed to parse config.yaml")?
        } else {
            // Use built-in defaults if no config.yaml
            Self::built_in_policy()
        };
        
        Ok(Self { policy })
    }
    
    /// Resolve input config to runtime config
    pub fn resolve(
        &self,
        session_id: &str,
        input: ChatConfigInput,
    ) -> Result<ChatConfigResolved> {
        // 1. Get preset definition
        let preset = self.policy.presets.get(&input.preset)
            .ok_or_else(|| anyhow::anyhow!("Unknown preset: {}", input.preset))?;
        
        // 2. Start with preset defaults
        let mut provider_config = ProviderConfig::default();
        let mut agent_config = AgentConfig::default();
        let mut session_config = SessionConfig::default();
        
        // Apply preset defaults
        if let Some(ref p) = preset.defaults.provider {
            if let Some(ref model) = p.model {
                // Model is applied via override, not here
            }
            if let Some(temp) = p.temperature {
                provider_config.temperature = temp;
            }
            if let Some(max) = p.max_tokens {
                provider_config.max_tokens = max;
            }
        }
        
        if let Some(ref a) = preset.defaults.agent {
            if let Some(rounds) = a.max_rounds {
                agent_config.max_rounds = rounds;
            }
        }
        
        if let Some(ref s) = preset.defaults.session {
            if let Some(max) = s.max_context_tokens {
                session_config.max_context_tokens = max;
            }
        }
        
        // 3. Apply intent mappings
        if let Some(ref intent) = input.intent {
            self.apply_intent(&preset.mapping, intent, 
                &mut provider_config, &mut agent_config, &mut session_config)?;
        }
        
        // 4. Apply whitelisted overrides
        if let Some(ref overrides) = input.overrides {
            self.apply_overrides(overrides, 
                &mut provider_config, &mut session_config)?;
        }
        
        // 5. System prompt belongs to session, not provider
        // (Already handled in apply_overrides)
        
        Ok(ChatConfigResolved {
            provider: provider_config,
            agent: agent_config,
            session: session_config,
            original_intent: input.intent.clone(),
        })
    }
    
    fn apply_intent(
        &self,
        mapping: &IntentMapping,
        intent: &ChatIntent,
        provider: &mut ProviderConfig,
        agent: &mut AgentConfig,
        session: &mut SessionConfig,
    ) -> Result<()> {
        // IMPORTANT: Order matters - later overrides earlier
        
        // 1. Apply style (domain-specific defaults, sets base persona)
        if let Some(ref style) = intent.style {
            if let Some(rule) = mapping.style.get(style) {
                if let Some(ref prov_adj) = rule.provider {
                    if let Some(reasoning) = prov_adj.enable_reasoning {
                        provider.enable_reasoning = reasoning;
                    }
                    if let Some(max) = prov_adj.max_tokens {
                        provider.max_tokens = max;
                    }
                    if let Some(temp) = prov_adj.temperature {
                        provider.temperature = temp;
                    }
                }
                
                // Apply system prompt template (only if user hasn't provided custom prompt)
                if let Some(ref sess_adj) = rule.session {
                    if let Some(ref template) = sess_adj.system_prompt_template {
                        if session.system_prompt.is_none() {
                            session.system_prompt = Some(template.clone());
                        }
                    }
                }
            }
        }
        
        // 2. Apply budget (optimization strategy)
        if let Some(ref budget) = intent.budget {
            if let Some(rule) = mapping.budget.get(budget) {
                if let Some(ref agent_adj) = rule.agent {
                    if let Some(rounds) = agent_adj.max_rounds {
                        agent.max_rounds = rounds;
                    }
                }
                
                if let Some(ref opt_adj) = rule.optimization {
                    if let Some(ref compression) = opt_adj.compression {
                        session.optimization.compression = match compression.as_str() {
                            "aggressive" => CompressionConfig::aggressive(),
                            "balanced" => CompressionConfig::default(),
                            "minimal" => CompressionConfig::minimal(),
                            _ => CompressionConfig::default(),
                        };
                    }
                    
                    if let Some(turns) = opt_adj.checkpoint_every_n_turns {
                        session.optimization.checkpoint.every_n_turns = Some(turns);
                    }
                }
            }
        }
        
        // 3. Apply verbosity (explicit length preference - OVERRIDES style)
        if let Some(ref verb) = intent.verbosity {
            if let Some(rule) = mapping.verbosity.get(verb) {
                if let Some(max) = rule.provider.max_tokens {
                    provider.max_tokens = max;  // Overrides style setting
                }
            }
        }
        
        // 4. Apply tooling (explicit tool behavior)
        if let Some(ref tooling) = intent.tooling {
            if let Some(rule) = mapping.tooling.get(tooling) {
                if let Some(ref agent_adj) = rule.agent {
                    if let Some(enabled) = agent_adj.tools_enabled {
                        agent.tools_enabled = enabled;
                    }
                    if let Some(rounds) = agent_adj.max_rounds {
                        agent.max_rounds = rounds;
                    }
                }
            }
        }
        
        // 5. Apply creativity (model-specific temperature - FINAL temperature)
        if let Some(creativity) = intent.creativity {
            Self::validate_creativity(creativity)?;
            
            // Use model-specific temperature profile
            let temperature = self.policy.map_creativity_to_temperature(
                &provider.model,
                creativity,
            );
            
            provider.temperature = temperature;  // Overrides all previous temperature settings
        }
        
        Ok(())
    }
    
    fn apply_overrides(
        &self,
        overrides: &ChatOverrides,
        provider: &mut ProviderConfig,
        session: &mut SessionConfig,
    ) -> Result<()> {
        // Whitelist: provider overrides
        if let Some(ref prov_override) = overrides.provider {
            // Model selection
            if let Some(ref model) = prov_override.model {
                Self::validate_model(model)?;
                provider.model = model.clone();  // Store in ProviderConfig
            }
            
            // Sampling parameters (power user overrides)
            if let Some(top_p) = prov_override.top_p {
                Self::validate_top_p(top_p)?;
                provider.top_p = Some(top_p);
            }
            
            if let Some(freq) = prov_override.frequency_penalty {
                Self::validate_frequency_penalty(freq)?;
                provider.extra_options.insert(
                    "frequency_penalty".to_string(), 
                    serde_json::json!(freq)
                );
            }
            
            if let Some(pres) = prov_override.presence_penalty {
                Self::validate_presence_penalty(pres)?;
                provider.extra_options.insert(
                    "presence_penalty".to_string(),
                    serde_json::json!(pres)
                );
            }
        }
        
        // Whitelist: session.system_prompt
        if let Some(ref sess_override) = overrides.session {
            if let Some(ref prompt) = sess_override.system_prompt {
                Self::validate_system_prompt(prompt)?;
                session.system_prompt = Some(prompt.clone());
            }
        }
        
        Ok(())
    }
    
    /// Apply temporary overrides to existing resolved config
    pub fn apply_temporary_overrides(
        &self,
        mut base_config: ChatConfigResolved,
        temp_config: &ChatConfigInput,
    ) -> Result<ChatConfigResolved> {
        // Get preset mapping (use base config's preset or default to "balanced")
        let preset_name = &temp_config.preset;
        let preset = self.policy.presets.get(preset_name)
            .ok_or_else(|| anyhow::anyhow!("Unknown preset: {}", preset_name))?;
        
        // Apply temporary intent changes
        if let Some(ref intent) = temp_config.intent {
            self.apply_intent(
                &preset.mapping, 
                intent,
                &mut base_config.provider,
                &mut base_config.agent,
                &mut base_config.session,
            )?;
        }
        
        // Apply temporary overrides (if any)
        if let Some(ref overrides) = temp_config.overrides {
            self.apply_overrides(
                overrides,
                &mut base_config.provider,
                &mut base_config.session,
            )?;
        }
        
        Ok(base_config)
    }
    
    fn validate_creativity(creativity: f32) -> Result<()> {
        if creativity < 0.0 || creativity > 1.0 {
            bail!("creativity must be between 0.0 and 1.0, got {}", creativity);
        }
        Ok(())
    }
    
    fn validate_model(model: &str) -> Result<()> {
        const ALLOWED_MODELS: &[&str] = &[
            // OpenAI GPT-5 series (2026)
            "gpt-5",
            "gpt-5-mini",
            "gpt-5-nano",
            "gpt-5.2",
            // Google Gemini 3 series (2026)
            "gemini-3-flash-preview",
            "gemini-3-pro-preview",
        ];
        
        if !ALLOWED_MODELS.contains(&model) {
            bail!("Model '{}' not in whitelist. Allowed: {:?}", model, ALLOWED_MODELS);
        }
        Ok(())
    }
    
    fn validate_top_p(value: f32) -> Result<()> {
        if !(0.0..=1.0).contains(&value) {
            bail!("top_p must be between 0.0 and 1.0, got {}", value);
        }
        Ok(())
    }
    
    fn validate_frequency_penalty(value: f32) -> Result<()> {
        if !(-2.0..=2.0).contains(&value) {
            bail!("frequency_penalty must be between -2.0 and 2.0, got {}", value);
        }
        Ok(())
    }
    
    fn validate_presence_penalty(value: f32) -> Result<()> {
        if !(-2.0..=2.0).contains(&value) {
            bail!("presence_penalty must be between -2.0 and 2.0, got {}", value);
        }
        Ok(())
    }
    
    fn validate_system_prompt(prompt: &str) -> Result<()> {
        if prompt.len() > 10000 {
            bail!("system_prompt too long (max 10000 chars)");
        }
        Ok(())
    }
    
    /// Built-in policy as safety net
    /// Used when config.yaml doesn't exist or fails to parse
    fn built_in_policy() -> ChatConfigPolicy {
        // SAFETY NET: Hardcoded defaults ensure system always works
        // Even if config.yaml is missing or malformed
        
        const BUILT_IN_YAML: &str = include_str!("../../../config.default.yaml");
        
        serde_yaml::from_str(BUILT_IN_YAML)
            .expect("Built-in policy YAML must be valid (this is a compile-time guarantee)")
    }
    
    /// Load with graceful fallback
    pub fn new_with_fallback() -> Self {
        match Self::new() {
            Ok(resolver) => {
                log::info!("Loaded config from config.yaml");
                resolver
            }
            Err(e) => {
                log::warn!("Failed to load config.yaml: {}. Using built-in defaults.", e);
                Self {
                    policy: Self::built_in_policy(),
                }
            }
        }
    }
}

// Ensure ProviderConfig has safe defaults
impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            model: "gpt-5-mini".to_string(),  // Safe default (2026)
            temperature: 1.0,
            max_tokens: 4096,
            top_p: None,
            top_k: None,
            enable_reasoning: false,
            stop_sequences: Vec::new(),
            extra_options: HashMap::new(),
        }
    }
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            max_rounds: 12,
            tools_enabled: true,  // Safe default
            loop_detection: Some(LoopDetectorConfig::default()),
        }
    }
}

impl CompressionConfig {
    pub fn aggressive() -> Self {
        Self {
            full_context_turns: 1,
            summary_threshold_turns: 5,
            result_size_threshold: 300,
            preview_size: 200,
        }
    }
    
    pub fn minimal() -> Self {
        Self {
            full_context_turns: 100,
            summary_threshold_turns: 1000,
            result_size_threshold: 10000,
            preview_size: 1000,
        }
    }
}
```

## 8) API Integration

### 8.1) Updated Chat Endpoint

```rust
// src/api/chat.rs

use axum::{
    extract::{Path, Json, State},
    http::StatusCode,
};

#[derive(Debug, Deserialize)]
pub struct ChatRequest {
    pub message: String,
    
    /// Persistent config (only for new sessions)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<ChatConfigInput>,
    
    /// Temporary config (only affects this request, not saved to metadata)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temporary_config: Option<ChatConfigInput>,
}

#[derive(Debug, Serialize)]
pub struct ChatResponse {
    pub stream_id: String,
    
    /// Preview of resolved config (for debugging)
    pub config_preview: ConfigPreview,
}

#[derive(Debug, Serialize)]
pub struct ConfigPreview {
    pub provider: ProviderPreview,
    pub agent: AgentPreview,
    pub session: SessionPreview,
}

#[derive(Debug, Serialize)]
pub struct ProviderPreview {
    pub model: String,
    pub temperature: f32,
    pub max_tokens: u32,
}

#[derive(Debug, Serialize)]
pub struct AgentPreview {
    pub max_rounds: usize,
}

#[derive(Debug, Serialize)]
pub struct SessionPreview {
    pub system_prompt: Option<String>,
    pub max_context_tokens: u32,
}

async fn chat(
    Path(session_id): Path<String>,
    Json(request): Json<ChatRequest>,
) -> Result<Json<ChatResponse>, (StatusCode, String)> {
    // 1. Resolve configuration
    let resolver = ConfigResolver::new()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    
    let input = request.config.unwrap_or_else(|| ChatConfigInput {
        preset: "balanced".to_string(),
        intent: None,
        overrides: None,
    });
    
    let resolved = resolver.resolve(&session_id, input)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    
    // 2. Check if session exists
    let store = Arc::new(MemoryStore::new());  // TODO: Use persistent store
    
    let mut session = if let Some(existing) = load_session(&session_id).await? {
        existing
    } else {
        // New session - create with resolved config
        let mut sess = Session::new(store, resolved.session_config().clone()).await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        
        // Save resolved config to metadata (single source of truth)
        sess.save_resolved_config(&resolved)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        
        sess
    };
    
    // 3. For existing sessions, load config from metadata
    let resolved = if session.is_new() {
        resolved
    } else {
        session.load_resolved_config()
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
            .ok_or_else(|| (StatusCode::INTERNAL_SERVER_ERROR, 
                "Session exists but has no resolved config".to_string()))?
    };
    
    // 4. Create provider with model from overrides
    let model = request.config
        .and_then(|c| c.overrides)
        .and_then(|o| o.provider)
        .and_then(|p| p.model)
        .unwrap_or_else(|| "gpt-4o-mini".to_string());
    
    let provider = OpenAIProvider::create(model.clone(), api_key)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    
    // Apply provider config
    provider.update_config(Box::new(move |cfg| {
        *cfg = resolved.provider_config().clone();
    }));
    
    // 5. Create agent with resolved config
    let agent = Agent::with_config(
        session,
        provider,
        tools,
        resolved.agent_config().clone(),
    );
    
    // 6. Generate stream ID and start chat
    let stream_id = generate_stream_id();
    
    // Start chat in background...
    
    // 7. Return response with config preview
    Ok(Json(ChatResponse {
        stream_id,
        config_preview: ConfigPreview {
            provider: ProviderPreview {
                model,
                temperature: resolved.provider.temperature,
                max_tokens: resolved.provider.max_tokens,
            },
            agent: AgentPreview {
                max_rounds: resolved.agent.max_rounds,
            },
            session: SessionPreview {
                system_prompt: resolved.session.system_prompt.clone(),
                max_context_tokens: resolved.session.max_context_tokens,
            },
        },
    }))
}
```

### 8.2) Session Metadata Management

```rust
// src/history/session.rs

impl Session {
    /// Save resolved config to metadata (single source of truth)
    pub fn save_resolved_config(&mut self, config: &ChatConfigResolved) -> Result<()> {
        let config_json = serde_json::to_value(config)?;
        
        if self.metadata.is_none() {
            self.metadata = Some(serde_json::json!({}));
        }
        
        if let Some(serde_json::Value::Object(ref mut map)) = self.metadata {
            map.insert("resolved_config".to_string(), config_json);
        }
        
        Ok(())
    }
    
    /// Load resolved config from metadata
    pub fn load_resolved_config(&self) -> Result<Option<ChatConfigResolved>> {
        if let Some(serde_json::Value::Object(ref map)) = self.metadata {
            if let Some(config_value) = map.get("resolved_config") {
                let config: ChatConfigResolved = serde_json::from_value(config_value.clone())?;
                return Ok(Some(config));
            }
        }
        Ok(None)
    }
}
```

## 9) Frontend Integration

### 9.1) Two-Layer Configuration UI

```typescript
// web/src/components/ConfigPanel.tsx

interface ConfigPanelProps {
  onSubmit: (config: ChatConfigInput) => void;
  onPreview?: (preview: ConfigPreview) => void;
}

export function ConfigPanel({ onSubmit, onPreview }: ConfigPanelProps) {
  const [preset, setPreset] = useState<'balanced' | 'aggressive' | 'minimal' | 'custom'>('balanced');
  const [intent, setIntent] = useState<ChatIntent>({
    style: 'general',
    creativity: 0.5,
    verbosity: 'normal',
    tooling: 'auto',
    budget: 'normal',
  });
  const [overrides, setOverrides] = useState<ChatOverrides>({});
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [preview, setPreview] = useState<ConfigPreview | null>(null);
  
  const handlePreviewClick = async () => {
    // Call API to get resolved config preview
    const response = await fetch('/api/config/preview', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ preset, intent, overrides }),
    });
    
    const data = await response.json();
    setPreview(data.preview);
    onPreview?.(data.preview);
  };
  
  return (
    <div className="card bg-base-200">
      <div className="card-body">
        <h2 className="card-title">Session Configuration</h2>
        
        {/* Layer 1: Simple Intent (99% users) */}
        <div className="space-y-4">
          {/* Preset selector */}
          <div className="form-control">
            <label className="label">
              <span className="label-text">Preset</span>
            </label>
            <select 
              className="select select-bordered"
              value={preset}
              onChange={(e) => setPreset(e.target.value as any)}
            >
              <option value="balanced">Balanced - General Chat</option>
              <option value="agent">Agent - Autonomous Tasks (Recommended)</option>
              <option value="aggressive">Aggressive - Quick Sessions</option>
              <option value="minimal">Minimal - Maximum Context</option>
            </select>
          </div>
          
          {/* Style */}
          <div className="form-control">
            <label className="label">
              <span className="label-text">Conversation Style</span>
            </label>
            <select 
              className="select select-bordered"
              value={intent.style}
              onChange={(e) => setIntent({...intent, style: e.target.value})}
            >
              <option value="general">General Purpose</option>
              <option value="coding">Code Generation</option>
              <option value="analysis">Data Analysis</option>
              <option value="support">Customer Support</option>
            </select>
          </div>
          
          {/* Creativity slider */}
          <div className="form-control">
            <label className="label">
              <span className="label-text">
                Creativity: {intent.creativity?.toFixed(1)}
              </span>
            </label>
            <input 
              type="range" 
              min="0" 
              max="1" 
              step="0.1"
              value={intent.creativity}
              onChange={(e) => setIntent({...intent, creativity: parseFloat(e.target.value)})}
              className="range range-primary"
            />
            <div className="flex justify-between text-xs px-2">
              <span>Factual</span>
              <span>Balanced</span>
              <span>Creative</span>
            </div>
          </div>
          
          {/* Verbosity */}
          <div className="form-control">
            <label className="label">
              <span className="label-text">Response Length</span>
            </label>
            <div className="btn-group w-full">
              {['short', 'normal', 'long'].map((v) => (
                <button
                  key={v}
                  className={`btn btn-sm flex-1 ${intent.verbosity === v ? 'btn-active' : ''}`}
                  onClick={() => setIntent({...intent, verbosity: v})}
                >
                  {v.charAt(0).toUpperCase() + v.slice(1)}
                </button>
              ))}
            </div>
          </div>
          
          {/* Budget */}
          <div className="form-control">
            <label className="label">
              <span className="label-text">Budget</span>
            </label>
            <div className="btn-group w-full">
              {['low', 'normal', 'high'].map((b) => (
                <button
                  key={b}
                  className={`btn btn-sm flex-1 ${intent.budget === b ? 'btn-active' : ''}`}
                  onClick={() => setIntent({...intent, budget: b})}
                >
                  {b.charAt(0).toUpperCase() + b.slice(1)}
                </button>
              ))}
            </div>
          </div>
          
          {/* Tooling */}
          <div className="form-control">
            <label className="label">
              <span className="label-text">Tool Usage</span>
            </label>
            <div className="btn-group w-full">
              {['off', 'auto', 'max'].map((t) => (
                <button
                  key={t}
                  className={`btn btn-sm flex-1 ${intent.tooling === t ? 'btn-active' : ''}`}
                  onClick={() => setIntent({...intent, tooling: t})}
                >
                  {t.charAt(0).toUpperCase() + t.slice(1)}
                </button>
              ))}
            </div>
          </div>
        </div>
        
        {/* Layer 2: Advanced (power users) */}
        <div className="collapse collapse-arrow bg-base-100 mt-4">
          <input 
            type="checkbox" 
            checked={showAdvanced}
            onChange={(e) => setShowAdvanced(e.target.checked)}
          />
          <div className="collapse-title font-medium">
            Advanced Overrides (Power Users)
          </div>
          <div className="collapse-content space-y-4">
            {/* Model selection (whitelist) */}
            <div className="form-control">
              <label className="label">
                <span className="label-text">Model Override</span>
              </label>
              <select 
                className="select select-bordered"
                value={overrides.provider?.model || ''}
                onChange={(e) => setOverrides({
                  ...overrides,
                  provider: { model: e.target.value || undefined }
                })}
              >
                <option value="">Use Preset Default</option>
                <option value="gpt-4o">GPT-4o</option>
                <option value="gpt-4o-mini">GPT-4o Mini</option>
                <option value="claude-3-5-sonnet-20241022">Claude 3.5 Sonnet</option>
                <option value="gemini-2.0-flash-exp">Gemini 2.0 Flash</option>
              </select>
            </div>
            
            {/* Custom system prompt */}
            <div className="form-control">
              <label className="label">
                <span className="label-text">System Prompt Override</span>
              </label>
              <textarea
                className="textarea textarea-bordered"
                placeholder="Custom system prompt..."
                value={overrides.session?.system_prompt || ''}
                onChange={(e) => setOverrides({
                  ...overrides,
                  session: { system_prompt: e.target.value || undefined }
                })}
                maxLength={10000}
              />
            </div>
          </div>
        </div>
        
        {/* Resolved Config Preview */}
        <div className="mt-4">
          <button 
            className="btn btn-sm btn-outline"
            onClick={handlePreviewClick}
          >
            Show Resolved Config
          </button>
          
          {preview && (
            <div className="mockup-code mt-2">
              <pre><code>{JSON.stringify(preview, null, 2)}</code></pre>
            </div>
          )}
        </div>
        
        {/* Submit */}
        <div className="card-actions justify-end mt-4">
          <button 
            className="btn btn-primary"
            onClick={() => onSubmit({ preset, intent, overrides })}
          >
            Start Session
          </button>
        </div>
      </div>
    </div>
  );
}
```

### 9.2) Config Preview API

```rust
// src/api/mod.rs

/// POST /api/config/preview
/// Preview resolved configuration without creating a session
async fn preview_config(
    Json(input): Json<ChatConfigInput>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let resolver = ConfigResolver::new()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    
    let resolved = resolver.resolve("preview", input)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    
    let model = input.overrides
        .and_then(|o| o.provider)
        .and_then(|p| p.model)
        .unwrap_or_else(|| "gpt-4o-mini".to_string());
    
    Ok(Json(serde_json::json!({
        "preview": {
            "provider": {
                "model": model,
                "temperature": resolved.provider.temperature,
                "max_tokens": resolved.provider.max_tokens,
                "enable_reasoning": resolved.provider.enable_reasoning,
            },
            "agent": {
                "max_rounds": resolved.agent.max_rounds,
            },
            "session": {
                "system_prompt": resolved.session.system_prompt,
                "max_context_tokens": resolved.session.max_context_tokens,
            },
        }
    })))
}
```

## 10) Implementation Tasks

### Phase 1: Core Configuration System
- [ ] Create `src/api/config/input.rs` with ChatConfigInput, ChatIntent, ChatOverrides
- [ ] Create `src/api/config/resolved.rs` with ChatConfigResolved
- [ ] Create `src/api/config/policy.rs` with ChatConfigPolicy, TemperatureProfile
- [ ] Create `src/api/config/resolver.rs` with ConfigResolver
- [ ] Add `serde_yaml` dependency to Cargo.toml
- [ ] Add `rust-embed` for embedding config.default.yaml

### Phase 2: Configuration Files
- [ ] Create `config.yaml` with all 4 presets (balanced, agent, aggressive, minimal)
- [ ] Create `config.default.yaml` as built-in safety net
- [ ] Define temperature_profiles for all supported models
- [ ] Add system_prompt_template for each style (coding, analysis, support)

### Phase 3: Intent Mapping
- [ ] Implement `apply_intent()` with correct order (style → budget → verbosity → tooling → creativity)
- [ ] Implement `map_creativity_to_temperature()` with linear interpolation
- [ ] Implement model-specific temperature profiles
- [ ] Add validation functions (validate_model, validate_creativity, validate_top_p, etc.)

### Phase 4: Override System
- [ ] Implement `apply_overrides()` with whitelist enforcement
- [ ] Add support for top_p, frequency_penalty, presence_penalty
- [ ] Store model in ProviderConfig.model
- [ ] Implement `apply_temporary_overrides()` for per-request config

### Phase 5: Session Integration
- [ ] Add `tools_enabled: bool` to AgentConfig
- [ ] Add `model: String` to ProviderConfig
- [ ] Update `Session::save_resolved_config()` to store in metadata
- [ ] Update `Session::load_resolved_config()` to read from metadata
- [ ] Remove session-specific config files (only use metadata)

### Phase 6: API Endpoints
- [ ] Update `POST /api/sessions/{id}/chat` to support config + temporary_config
- [ ] Add `PATCH /api/sessions/{id}/config` for permanent updates
- [ ] Add `POST /api/config/preview` for config preview
- [ ] Implement update safety policy (freely updatable vs immutable)

### Phase 7: Default Implementations
- [ ] Add safe defaults to ProviderConfig::default() (gpt-5-mini, temp 1.0, 16K tokens)
- [ ] Add safe defaults to AgentConfig::default() (25 rounds, tools_enabled=true)
- [ ] Implement `ConfigResolver::new_with_fallback()` for graceful degradation
- [ ] Implement `built_in_policy()` with embedded YAML

### Phase 8: Frontend
- [ ] Create `ConfigPanel.tsx` with two-layer UI
- [ ] Add preset selector (balanced/agent/aggressive/minimal)
- [ ] Add intent controls (style, creativity slider, verbosity, tooling, budget)
- [ ] Add advanced overrides panel (model, top_p, penalties, system_prompt)
- [ ] Add resolved config preview
- [ ] Add temporary config support in chat UI

### Phase 9: Testing
- [ ] Unit tests for temperature mapping (model-specific profiles)
- [ ] Unit tests for intent merge order
- [ ] Integration tests for config resolution
- [ ] Integration tests for temporary_config
- [ ] Integration tests for PATCH /config endpoint
- [ ] Test agent preset with high limits (50 rounds, 32K tokens)

### Phase 10: Documentation
- [ ] Update API documentation with examples
- [ ] Document all 4 presets and their use cases
- [ ] Document agent preset as recommended default
- [ ] Document update safety policy
- [ ] Add migration guide from old config system

## 11) Acceptance Criteria

- [ ] Users can specify intent (style, creativity, verbosity, budget, tooling)
- [ ] Creativity maps linearly to temperature (0.0-1.0 → 0.0-1.2)
- [ ] Verbosity controls max_tokens (short/normal/long)
- [ ] Budget controls rounds + compression + checkpointing
- [ ] Tooling controls max_rounds and tool behavior
- [ ] Only whitelisted overrides allowed (model, system_prompt)
- [ ] Deep merging rejected with 400 Bad Request
- [ ] Resolved config stored in session metadata
- [ ] Config loaded from metadata for existing sessions
- [ ] Config preview API returns accurate resolved config
- [ ] Frontend shows simple UI for 99% of users
- [ ] Frontend shows advanced panel for power users
- [ ] System prompt belongs to session, not provider

## 12) Migration from Old Design

### Breaking Changes

1. **API payload structure changed** - Old `config` structure no longer works
2. **No arbitrary deep overrides** - Only whitelist allowed
3. **System prompt moved** - Was `provider.system_prompt`, now `session.system_prompt`

### Migration Path

1. Update all API clients to use new ChatConfigInput structure
2. Convert old config files to new preset-based format
3. Add deprecation warnings for old endpoints
4. Provide migration script for existing session configs

---

## 13) Supported Models (2026)

### Model Lineup

We support only the newest models from OpenAI and Google:

| Model | Provider | Type | Context | Temperature | Pricing (per 1M tokens) |
|-------|----------|------|---------|-------------|------------------------|
| **gpt-5.2** | OpenAI | General | 400K | 0.0-1.0+ configurable | (check latest) |
| **gpt-5** | OpenAI | Reasoning | 400K | Fixed 1.0 | (check latest) |
| **gpt-5-mini** | OpenAI | Reasoning | 400K | Fixed 1.0 | $0.25 / $2.00 |
| **gpt-5-nano** | OpenAI | Reasoning | 400K | Fixed 1.0 | $0.05 / $0.40 |
| **gemini-3-flash-preview** | Google | Reasoning | (TBD) | Fixed 1.0 | (check latest) |
| **gemini-3-pro-preview** | Google | Reasoning | (TBD) | Fixed 1.0 | (check latest) |

### Key Characteristics

**OpenAI GPT-5 Series:**
- **gpt-5.2**: Only variant with configurable temperature (0.0-1.0+, enterprise default 0.0-0.7)
- **gpt-5/mini/nano**: Reasoning models with FIXED temperature 1.0 (cannot customize)
- All have 400,000 token context windows
- `gpt-5-nano` is fastest and cheapest option

**Google Gemini 3 Series:**
- **Both Flash and Pro**: Fixed temperature 1.0 (Google strongly recommends)
- Lowering temperature may cause **looping or degraded chain-of-thought performance**
- Support new parameters: `thinking_level`, `media_resolution`

### Temperature Behavior Summary

```yaml
# Creativity parameter mapping per model:

gpt-5.2:
  - creativity: 0.0 → temperature: 0.0 (reproducible outputs)
  - creativity: 0.5 → temperature: 0.35 (balanced)
  - creativity: 1.0 → temperature: 0.7 (creative, enterprise max)

gpt-5, gpt-5-mini, gpt-5-nano:
  - creativity: IGNORED
  - temperature: ALWAYS 1.0 (fixed by OpenAI)

gemini-3-flash-preview, gemini-3-pro-preview:
  - creativity: IGNORED
  - temperature: ALWAYS 1.0 (Google recommendation)
```

### Preset Comparison

Our four presets are optimized for different use cases:

| Preset | Default Model | max_tokens | max_rounds | max_context | Use Case |
|--------|---------------|------------|------------|-------------|----------|
| **balanced** | gpt-5-mini | 8K | 15 | 200K | General chat, simple assistance |
| **agent** | gpt-5-mini | 32K | 50 | 400K | **Autonomous agents, complex tasks** |
| **aggressive** | gpt-5-nano | 8K | 10 | 150K | Quick sessions, cost-sensitive |
| **minimal** | gemini-3-flash | 16K | 20 | 300K | Maximum context preservation |

**Recommendation for agent workloads**: Use `preset: "agent"` as your default. The high limits (50 rounds, 32K output) are designed for real agent scenarios where the system needs to:
- Execute multiple tool calls
- Generate long reasoning chains
- Produce detailed code/analysis
- Recover from errors and retry

**Cost consideration**: Even with high limits, actual cost depends on usage. An agent that completes a task in 10 rounds with 5K tokens/round costs less than a chat session with 100 back-and-forth messages.

### Why No Older Models?

We removed support for older models (gpt-4o, claude-3-5-sonnet, etc.) because:
1. **GPT-5 series** provides better reasoning at similar/lower cost
2. **Gemini 3** has superior context windows and performance
3. **Simplicity** - fewer models = easier to maintain temperature profiles
4. **2026 best practices** - reasoning models are now standard

Users can still override to use older models via `overrides.provider.model`, but they won't have optimized temperature profiles.

---

## References

- Parent plan: [chat-ui-plan.md](./chat-ui-plan.md)
- Fixes document: [chat-ui-config-fixes.md](./chat-ui-config-fixes.md)
- Related code:
  - `src/agent/mod.rs` - AgentConfig
  - `src/llm/provider.rs` - ProviderConfig
  - `src/history/session.rs` - SessionConfig
  - `src/history/compressor.rs` - CompressionConfig

### External Documentation

- [GPT-5 Temperature Behavior](https://hippocampus-garden.com/llm_temperature/)
- [Gemini 3 Developer Guide](https://ai.google.dev/gemini-api/docs/gemini-3)
- [GPT-5 API Documentation](https://platform.openai.com/docs/models/gpt-5-nano)
- [Gemini 3 Flash Documentation](https://docs.cloud.google.com/vertex-ai/generative-ai/docs/models/gemini/3-flash)
