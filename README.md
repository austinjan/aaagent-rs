# aaagent

Unified LLM provider abstraction with streaming, tool calling, and agent support for Rust.

## Quick Start

### Development Mode (Recommended)

Start both frontend and backend with one command:

```bash
python develop.py start
```

This will:
- Start Vite dev server on http://localhost:5173 (hot reload)
- Start Rust backend on http://localhost:3000 (API + embedded UI)
- Manage both processes automatically

**Stop everything:**
```bash
python develop.py stop
```

**Restart backend only (after Rust code changes):**
```bash
python develop.py restart
```

### Manual Development Mode

If you prefer separate terminals:

**Terminal 1: Backend**
```bash
cargo run --features dev-server -- serve
# Runs on http://localhost:3000
```

**Terminal 2: Frontend**
```bash
cd web
npm run dev
# Runs on http://localhost:5173 with hot reload
# Proxies /api/* to backend on port 3000
```

### Production Build

Build a single binary that serves both UI and API:

```bash
# Step 1: Build frontend
cd web
npm run build

# Step 2: Build Rust binary (embeds frontend)
cd ..
cargo build --release

# Step 3: Run
./target/release/aaagent serve
# Serves on http://localhost:3000
```

## Configuration

### LLM API Keys

API keys can be provided in three ways (in order of precedence):

#### 1. Environment Variables (Recommended)

Create a `.env` file in the project root:

```bash
# OpenAI API Key
# Get yours at: https://platform.openai.com/api-keys
OPENAI_API_KEY=sk-...

# Anthropic API Key (for Claude models)
# Get yours at: https://console.anthropic.com/settings/keys
ANTHROPIC_API_KEY=sk-ant-...

# Google AI API Key (for Gemini models)
# Get yours at: https://aistudio.google.com/app/apikey
GOOGLE_API_KEY=...
```

The `.env` file is gitignored and loaded automatically at startup.

#### 2. secrets.yaml (Local Development Only)

For local development, you can use `secrets.yaml`:

```yaml
api_keys:
  openai: "sk-..."
  anthropic: "sk-ant-..."
  google: "..."
```

> ⚠️ **Warning**: `secrets.yaml` shows a security warning in development mode and is blocked in production builds. Use environment variables for production.

#### 3. Direct Reference in config.yaml (Not Recommended)

```yaml
api_keys:
  openai:
    key: "sk-..."  # UNSAFE - only for testing
```

---

### config.yaml

The main configuration file. It's auto-generated with defaults on first run.

#### API Key References

Configure how API keys are loaded:

```yaml
api_keys:
  openai:
    env: OPENAI_API_KEY      # Load from environment variable
  anthropic:
    env: ANTHROPIC_API_KEY
  google:
    env: GEMINI_API_KEY

# Or use secrets.yaml (local dev only):
# api_keys:
#   openai:
#     file: secrets.yaml
```

#### Temperature Profiles

Map creativity intent (0.0-1.0) to model-specific temperatures:

```yaml
temperature_profiles:
  profiles:
    # Fixed temperature (reasoning models)
    gpt-5:
      fixed: 1.0
      ignore_creativity: true

    # Linear mapping with control points
    gpt-5.2:
      creativity_map:
        - [0.0, 0.0]   # creativity 0 → temperature 0
        - [0.5, 0.35]  # creativity 0.5 → temperature 0.35
        - [1.0, 0.7]   # creativity 1 → temperature 0.7

    # Default fallback for unknown models
    default:
      creativity_map:
        - [0.0, 0.0]
        - [1.0, 1.0]
```

#### System LLM Profiles

Profiles for internal system tasks (auto-compact, summarization):

```yaml
system_llm_profiles:
  default:
    model: gpt-5-mini
    temperature: 1.0
    max_tokens: 16384

  quick:
    model: gpt-5-nano
    temperature: 1.0
    max_tokens: 4096
```

#### Maintenance Configuration

Automatic cleanup tasks:

```yaml
maintenance:
  enabled: true
  interval_hours: 6

  tasks:
    temp_files:
      enabled: true
      retention_hours: 6
```

#### Skills Configuration

Enable/disable skills and provide skill-specific API keys:

```yaml
skills:
  entries:
    # Enable a skill with API key
    github:
      enabled: true
      apiKey: ghp_xxxxx

    # Enable with environment variables
    weather:
      enabled: true
      env:
        WEATHER_API_KEY: my-key
        WEATHER_CACHE: /tmp/weather

    # Disable a skill
    spotify:
      enabled: false
```

Skills are automatically filtered based on:
1. `enabled: false` in config → skill disabled
2. OS requirements (e.g., macOS-only skills)
3. Required binaries (e.g., `git`, `docker`)
4. Required environment variables

---

### .env File

Optional environment file for local configuration:

```bash
# API Keys (RECOMMENDED METHOD)
OPENAI_API_KEY=sk-...
ANTHROPIC_API_KEY=sk-ant-...
GOOGLE_API_KEY=...

# Server Configuration (optional)
# PORT=3000
# HOST=0.0.0.0
```

Copy from `.env.example` to get started:

```bash
cp .env.example .env
# Edit .env with your API keys
```

## License

MIT
