# Chat UI Configuration Plan

- Feature name: `chat-ui-config`
- Status: Implemented
- Created: 2026-01-07
- Updated: 2026-01-08
- Implemented: 2026-01-08
- Parent plan: [chat-ui-plan.md](./chat-ui-plan.md)
- Implementation notes: [chat-ui-config-implementation.md](../implementation/chat-ui-config-implementation.md)

## 1) Overview

### Goal
Simple, intent-first configuration API. Users specify what they want (creativity, verbosity, rounds), not how to achieve it.

### Design Principles

1. **Intent over parameters** - Expose goals, not technical details
2. **Resolved config in metadata** - Single source of truth, never recalculated
3. **Presets for common cases** - general, coding, research, quick
4. **Overrides for power users** - Whitelist: model, sampling params

## 2) API Design

### Request Example

```json
POST /api/sessions/{session_id}/chat
{
  "message": "Explain quantum physics",
  "config": {
    "preset": "general",
    "system_prompt": "You are a helpful physics tutor.",
    "tools_enabled": true,
    "intent": {
      "creativity": 0.3,
      "verbosity": "normal",
      "rounds": 30
    },
    "overrides": {
      "model": "gpt-5-mini",
      "top_p": 0.9
    }
  }
}
```

### Top-Level Config Fields

**`preset`** - Preset name (default: "general")
- `"general"`, `"coding"`, `"research"`, `"quick"`

**`system_prompt`** - Custom system prompt (optional)
- String, max 10,000 characters
- **Immutable**: Can ONLY be set during session creation
- If not provided, uses preset's default system prompt
- To change: Create a new session

**`tools_enabled`** - Enable/disable tool calling (boolean)
- Default: `true`
- When `false`, agent gives single response without calling tools

### Intent Fields

**`creativity`** - Temperature mapping (0.0 - 1.0)
- `0.0` = Deterministic, factual
- `0.5` = Balanced
- `1.0` = Creative, exploratory

**`verbosity`** - Output length
- `"short"` = 8K tokens max
- `"normal"` = 16K tokens max
- `"long"` = 32K tokens max

**`rounds`** - Maximum agent execution rounds (number)
- Default: `30`
- Controls how many turns the agent can take
- Higher values allow more complex multi-step tasks
- Common values: `10` (quick), `30` (standard), `50` (complex)

### Overrides (Whitelist)

**Allowed:**
- `model` - Switch LLM provider
- `top_p` - Nucleus sampling (0.0-1.0)
- `frequency_penalty` - Reduce repetition (-2.0 to 2.0)
- `presence_penalty` - Topic diversity (-2.0 to 2.0)

**Not allowed:**
- Direct `temperature` (use `creativity` intent)
- Direct `max_tokens` (use `verbosity` intent)
- Direct `max_rounds` (use `rounds` intent)
- Direct `tools_enabled` (use top-level field)
- Direct `system_prompt` (use top-level field, and only at session creation)
- Any other provider/agent parameters

### Response

```json
{
  "stream_id": "stream-abc123",
  "resolved_config": {
    "provider": {
      "model": "gpt-5-mini",
      "temperature": 0.3,
      "max_tokens": 16384
    },
    "agent": {
      "max_rounds": 25,
      "tools_enabled": true
    },
    "session": {
      "system_prompt": "You are a helpful physics tutor.",
      "max_context_tokens": 200000
    }
  }
}
```

## 3) Presets (Purpose-Oriented)

Each preset includes **default system prompt** + **optimized parameters** for its use case.

### general (default)
```yaml
system_prompt: "You are a helpful, friendly assistant."
defaults:
  model: gpt-5-mini
  temperature: 1.0
  max_tokens: 16384
  max_rounds: 30
  max_context_tokens: 200000
  compression: balanced
  tools_enabled: true
```

**Use case**: General conversation, Q&A, everyday tasks

### coding
```yaml
system_prompt: |
  You are an expert software engineer with deep knowledge of multiple programming languages.
  - Write clean, well-documented code following best practices
  - Consider performance, security, and maintainability
  - Use tools to read/write files when needed
  - Explain complex concepts clearly with examples
defaults:
  model: gpt-5-mini
  temperature: 1.0
  max_tokens: 32768  # Allow long code generation
  max_rounds: 40
  max_context_tokens: 300000
  compression: minimal  # Preserve code context
  tools_enabled: true
```

**Use case**: Code generation, debugging, refactoring, code review

### research
```yaml
system_prompt: |
  You are a thorough research assistant specializing in systematic analysis.
  - Break down complex problems into clear components
  - Use tools to search and analyze information
  - Provide evidence-based reasoning with citations when possible
  - Consider multiple perspectives and trade-offs
defaults:
  model: gpt-5-mini
  temperature: 1.0
  max_tokens: 32768
  max_rounds: 50  # Allow deep exploration
  max_context_tokens: 400000
  compression: minimal  # Preserve research context
  tools_enabled: true
```

**Use case**: Research tasks, data analysis, complex problem-solving

### quick
```yaml
system_prompt: "You are a concise, efficient assistant focused on quick answers."
defaults:
  model: gpt-5-nano  # Fastest, cheapest
  temperature: 1.0
  max_tokens: 8192
  max_rounds: 15
  max_context_tokens: 150000
  compression: aggressive
  tools_enabled: true
```

**Use case**: Quick questions, simple tasks, cost-sensitive operations

### Notes

- **system_prompt is included in every preset** - users get working behavior out-of-the-box
- **Users can override** with custom `system_prompt` in request (but only at creation time)
- **Parameters are optimized** for each use case (coding needs more tokens/rounds than quick tasks)

## 4) Intent Mapping

### Creativity → Temperature

**Model-specific handling:**

- **GPT-5.2**: Configurable (0.0 → 0.0, 0.5 → 0.35, 1.0 → 0.7)
- **GPT-5/5-mini/5-nano**: Fixed at 1.0 (reasoning models ignore creativity)
- **Gemini-3**: Fixed at 1.0 (recommended by Google)

### Verbosity → max_tokens

| Verbosity | Tokens |
|-----------|--------|
| short     | 8192   |
| normal    | 16384  |
| long      | 32768  |

### Rounds → max_rounds

Direct mapping - the `rounds` intent value is used as-is for `agent.max_rounds`.

### tools_enabled → agent.tools_enabled

Direct mapping - the `tools_enabled` top-level boolean is used as-is.

## 5) Configuration Flow

### New Session
```
1. Load preset defaults (e.g., "general")
   → Includes preset's system_prompt, model, max_rounds, etc.
2. Apply custom system_prompt (if provided, overrides preset's default)
   → This is the ONLY time system_prompt can be set
3. Apply top-level config (tools_enabled)
4. Apply intent mappings (creativity, verbosity, rounds)
5. Apply overrides (model, top_p, etc.)
6. Resolve to final config
7. Save to session.metadata.resolved_config
   → system_prompt is now LOCKED
```

### Existing Session
```
1. Load resolved_config from session.metadata
2. Use as-is (system_prompt and max_context_tokens are immutable)
3. Other parameters (creativity, verbosity, rounds, model) can be updated via PATCH
```

### Temporary Override (per-request)
```json
{
  "message": "Explain in detail",
  "temporary_config": {
    "intent": {
      "verbosity": "long"  // Just this request
    }
  }
}
```

**Important**: 
- Config change is NOT saved to metadata
- **Cannot include `system_prompt`** - it's immutable after creation

## 6) Implementation

### Rust Schema (Simplified)

```rust
// API request
pub struct ChatConfig {
    pub preset: String,                    // "general" | "coding" | "research" | "quick"
    pub system_prompt: Option<String>,
    pub tools_enabled: Option<bool>,       // Default: true
    pub intent: ChatIntent,
    pub overrides: Option<ChatOverrides>,
}

pub struct ChatIntent {
    pub creativity: f32,      // 0.0-1.0
    pub verbosity: String,    // "short" | "normal" | "long"
    pub rounds: u32,          // Default: 30
}

pub struct ChatOverrides {
    pub model: Option<String>,
    pub top_p: Option<f32>,
    pub frequency_penalty: Option<f32>,
    pub presence_penalty: Option<f32>,
}

// Resolved config (stored in metadata)
pub struct ResolvedConfig {
    pub provider: ProviderConfig,   // model, temperature, max_tokens
    pub agent: AgentConfig,         // max_rounds, tools_enabled
    pub session: SessionConfig,     // system_prompt, max_context_tokens
}
```

### Session Metadata

```rust
impl Session {
    pub fn save_config(&mut self, config: &ResolvedConfig) {
        self.metadata.insert("resolved_config", serde_json::to_value(config));
    }
    
    pub fn load_config(&self) -> Option<ResolvedConfig> {
        self.metadata.get("resolved_config")
            .and_then(|v| serde_json::from_value(v).ok())
    }
}
```

### Config File Loading

```rust
use std::path::PathBuf;
use std::fs;

pub struct ConfigManager {
    config_path: PathBuf,
    profiles: TemperatureProfiles,
}

impl ConfigManager {
    pub fn new() -> anyhow::Result<Self> {
        let config_path = PathBuf::from("config.yaml");
        let profiles = if config_path.exists() {
            // Load from file
            let content = fs::read_to_string(&config_path)?;
            serde_yaml::from_str(&content)?
        } else {
            // Create default config
            let profiles = TemperatureProfiles::default();
            let yaml = serde_yaml::to_string(&profiles)?;
            fs::write(&config_path, yaml)?;
            profiles
        };
        
        Ok(Self { config_path, profiles })
    }
    
    pub fn map_creativity(&self, model: &str, creativity: f32) -> f32 {
        self.profiles.get_temperature(model, creativity)
    }
}
```

## 7) Temperature Profiles (2026 Models)

**Location**: `config.yaml` in working directory  
**Loading**: If file doesn't exist, create with default profiles below

```yaml
# config.yaml
temperature_profiles:
  gpt-5.2:
    # Only GPT-5 variant with configurable temperature
    creativity_map:
      - [0.0, 0.0]
      - [0.5, 0.35]
      - [1.0, 0.7]
  
  gpt-5:
    # Reasoning model - fixed temperature
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
    # Conservative fallback
    creativity_map:
      - [0.0, 0.0]
      - [1.0, 1.0]
```

## 8) Frontend Integration

### Config Panel Component

```typescript
interface ChatConfigProps {
  onSubmit: (config: ChatConfig) => void;
}

function ConfigPanel({ onSubmit }: ChatConfigProps) {
  const [preset, setPreset] = useState("general");
  const [systemPrompt, setSystemPrompt] = useState("");
  const [toolsEnabled, setToolsEnabled] = useState(true);
  const [creativity, setCreativity] = useState(0.5);
  const [verbosity, setVerbosity] = useState("normal");
  const [rounds, setRounds] = useState(30);
  
  return (
    <div>
      <select value={preset} onChange={e => setPreset(e.target.value)}>
        <option value="general">General Assistant</option>
        <option value="coding">Software Engineer</option>
        <option value="research">Research Assistant</option>
        <option value="quick">Quick & Efficient</option>
      </select>
      
      <textarea 
        placeholder="System prompt (optional)"
        value={systemPrompt}
        onChange={e => setSystemPrompt(e.target.value)}
      />
      
      <label>
        <input 
          type="checkbox" 
          checked={toolsEnabled}
          onChange={e => setToolsEnabled(e.target.checked)}
        />
        Enable Tools
      </label>
      
      <label>Creativity: {creativity}</label>
      <input 
        type="range" 
        min="0" 
        max="1" 
        step="0.1"
        value={creativity}
        onChange={e => setCreativity(parseFloat(e.target.value))}
      />
      
      <select value={verbosity} onChange={e => setVerbosity(e.target.value)}>
        <option value="short">Short (8K)</option>
        <option value="normal">Normal (16K)</option>
        <option value="long">Long (32K)</option>
      </select>
      
      <label>Max Rounds: {rounds}</label>
      <input 
        type="number" 
        min="1" 
        max="100"
        value={rounds}
        onChange={e => setRounds(parseInt(e.target.value))}
      />
      
      <button onClick={() => onSubmit({
        preset,
        system_prompt: systemPrompt || undefined,
        tools_enabled: toolsEnabled,
        intent: { creativity, verbosity, rounds }
      })}>
        Apply Config
      </button>
    </div>
  );
}
```

## 9) Validation Rules

**Creativity:**
- Must be between 0.0 and 1.0
- Ignored for reasoning models (GPT-5, Gemini-3)

**Verbosity:**
- Must be "short" | "normal" | "long"

**Rounds:**
- Must be a positive integer (1-100 recommended)
- Default: 30

**Model (override):**
- Must be in whitelist: gpt-5, gpt-5-mini, gpt-5-nano, gpt-5.2, gemini-3-flash-preview, gemini-3-pro-preview

**Sampling parameters (overrides):**
- top_p: 0.0-1.0
- frequency_penalty: -2.0 to 2.0
- presence_penalty: -2.0 to 2.0

**System prompt:**
- Max 10,000 characters
- **Immutable after session creation**
- Returns 400 error if update attempted: `"system_prompt is immutable. Create a new session to use a different prompt."`

**tools_enabled:**
- Must be boolean (true/false)
- Default: true

## 10) Acceptance Criteria

- [x] Config API accepts preset, system_prompt, intent, overrides
- [x] Each preset includes default system_prompt
- [x] system_prompt can ONLY be set during session creation
- [x] Attempting to change system_prompt after creation returns 400 error
- [x] Intent fields map to correct runtime parameters
- [x] Resolved config saves to session metadata
- [x] Existing sessions load config from metadata
- [x] Temporary overrides work without saving to metadata
- [x] Temporary overrides reject system_prompt changes
- [x] Temperature profiles handle model-specific behavior
- [x] Validation rejects invalid values with clear errors
- [ ] Frontend config panel reads/writes real session config (currently UI-only)
- [x] Power users can override model and sampling parameters
- [x] Error messages guide users to create new session for prompt changes

## 11) Remaining Work (Detailed)

### Backend
- [x] Add API to fetch session config (e.g., `GET /api/sessions/:id/config`) returning resolved_config + editable fields.
- [x] Add API to update session config (e.g., `PATCH /api/sessions/:id/config`) with validation against immutable fields.
- [x] Persist resolved_config in session metadata on create/update.
- [x] Ensure API returns helpful errors for immutable/invalid updates.

### Frontend
- [x] Replace console-only submit with real API calls in ConfigPanel consumer (or add a ConfigPanel container).
- [x] Load existing session config from API and map into UI fields (creativity/verbosity/rounds/tools + overrides).
- [x] Map UI → payload correctly (exclude system_prompt for existing sessions, omit overrides unless set).
- [x] Show success/failure status (inline message or toast).

### Mapping/Validation
- [x] Provide reverse mapping from resolved_config to intent (creativity/verbosity) for display.
- [x] Confirm model whitelist and numeric ranges on the client (optional, server remains source of truth).

### Testing/Verification
- [ ] Manual: new session flow submits config and returns resolved_config (needs actual session storage).
- [ ] Manual: existing session loads config, blocks system_prompt changes, updates allowed fields (needs actual session storage).
- [ ] Manual: overrides round-trip (model/top_p/frequency/presence) (needs actual session storage).

**Note**: API endpoints are implemented but use placeholder data until session storage backend is connected.

---

## Changelog

- 2026-01-07: Initial draft with style field
- 2026-01-08: **Removed `style` field** - too vague, overlaps with system_prompt
- 2026-01-08: **Simplified** - removed redundant sections, consolidated examples
- 2026-01-08: **Made system_prompt top-level only** - removed from overrides to avoid dual entry points
- 2026-01-08: **Replaced budget with rounds** - direct number instead of abstract low/normal/high, default 30
- 2026-01-08: **Removed tooling intent field** - moved to top-level `tools_enabled` boolean to avoid conflict with `rounds`
- 2026-01-08: **Made system_prompt immutable** - can only be set during session creation, never updated
- 2026-01-08: **Changed presets to purpose-oriented** - general/coding/research/quick (instead of balanced/agent/aggressive/minimal)
- 2026-01-08: **Added default system_prompt to all presets** - out-of-the-box working behavior
- 2026-01-08: **Fixed preset naming consistency** - updated all sections to use general/coding/research/quick
- 2026-01-08: **Clarified config.yaml location** - working directory, auto-created with defaults if missing
- 2026-01-08: **Added ConfigManager implementation** - handles config file loading and temperature mapping
