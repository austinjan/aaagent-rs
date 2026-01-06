# Repository Guidelines

## Project Structure & Module Organization

- `src/` holds the library and CLI entry points: `src/lib.rs` for the library API, `src/main.rs` for the CLI binary.
- Core modules live under `src/agent/`, `src/history/`, `src/llm/`, and `src/tools/` with submodules grouped by feature.
- `examples/` contains runnable demos (OpenAI, tool calling, chat loops) referenced in `examples/README.md`.
- `doc/` stores design notes, plans, and reference material; treat it as documentation-only content.
- `target/` is build output and should not be edited or committed.

## Build, Test, and Development Commands

- `cargo build` compiles the library and binary with default features.
- `cargo test` runs unit tests across modules.
- `cargo run --example openai_basic --features openai` runs a simple OpenAI streaming demo.
- `cargo run --example interactive_agent --features openai` starts the interactive agent CLI example.
- `cargo check` provides a fast compile/type-check pass during development.

## Coding Style & Naming Conventions

- Use Rust 2021 idioms: `snake_case` for modules/functions, `CamelCase` for types, and `SCREAMING_SNAKE_CASE` for constants.
- Keep indentation at 4 spaces and follow `rustfmt` defaults; run `cargo fmt` before submitting.
- Keep module boundaries clear: public APIs in `lib.rs` or `mod.rs`, helpers in local modules.

## Testing Guidelines

- Unit tests are colocated with modules (e.g., `src/llm/tests.rs`, `src/llm/openai_tests.rs`).
- Prefer small, focused tests for provider behavior and history utilities.
- Example programs are for manual verification and may require API keys; they are not part of `cargo test`.

## Commit & Pull Request Guidelines

- Follow the existing history pattern: short, imperative summaries with optional prefixes like `feat:` or `chore:`.
- Include a concise PR description, testing notes (`cargo test`, example command), and linked issues if available.
- Highlight any user-facing changes (CLI behavior, provider support) in the PR body.

## Configuration & Secrets

- Set `OPENAI_API_KEY` (and `GEMINI_API_KEY` when using Gemini examples) in your environment before running examples.
- Avoid committing secrets or sample keys; use `.env` files locally if needed.
