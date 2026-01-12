# Mini Map 設計規範

> **⚠️ ARCHIVED**: This design has been superseded by [conversation-tree-design.md](../conversation-tree-design.md).
> The linear mini-map approach had scalability issues and couldn't represent the tree structure of conversations.
> See conversation-tree-design.md for the current Tree Visualization design.

---

## 1) 目標與對應關係
- mini map 對應真實 chat component 的垂直位置，不採等距排列。
- 主要用途：快速定位訊息段落、辨識狀態、視窗範圍提示。

## 2) 節點視覺語意
- 外圈顏色 = 節點類型。
- 中心圖示 = 節點狀態。
- Icon 使用 Heroicons inline SVG（gear / exclamation-circle / dot）。

## 3) 類型顏色
| Type | Hex | 說明 |
| --- | --- | --- |
| System | `#7B9FBF` | 系統訊息 |
| User | `#595757` | 使用者輸入 |
| Assistant | `#5785A3` | AI 回覆 |
| Checkpoint | `#D4C257` | 檢查點 |
| Tool | `#57A357` | 工具呼叫 |

## 4) 狀態規則
| 狀態 | 中心填色 | Icon | 動畫 | 說明 |
| --- | --- | --- | --- | --- |
| Idle | 透明 | 無 | - | 已完成/一般 |
| Running | 透明 | Gear，顏色=節點類型色 | 旋轉 | 執行中 |
| Error | `#C23C3C` | Exclamation-circle，白色 | 無 | 錯誤 |
| Streaming | `#5785A3` | Dot，白色 | Pulse | 串流中 |

## 5) 尺寸與比例
| 元素 | 尺寸 |
| --- | --- |
| 容器寬度 | 64px |
| 一般節點 | 32 × 32px |
| 外圈邊框 | 4px |
| 中心區域 | inset 5px（中心約 22 × 22px） |
| 狀態 icon | 22 × 22px |
| Streaming dot | 18 × 18px |
| 摺疊節點 | 40 × 24px（圓角矩形） |
| 節點間距 | 8px |
| 連接線 | 4px × 8px，色 `#7B9FBF` |
| 選取外圈 | ring 2px + offset 1px |

## 6) 版面結構
- Header：顯示 “Path”。
- Body：垂直節點清單，支援滾動。
- Footer：顯示節點總數。

## 7) 摺疊節點
- 以矩形顯示數量，邊框色=dominant type。
- 背景使用該色 20% 透明度。
- 點擊摺疊群組後展開回節點序列。

## 8) 動畫
- Running: gear 旋轉（2s linear infinite）。
- Streaming: pulse（1.5s ease-in-out infinite）。

## 9) 互動
- 一般節點 hover 放大 1.25x。
- 摺疊節點 hover 放大 1.1x。
- 點擊節點更新選取狀態與對應訊息位置。
- Tooltip 顯示訊息摘要（可選）。

## 10) 與 Chat Component 對齊
- 節點位置依聊天卡片高度比例映射，避免等距誤導。
- 長內容不放大節點，改拉長連接線保持節點尺寸一致。
- 建議使用壓縮比例：`mapped = base + height * scale`（例如 0.2）。
- 顯示 viewport 範圍（陰影或框），對應聊天視窗高度。
- 點擊節點時，scroll 到卡片頂部或中線對齊。
- 使用共同的 scroll container 包住 chat 與 mini map，兩者一起滾動。
- 點擊 mini map 節點或 chat card 時，兩側同步選取並把目標卡片對齊容器頂端。

### 對齊公式（示例）
```
scale = 0.2
min_gap = 8
pos_y[i+1] = pos_y[i] + max(min_gap, card_height[i] * scale)
```

### 視覺提示
```
Chat 內容高度:  ████████
Mini map 位置:  ●───●────●
Viewport:       [====]
```

## 11) 狀態與選取樣式
- 選取狀態以外圈 ring 表示，不改變節點顏色。
- Error 與 Streaming 的中心填色固定，不受節點類型影響。

## 12) Accessibility
- Icon 使用 inline SVG，支援高解析縮放。
- 維持 hover 與 focus 視覺差異，避免誤觸。
- 文字對比保持可讀性（白色 icon 對深色底）。

## 13) 實作建議（與 sample 一致）
- Status icon 使用 `<svg><use href="#icon-..."></use></svg>`。
- Streaming dot 用簡單 circle icon。
- Scrollbar 使用細滾動條（4px）。

## 14) CSS Tokens 與範例樣式
```css
:root {
  --system: #7B9FBF;
  --user: #595757;
  --assistant: #5785A3;
  --checkpoint: #D4C257;
  --tool: #57A357;
  --error: #C23C3C;
  --base-200: #DDE5ED;
  --base-300: #D7D2CB;
  --primary: #E8C236;
}

.minimap {
  width: 64px;
  background: var(--base-200);
  border-right: 1px solid var(--base-300);
  display: flex;
  flex-direction: column;
  height: 100%;
}

.node {
  width: 32px;
  height: 32px;
  border-radius: 999px;
  border: 4px solid;
  background: #fff;
  position: relative;
  transition: transform 0.15s ease;
}

.node-center {
  position: absolute;
  top: 5px;
  left: 5px;
  right: 5px;
  bottom: 5px;
  border-radius: 999px;
  display: flex;
  align-items: center;
  justify-content: center;
}

.status-icon { width: 22px; height: 22px; }
.status-icon-dot { width: 18px; height: 18px; }

.animate-spin-slow { animation: spin 2s linear infinite; }
.animate-pulse { animation: pulse 1.5s ease-in-out infinite; }
```

## 15) SVG Sprite 與 Icon 使用
```html
<svg class="icon-sprite" aria-hidden="true">
  <symbol id="icon-gear" viewBox="0 0 24 24">
    <!-- heroicons/cog-6-tooth path -->
    <path d="..."/>
  </symbol>
  <symbol id="icon-exclamation" viewBox="0 0 24 24">
    <!-- heroicons/exclamation-circle path -->
    <path d="..."/>
  </symbol>
  <symbol id="icon-dot" viewBox="0 0 20 20">
    <circle cx="10" cy="10" r="6"/>
  </symbol>
</svg>

<div class="node-center running-tool animate-spin-slow" style="color: var(--tool);">
  <svg class="status-icon status-icon-solid" viewBox="0 0 24 24" aria-hidden="true">
    <use href="#icon-gear"></use>
  </svg>
</div>
```

## 16) 對齊 Mapping Algorithm（含邊界）
```ts
// input: cards[] { id, heightPx }
// output: nodes[] { id, y }
const scale = 0.2;
const minGap = 8;
const topPadding = 6;

let y = topPadding;
for (const card of cards) {
  const gap = Math.max(minGap, card.heightPx * scale);
  nodes.push({ id: card.id, y });
  y += gap;
}

// viewport indicator
// viewportTop / viewportHeight are in chat px
const viewportTopMapped = topPadding + viewportTop * scale;
const viewportHeightMapped = Math.max(12, viewportHeight * scale);
```

## 17) 互動 + Accessibility 細節
- Keyboard：上下鍵移動選取節點，Enter/Space 觸發跳轉。
- Focus：focus ring 與 selected ring 同系統色系但加明亮 offset。
- Tooltip：hover/long-press 顯示 preview，避免遮擋 mini map 主體。
- Reduced motion：使用 `prefers-reduced-motion` 關閉旋轉與 pulse。
- 點擊區域：節點以 32px 尺寸為準，避免過小命中。
