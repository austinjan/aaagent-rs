# Sub-Agent UI Testing Guide

**Status**: Ready for Testing  
**Date**: 2026-02-05

## 前置準備

### 1. 停止正在運行的服務

```powershell
# 如果 backend server 正在運行，停止它
# 按 Ctrl+C 停止，或者使用 Task Manager 結束 aaagent-serve.exe 進程
```

### 2. 重新編譯 Backend

```powershell
cd E:\code\aaagent-rs
cargo build --release
```

### 3. 編譯 Frontend

```powershell
cd E:\code\aaagent-rs\web
npm run build
```

---

## 啟動服務

### 方法 1: 使用開發腳本（推薦）

```powershell
python develop.py start
```

這會啟動：
- Frontend dev server: `http://localhost:5173` (Vite)
- Backend server: `http://localhost:3000`

### 方法 2: 手動啟動

**Terminal 1 - Backend:**
```powershell
cargo run --features dev-server -- serve
```

**Terminal 2 - Frontend:**
```powershell
cd web
npm run dev
```

---

## 測試步驟

### 步驟 1: 開啟 Chat 頁面

1. 瀏覽器打開: `http://localhost:5173/chat`
2. 應該看到正常的 chat 界面
3. 檢查右上角 header，應該看到 🤖 圖標（SubAgentIndicator）

**預期結果**:
- ✅ 🤖 圖標顯示（灰色，無 badge）
- ✅ 沒有 spinner
- ✅ 點擊圖標會打開空的 detail panel

---

### 步驟 2: 觸發 Sub-Agent

在 chat input 中輸入以下任一指令：

#### 選項 A: 直接要求使用 sub-agent
```
請使用 spawn_subagent 工具來搜索專案中所有包含 "TODO" 的檔案
```

#### 選項 B: 讓 LLM 自己決定使用 sub-agent
```
請分析 src/ 目錄中所有 Rust 檔案的結構，並告訴我有哪些主要模組
```

#### 選項 C: 明確要求並行處理
```
請同時執行兩個獨立的任務：
1. 搜索所有包含 "TODO" 的檔案
2. 列出所有 API endpoints

請使用 sub-agent 來並行執行
```

**重要**: 如果 LLM 沒有自動使用 spawn_subagent tool，你可以在 system prompt 中添加：
```
You have access to spawn_subagent tool for parallel task execution. Use it when appropriate.
```

---

### 步驟 3: 觀察 Sub-Agent UI

當 sub-agent 啟動後，應該看到：

#### 3.1 SubAgentIndicator 變化

**立即發生**:
- ✅ 🤖 圖標變成黃色
- ✅ Badge 出現顯示 "1" 
- ✅ 黃色 spinner 旋轉
- ✅ 點擊圖標打開 detail panel

**Console 日誌** (F12 開發者工具):
```
[SubAgentSSE] Connecting to: /api/sessions/01XXXXX/stream
[SubAgentSSE] Connection opened
[SubAgentSSE] Sub-agent spawned: { run_id: "01YYYYY", task_label: "Search: TODO files" }
```

#### 3.2 Detail Panel 內容

**List View 應該顯示**:
```
┌────────────────────────────────────┐
│ Sub-Agent Activity          [×]    │
├────────────────────────────────────┤
│ ┌────────────────────────────────┐ │
│ │ Search: TODO files             │ │
│ │ 5s elapsed                     │ │
│ │ [🚀 spawning] 或 [⚙️ running]   │ │
│ │ ⚙️ 0 tool calls                │ │
│ └────────────────────────────────┘ │
└────────────────────────────────────┘
```

**點擊 run 進入 Detail View**:
```
┌────────────────────────────────────┐
│ [← Back]                    [×]    │
├────────────────────────────────────┤
│ Search: TODO files                 │
│ Duration: 5s                       │
│ Started: 14:32:15                  │
│                                    │
│ Tool Calls                         │
│ ┌────────────────────────────────┐ │
│ │ shell_tool         running ⏳  │ │
│ │ 14:32:16                       │ │
│ └────────────────────────────────┘ │
└────────────────────────────────────┘
```

#### 3.3 Sub-Agent 執行期間

當 sub-agent 調用工具時：

**Console 日誌**:
```
[SubAgentSSE] Sub-agent event: { run_id: "01YYYYY", eventType: "tool_calls" }
[SubAgentSSE] Sub-agent event: { run_id: "01YYYYY", eventType: "tool_result" }
```

**Detail Panel 更新**:
- ✅ Tool calls 列表增加
- ✅ Status 從 "running" → "success" 或 "error"
- ✅ Duration 持續更新

#### 3.4 Sub-Agent 完成

**Console 日誌**:
```
[SubAgentSSE] Sub-agent event: { run_id: "01YYYYY", eventType: "done" }
[SubAgentSSE] Sub-agent completed: { run_id: "01YYYYY", success: true }
```

**UI 變化**:
- ✅ Indicator badge 消失（如果只有 1 個 sub-agent）
- ✅ Spinner 停止
- ✅ Detail panel 顯示 [✅ completed]
- ✅ Run 移到 completed list

#### 3.5 Chat Flow 中的 Message

檢查 chat messages，應該看到：

**Sub-Agent 的 Message 帶有 Badge**:
```
┌────────────────────────────────────┐
│ ASSISTANT [🤖 Search Agent] 14:35  │
│                                    │
│ Found 15 TODO comments in:         │
│ - src/main.rs (3 items)            │
│ - src/api/mod.rs (5 items)         │
│ ...                                │
└────────────────────────────────────┘
```

---

### 步驟 4: 測試多個並發 Sub-Agents

輸入需要多個 sub-agent 的指令：

```
請同時執行三個任務：
1. 搜索所有 TODO comments
2. 列出所有 pub fn 函數
3. 查找所有 TODO 標註的 struct

使用 spawn_subagent 並行執行
```

**預期結果**:
- ✅ Indicator badge 顯示 "3"
- ✅ Detail panel 列出 3 個 active runs
- ✅ 每個 run 獨立更新
- ✅ 完成時 badge 逐步減少 (3 → 2 → 1 → 消失)

---

### 步驟 5: 測試錯誤場景

觸發一個會失敗的 sub-agent：

```
請使用 spawn_subagent 讀取不存在的檔案 /nonexistent/file.txt
```

**預期結果**:
- ✅ Indicator 顯示
- ✅ Detail panel 顯示 [❌ error]
- ✅ Error message 顯示在 panel 中
- ✅ Status card 顯示紅色 border

---

## 測試檢查清單

### UI Components

- [ ] **SubAgentIndicator**
  - [ ] 無 active sub-agent 時顯示灰色
  - [ ] 有 active sub-agent 時顯示黃色 + badge + spinner
  - [ ] Badge 數字正確（1, 2, 3...）
  - [ ] 點擊切換 panel 開關
  - [ ] Hover 顯示 tooltip

- [ ] **SubAgentDetailPanel**
  - [ ] 初始狀態為關閉
  - [ ] 點擊 indicator 打開
  - [ ] List view 顯示所有 runs
  - [ ] 點擊 run 進入 detail view
  - [ ] Detail view 顯示完整 timeline
  - [ ] Tool calls 正確顯示
  - [ ] Error messages 正確顯示
  - [ ] "Back" 按鈕返回 list view
  - [ ] [×] 按鈕關閉 panel
  - [ ] Auto-prune old runs (5分鐘後)

- [ ] **MessageCard Badge**
  - [ ] Sub-agent messages 顯示黃色 badge
  - [ ] Badge 包含 robot icon
  - [ ] Hover 顯示 sub-agent label

### Functionality

- [ ] **SSE Connection**
  - [ ] Auto-connect when session loaded
  - [ ] Receive sub-agent events
  - [ ] Filter events by run_id
  - [ ] Console logs show connection status

- [ ] **Store Updates**
  - [ ] addRun when sub-agent spawns
  - [ ] updateRun when phase changes
  - [ ] addToolCall for each tool execution
  - [ ] completeRun when sub-agent finishes
  - [ ] pruneOldRuns after 5 minutes

- [ ] **Tool Execution**
  - [ ] spawn_subagent tool available
  - [ ] LLM can invoke spawn_subagent
  - [ ] Sub-agent inherits tools (shell, read, edit)
  - [ ] Sub-agent cannot spawn nested sub-agents

### Performance

- [ ] No lag when opening panel
- [ ] Smooth animations
- [ ] No memory leaks (check Chrome DevTools)
- [ ] Works with 5+ concurrent sub-agents

### Edge Cases

- [ ] Page refresh clears sub-agent state (expected behavior)
- [ ] Panel state persists during navigation
- [ ] SSE reconnection after disconnect
- [ ] Handle malformed SSE events gracefully

---

## 故障排查

### 問題: 沒有看到 spawn_subagent tool

**檢查**:
1. Backend logs 中搜索 "spawn"
2. 檢查 `cargo build` 是否成功
3. 確認 `run_agent_chat` 調用 `agent_factory.create_agent_with_custom_provider(..., true, ...)`

**解決方案**:
```bash
# 重新編譯
cargo clean
cargo build --release
```

### 問題: SubAgentIndicator 不更新

**檢查**:
1. Console 是否有 SSE 錯誤
2. `/api/sessions/:id/stream` endpoint 是否可訪問
3. Backend 是否 emit events

**解決方案**:
```bash
# 檢查 backend logs
tail -f app.log

# 檢查 SSE endpoint
curl http://localhost:3000/api/sessions/YOUR_SESSION_ID/stream
```

### 問題: Detail Panel 背景透明

**已修復**: 使用 `bg-base-100` + `backdrop-blur-sm`

### 問題: LLM 不使用 spawn_subagent

**可能原因**:
1. System prompt 沒有提示 tool 的存在
2. 任務不適合使用 sub-agent
3. LLM 認為直接執行更快

**解決方案**:
- 明確要求使用 spawn_subagent
- 提供需要並行處理的任務
- 檢查 tool description 是否清晰

---

## Backend Events 檢查

### 查看 Backend Logs

```bash
# Windows PowerShell
Get-Content app.log -Tail 50 -Wait

# 或使用 bat (如果安裝)
bat app.log --paging=never -f
```

**預期 Logs**:
```
[Startup] Sub-agent system initialized
🔧 Registered tools (5): [shell_tool, read_tool, editor_edit_tool, web_search, spawn_subagent]
[SubAgent] Spawned sub-agent: 01YYYYY for task: "Search: TODO files"
[SubAgent] Tool call: shell_tool
[SubAgent] Tool result: success
[SubAgent] Sub-agent completed: 01YYYYY
```

### 檢查 SSE Stream

使用 curl 或 PowerShell 查看 SSE events:

```powershell
# Windows (需要 curl)
curl -N http://localhost:3000/api/sessions/YOUR_SESSION_ID/stream
```

**預期輸出**:
```
event: agent_event
data: {"session_id":"01XXX","run_id":"01YYY","seq":1,"event":{"type":"content","content":"..."}}

event: subagent_spawned
data: {"run_id":"01YYY","task_label":"Search: TODO files"}

event: subagent_completed
data: {"run_id":"01YYY","success":true}
```

---

## 成功標準

✅ 所有測試檢查清單項目通過  
✅ 無 Console 錯誤  
✅ UI 響應流暢  
✅ Backend logs 顯示正確 events  
✅ SSE stream 正常工作  
✅ Multi-agent 並發正常  

---

## 下一步

測試成功後：
1. 記錄任何發現的 bug
2. 截圖展示 UI 效果
3. 測試不同的 sub-agent 場景
4. 性能測試（10+ concurrent sub-agents）
5. 更新文檔

測試失敗時：
1. 收集 Console logs
2. 收集 Backend logs
3. 記錄重現步驟
4. 提供 session_id 和 run_id

---

祝測試順利！🎉
