# Separate Serve Binary

- Feature name: `separate-serve-binary`
- Status: Achieved
- Created: 2026-01-10
- Last updated: 2026-01-10

## 1) Overview

### Goal
- Extract the `serve` subcommand from the monolithic `aaagent` binary into an independent `aaagent-serve` binary

### Scope (In)
- Create new binary entry point for web server
- Expose `web` and `api` modules from library
- Remove `serve` subcommand from main CLI binary
- Ensure build.rs works for both binaries

### Non-goals (Out)
- Changing web server functionality
- Adding new features to either binary
- Conditional compilation/feature flags for dependencies (future optimization)

### User stories
- As a developer, I want to build only the CLI tools without web dependencies
- As a deployer, I want a dedicated web server binary for containerization

## 2) Requirements

### Functional requirements
- [ ] `aaagent-serve` binary starts web server on configurable port
- [ ] `aaagent` binary retains `missing-readme` and `generate-map` commands
- [ ] Both binaries can be built independently via `cargo build --bin <name>`

### Non-functional requirements
- Performance: No regression in build time or runtime
- Reliability: Same behavior as current `serve` subcommand
- Compatibility: Existing CLI usage unchanged for `missing-readme` and `generate-map`

## 3) References
- Current main.rs: `src/main.rs`
- API module: `src/api/mod.rs`
- Web module: `src/web/mod.rs`
- Build script: `build.rs`

## 4) Design

### Proposed approach
1. Expose `api` and `web` modules publicly from `src/lib.rs`
2. Create `src/bin/serve.rs` as new binary entry point
3. Add `[[bin]]` entry in `Cargo.toml` for `aaagent-serve`
4. Remove `Serve` command variant from `src/main.rs`

### File structure changes
```
src/
├── bin/
│   └── serve.rs      # NEW: aaagent-serve entry point
├── main.rs           # MODIFY: remove serve command
├── lib.rs            # MODIFY: add pub mod api, web
├── api/mod.rs        # UNCHANGED
└── web/mod.rs        # UNCHANGED
```

### API changes
- None (internal restructuring only)

### Migration / backward compatibility
- Users calling `aaagent serve` must switch to `aaagent-serve`
- CLI tools (`aaagent missing-readme`, `aaagent generate-map`) unchanged

## 5) Implementation plan

### Task breakdown (TODO)
(All tasks completed)

### Completed (DONE)
- [x] Explored current binary structure
- [x] Identified files to modify
- [x] Added `pub mod api;` and `pub mod web;` to `src/lib.rs`
- [x] Created `src/bin/serve.rs` with clap CLI and server startup
- [x] Added `[[bin]]` entry to `Cargo.toml`
- [x] Removed `Serve` variant and handler from `src/main.rs`
- [x] Removed unused `mod web;` and `mod api;` from `src/main.rs`
- [x] Tested both binaries compile and run correctly

## 6) Testing plan
- Unit tests: Existing tests should pass
- Integration tests: Verify `aaagent-serve --port 3000` starts server
- Manual tests: 
  - `cargo build --bin aaagent` succeeds
  - `cargo build --bin aaagent-serve` succeeds
  - `./target/debug/aaagent missing-readme --help` works
  - `./target/debug/aaagent-serve --help` works

## 7) Rollout plan
- Feature flag: N/A
- Staging validation: Build and test locally
- Rollback: Revert commits if issues arise

## 8) Risks & mitigations
- Risk: Build.rs may not trigger frontend build for serve binary
  - Mitigation: Verify build.rs runs on release profile regardless of binary target

## 9) Acceptance criteria
- [x] `cargo build --bin aaagent` produces CLI-only binary
- [x] `cargo build --bin aaagent-serve` produces web server binary
- [x] `aaagent-serve` starts server identical to previous `aaagent serve`
- [x] `aaagent` no longer has `serve` subcommand

---

## Changelog
- 2026-01-10: Created
- 2026-01-10: Completed implementation and testing
