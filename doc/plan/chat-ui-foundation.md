# Chat UI Foundation Plan

- Feature name: `chat-ui-foundation`
- Status: Draft
- Created: 2026-01-06
- Parent plan: [chat-ui-plan.md](./chat-ui-plan.md)

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

### Development Mode

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

- [ ] `cargo build --release` produces single binary
- [ ] Binary serves UI at `http://localhost:3000/`
- [ ] API routes accessible at `/api/*`
- [ ] Static assets (CSS, JS, images) load correctly
- [ ] SPA routing works (refresh doesn't 404)
- [ ] Dev mode supports hot reload
- [ ] No CORS errors in development
- [ ] Frontend build outputs to `web/dist/`
- [ ] daisyUI theme applied with BlackBear colors

## 9) Implementation Tasks

- [ ] Create `web/` directory structure
- [ ] Initialize Vite + React + TypeScript project
- [ ] Configure Tailwind CSS + daisyUI
- [ ] Add `rust-embed` to Cargo.toml
- [ ] Implement `src/web/mod.rs` with asset handler
- [ ] Implement `src/api/mod.rs` with router
- [ ] Update `src/main.rs` with server setup
- [ ] Create `build.rs` for automated builds
- [ ] Test development workflow (two terminals)
- [ ] Test production build (single binary)
- [ ] Document setup in README

---

## References
- Parent plan: [chat-ui-plan.md](./chat-ui-plan.md)
- Related plans: 
  - [chat-ui-sse-streaming.md](./chat-ui-sse-streaming.md)
  - [chat-ui-state-management.md](./chat-ui-state-management.md)
