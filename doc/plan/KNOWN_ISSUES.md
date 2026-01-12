# Known Issues

## 1. Session History Not Preserved Across Requests

**Status:** Known limitation, needs architectural fix

**Symptom:**
- Each chat request creates a new empty session
- LLM cannot see previous conversation history
- Multi-turn conversations don't work (e.g., "what is this folder?" after "pwd" doesn't remember the previous response)

**Root Cause:**
- `src/api/mod.rs:507-509`: Creates new Session instead of reconstructing from stored data
- Session store only saves metadata, not the tree nodes
- MemoryStore is not persistent across requests

**Current Workaround:**
None - this is a fundamental limitation

**Fix Required:**
Option 1: Persist tree nodes in session store
- Serialize all nodes when saving session
- Deserialize and reconstruct tree when loading session

Option 2: Use persistent tree store (not MemoryStore)
- Implement FileStore or DatabaseStore for tree nodes
- Associate tree store with session_id

**Code Location:**
```rust
// src/api/mod.rs:495-510
async fn run_agent_chat(
    session: aaagent::history::Session,
    ...
) -> anyhow::Result<()> {
    let tree_store = Arc::new(MemoryStore::new());
    
    // TODO: Properly reconstruct session with tree store
    // For now, create a new session - this is a limitation we need to fix
    let session = aaagent::history::Session::new(tree_store.clone(), session.config.clone()).await?;
    ...
}
```

**Impact:**
- High: Breaks multi-turn conversations
- Users must repeat context in every message

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
