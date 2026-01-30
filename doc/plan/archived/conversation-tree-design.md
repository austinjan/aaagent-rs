# Conversation Tree Visualization 設計規範

## 概述

Conversation Tree 是對話歷史的樹狀視覺化，類似 Git Graph，用於展示分支結構、節點狀態、和當前激活路徑。

**關鍵原則**：
- 與 Chat Message Card Header 共用 Node 視覺語言
- Tree 中使用縮小版 Node（12×12px），保留外圈顏色和狀態邏輯
- 支援分支、Checkpoint 標記、Active Path 高亮

---

## 1) Node 視覺規範（共用設計）

### 標準 Node（用於 Chat Card Header）

```
尺寸：32×32px
├─ 外圈：4px border（box-sizing: border-box）
├─ 背景：白色 #FFFFFF
└─ 中心區域：inset 5px（實際 22×22px）
   ├─ 背景：根據狀態變化
   └─ Icon：22×22px（gear/exclamation） 或 18×18px（dot）
```

### 縮小版 Node（用於 Tree Visualization）

```
尺寸：12×12px（標準 Node 的 0.375x scale）
├─ 外圈：1.5px border
├─ 背景：白色
└─ 中心區域：inset 2px（實際 7×7px）
   ├─ 背景：根據狀態變化
   └─ Icon：簡化版（8×8px 或省略）
```

---

## 2) 類型與顏色（共用）

| Type | Hex | 說明 |
| --- | --- | --- |
| System | `#7B9FBF` | 系統訊息 |
| User | `#595757` | 使用者輸入 |
| Assistant | `#5785A3` | AI 回覆 |
| Checkpoint | `#D4C257` | 檢查點（疊加標記） |
| Tool | `#57A357` | 工具呼叫 |

---

## 3) 狀態規則（共用邏輯，Tree 簡化實作）

| 狀態 | 標準 Node（32px） | Tree Node（12px） | 動畫 |
| --- | --- | --- | --- |
| **Idle** | 透明中心，無 icon | 透明中心 | - |
| **Running** | 透明中心，gear icon（類型色） | 簡化 spinner（1px 虛線圓） | 旋轉 2s |
| **Error** | `#C23C3C` 填充，白色 exclamation | `#C23C3C` 填充，白色圓點 | - |
| **Streaming** | `#5785A3` 填充，白色 dot | 節點類型色填充 | Pulse 1.5s |

---

## 4) Tree Visualization 特定元素

### 連接線

| 類型 | 樣式 | 顏色 | 寬度 |
| --- | --- | --- | --- |
| Active Path | 實線 | `#5785A3` | 2px |
| Inactive Branch | 虛線（4 2） | `#9CA3AF` | 1.5px，opacity 0.5 |

### Checkpoint 標記

```
形狀：菱形（旋轉 45° 的正方形）
尺寸：
  - 標準 Node：14×14px（疊加在 32px node 上）
  - Tree Node：5×5px（疊加在 12px node 上）
顏色：#D4C257，opacity 0.9
位置：覆蓋在節點中心
```

### 節點互動（僅 Tree）

```
Default:  12×12px
Hover:    14×14px（scale 1.17x）
Selected: 外圈 ring 2px，顏色 #E8C236
```

---

## 5) 布局規則（Tree Visualization）

### 間距

```
節點垂直間距：32px（depth * 32）
分支水平間距：24px（branch index * 24）
容器內邊距：16px（top/bottom）
```

### 布局算法

```typescript
interface LayoutConfig {
  nodeSpacing: 32;      // 垂直間距
  branchSpacing: 24;    // 水平間距
  baseX: 100;           // 起始 X 座標（容器中心）
}

function layoutTree(nodes: Node[], activeLeafId: string) {
  const childrenMap = buildChildrenMap(nodes);
  const activePath = getActivePath(nodes, activeLeafId);

  function dfs(nodeId: string, depth: number, baseX: number) {
    const children = childrenMap.get(nodeId) || [];

    // 單一子節點：維持 x 位置
    if (children.length === 1) {
      dfs(children[0].id, depth + 1, baseX);
    }
    // 多分支：水平展開
    else if (children.length > 1) {
      const totalWidth = (children.length - 1) * CONFIG.branchSpacing;
      const startX = baseX - totalWidth / 2;

      children.forEach((child, i) => {
        const childX = startX + i * CONFIG.branchSpacing;
        dfs(child.id, depth + 1, childX);
      });
    }
  }
}
```

### 容器尺寸

```
寬度：動態計算（min 120px，根據最寬分支調整）
高度：vh 或固定高度，overflow-y: auto
背景：#DDE5ED
邊框：1px solid #D7D2CB
```

---

## 6) CSS 組件實作

### 標準 Node（Chat Card Header）

```css
.node-standard {
  width: 32px;
  height: 32px;
  border-radius: 50%;
  border: 4px solid; /* 顏色由 type 決定 */
  background: #fff;
  position: relative;
  box-sizing: border-box;
}

.node-standard .node-center {
  position: absolute;
  top: 5px;
  left: 5px;
  right: 5px;
  bottom: 5px;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
}

.node-standard .status-icon {
  width: 22px;
  height: 22px;
}

.node-standard .status-icon-dot {
  width: 18px;
  height: 18px;
}
```

### Tree Node（縮小版）

```css
.node-tree {
  width: 12px;
  height: 12px;
  border-radius: 50%;
  border: 1.5px solid;
  background: #fff;
  position: relative;
  box-sizing: border-box;
  transition: transform 0.15s ease;
}

.node-tree:hover {
  transform: scale(1.17); /* 12px -> 14px */
  cursor: pointer;
}

.node-tree .node-center {
  position: absolute;
  top: 2px;
  left: 2px;
  right: 2px;
  bottom: 2px;
  border-radius: 50%;
}

/* Running 狀態：簡化 spinner */
.node-tree .spinner {
  position: absolute;
  top: -1px;
  left: -1px;
  width: 12px;
  height: 12px;
  border: 1px dashed;
  border-radius: 50%;
  opacity: 0.6;
  animation: spin 2s linear infinite;
}

/* Error 狀態：白色圓點 */
.node-tree .error-dot {
  width: 4px;
  height: 4px;
  background: white;
  border-radius: 50%;
  position: absolute;
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
}
```

### Checkpoint 標記

```css
/* 標準 Node 上的 Checkpoint */
.checkpoint-marker-standard {
  position: absolute;
  width: 14px;
  height: 14px;
  background: #D4C257;
  opacity: 0.9;
  transform: rotate(45deg);
  top: 50%;
  left: 50%;
  margin-top: -7px;
  margin-left: -7px;
  pointer-events: none;
}

/* Tree Node 上的 Checkpoint */
.checkpoint-marker-tree {
  position: absolute;
  width: 5px;
  height: 5px;
  background: #D4C257;
  opacity: 0.9;
  transform: rotate(45deg);
  top: 50%;
  left: 50%;
  margin-top: -2.5px;
  margin-left: -2.5px;
  pointer-events: none;
}
```

### 動畫

```css
@keyframes spin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}

@keyframes pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.5; }
}

.animate-spin {
  animation: spin 2s linear infinite;
}

.animate-pulse {
  animation: pulse 1.5s ease-in-out infinite;
}

/* Reduced motion */
@media (prefers-reduced-motion: reduce) {
  .animate-spin,
  .animate-pulse {
    animation: none;
  }
}
```

---

## 7) SVG 實作範例（Tree Visualization）

```svg
<svg width="300" height="400">
  <!-- Edge (Active Path) -->
  <line x1="100" y1="50" x2="100" y2="82"
        stroke="#5785A3" stroke-width="2" stroke-linecap="round" />

  <!-- Edge (Inactive Branch) -->
  <line x1="100" y1="82" x2="120" y2="114"
        stroke="#9CA3AF" stroke-width="1.5"
        stroke-dasharray="4 2" opacity="0.5" />

  <!-- Node Group: User (Idle) -->
  <g transform="translate(100, 50)">
    <circle r="6" fill="#fff" stroke="#595757" stroke-width="1.5" />
    <circle r="3.5" fill="transparent" />
  </g>

  <!-- Node Group: Assistant (Running) -->
  <g transform="translate(100, 82)">
    <circle r="6" fill="#fff" stroke="#5785A3" stroke-width="1.5" />
    <circle r="3.5" fill="transparent" />
    <!-- Spinner -->
    <circle r="6" fill="none" stroke="#5785A3" stroke-width="1"
            stroke-dasharray="3 2" opacity="0.6" class="animate-spin" />
  </g>

  <!-- Node Group: Tool (Checkpoint, Error) -->
  <g transform="translate(120, 114)">
    <circle r="6" fill="#fff" stroke="#57A357" stroke-width="1.5" />
    <circle r="3.5" fill="#C23C3C" />
    <!-- Error dot -->
    <circle r="2" fill="white" />
    <!-- Checkpoint marker -->
    <rect width="5" height="5" x="-2.5" y="-2.5"
          fill="#D4C257" opacity="0.9" transform="rotate(45)" />
  </g>

  <!-- Node Group: Assistant (Streaming) -->
  <g transform="translate(100, 146)">
    <circle r="6" fill="#fff" stroke="#5785A3" stroke-width="1.5" />
    <circle r="3.5" fill="#5785A3" class="animate-pulse" />
  </g>
</svg>
```

---

## 8) 資料結構（API）

```typescript
// GET /api/sessions/:id/tree
interface TreeVisualizationData {
  nodes: TreeNode[];
  active_leaf_id: string;
  checkpoints: Record<string, CheckpointInfo>;
  runtime_status: Record<string, NodeStatus>;
}

interface TreeNode {
  node_id: string;
  parent_id: string | null;
  role: 'system' | 'user' | 'assistant' | 'tool';
  content_preview: string; // 前 60 字
  created_at: number;
  seq: number;
}

interface CheckpointInfo {
  created_at: number;
  strategy: string; // "manual" | "auto_turns" | "auto_token_limit"
}

type NodeStatus = 'idle' | 'running' | 'error' | 'streaming';
```

---

## 9) React 組件範例

```tsx
// 共用 Node 組件
interface NodeProps {
  type: 'system' | 'user' | 'assistant' | 'tool';
  status: 'idle' | 'running' | 'error' | 'streaming';
  hasCheckpoint?: boolean;
  size: 'standard' | 'tree'; // 32px or 12px
  className?: string;
}

function Node({ type, status, hasCheckpoint, size }: NodeProps) {
  const sizeClass = size === 'standard' ? 'node-standard' : 'node-tree';
  const checkpointClass = size === 'standard'
    ? 'checkpoint-marker-standard'
    : 'checkpoint-marker-tree';

  return (
    <div className={`${sizeClass} node-${type}`} style={{ borderColor: NODE_COLORS[type] }}>
      <div className="node-center" style={{ background: getCenterColor(status, type) }}>
        {status === 'running' && <RunningIndicator size={size} />}
        {status === 'error' && <ErrorIndicator size={size} />}
        {status === 'streaming' && <div className="animate-pulse" />}
      </div>
      {hasCheckpoint && <div className={checkpointClass} />}
    </div>
  );
}

// Tree Visualization 組件
function ConversationTree({ data }: { data: TreeVisualizationData }) {
  const positioned = layoutTree(data);

  return (
    <svg width={containerWidth} height={containerHeight}>
      {/* Edges */}
      {positioned.map(node => (
        <Edge key={node.id} from={node.parent} to={node} isActive={node.isActive} />
      ))}

      {/* Nodes */}
      {positioned.map(node => (
        <g key={node.id} transform={`translate(${node.x}, ${node.y})`}>
          <Node
            type={node.role}
            status={data.runtime_status[node.id] || 'idle'}
            hasCheckpoint={!!data.checkpoints[node.id]}
            size="tree"
          />
        </g>
      ))}
    </svg>
  );
}
```

---

## 10) Accessibility

- **Keyboard Navigation**: Tab 遍歷節點，Enter/Space 選取
- **Screen Reader**: 節點帶 `aria-label="User message, idle"`
- **Focus Ring**: 使用 `outline` 代替 `border` 變化
- **Reduced Motion**: 透過 `prefers-reduced-motion` 關閉動畫

---

## 11) 與原 mini-map-design.md 的差異

| 項目 | 原設計（已廢棄） | 新設計（Tree） |
| --- | --- | --- |
| 目標 | 內容位置縮略圖 | 對話樹結構圖 |
| 映射方式 | 垂直位置等比縮放 | 樹狀布局算法 |
| 節點意義 | 訊息卡片的代表 | Session tree node |
| 支援分支 | ❌ | ✅ |
| Checkpoint | 節點類型 | 疊加標記 |
| Scalability | 有問題（4000px） | 解決（固定間距） |

---

## 12) 實作檢查清單

- [ ] 實作共用 Node 組件（標準 + Tree 兩種尺寸）
- [ ] 實作 Tree 布局算法（DFS + 分支展開）
- [ ] 實作連接線渲染（Active/Inactive）
- [ ] 實作 Checkpoint 標記疊加
- [ ] 實作狀態動畫（Running/Streaming/Error）
- [ ] 實作節點互動（Click/Hover/Select）
- [ ] 實作 API endpoint (`GET /api/sessions/:id/tree`)
- [ ] Accessibility 測試（鍵盤導航、Screen Reader）
- [ ] Reduced Motion 支援
- [ ] 響應式容器寬度調整