# Consistent Configuration Plan

## Goals
- Single source of truth for session configuration across frontend and backend.
- Remove non-executable config fields (`preset`, `intent`) to avoid duplicated sources of truth.
- Store full session configuration in session metadata and return it on every `GET /config`.
- Remove per-request config overrides; all updates go through session config API.
- Eliminate UI/behavior drift after refresh or new requests.

## Non-goals
- Redesign of presets or provider registry.
- Changing persistence layer or session store implementation.
- Adding new config fields beyond current schema.

## Requirements
- Single unified `SessionConfig` is the only config type across frontend and backend.
- `SessionConfig` contains only runtime-mapped fields (no `preset`, no `intent`).
- Backend session metadata MUST include `session_config` for every session.
- `GET /sessions/:id/config` returns only `session_config`.
- `PATCH /sessions/:id/config` updates `session_config` in metadata.
- `POST /sessions/:id/chat` does not accept config overrides in payload.
- Frontend Session Settings:
  - UI state loads exclusively from backend `session_config`.
  - Save writes to backend and becomes the new session baseline.

## Fields
- All fields are mutable:
  - `provider.model`
  - `provider.temperature`
  - `provider.max_tokens`
  - `provider.top_p`
  - `provider.frequency_penalty`
  - `provider.presence_penalty`
  - `agent.max_rounds`
  - `agent.tools_enabled`
  - `session.system_prompt`
  - `session.max_context_tokens`

## Design
- Single config type:
  - `SessionConfig` (provider/agent/session)
  - Replaces `ChatConfig` and `ResolvedConfig` in API payloads and UI state.
- Session metadata schema:
  - `session_config` only.
- Backfill strategy:
  - None (drop compatibility); require new sessions only.
- Frontend state:
  - `sessionConfig` reflects backend `session_config`.
  - Save button persists updates to backend.

## Milestones
1. Define unified `SessionConfig` schema without `preset`/`intent`.
2. Migrate API and internal flow to use `SessionConfig`.
3. Purge existing sessions (no backward compatibility).
4. Backend metadata stores `session_config` for new sessions only.
5. Frontend reads `session_config` only with detailed settings UI.
6. Validation and tests for session config behavior.

## Tasks
- [x] Define unified `SessionConfig` type (provider/agent/session fields only).
- [x] Remove `ChatConfig` and `ResolvedConfig` usage from API payloads and UI state.
- [x] Add metadata storage of `session_config` on session creation.
- [ ] Remove backfill for old sessions; require new sessions only.
- [x] Update `GET /config` to return metadata `session_config`.
- [x] Update `PATCH /config` to persist `session_config`.
- [x] Remove config overrides from chat request payloads.
- [x] Update frontend to source Session Settings from `session_config`.
- [x] Provide detailed settings UI (all fields) with quick model selector.
- [ ] Add tests for config endpoints and session config behavior.

## Risks
- Incorrect backfill could misrepresent existing sessions.
- Mismatch between UI draft and actual request payloads.
- Mutable/immutable list drifting from backend validation rules.

## Acceptance Criteria
- Refresh retains Session Settings as last saved.
- Chat requests no longer accept config overrides.
- All config fields are editable and persisted via session config API.
- Sessions without `session_config` are not supported.

## Open Questions
- None.
