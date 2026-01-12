# Chat UI Foundation Plan

- Feature name: `chat-ui-foundation`
- Status: **Implemented** ✅
- Created: 2026-01-06
- Completed: 2026-01-07
- Parent plan: [chat-ui-plan.md](./chat-ui-plan.md)

## Implementation Summary

**Status: 100% Complete** ✅

### What's Working ✅
- **Frontend**: Vite + React 18 + TypeScript with hot reload
- **UI Framework**: daisyUI + Tailwind CSS with BlackBear TechHive theme  
- **Backend**: Rust + axum web server with embedded assets
- **Development Workflow**: `develop.py` script manages both servers
- **API Endpoints**: Health check and placeholder routes
- **Feature Flags**: `dev-server` for CORS in development
- **Production Build**: Automated frontend build in `build.rs`
- **Single Binary**: Release binary serves both UI and API

### Quick Start

**Development Mode:**
```bash
# Start development environment (single command)
python develop.py start
# Frontend: http://localhost:5173 (hot reload)
# Backend:  http://localhost:3000 (API + embedded UI)

# Stop everything
python develop.py stop

# Restart backend after Rust code changes
python develop.py restart
```

**Production Mode:**
```bash
# Build release binary (auto-builds frontend)
cargo build --release

# Run single binary
./target/release/aaagent serve
# Serves on http://localhost:3000
```

### Test Backend
```bash
curl http://localhost:3000/api/health
# {"status":"ok","message":"aaagent-rs chat UI backend is running","version":"0.1.0"}
```

## 1) Overview

### Goal
Establish the foundational architecture for the chat UI using a single binary + embedded frontend approach.

### Scope (In)
- Project structure (frontend/backend separation)
- Build process and tooling
- Development workflow
- Production deployment
- Asset embedding strategy
- Basic routing setup

### Non-goals (Out)
- Specific UI components
- Business logic
- Feature-specific APIs

## 2) Tech Stack

**Architecture: Single Binary + Embedded Frontend**

We use a unified binary approach where the Rust application embeds and serves the compiled frontend assets.

**Frontend:**
- **Framework**: React 18+ with TypeScript
- **Build Tool**: Vite (fast builds, hot reload during development)
- **UI Components**: daisyUI (Tailwind CSS component library)
- **State Management**: Zustand (lightweight, no boilerplate)
- **Styling**: Tailwind CSS (via daisyUI)
- **HTTP Client**: Native `fetch` API with EventSource for SSE

**Backend:**
- **Framework**: Rust with axum
- **Static File Serving**: `tower-http` with `ServeDir`
- **Asset Embedding**: `rust-embed` for bundling frontend into binary
- **Serialization**: serde, serde_json
- **Async Runtime**: tokio

**Build & Deployment:**
- **Development**: Frontend dev server (Vite) proxies API to Rust backend
- **Production**: Frontend builds to `dist/`, embedded in Rust binary via `rust-embed`
- **Single Artifact**: One executable serves both API and static assets

**Why This Approach:**
- **Simple Deployment**: Single binary to distribute, no separate frontend hosting
- **Zero Configuration**: Users run one command, no CORS setup needed
- **Offline Capable**: All assets bundled, works without internet
- **Fast Iteration**: Vite dev server for hot reload, production build optimized
- **Type Safety**: TypeScript frontend + Rust backend with shared types via codegen

## 3) Directory Structure

```
aaagent-rs/
├── src/
│   ├── web/
│   │   ├── mod.rs           # Embedded asset serving
│   │   └── static.rs        # Static file handlers
│   ├── api/
│   │   ├── mod.rs           # API route definitions
│   │   ├── sessions.rs      # Session endpoints
│   │   └── sse.rs           # SSE streaming
│   ├── main.rs
│   └── lib.rs
├── web/                      # Frontend code (React + TypeScript)
│   ├── src/
│   │   ├── components/      # UI components
│   │   │   ├── chat/
│   │   │   ├── minimap/
│   │   │   └── shared/
│   │   ├── stores/          # Zustand stores
│   │   ├── hooks/           # Custom React hooks
│   │   ├── types/           # TypeScript types
│   │   ├── utils/           # Helper functions
│   │   ├── App.tsx
│   │   └── main.tsx
│   ├── public/              # Static assets
│   ├── dist/                # Build output (embedded in binary)
│   ├── package.json
│   ├── vite.config.ts
│   ├── tailwind.config.js
│   └── tsconfig.json
├── Cargo.toml
└── build.rs                 # Optional: auto-build frontend
```

## 4) Development Workflow

### Development Mode (Simplified with Python Script)

**Single-Command Development** (Recommended):

```bash
# Start both frontend (Vite) and backend (Rust) in one command
python develop.py start

# Restart backend only (rebuild if necessary)
python develop.py restart

# Stop both frontend and backend
python develop.py stop
```

**What `develop.py` does:**
- Starts Vite dev server on http://localhost:5173 (with hot reload)
- Builds and runs Rust backend on http://localhost:3000
- Manages both processes (no need for two terminals)
- Handles graceful shutdown on Ctrl+C or `stop` command
- Automatically rebuilds backend on `restart` command

**Manual Mode** (Alternative):

If you prefer separate terminals:

**Terminal 1: Run Rust backend**
```bash
cd aaagent-rs
cargo run --features dev-server
# Starts on http://localhost:3000 (API only)
```

**Terminal 2: Run Vite dev server**
```bash
cd web
npm run dev
# Starts on http://localhost:5173 (UI with hot reload)
# Proxies /api/* requests to http://localhost:3000
```

**Vite proxy configuration** (`web/vite.config.ts`):
```typescript
import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

export default defineConfig({
  plugins: [react()],
  server: {
    port: 5173,
    proxy: {
      '/api': {
        target: 'http://localhost:3000',
        changeOrigin: true,
      },
    },
  },
  build: {
    outDir: 'dist',
    emptyOutDir: true,
  },
});
```

**Benefits:**
- Hot module reload for instant UI updates
- No CORS issues (proxy handles it)
- Fast iteration (no rebuild needed)
- Backend and frontend run independently
- Single command with `develop.py` (no multiple terminals needed)

### Production Build

**Manual Build Process:**

**Step 1: Build frontend**
```bash
cd web
npm run build
# Outputs to web/dist/
```

**Step 2: Build Rust with embedded assets**
```bash
cd ..
cargo build --release
# Embeds web/dist/ into binary via rust-embed
```

**Result:** Single executable at `target/release/aaagent-rs`

**Running:**
```bash
./target/release/aaagent-rs
# Serves both API and UI on http://localhost:3000
# GET / → index.html
# GET /api/* → API endpoints
# GET /assets/* → CSS, JS, images
```

### Development Script (`develop.py`)

**Purpose**: Simplify development workflow by managing both frontend and backend in a single command.

**Location**: `aaagent-rs/develop.py`

**Commands**:

```bash
# Start both frontend dev server and backend
python develop.py start

# Restart backend only (kill, rebuild, restart)
python develop.py restart

# Stop both frontend and backend
python develop.py stop
```

**Requirements**:
```python
# Python 3.8+
# Standard library only (subprocess, signal, sys, os, time, pathlib, json)
```

**Behavior**:

1. **`start` command**:
   - Check if `web/node_modules` exists, if not run `npm install` in `web/`
   - Start Vite dev server: `cd web && npm run dev` (background process)
   - Build Rust backend: `cargo build --features dev-server` (if needed)
   - Run Rust backend: `cargo run --features dev-server` (background process)
   - Monitor both processes, print their output with prefixes `[vite]` and `[backend]`
   - Handle Ctrl+C gracefully (kill both processes)
   - Store PIDs in `.dev_pids.json` for `restart`/`stop` commands

2. **`restart` command**:
   - Read backend PID from `.dev_pids.json`
   - Kill backend process (SIGTERM, wait 5s, SIGKILL if needed)
   - Keep frontend running (no interruption to hot reload)
   - Rebuild backend: `cargo build --features dev-server`
   - Restart backend: `cargo run --features dev-server`
   - Update backend PID in `.dev_pids.json`

3. **`stop` command**:
   - Read PIDs from `.dev_pids.json`
   - Kill both frontend and backend processes (SIGTERM, then SIGKILL if needed)
   - Remove `.dev_pids.json`
   - Clean exit

**Output Format**:
```
[develop.py] Starting development environment...
[develop.py] Checking frontend dependencies...
[npm] added 245 packages in 8s
[develop.py] Starting Vite dev server...
[vite] VITE v5.3.0  ready in 523 ms
[vite] ➜  Local:   http://localhost:5173/
[develop.py] Building Rust backend...
[cargo] Compiling aaagent-rs v0.1.0
[cargo] Finished dev [unoptimized + debuginfo] target(s) in 12.34s
[develop.py] Starting Rust backend...
[backend] Server running on http://127.0.0.1:3000
[develop.py] ✓ Development environment ready
[develop.py] 
[develop.py]   Frontend: http://localhost:5173
[develop.py]   Backend:  http://localhost:3000
[develop.py] 
[develop.py] Press Ctrl+C to stop both servers
```

**Error Handling**:
- If Vite fails to start: Print error, kill backend, exit
- If backend fails to build: Print error, kill Vite, exit
- If backend fails to run: Print error, kill Vite, exit
- If ports already in use: Print error with instructions
- If `.dev_pids.json` missing on `restart`/`stop`: Print helpful error

**Cross-Platform Support**:
- Windows: Use `subprocess.CREATE_NEW_PROCESS_GROUP` and `taskkill /F /T /PID` for cleanup
- Unix/Linux/macOS: Use process groups with `os.setpgid()` and `SIGTERM`/`SIGKILL`

**Implementation Details**:
- Use `subprocess.Popen()` with `stdout=subprocess.PIPE`, `stderr=subprocess.STDOUT`
- Use separate threads to read and print output from both processes in real-time
- Prefix each output line with `[vite]` or `[backend]` for clarity
- Store PIDs in JSON: `{"frontend": 12345, "backend": 67890}`
- Graceful shutdown: Try SIGTERM first, wait 5s, then SIGKILL if process still alive
- Color output (optional): green for success, red for errors, yellow for warnings (use `colorama` if available)

**PID File Format** (`.dev_pids.json`):
```json
{
  "frontend": 12345,
  "backend": 67890,
  "started_at": "2026-01-07T10:30:00"
}
```

### Automated Build (Optional)

**Cargo build script** (`build.rs`):
```rust
use std::process::Command;
use std::env;

fn main() {
    // Only build frontend in release mode
    let profile = env::var("PROFILE").unwrap_or_default();
    
    if profile == "release" {
        println!("cargo:rerun-if-changed=web/src");
        println!("cargo:rerun-if-changed=web/package.json");
        println!("cargo:rerun-if-changed=web/vite.config.ts");
        
        println!("Building frontend assets...");
        
        // Install dependencies if needed
        let npm_install = Command::new("npm")
            .args(&["install"])
            .current_dir("web")
            .status()
            .expect("Failed to install npm dependencies");
        
        if !npm_install.success() {
            panic!("npm install failed");
        }
        
        // Build frontend
        let npm_build = Command::new("npm")
            .args(&["run", "build"])
            .current_dir("web")
            .status()
            .expect("Failed to build frontend");
        
        if !npm_build.success() {
            panic!("Frontend build failed");
        }
        
        println!("Frontend build completed successfully");
    }
}
```

After finished Automated Build, Do not forget write usage in the README.md file.

**Benefits:**
- `cargo build --release` automatically builds frontend
- Single command for full production build
- CI/CD friendly
- Ensures frontend is always up-to-date

## 5) Implementation Details

### Backend: Embedded Asset Serving

**Dependencies** (`Cargo.toml`):
```toml
[dependencies]
axum = "0.7"
tokio = { version = "1", features = ["full"] }
tower = "0.4"
tower-http = { version = "0.5", features = ["fs", "cors"] }
rust-embed = "8"
mime_guess = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"

[features]
default = []
dev-server = ["tower-http/cors"]
```

**Asset Embedding** (`src/web/mod.rs`):
```rust
use rust_embed::RustEmbed;
use axum::{
    response::{Html, IntoResponse, Response},
    http::{StatusCode, Uri, header},
};

#[derive(RustEmbed)]
#[folder = "web/dist/"]
struct Assets;

pub async fn static_handler(uri: Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/');
    
    // Serve index.html for root
    if path.is_empty() || path == "index.html" {
        return serve_index();
    }
    
    // Try to serve the requested file
    if let Some(content) = Assets::get(path) {
        let mime = mime_guess::from_path(path).first_or_octet_stream();
        return Response::builder()
            .header(header::CONTENT_TYPE, mime.as_ref())
            .body(content.data.into())
            .unwrap()
            .into_response();
    }
    
    // SPA fallback: serve index.html for non-API routes
    // This allows React Router to handle client-side routing
    if !path.starts_with("api/") {
        return serve_index();
    }
    
    // 404 for everything else
    (StatusCode::NOT_FOUND, "Not found").into_response()
}

fn serve_index() -> Response {
    if let Some(index) = Assets::get("index.html") {
        Response::builder()
            .header(header::CONTENT_TYPE, "text/html")
            .body(index.data.into())
            .unwrap()
            .into_response()
    } else {
        (StatusCode::INTERNAL_SERVER_ERROR, "index.html not found").into_response()
    }
}
```

**Router Setup** (`src/api/mod.rs`):
```rust
use axum::{
    Router, 
    routing::{get, post},
};
use tower_http::cors::CorsLayer;

pub fn create_router() -> Router {
    Router::new()
        // API routes (under /api prefix)
        .nest("/api", api_routes())
        
        // Static files (fallback for everything else)
        .fallback(crate::web::static_handler)
        
        // CORS middleware (only in dev mode)
        .layer(
            #[cfg(feature = "dev-server")]
            CorsLayer::permissive()
        )
}

fn api_routes() -> Router {
    Router::new()
        .route("/sessions/:session_id/chat", post(sessions::chat))
        .route("/sessions/:session_id/stream/:stream_id", get(sse::stream))
        .route("/sessions/:session_id/path", get(sessions::get_path))
        .route("/sessions/:session_id/path/metadata", get(sessions::get_metadata))
        .route("/sessions/:session_id/checkpoints", get(sessions::get_checkpoints))
        .route("/sessions/:session_id/system-prompt", get(sessions::get_system_prompt))
}

// Placeholder handlers
mod sessions {
    use axum::Json;
    use serde_json::{json, Value};
    
    pub async fn chat() -> Json<Value> {
        Json(json!({"status": "not implemented"}))
    }
    
    pub async fn get_path() -> Json<Value> {
        Json(json!({"nodes": []}))
    }
    
    pub async fn get_metadata() -> Json<Value> {
        Json(json!({"total_nodes": 0}))
    }
    
    pub async fn get_checkpoints() -> Json<Value> {
        Json(json!({"checkpoints": []}))
    }
    
    pub async fn get_system_prompt() -> Json<Value> {
        Json(json!({"prompt": ""}))
    }
}

mod sse {
    use axum::response::sse::Event;
    
    pub async fn stream() -> impl axum::response::IntoResponse {
        // Placeholder
        axum::response::sse::Sse::new(futures::stream::empty::<Result<Event, std::convert::Infallible>>())
    }
}
```

**Main Entry Point** (`src/main.rs`):
```rust
mod web;
mod api;

#[tokio::main]
async fn main() {
    // Create router
    let app = api::create_router();
    
    // Start server
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("Server running on http://{}", addr);
    println!("Open http://localhost:3000 in your browser");
    
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind to address");
    
    axum::serve(listener, app)
        .await
        .expect("Server error");
}
```

### Frontend: Initial Setup

**Package.json** (`web/package.json`):
```json
{
  "name": "aaagent-web",
  "private": true,
  "version": "0.1.0",
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "tsc && vite build",
    "preview": "vite preview",
    "type-check": "tsc --noEmit"
  },
  "dependencies": {
    "react": "^18.3.1",
    "react-dom": "^18.3.1",
    "zustand": "^4.5.0"
  },
  "devDependencies": {
    "@types/react": "^18.3.0",
    "@types/react-dom": "^18.3.0",
    "@vitejs/plugin-react": "^4.3.0",
    "autoprefixer": "^10.4.19",
    "daisyui": "^4.12.0",
    "postcss": "^8.4.38",
    "tailwindcss": "^3.4.4",
    "typescript": "^5.5.0",
    "vite": "^5.3.0"
  }
}
```

**Tailwind Config** (`web/tailwind.config.js`):
```javascript
/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {
      colors: {
        // BlackBear TechHive brand colors
        'brand-black': '#000000',
        'brand-yellow': '#E8C236',
      },
    },
  },
  plugins: [require("daisyui")],
  daisyui: {
    themes: [
      {
        blackbear: {
          "primary": "#E8C236",      // Brand yellow
          "secondary": "#A7A8A9",     // Dark gray
          "accent": "#66CC33",        // Green
          "neutral": "#000000",       // Brand black
          "base-100": "#FFFFFF",      // White
          "base-200": "#DDE5ED",      // Blue gray
          "base-300": "#D7D2CB",      // Warm gray
          "info": "#33CCFF",          // Cyan
          "success": "#339933",       // Dark green
          "warning": "#EEC049",       // Yellow light
          "error": "#EB0FFF",         // Magenta
        },
      },
    ],
  },
};
```

**TypeScript Config** (`web/tsconfig.json`):
```json
{
  "compilerOptions": {
    "target": "ES2020",
    "useDefineForClassFields": true,
    "lib": ["ES2020", "DOM", "DOM.Iterable"],
    "module": "ESNext",
    "skipLibCheck": true,
    "moduleResolution": "bundler",
    "allowImportingTsExtensions": true,
    "resolveJsonModule": true,
    "isolatedModules": true,
    "noEmit": true,
    "jsx": "react-jsx",
    "strict": true,
    "noUnusedLocals": true,
    "noUnusedParameters": true,
    "noFallthroughCasesInSwitch": true,
    "baseUrl": ".",
    "paths": {
      "@/*": ["./src/*"]
    }
  },
  "include": ["src"],
  "references": [{ "path": "./tsconfig.node.json" }]
}
```

**Basic App** (`web/src/App.tsx`):
```typescript
function App() {
  return (
    <div className="min-h-screen bg-base-100">
      <header className="navbar bg-neutral text-primary">
        <div className="flex-1">
          <a className="btn btn-ghost text-xl">aaagent-rs</a>
        </div>
      </header>
      
      <main className="container mx-auto p-4">
        <div className="alert alert-info">
          <span>Chat UI Foundation - Ready for implementation</span>
        </div>
      </main>
    </div>
  );
}

export default App;
```

## 6) Feature Flags

**Cargo.toml**:
```toml
[features]
default = []
dev-server = ["tower-http/cors"]  # Enable CORS for dev mode
```

**Usage:**
- Development: `cargo run --features dev-server`
- Production: `cargo build --release` (no dev-server feature)

## 7) Testing Plan

**Backend Tests:**
- [ ] Asset embedding: Verify all files from `web/dist/` are embedded
- [ ] Static handler: Serve correct MIME types
- [ ] SPA fallback: Non-API routes serve index.html
- [ ] API routes: Return 404 for unknown API endpoints

**Frontend Tests:**
- [ ] Build process: Vite builds without errors
- [ ] Type checking: No TypeScript errors
- [ ] Development proxy: API requests reach backend

**Integration Tests:**
- [ ] Production build: Single binary serves UI
- [ ] Hot reload: Dev mode updates UI instantly
- [ ] Routing: Client-side routing works

## 8) Acceptance Criteria

- [x] Frontend initializes with Vite + React + TypeScript
- [x] daisyUI theme applied with BlackBear colors (#E8C236, #000000)
- [x] Tailwind CSS configured and working
- [x] Backend has `serve` subcommand
- [x] Backend serves embedded assets from `web/dist/`
- [x] API routes accessible at `/api/*`
- [x] Health check endpoint `/api/health` returns JSON
- [x] Dev mode supports hot reload (Vite on 5173, backend on 3000)
- [x] No CORS errors in development (dev-server feature)
- [x] `develop.py start` launches both servers
- [x] `develop.py stop` gracefully shuts down both servers
- [x] `develop.py restart` restarts backend only
- [x] `cargo build --release` produces single binary
- [x] Binary serves UI at `http://localhost:3000/`
- [x] SPA routing works (refresh doesn't 404)
- [x] Frontend build outputs to `web/dist/`

## 9) Implementation Tasks

- [x] Create `web/` directory structure
- [x] Initialize Vite + React + TypeScript project
- [x] Configure Tailwind CSS + daisyUI with BlackBear theme
- [x] Add `rust-embed` to Cargo.toml
- [x] Implement `src/web/mod.rs` with asset handler
- [x] Implement `src/api/mod.rs` with router
- [x] Update `src/main.rs` with server setup (`serve` subcommand)
- [x] Add health check endpoint `/api/health`
- [x] **Create `develop.py` script for simplified development workflow**
- [x] Test development workflow with `python develop.py start`
- [x] Test backend restart with `python develop.py restart`
- [x] Test graceful shutdown with `python develop.py stop`
- [x] Create `build.rs` for automated builds
- [x] Test production build (single binary)
- [x] Document setup in README (include `develop.py` usage)

---

## References
- Parent plan: [chat-ui-plan.md](./chat-ui-plan.md)
- Related plans: 
  - [chat-ui-sse-streaming.md](./chat-ui-sse-streaming.md)
  - [chat-ui-state-management.md](./chat-ui-state-management.md)
