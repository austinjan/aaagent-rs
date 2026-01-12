# Debugging SSE Streaming Issues

## Current Status

Backend integration is complete with debug logging added. If you see "SSE error occurred" in the browser console, follow these steps to diagnose:

## Debug Logs Added

### Frontend (`useSSEStream.ts`)
- `[SSE] Connecting to: <url>` - When connection attempt starts
- `[SSE] Connection opened` - When EventSource connects successfully
- `[SSE] Error occurred` - When connection fails (with readyState and URL)
- `[SSE] Received content event` - When content events arrive

### Backend (`src/api/mod.rs`)
- `Chat request received for session: <id>` - When chat endpoint receives a message
- `Created stream: <stream_id> for session: <session_id>` - When SSE stream is created
- `SSE stream requested: <stream_id>` - When SSE endpoint is called
- `Stream <id> not found` - If stream doesn't exist when SSE connects
- `SSE stream connection established: <id>` - When SSE successfully connects

## Testing Steps

### 1. Check if secrets.yaml exists
```bash
ls secrets.yaml
```

If not, create it:
```yaml
api_keys:
  anthropic: "sk-ant-your-key-here"
  # or
  openai: "sk-your-key-here"
```

### 2. Start the server with logging
```bash
# The backend logs to stdout and app.log
cargo run --release -- serve

# Or for development:
python develop.py start
```

### 3. Open browser console
- Open `http://localhost:3000`
- Open DevTools (F12)
- Go to Console tab
- Clear console (Ctrl+L)

### 4. Send a test message
Type a message and send it. Watch the console for:

**Expected logs:**
```
[SSE] Connecting to: /api/sessions/<session_id>/stream/<stream_id>
[SSE] Connection opened
[SSE] Received content event: {...}
```

**If error occurs:**
```
[SSE] Error occurred
[SSE] EventSource readyState: <0|1|2>
[SSE] Stream URL: <url>
```

**readyState meanings:**
- `0` (CONNECTING) - Connection not yet established
- `1` (OPEN) - Connection open and receiving events
- `2` (CLOSED) - Connection closed

### 5. Check backend logs

Look for these patterns in terminal or `app.log`:

**Success flow:**
```
Chat request received for session: 01...
Created stream: stream-01... for session: 01...
SSE stream requested: stream-01...
SSE stream connection established: stream-01...
```

**Failure - Stream not found:**
```
Chat request received for session: 01...
Created stream: stream-01... for session: 01...
SSE stream requested: stream-01...
Stream stream-01... not found
```

## Common Issues

### Issue 1: Stream timing
**Problem**: SSE connects before stream is created
**Symptom**: "Stream not found" in backend logs
**Solution**: Already handled - stream is created before returning stream_id to frontend

### Issue 2: CORS in development
**Problem**: Browser blocks SSE connection
**Symptom**: `net::ERR_FAILED` in Network tab
**Solution**: Use `--features dev-server` flag:
```bash
cargo run --features dev-server -- serve
```

### Issue 3: No API key
**Problem**: Backend fails to create provider
**Symptom**: Backend error before SSE connection
**Solution**: Add valid API key to `secrets.yaml`

### Issue 4: Double connection
**Problem**: SSE connects twice, first connection takes the stream
**Symptom**: Second connection gets "Stream not found"
**Solution**: Check React StrictMode (may cause double mount in dev)

## Network Tab Inspection

1. Open DevTools → Network tab
2. Filter by `EventStream` or `stream`
3. Send a message
4. Look for: `GET /api/sessions/.../stream/...`

**Expected:**
- Status: `200 OK`
- Type: `eventsource`
- EventStream tab shows events arriving

**If error:**
- Status: `404 Not Found` - Stream not found
- Status: `500 Internal Server Error` - Backend error
- Status: `0` (canceled) - CORS or network issue

## Next Steps

After adding debug logs, if you still see SSE errors:

1. **Capture the logs** - Both frontend console and backend terminal
2. **Check Network tab** - See if request reaches backend
3. **Verify stream_id** - Ensure frontend receives valid stream_id from chat endpoint
4. **Check timing** - Ensure stream exists when SSE connects

The integration is complete - debugging will help identify any runtime issues!
