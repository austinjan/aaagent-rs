# Sub-Agent 通知方案 - Toast 替代方案

**問題**: Toast 通知太打擾、短暫、容易錯過

讓我提供幾個更好的替代方案：

---

## 方案 1: **固定通知欄** (推薦 ⭐⭐⭐⭐⭐)

### 概念：
在聊天界面頂部或底部有一個**固定的通知欄**，顯示 sub-agent 活動。

### 視覺效果：
```
┌────────────────────────────────────────────────────────┐
│ 🤖 2 Sub-Agents Running                        [Hide] │
│ • Searching codebase (15s) ⋯                           │
│ • Analyzing files (8s) ⋯                               │
└────────────────────────────────────────────────────────┘
```

### 優點：
- ✅ 不會消失 - 用戶可以隨時查看
- ✅ 不打擾 - 固定位置，不遮擋內容
- ✅ 信息豐富 - 可以顯示多個 sub-agent
- ✅ 可折疊 - 用戶可以選擇隱藏
- ✅ 持續更新 - 實時顯示進度

### 實現：
```tsx
// web/src/components/agent/SubAgentNotificationBar.tsx
export function SubAgentNotificationBar() {
  const { activeRuns } = useSubAgentStore();
  const [isExpanded, setIsExpanded] = useState(true);
  
  if (activeRuns.size === 0) return null;
  
  return (
    <div className="fixed top-16 left-0 right-0 z-40 bg-blue-50 border-b border-blue-200 shadow-sm">
      <div className="container mx-auto px-4 py-2">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-4">
            <span className="font-semibold text-blue-900">
              🤖 {activeRuns.size} Sub-Agent{activeRuns.size > 1 ? 's' : ''} Running
            </span>
            
            {isExpanded && (
              <div className="flex gap-3">
                {Array.from(activeRuns.values()).map(run => (
                  <div key={run.runId} className="flex items-center gap-2 text-sm">
                    <span className="loading loading-spinner loading-xs"></span>
                    <span>{run.taskLabel}</span>
                    <span className="text-gray-500">({formatElapsed(run.startTime)})</span>
                  </div>
                ))}
              </div>
            )}
          </div>
          
          <button 
            className="btn btn-ghost btn-xs"
            onClick={() => setIsExpanded(!isExpanded)}
          >
            {isExpanded ? 'Hide' : 'Show'}
          </button>
        </div>
      </div>
    </div>
  );
}
```

### 位置選項：
- **頂部** (推薦): 在導航欄下方，不遮擋聊天
- **底部**: 在輸入框上方
- **側邊**: 作為左側或右側浮動條

---

## 方案 2: **內聯狀態卡片** (推薦 ⭐⭐⭐⭐)

### 概念：
在聊天流中插入一個**狀態卡片**，就像一條特殊消息。

### 視覺效果：
```
┌─────────────────────────────────────────┐
│ User: 幫我搜索 TODO                      │
└─────────────────────────────────────────┘

╔═════════════════════════════════════════╗
║ 🚀 Sub-Agent Started                    ║
║ Task: Searching codebase for TODOs      ║
║ Status: Running... (12s elapsed)        ║
╚═════════════════════════════════════════╝

┌─────────────────────────────────────────┐
│ Assistant: 我已經啟動了搜索...           │
└─────────────────────────────────────────┘

╔═════════════════════════════════════════╗
║ ✅ Sub-Agent Completed                  ║
║ Task: Searching codebase for TODOs      ║
║ Found: 23 TODO items in 15 files        ║
║ Duration: 18s                           ║
╚═════════════════════════════════════════╝
```

### 優點：
- ✅ 與聊天流集成 - 自然的閱讀體驗
- ✅ 持久化 - 不會消失，可以回顧
- ✅ 上下文清晰 - 看到 sub-agent 在哪個對話中運行
- ✅ 可摺疊 - 不佔用過多空間

### 實現：
```tsx
// web/src/components/chat/SubAgentStatusCard.tsx
interface SubAgentStatusCardProps {
  runId: string;
  status: 'running' | 'completed' | 'error';
}

export function SubAgentStatusCard({ runId, status }: SubAgentStatusCardProps) {
  const run = useSubAgentStore(state => 
    state.activeRuns.get(runId) || state.completedRuns.get(runId)
  );
  
  if (!run) return null;
  
  const colors = {
    running: 'bg-blue-50 border-blue-300',
    completed: 'bg-green-50 border-green-300',
    error: 'bg-red-50 border-red-300',
  };
  
  const icons = {
    running: '🚀',
    completed: '✅',
    error: '❌',
  };
  
  return (
    <div className={`card ${colors[status]} border-2 my-4 mx-auto max-w-2xl`}>
      <div className="card-body p-4">
        <div className="flex items-center gap-3">
          <span className="text-3xl">{icons[status]}</span>
          <div className="flex-1">
            <h4 className="font-bold">
              Sub-Agent {status === 'running' ? 'Running' : status === 'completed' ? 'Completed' : 'Failed'}
            </h4>
            <p className="text-sm text-gray-700">{run.taskLabel}</p>
            
            {status === 'running' && (
              <div className="flex items-center gap-2 mt-2">
                <span className="loading loading-spinner loading-xs"></span>
                <span className="text-xs text-gray-600">
                  {formatElapsed(run.startTime)} elapsed
                </span>
              </div>
            )}
            
            {status === 'completed' && run.endTime && (
              <div className="text-xs text-gray-600 mt-1">
                Duration: {formatDuration(run.startTime, run.endTime)}
              </div>
            )}
          </div>
        </div>
        
        {/* Tool calls list (if any) */}
        {run.toolCalls.length > 0 && (
          <details className="mt-2">
            <summary className="text-xs cursor-pointer">
              {run.toolCalls.length} tool{run.toolCalls.length > 1 ? 's' : ''} executed
            </summary>
            <ul className="text-xs ml-4 mt-1">
              {run.toolCalls.map((tool, idx) => (
                <li key={idx}>• {tool.name}</li>
              ))}
            </ul>
          </details>
        )}
      </div>
    </div>
  );
}
```

### 插入時機：
- Sub-agent 啟動時 → 插入 "Running" 卡片
- Sub-agent 完成時 → 更新為 "Completed" 卡片
- 保留在聊天歷史中

---

## 方案 3: **狀態指示器 + 側邊詳情** (推薦 ⭐⭐⭐⭐⭐)

### 概念：
**兩層設計**：
1. **微妙的狀態指示器** - 頂部小圖標，不打擾
2. **詳細側邊面板** - 點擊圖標打開詳情

### 視覺效果：

**狀態指示器** (頂部右側):
```
┌────────────────────────────────┐
│ Chat UI            🤖(2) ⋯ [≡] │ ← 圖標顯示 2 個運行中
└────────────────────────────────┘
```

**點擊後展開側邊面板**:
```
┌─────────────────┬────────────────────────┐
│                 │ 🤖 Sub-Agents (2)      │
│   Chat          │                        │
│   Messages      │ 🔵 Running             │
│   Here          │ • Searching (15s)      │
│                 │   └ ShellTool: rg      │
│                 │                        │
│                 │ • Analyzing (8s)       │
│                 │   └ ReadTool: file.rs  │
│                 │                        │
│                 │ ✅ Completed (3)       │
│                 │ • Task 1 (2m ago)      │
│                 │ • Task 2 (5m ago)      │
│                 │ • Task 3 (10m ago)     │
└─────────────────┴────────────────────────┘
```

### 優點：
- ✅ 不打擾 - 小圖標，用戶可以忽略
- ✅ 信息豐富 - 面板顯示所有詳情
- ✅ 按需查看 - 用戶主動打開
- ✅ 持久化 - 保留歷史記錄
- ✅ 空間效率 - 不佔用主要聊天區域

### 實現：

**狀態指示器** (header):
```tsx
// web/src/components/layout/SubAgentIndicator.tsx
export function SubAgentIndicator() {
  const { activeRuns } = useSubAgentStore();
  const [isPanelOpen, setIsPanelOpen] = useState(false);
  
  if (activeRuns.size === 0) return null;
  
  return (
    <>
      <button 
        className="btn btn-ghost btn-circle relative"
        onClick={() => setIsPanelOpen(true)}
      >
        🤖
        {activeRuns.size > 0 && (
          <span className="absolute top-0 right-0 badge badge-primary badge-sm">
            {activeRuns.size}
          </span>
        )}
        <span className="loading loading-spinner loading-xs absolute bottom-1 right-1"></span>
      </button>
      
      {isPanelOpen && (
        <SubAgentDetailPanel onClose={() => setIsPanelOpen(false)} />
      )}
    </>
  );
}
```

**詳情面板**:
```tsx
// web/src/components/agent/SubAgentDetailPanel.tsx
export function SubAgentDetailPanel({ onClose }: { onClose: () => void }) {
  const { activeRuns, completedRuns } = useSubAgentStore();
  
  return (
    <div className="fixed inset-0 z-50">
      {/* Backdrop */}
      <div className="absolute inset-0 bg-black/20" onClick={onClose}></div>
      
      {/* Side Panel */}
      <div className="absolute right-0 top-0 bottom-0 w-96 bg-base-100 shadow-2xl overflow-y-auto">
        <div className="p-4">
          <div className="flex items-center justify-between mb-4">
            <h2 className="text-xl font-bold">🤖 Sub-Agents</h2>
            <button className="btn btn-ghost btn-sm" onClick={onClose}>✕</button>
          </div>
          
          {/* Active Runs */}
          {activeRuns.size > 0 && (
            <div className="mb-6">
              <h3 className="text-sm font-semibold mb-2 text-blue-600">
                🔵 Running ({activeRuns.size})
              </h3>
              {Array.from(activeRuns.values()).map(run => (
                <SubAgentDetailCard key={run.runId} run={run} status="running" />
              ))}
            </div>
          )}
          
          {/* Completed Runs */}
          {completedRuns.size > 0 && (
            <div>
              <h3 className="text-sm font-semibold mb-2 text-green-600">
                ✅ Completed ({completedRuns.size})
              </h3>
              {Array.from(completedRuns.values()).map(run => (
                <SubAgentDetailCard key={run.runId} run={run} status="completed" />
              ))}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
```

---

## 方案 4: **進度條集成** (推薦 ⭐⭐⭐)

### 概念：
在**輸入框上方**顯示細長的進度條，顯示 sub-agent 活動。

### 視覺效果：
```
┌────────────────────────────────────────────────┐
│ ▓▓▓▓▓▓▓▓░░░░░░░░░ 2 sub-agents running...     │ ← 進度條
└────────────────────────────────────────────────┘
┌────────────────────────────────────────────────┐
│ Type your message...                      [Send]│
└────────────────────────────────────────────────┘
```

### 優點：
- ✅ 極簡 - 不佔用太多空間
- ✅ 視覺清晰 - 進度條易於理解
- ✅ 不打擾 - 位於自然位置
- ✅ 可點擊 - 展開詳情面板

---

## 推薦方案組合 🎯

### **最佳組合**: 方案 3 (狀態指示器) + 方案 2 (內聯卡片)

#### 為什麼？

1. **狀態指示器** (頂部圖標)
   - 用戶隨時知道有 sub-agent 在運行
   - 不打擾，可以忽略
   - 點擊查看詳情

2. **內聯狀態卡片** (聊天流中)
   - 啟動和完成時自動插入
   - 保留在聊天歷史中
   - 提供上下文

#### 用戶體驗流程：

```
1. User: "幫我搜索 TODO"
   ↓
2. [內聯卡片] 🚀 Sub-Agent Started
   ↓
3. [頂部圖標] 🤖(1) ⋯  ← 顯示 1 個運行中
   ↓
4. [點擊圖標] → 打開側邊面板 → 看到詳細進度
   ↓
5. [內聯卡片] ✅ Sub-Agent Completed
   ↓
6. [頂部圖標消失]
```

---

## 實現優先級

### Phase 1 (必須):
1. ✅ **狀態指示器** (頂部小圖標) - 2h
2. ✅ **側邊詳情面板** - 3h
3. ✅ **內聯狀態卡片** - 3h

### Phase 2 (可選):
4. ⭐ **固定通知欄** - 2h (如果用戶覺得指示器不夠明顯)
5. ⭐ **進度條** - 2h (視覺增強)

---

## 總結

### 移除 Toast 的原因：
- ❌ 會消失，容易錯過
- ❌ 打斷用戶
- ❌ 無法查看歷史
- ❌ 多個 toast 很混亂

### 替代方案的優勢：
- ✅ 持久可見
- ✅ 不打擾
- ✅ 信息豐富
- ✅ 可以查看歷史
- ✅ 更專業的 UX

你更喜歡哪個方案？我推薦 **方案 3 (狀態指示器 + 側邊面板) + 方案 2 (內聯卡片)** 的組合！
