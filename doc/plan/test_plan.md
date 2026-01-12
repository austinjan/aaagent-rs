## 🧪 How to Test the Config System

### **Option 1: Browser Testing (Easiest)**

The servers are running at:
- Frontend: http://localhost:5173
- Backend: http://localhost:3000

**Steps:**

1. **Open your browser** to `http://localhost:5173/testing`

2. **Test ConfigPanel UI:**
   - You'll see the config panel with all controls
   - Change the **preset** dropdown → system prompt should update
   - Adjust **creativity** slider (0.0 - 1.0)
   - Change **verbosity** (short/normal/long)
   - Modify **rounds** (max execution rounds)
   - Toggle **tools enabled**
   - Expand **Advanced Overrides** to test model selection

3. **Click "Apply Config":**
   - Watch for loading state: button shows "Saving..."
   - Success: Green message "Configuration updated successfully!"
   - Error: Red message with error details

4. **Test system_prompt immutability:**
   - The UI currently doesn't have a sessionId, so system_prompt is editable
   - To test immutability, you'd need to pass a sessionId prop (we can add that)

---

### **Option 2: API Testing with Command Line**

Wait a few more seconds for the backend to fully start, then:

```bash
# 1. List sessions (returns 3 placeholder sessions)
xh GET http://localhost:3000/api/sessions

# 2. Get config for a session
xh GET http://localhost:3000/api/sessions/session-1/config

# 3. Create a new session
echo '{"name":"My Test Session","preset":"coding"}' | xh POST http://localhost:3000/api/sessions

# 4. Update config (valid - no system_prompt change)
echo '{
  "preset": "coding",
  "tools_enabled": true,
  "intent": {
    "creativity": 0.8,
    "verbosity": "long",
    "rounds": 50
  }
}' | xh PATCH http://localhost:3000/api/sessions/session-1/config

# 5. Try to change system_prompt (should fail with 400 error)
echo '{
  "preset": "general",
  "system_prompt": "New prompt",
  "tools_enabled": true,
  "intent": {
    "creativity": 0.5,
    "verbosity": "normal",
    "rounds": 30
  }
}' | xh PATCH http://localhost:3000/api/sessions/session-1/config

# Expected error: "system_prompt is immutable. Create a new session to use a different prompt."
```

---

### **Option 3: Check What's Working Now**

Open Chrome DevTools (F12) and go to http://localhost:5173/testing, then check the **Console** tab. You should see:

- ConfigPanel component loaded
- Any API calls being made
- Config objects being logged when you click "Apply Config"

---
