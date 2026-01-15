# Chat UI Message Card Component Plan

- Feature name: `chat-ui-message-card`
- Status: Completed ✅
- Created: 2026-01-12
- Completed: 2026-01-12
- Parent plan: [chat-ui-component-plan.md](./chat-ui-component-plan.md)

## 1) Overview

### Goal
Create reusable message card components that display user/assistant/system/tool messages with role-specific styling, support for thinking blocks, tool calls, and streaming states.

### Design Principles

1. **Role-based visual identity** - Clear color coding for each message type
2. **Hierarchical information** - Header → Thinking → Tools → Content
3. **Progressive disclosure** - Collapsible tool calls and results
4. **Streaming feedback** - Animated indicators during live updates
5. **Accessibility** - Semantic HTML, ARIA labels, keyboard navigation

## 2) Component Types

### MessageCard (Primary)
Main message display for user/assistant/system messages.

**Props:**
```typescript
interface MessageCardProps {
  id: string;                    // Node ID for tree sync
  role: 'user' | 'assistant' | 'system';
  content: string;
  thinking?: string;             // Reasoning content (optional)
  toolCalls?: ToolCall[];        // Tool execution requests
  toolResults?: ToolResult[];    // Tool execution results
  timestamp?: Date;
  isStreaming?: boolean;         // Show streaming indicator
  isSelected?: boolean;          // Highlight for tree sync
  onSelect?: (id: string) => void;
}
```

**Visual Structure:**
```
┌────────────────────────────────────────┐
│ Role Badge    Timestamp    [Streaming] │ ← Header
├────────────────────────────────────────┤
│ 💭 Thinking                            │ ← Thinking Block (purple)
│ [Reasoning content...]                 │
├────────────────────────────────────────┤
│ 🔧 Tool Call: search_code              │ ← Tool Call (cyan, collapsible)
│ { "query": "useEffect", "path": "..." }│
│                                         │
│ ✅ Tool Result: search_code            │ ← Tool Result (green/red)
│ Found 12 matches in 3 files            │
├────────────────────────────────────────┤
│ [Main message content...]              │ ← Content
│ Multiple lines of text                 │
│ with proper formatting                 │
└────────────────────────────────────────┘
```

### CheckpointCard
Special card for checkpoint summaries.

**Props:**
```typescript
interface CheckpointCardProps {
  id: string;
  nodeId: string;               // Checkpoint node ID
  summary: string;
  strategy: string;             // e.g., "auto_turns"
  createdAt: Date;
  isExpanded?: boolean;
  onToggle?: () => void;
  onSelect?: (nodeId: string) => void;
}
```

**Visual Structure:**
```
┌────────────────────────────────────────┐
│ 📦 Checkpoint    auto_turns    [Date] │ ← Header (purple theme)
├────────────────────────────────────────┤
│ [Collapsed] Click to expand summary   │ ← Collapsed state
└────────────────────────────────────────┘

┌────────────────────────────────────────┐
│ 📦 Checkpoint    auto_turns    [Date] │
├────────────────────────────────────────┤
│ Summary:                               │ ← Expanded state
│ [Full checkpoint summary text...]      │
│ [Messages summarized: 15]              │
└────────────────────────────────────────┘
```

### ToolCallCard
Embedded card for tool execution details.

**Props:**
```typescript
interface ToolCallCardProps {
  id: string;                   // tool_call_id
  name: string;                 // Tool function name
  input: any;                   // Tool input parameters
  result?: ToolResult;          // Result if available
  isExpanded?: boolean;
  onToggle?: () => void;
}

interface ToolResult {
  tool_call_id: string;
  tool_name: string;
  result: string;
  is_error: boolean;
}
```

**Visual Structure:**
```
┌────────────────────────────────────────┐
│ 🔧 search_code                    [▼] │ ← Tool name + toggle
├────────────────────────────────────────┤
│ Input:                                 │ ← Input params (JSON)
│ {                                      │
│   "query": "useEffect",                │
│   "path": "src/"                       │
│ }                                      │
├────────────────────────────────────────┤
│ ✅ Result (125ms)                     │ ← Result status
│ Found 12 matches in 3 files:           │
│ - src/hooks/useData.ts (5)            │
│ - src/components/App.tsx (4)          │
│ - src/utils/helpers.ts (3)            │
└────────────────────────────────────────┘

[Collapsed view]
┌────────────────────────────────────────┐
│ 🔧 search_code                    [▶] │
│ ✅ Success                             │
└────────────────────────────────────────┘
```

## 3) Styling Specifications

### Color Theme (from tree-visualization-demo.html)

**Role Colors (CSS Variables in `web/src/index.css`):**
```css
:root {
  /* Role colors from tree-visualization-demo.html */
  --role-user: 0 0% 35%;           /* #595757 深灰色 */
  --role-assistant: 201 32% 49%;    /* #5785A3 藍灰色 */
  --role-system: 206 30% 61%;       /* #7B9FBF 淺藍色 */
  --role-tool: 120 40% 49%;         /* #57A357 綠色 */
  --role-checkpoint: 47 58% 58%;    /* #D4C257 黃色 */
  --role-error: 0 55% 50%;          /* #C23C3C 紅色 */
}
```

**Tailwind Usage:**
```tsx
// Background with 8% opacity
className="bg-[hsl(var(--role-user)/0.08)]"

// Border with 25% opacity
className="border-[hsl(var(--role-user)/0.25)]"

// Hover border with 40% opacity
className="hover:border-[hsl(var(--role-user)/0.4)]"

// Full opacity text
className="text-[hsl(var(--role-user))]"
```

**Thinking Block:**
```css
.thinking-block {
  background: rgba(147, 51, 234, 0.1);  /* Purple-600 with opacity */
  border: 1px solid rgba(168, 85, 247, 0.3);
  border-radius: 8px;
  padding: 12px;
  margin-bottom: 12px;
}

.thinking-header {
  color: #a855f7;  /* Purple-500 */
  font-size: 0.75rem;
  font-weight: 600;
  margin-bottom: 6px;
}
```

**Tool Cards:**
```css
.tool-call-card {
  background: rgba(6, 182, 212, 0.1);   /* Cyan with opacity */
  border: 1px solid rgba(6, 182, 212, 0.3);
}

.tool-result-success {
  background: rgba(34, 197, 94, 0.1);   /* Green */
  border: 1px solid rgba(34, 197, 94, 0.3);
}

.tool-result-error {
  background: rgba(239, 68, 68, 0.1);   /* Red */
  border: 1px solid rgba(239, 68, 68, 0.3);
}
```

**Checkpoint Card:**
```css
.checkpoint-card {
  background: rgba(88, 28, 135, 0.2);   /* Purple-900 */
  border: 1px solid rgba(168, 85, 247, 0.4);
  border-left: 4px solid #a855f7;       /* Purple accent */
}

.checkpoint-card:hover {
  background: rgba(88, 28, 135, 0.3);
  border-color: #a855f7;
}
```

**Streaming Indicator:**
```css
.streaming-cursor {
  display: inline-block;
  width: 2px;
  height: 1em;
  background: #6b7280;
  margin-left: 2px;
  animation: blink 1s infinite;
}

@keyframes blink {
  0%, 49% { opacity: 1; }
  50%, 100% { opacity: 0; }
}
```

### Layout Specifications

**Card Dimensions:**
- Max width: 800px (centered)
- Padding: 16px
- Border radius: 8px
- Border width: 1px
- Margin between cards: 12px

**Typography:**
- Role label: 12px, bold, uppercase
- Timestamp: 11px, gray-500
- Content: 14px, line-height 1.6
- Code blocks: 13px, monospace
- Tool names: 13px, cyan-400

**Spacing:**
- Header: 8px bottom margin
- Thinking block: 12px bottom margin
- Tool cards: 8px between each
- Content: 0 margin (last element)

## 4) Component Implementation

### MessageCard Component

```typescript
// web/src/components/chat/MessageCard.tsx

import React, { useState } from 'react';
import { format } from 'date-fns';
import { ToolCallCard } from './ToolCallCard';
import { ThinkingBlock } from './ThinkingBlock';

export interface MessageCardProps {
  id: string;
  role: 'user' | 'assistant' | 'system';
  content: string;
  thinking?: string;
  toolCalls?: ToolCall[];
  toolResults?: ToolResult[];
  timestamp?: Date;
  isStreaming?: boolean;
  isSelected?: boolean;
  onSelect?: (id: string) => void;
}

export function MessageCard({
  id,
  role,
  content,
  thinking,
  toolCalls = [],
  toolResults = [],
  timestamp,
  isStreaming = false,
  isSelected = false,
  onSelect,
}: MessageCardProps) {
  const roleColors = {
    user: 'bg-blue-900/30 border-blue-700 hover:border-blue-500',
    assistant: 'bg-gray-800/50 border-gray-700 hover:border-gray-500',
    system: 'bg-yellow-900/30 border-yellow-700 hover:border-yellow-500',
  };

  const roleLabels = {
    user: 'You',
    assistant: 'Assistant',
    system: 'System',
  };

  const handleClick = () => {
    if (onSelect) {
      onSelect(id);
    }
  };

  return (
    <div
      id={`message-${id}`}
      className={`
        message-card rounded-lg border p-4 transition-all cursor-pointer
        ${roleColors[role]}
        ${isSelected ? 'ring-2 ring-offset-2 ring-blue-500' : ''}
      `}
      onClick={handleClick}
    >
      {/* Header */}
      <div className="message-header flex items-center justify-between mb-3">
        <div className="flex items-center gap-2">
          <span className="role-label text-xs font-bold uppercase text-gray-300">
            {roleLabels[role]}
          </span>
          {isStreaming && (
            <span className="text-xs text-gray-500 flex items-center gap-1">
              <span className="inline-block w-2 h-2 bg-green-500 rounded-full animate-pulse" />
              streaming...
            </span>
          )}
        </div>
        {timestamp && (
          <span className="text-xs text-gray-500">
            {format(timestamp, 'HH:mm:ss')}
          </span>
        )}
      </div>

      {/* Thinking block */}
      {thinking && <ThinkingBlock content={thinking} />}

      {/* Tool calls */}
      {toolCalls.length > 0 && (
        <div className="tool-calls space-y-2 mb-3">
          {toolCalls.map((call) => {
            const result = toolResults.find((r) => r.tool_call_id === call.id);
            return (
              <ToolCallCard
                key={call.id}
                id={call.id}
                name={call.name}
                input={call.input}
                result={result}
              />
            );
          })}
        </div>
      )}

      {/* Main content */}
      {content && (
        <div className="message-content text-sm text-gray-200 whitespace-pre-wrap">
          {content}
          {isStreaming && <span className="streaming-cursor" />}
        </div>
      )}
    </div>
  );
}
```

### CheckpointCard Component

```typescript
// web/src/components/chat/CheckpointCard.tsx

import React, { useState } from 'react';
import { format } from 'date-fns';

export interface CheckpointCardProps {
  id: string;
  nodeId: string;
  summary: string;
  strategy: string;
  createdAt: Date;
  isExpanded?: boolean;
  onToggle?: () => void;
  onSelect?: (nodeId: string) => void;
}

export function CheckpointCard({
  id,
  nodeId,
  summary,
  strategy,
  createdAt,
  isExpanded: controlledExpanded,
  onToggle,
  onSelect,
}: CheckpointCardProps) {
  const [internalExpanded, setInternalExpanded] = useState(false);
  const isExpanded = controlledExpanded ?? internalExpanded;

  const handleToggle = () => {
    if (onToggle) {
      onToggle();
    } else {
      setInternalExpanded(!internalExpanded);
    }
  };

  const handleClick = () => {
    if (onSelect) {
      onSelect(nodeId);
    }
  };

  return (
    <div
      id={`checkpoint-${id}`}
      className="checkpoint-card bg-purple-900/20 border border-purple-700/40 border-l-4 border-l-purple-500 rounded-lg p-4 hover:bg-purple-900/30 transition-all cursor-pointer"
      onClick={handleClick}
    >
      {/* Header */}
      <div className="flex items-center justify-between mb-2">
        <div className="flex items-center gap-2">
          <span className="text-lg">📦</span>
          <span className="text-xs font-bold uppercase text-purple-400">
            Checkpoint
          </span>
          <span className="text-xs text-gray-500">{strategy}</span>
        </div>
        <div className="flex items-center gap-2">
          <span className="text-xs text-gray-500">
            {format(createdAt, 'HH:mm:ss')}
          </span>
          <button
            onClick={(e) => {
              e.stopPropagation();
              handleToggle();
            }}
            className="text-xs text-purple-400 hover:text-purple-300"
          >
            {isExpanded ? '▼' : '▶'}
          </button>
        </div>
      </div>

      {/* Content */}
      {isExpanded ? (
        <div className="text-sm text-gray-300 whitespace-pre-wrap">
          <div className="font-semibold mb-1">Summary:</div>
          {summary}
        </div>
      ) : (
        <div className="text-xs text-gray-500 italic">
          Click to expand summary
        </div>
      )}
    </div>
  );
}
```

### ToolCallCard Component

```typescript
// web/src/components/chat/ToolCallCard.tsx

import React, { useState } from 'react';

export interface ToolCallCardProps {
  id: string;
  name: string;
  input: any;
  result?: {
    tool_call_id: string;
    tool_name: string;
    result: string;
    is_error: boolean;
  };
  isExpanded?: boolean;
  onToggle?: () => void;
}

export function ToolCallCard({
  id,
  name,
  input,
  result,
  isExpanded: controlledExpanded,
  onToggle,
}: ToolCallCardProps) {
  const [internalExpanded, setInternalExpanded] = useState(true);
  const isExpanded = controlledExpanded ?? internalExpanded;

  const handleToggle = () => {
    if (onToggle) {
      onToggle();
    } else {
      setInternalExpanded(!internalExpanded);
    }
  };

  return (
    <div className="tool-call-card bg-cyan-900/10 border border-cyan-700/30 rounded-lg overflow-hidden">
      {/* Tool Call Header */}
      <div
        className="flex items-center justify-between p-3 cursor-pointer hover:bg-cyan-900/20 transition-colors"
        onClick={handleToggle}
      >
        <div className="flex items-center gap-2">
          <span className="text-cyan-400">🔧</span>
          <span className="text-sm font-semibold text-cyan-400">{name}</span>
        </div>
        <span className="text-xs text-gray-500">{isExpanded ? '▼' : '▶'}</span>
      </div>

      {/* Expanded Content */}
      {isExpanded && (
        <div className="px-3 pb-3 space-y-2">
          {/* Input */}
          <div>
            <div className="text-xs font-semibold text-gray-400 mb-1">Input:</div>
            <pre className="text-xs text-gray-300 bg-black/20 rounded p-2 overflow-x-auto">
              {JSON.stringify(input, null, 2)}
            </pre>
          </div>

          {/* Result */}
          {result && (
            <div
              className={`rounded p-2 ${
                result.is_error
                  ? 'bg-red-900/20 border border-red-700/30'
                  : 'bg-green-900/20 border border-green-700/30'
              }`}
            >
              <div
                className={`text-xs font-semibold mb-1 flex items-center gap-1 ${
                  result.is_error ? 'text-red-400' : 'text-green-400'
                }`}
              >
                <span>{result.is_error ? '❌' : '✅'}</span>
                <span>Result</span>
              </div>
              <pre className="text-xs text-gray-300 overflow-x-auto max-h-40">
                {result.result}
              </pre>
            </div>
          )}
        </div>
      )}

      {/* Collapsed View */}
      {!isExpanded && result && (
        <div className="px-3 pb-2">
          <span
            className={`text-xs ${
              result.is_error ? 'text-red-400' : 'text-green-400'
            }`}
          >
            {result.is_error ? '❌ Error' : '✅ Success'}
          </span>
        </div>
      )}
    </div>
  );
}
```

### ThinkingBlock Component

```typescript
// web/src/components/chat/ThinkingBlock.tsx

import React from 'react';

export interface ThinkingBlockProps {
  content: string;
}

export function ThinkingBlock({ content }: ThinkingBlockProps) {
  return (
    <div className="thinking-block bg-purple-900/10 border border-purple-700/30 rounded-lg p-3 mb-3">
      <div className="text-xs font-semibold text-purple-400 mb-1 flex items-center gap-1">
        <span>💭</span>
        <span>Thinking</span>
      </div>
      <div className="text-sm text-gray-300 whitespace-pre-wrap">
        {content}
      </div>
    </div>
  );
}
```

## 5) Acceptance Criteria

### Visual Styling
- [x] User messages use standardized gray theme from CSS variables
- [x] Assistant messages use standardized blue-gray theme from CSS variables
- [x] System messages use standardized light blue theme from CSS variables
- [x] Thinking blocks use purple theme with Brain icon (lucide-react)
- [x] Tool calls use green theme with Wrench icon (lucide-react) and collapsible UI
- [x] Tool results show success (CheckCircle) or error (XCircle) state with lucide-react icons
- [x] All colors use CSS variables with HSL format and opacity modifiers

### Interaction
- [x] Clicking message card triggers onSelect callback
- [x] Selected message has visible highlight
- [x] Tool cards expand/collapse on click
- [x] Hover states provide visual feedback

### Streaming
- [x] Streaming indicator shows in demo
- [x] Content displays properly
- [x] Thinking block displays properly
- [x] Tool calls display properly
- [x] Tool results display properly

### Accessibility
- [x] Semantic HTML structure
- [x] Role labels visible
- [x] Keyboard navigation support via click handlers
- [x] Timestamps formatted properly

## 6) Testing Checklist

- [x] Render user message with content only
- [x] Render assistant message with thinking block
- [x] Render message with multiple tool calls
- [x] Render message with tool results (success and error)
- [x] Test streaming state indicator
- [x] Test message selection highlight
- [x] Test tool card expand/collapse
- [x] Test timestamp formatting
- [x] Test long content overflow handling
- [x] Test JSON formatting in tool inputs

## 7) Implementation Summary

**Completed Components:**
- ✅ `web/src/components/chat/MessageCard.tsx` - Main message container
- ✅ `web/src/components/chat/ThinkingBlock.tsx` - Reasoning display with Brain icon
- ✅ `web/src/components/chat/ToolCallCard.tsx` - Tool execution with Wrench/CheckCircle/XCircle icons
- ✅ `web/src/pages/MessageCardDemo.tsx` - Demo page with 11 mock messages

**Key Features Implemented:**
- Light theme with global CSS variables
- Standardized role colors from tree-visualization-demo.html
- Unified lucide-react icon library (Brain, Wrench, CheckCircle, XCircle)
- Collapsible tool calls with JSON formatting
- Message selection system
- Proper TypeScript interfaces and type exports

**Demo Routes:**
- `/message-demo` - Main demo route
- `/message-card` - Alternative demo route

---

**Status:** Completed ✅
**Dependencies:** Tailwind CSS v4, lucide-react, React 18, TypeScript  
**Actual effort:** 1 day
