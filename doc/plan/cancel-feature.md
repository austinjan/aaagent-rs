# Cancellation Support for Agent Conversation Flow

## Overview

Add ability to cancel in-progress agent conversations while preserving partial results. Currently, users cannot interrupt agentic loops - the input is disabled until completion. This plan adds a "Cancel" button that gracefully stops execution and preserves all completed messages/tool results.

## Architecture

```
Frontend (ChatInput)
  → Cancel button calls cancelChat API
  → Disconnects SSE stream
  ↓
Backend (API /sessions/{id}/cancel)
  → Triggers CancellationToken
  ↓
Agent Loop (chat_with_callback)
  → Checks token at control points:
    - Between rounds (after max_rounds check)
    - After tool execution
  → On cancel: emits AgentEvent::Cancelled
  → Returns Ok(partial_content)
  ↓
Session State (already auto-saved!)
  → All messages/tool results auto-persist via append_message()
  → No extra cleanup needed
```

**Key Insight:** Session auto-save already handles partial state perfectly. We just need to add cancellation checks in the agent loop and UI to trigger it.

## Critical Files

### Backend (Rust)
1. **`src/api/cancellation_manager.rs`** (NEW) - ~120 lines
   - Follows StreamManager pattern exactly
   - Maps stream_id → CancellationToken
   - Methods: create_token(), cancel(), remove()

2. **`src/api/mod.rs`** - ~110 lines modified/added
   - Add `cancellation_manager: Arc<CancellationManager>` to AppState (line 34)
   - Initialize in AppState::new() (after line 84)
   - Add route `/sessions/:id/cancel` (in api_routes())
   - Implement cancel_chat handler
   - Update sessions::chat to create token and pass to agent
   - Update run_agent_chat signature to accept cancel_token

3. **`src/agent/mod.rs`** - ~80 lines modified
   - Add `AgentEvent::Cancelled` variant (line ~145)
   - Update chat_with_callback signature: add `cancel_token: Option<CancellationToken>`
   - Add cancellation check after line 369 (between rounds)
   - Add cancellation check after line 459 (after tool execution)
   - Update chat() wrapper to pass None token

4. **`Cargo.toml`** - 1 line
   - Add `tokio-util = "0.7"` for CancellationToken

### Frontend (TypeScript)
5. **`web/src/services/api.ts`** - ~25 lines
   - Add cancelChat(sessionId, streamId) function
   - Add CancelChatRequest/Response types

6. **`web/src/hooks/useChat.ts`** - ~40 lines
   - Add activeStreamId state
   - Store stream_id from sendMessage response
   - Add cancelCurrentChat() function
   - Clear stream_id on completion
   - Return canCancel state

7. **`web/src/components/chat/ChatInput.tsx`** - ~20 lines
   - Add onCancel and canCancel props
   - Add Cancel button (shows when disabled && canCancel)
   - Button uses variant="destructive" size="sm"

8. **`web/src/hooks/useSSEStream.ts`** - ~15 lines
   - Add event listener for "cancelled" event
   - Set isDone and disconnect

9. **`web/src/types/backend.ts`** - ~5 lines
   - Add to AgentEvent union: `{ type: "cancelled"; reason: string; partial_content: string }`

## Implementation Steps

### Phase 1: Backend Foundation
1. Add `tokio-util = "0.7"` to Cargo.toml
2. Create `src/api/cancellation_manager.rs` following StreamManager pattern
3. Add unit tests (create_and_cancel, cancel_nonexistent, cleanup)
4. Update AppState in `src/api/mod.rs` (add field, initialize)
5. **Verify:** `cargo test` passes

### Phase 2: Agent Cancellation Logic
6. Add `AgentEvent::Cancelled { reason, partial_content }` variant
7. Update `chat_with_callback` signature (add `cancel_token` parameter)
8. Add cancellation checks:
   - After line 369: `if token.is_cancelled() { emit Cancelled, return Ok() }`
   - After line 459: Same check after tool execution
9. Update `chat()` wrapper to pass `None`
10. **Verify:** Compiles, agent tests pass

### Phase 3: API Endpoint
11. Add route `.route("/sessions/:session_id/cancel", post(sessions::cancel_chat))`
12. Implement cancel_chat handler:
    - Accept `{ stream_id: string }` in request body
    - Call `state.cancellation_manager.cancel(&stream_id)`
    - Return success/failure JSON
13. Update `sessions::chat`:
    - After creating stream: `let cancel_token = state.cancellation_manager.create_token(stream_id.clone())`
    - Pass token to run_agent_chat
    - Remove token on completion
14. Update `run_agent_chat` signature (add cancel_token parameter, pass to agent)
15. **Verify:** Endpoint returns 200, curl test works

### Phase 4: Frontend API Layer
16. Add `cancelChat(sessionId, streamId)` function to api.ts
17. Add CancelChatRequest/Response types
18. **Verify:** TypeScript compiles

### Phase 5: Frontend Hook Integration
19. Add `activeStreamId` state to useChat
20. Update sendMessage to store response.stream_id
21. Add `cancelCurrentChat()` function:
    - Call disconnect() on SSE
    - Call cancelChat API
    - Clear activeStreamId
22. Clear activeStreamId in "done" event handler
23. Return `cancelCurrentChat` and `canCancel` from hook
24. **Verify:** Hook compiles, state updates work

### Phase 6: UI Components
25. Update ChatInput props (add onCancel, canCancel)
26. Add Cancel button (inside button div, before Send):
    ```tsx
    {disabled && canCancel && (
      <Button variant="destructive" size="sm" onClick={onCancel}>
        Cancel
      </Button>
    )}
    ```
27. Update Chat.tsx to pass handlers to ChatInput
28. Add "cancelled" event handler in useSSEStream
29. Handle cancelled event in useChat (append "[Cancelled by user]" to message)
30. **Verify:** Button appears when loading, click triggers cancel

### Phase 7: Testing
31. **Scenario 1:** Send "Hello", cancel immediately → User message saved only
32. **Scenario 2:** Send "Long story", cancel after 2s → Partial response with "[Cancelled]"
33. **Scenario 3:** Send message with tools, cancel during execution → Completed tools preserved
34. **Scenario 4:** Spam cancel button → Only one cancel processed
35. **Scenario 5:** Cancel after completion → API returns success:false, no harm

## Verification

### Unit Tests
- CancellationManager tests (create, cancel, cleanup)
- Agent cancellation tests (between rounds, during tools)

### Integration Test
```bash
# Terminal 1: Start server
cargo run -- serve

# Terminal 2: Test flow
# 1. Open http://localhost:3000
# 2. Send message "Count from 1 to 100 slowly"
# 3. Click Cancel after ~2 seconds
# Expected: Partial response displayed, button disappears, can send new message
```

### Manual Checklist
- [ ] Cancel button appears when isLoading=true
- [ ] Button click calls backend API
- [ ] SSE stream disconnects immediately
- [ ] Partial messages are preserved in chat history
- [ ] Session state saved correctly (reload shows partial results)
- [ ] Can send new message after cancellation
- [ ] No memory leaks (tokens cleaned up)
- [ ] Error handling works (network failures, already completed)

## Key Design Decisions

1. **CancellationToken over AbortHandle:** Tokio-util token is more idiomatic, composable with select!
2. **Check at round boundaries:** Simpler than mid-tool cancellation, good enough UX
3. **Return Ok() not Err():** Cancellation is intentional, not an error - preserves partial content
4. **Cancel button in ChatInput:** Logical placement near action that started chat
5. **Stream-based identification:** Frontend knows stream_id from chat response, no session lookup needed

## Edge Cases Handled

- **Cancel during tool execution:** Checks after each tool completes, preserves partial results
- **Cancel after completion:** API returns success:false, no side effects
- **Network failure:** Frontend disconnects SSE immediately, shows feedback to user
- **Multiple rapid cancels:** canCancel state prevents duplicate requests
- **Token cleanup:** Removed in finally block of background task

## Estimated Time

- Backend: 5-6 hours (foundation, agent logic, API)
- Frontend: 3-4 hours (API, hook, UI)
- Testing: 2-3 hours (unit tests, integration, manual)
- **Total: 10-13 hours**

## Future Enhancements

- Tool-level cancellation (pass token to ToolRegistry::execute)
- Auto-timeout after N minutes
- Progress indicators ("Round 3/10, executing tool...")
- Resume from cancelled state
