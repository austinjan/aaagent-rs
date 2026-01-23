# Known Issues

## 1. Session History Not Preserved Across Requests (FIXED)

**Status:** ✅ RESOLVED - Implemented in JSONLStore

**Previous Issue:**
- Sessions were not persisted across requests
- Multi-turn conversations didn't work

**Fix Implemented:**
- ✅ JSONLStore with append-only JSONL format (`src/history/jsonl_store.rs`)
- ✅ Session metadata persisted in `data/sessions/{session_id}.meta.json`
- ✅ Tree nodes persisted in `data/sessions/{session_id}.nodes.jsonl`
- ✅ Lazy in-memory cache for performance
- ✅ AppState uses `Arc<JSONLStore>` for persistent storage (`src/api/mod.rs:32`)
- ✅ Sessions loaded from disk via `store.get_session()` (`src/api/mod.rs:405-418`)
- ✅ Agent receives persistent store via `session.set_store()` (`src/api/mod.rs:579`)

**Verification:**
Multi-turn conversations now work correctly. Sessions persist across server restarts.

**Fixed in commit:** ec91271 - "Add JSONLStore for append-only session and node storage"

---

## 2. OpenAI API "error decoding response body"

**Status:** Intermittent, needs investigation

**Symptom:**
- SSE stream fails mid-conversation with "error decoding response body"
- Error occurs randomly during streaming responses
- Frontend displays: "❌ Agent Error: API error: Stream error: Transport error: error decoding response body"

**Possible Causes:**
- OpenAI API returns malformed SSE chunks
- Network interruption during streaming
- Non-UTF8 content in response

**Code Location:**
- Error originates from `eventsource_stream` crate or `reqwest`
- Propagates through `src/llm/openai.rs` streaming logic

**Workaround:**
- User can retry the request
- Most requests succeed

**Fix Options:**
- Add detailed error logging to capture raw response data
- Implement automatic retry mechanism
- Add better error recovery in SSE stream handling

**Impact:**
- Medium: Degrades user experience but doesn't break functionality
- Errors are correctly displayed to user

---

## 3. Tool Result Display Order (FIXED)

**Status:** Fixed in latest commit

**Previous Issue:**
- Tool results appeared before Assistant's final response
- Incorrect message ordering in UI

**Fix Applied:**
- Changed tool results from separate messages to properties of Assistant message
- Added `toolResults` array to `MessageData`
- Tool results now display inline with tool calls in same message card

---

## 4. OpenAI Schema Compatibility (FIXED)

**Status:** Fixed in latest commit

**Previous Issue:**
- `editor__Edit` tool used `oneOf` in JSON Schema
- OpenAI API rejected schemas with top-level `oneOf`

**Fix Applied:**
- Removed `oneOf` validation from tool schema
- Added clear description explaining two mutually exclusive modes
- Tool execution logic unchanged (still validates mode combinations)
