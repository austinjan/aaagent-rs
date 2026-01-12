# Chat UI Input Component Plan

- Feature name: `chat-ui-input`
- Status: Planning
- Created: 2026-01-12
- Parent plan: [chat-ui-component-plan.md](./chat-ui-component-plan.md)

## 1) Overview

### Goal
Build an intelligent message input component with auto-resizing textarea, preset selection, config overrides, and seamless integration with SSE streaming state.

### Design Principles

1. **Quick access to common actions** - Preset dropdown, send button prominently placed
2. **Progressive disclosure** - Advanced config hidden until needed
3. **Smart input behavior** - Auto-resize, Enter to send, Shift+Enter for newlines
4. **State awareness** - Disable during streaming, show validation errors
5. **Mobile-friendly** - Touch-optimized, proper keyboard handling

## 2) Component Architecture

```
ChatInput
├── InputField (auto-resizing textarea)
├── InputControls (buttons and quick actions)
│   ├── PresetSelector (dropdown)
│   ├── SendButton (primary action)
│   └── AdvancedToggle (show/hide config)
└── ConfigOverrides (collapsible panel)
    ├── ModelSelector (dropdown)
    ├── CreativitySlider (0.0-1.0)
    ├── VerbositySelector (short/normal/long)
    ├── RoundsInput (number)
    └── SamplingParams (top_p, frequency, presence)
```

## 3) Component Props

```typescript
interface ChatInputProps {
  sessionId: string;
  isStreaming: boolean;          // Disable input during active stream
  currentConfig?: ResolvedConfig; // Current session config
  onSendMessage: (message: string, config?: ChatConfig) => Promise<void>;
  onConfigChange?: (config: ChatConfig) => void;
}
```

## 4) Visual Design

### Layout

```
┌────────────────────────────────────────────────────┐
│  Preset: [General ▼]              [⚙️ Advanced]   │
├────────────────────────────────────────────────────┤
│  ┌──────────────────────────────────────────────┐ │
│  │ Type your message here...                    │ │
│  │                                              │ │ ← Auto-resize
│  │                                              │ │   (1-10 lines)
│  └──────────────────────────────────────────────┘ │
│                                    [📤 Send (↵)] │
└────────────────────────────────────────────────────┘

[Expanded with Advanced Config]
┌────────────────────────────────────────────────────┐
│  Preset: [General ▼]              [⚙️ Advanced ▼] │
├────────────────────────────────────────────────────┤
│  ┌─ Advanced Configuration ────────────────────┐  │
│  │ Model:       [gpt-5-mini ▼]                 │  │
│  │ Creativity:  [●─────────] 0.5               │  │
│  │ Verbosity:   [Normal ▼]                     │  │
│  │ Max Rounds:  [30]                           │  │
│  │                                              │  │
│  │ Sampling (Optional):                        │  │
│  │   Top-p:              [0.9]                 │  │
│  │   Frequency Penalty:  [0.0]                 │  │
│  │   Presence Penalty:   [0.0]                 │  │
│  └──────────────────────────────────────────────┘  │
├────────────────────────────────────────────────────┤
│  ┌──────────────────────────────────────────────┐ │
│  │ Type your message here...                    │ │
│  └──────────────────────────────────────────────┘ │
│                                    [📤 Send (↵)] │
└────────────────────────────────────────────────────┘
```

### States

**Normal (Ready):**
- Textarea: White text, gray border
- Send button: Blue background, enabled
- Preset selector: Enabled

**Streaming (Disabled):**
- Textarea: Gray text, disabled cursor
- Send button: Gray background, disabled
- Overlay text: "Agent is responding..."

**Error:**
- Red border on textarea
- Error message below input
- Retry button available

**Validation Error:**
- Yellow border on textarea
- Warning message below input
- Example: "Message cannot be empty"

## 5) Input Behavior

### Keyboard Shortcuts

- **Enter**: Send message (if not empty)
- **Shift + Enter**: Insert newline
- **Ctrl/Cmd + Enter**: Send message (alternative)
- **Escape**: Clear input (if empty) or cancel edit (if editing)

### Auto-resize Logic

```typescript
function autoResizeTextarea(element: HTMLTextAreaElement) {
  // Reset height to recalculate
  element.style.height = 'auto';
  
  // Calculate new height based on scrollHeight
  const lineHeight = 24; // 1.5rem
  const minLines = 1;
  const maxLines = 10;
  
  const minHeight = lineHeight * minLines;
  const maxHeight = lineHeight * maxLines;
  
  const newHeight = Math.min(
    Math.max(element.scrollHeight, minHeight),
    maxHeight
  );
  
  element.style.height = `${newHeight}px`;
}
```

### Validation Rules

**Message:**
- Must not be empty (after trim)
- Max length: 10,000 characters
- No validation on newlines or special chars

**Config Overrides:**
- Creativity: 0.0 - 1.0
- Verbosity: "short" | "normal" | "long"
- Rounds: 1 - 100
- Top-p: 0.0 - 1.0 (optional)
- Frequency penalty: -2.0 - 2.0 (optional)
- Presence penalty: -2.0 - 2.0 (optional)

## 6) Component Implementation

### ChatInput Component

```typescript
// web/src/components/chat/ChatInput.tsx

import React, { useState, useRef, useEffect } from 'react';
import { PresetSelector } from './PresetSelector';
import { ConfigOverrides } from './ConfigOverrides';

export interface ChatInputProps {
  sessionId: string;
  isStreaming: boolean;
  currentConfig?: ResolvedConfig;
  onSendMessage: (message: string, config?: ChatConfig) => Promise<void>;
  onConfigChange?: (config: ChatConfig) => void;
}

export function ChatInput({
  sessionId,
  isStreaming,
  currentConfig,
  onSendMessage,
  onConfigChange,
}: ChatInputProps) {
  const [message, setMessage] = useState('');
  const [preset, setPreset] = useState('general');
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [overrides, setOverrides] = useState<ChatOverrides>({});
  
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  // Auto-resize textarea
  useEffect(() => {
    if (textareaRef.current) {
      autoResizeTextarea(textareaRef.current);
    }
  }, [message]);

  // Focus textarea on mount
  useEffect(() => {
    textareaRef.current?.focus();
  }, []);

  const handleSend = async () => {
    const trimmed = message.trim();
    
    // Validation
    if (!trimmed) {
      setError('Message cannot be empty');
      return;
    }
    
    if (trimmed.length > 10000) {
      setError('Message too long (max 10,000 characters)');
      return;
    }

    setError(null);

    // Build config
    const config: ChatConfig = {
      preset,
      tools_enabled: true,
      intent: {
        creativity: overrides.creativity ?? 0.5,
        verbosity: overrides.verbosity ?? 'normal',
        rounds: overrides.rounds ?? 30,
      },
      overrides: {
        model: overrides.model,
        top_p: overrides.top_p,
        frequency_penalty: overrides.frequency_penalty,
        presence_penalty: overrides.presence_penalty,
      },
    };

    try {
      await onSendMessage(trimmed, config);
      setMessage(''); // Clear input on success
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to send message');
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    // Send on Enter (without Shift)
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
    
    // Also allow Ctrl/Cmd + Enter
    if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      handleSend();
    }
  };

  return (
    <div className="chat-input border-t border-gray-700 bg-gray-900 p-4">
      {/* Top Controls */}
      <div className="flex items-center justify-between mb-3">
        <PresetSelector
          value={preset}
          onChange={setPreset}
          disabled={isStreaming}
        />
        
        <button
          className="text-sm text-gray-400 hover:text-gray-200 flex items-center gap-1"
          onClick={() => setShowAdvanced(!showAdvanced)}
          disabled={isStreaming}
        >
          <span>⚙️</span>
          <span>Advanced</span>
          <span>{showAdvanced ? '▼' : '▶'}</span>
        </button>
      </div>

      {/* Advanced Config Panel */}
      {showAdvanced && (
        <ConfigOverrides
          value={overrides}
          onChange={setOverrides}
          disabled={isStreaming}
        />
      )}

      {/* Error Display */}
      {error && (
        <div className="mb-3 text-sm text-red-400 bg-red-900/20 border border-red-700/30 rounded p-2">
          {error}
        </div>
      )}

      {/* Textarea */}
      <div className="flex gap-2">
        <textarea
          ref={textareaRef}
          value={message}
          onChange={(e) => setMessage(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder={
            isStreaming
              ? 'Agent is responding...'
              : 'Type your message here... (Enter to send, Shift+Enter for newline)'
          }
          disabled={isStreaming}
          className={`
            flex-1 bg-gray-800 text-gray-200 rounded-lg px-4 py-3
            resize-none overflow-y-auto
            placeholder:text-gray-500
            focus:outline-none focus:ring-2 focus:ring-blue-500
            disabled:opacity-50 disabled:cursor-not-allowed
            ${error ? 'border-red-500 border-2' : 'border border-gray-600'}
          `}
          style={{
            minHeight: '48px',
            maxHeight: '240px',
            lineHeight: '24px',
          }}
        />

        {/* Send Button */}
        <button
          onClick={handleSend}
          disabled={isStreaming || !message.trim()}
          className={`
            px-6 py-3 rounded-lg font-semibold
            transition-all flex items-center gap-2
            ${
              isStreaming || !message.trim()
                ? 'bg-gray-700 text-gray-500 cursor-not-allowed'
                : 'bg-blue-600 text-white hover:bg-blue-500 active:scale-95'
            }
          `}
        >
          <span>📤</span>
          <span className="hidden sm:inline">Send</span>
          <span className="text-xs text-gray-400">↵</span>
        </button>
      </div>

      {/* Hint Text */}
      <div className="mt-2 text-xs text-gray-500">
        Press Enter to send, Shift+Enter for new line
      </div>
    </div>
  );
}

function autoResizeTextarea(element: HTMLTextAreaElement) {
  element.style.height = 'auto';
  const lineHeight = 24;
  const minHeight = lineHeight;
  const maxHeight = lineHeight * 10;
  const newHeight = Math.min(
    Math.max(element.scrollHeight, minHeight),
    maxHeight
  );
  element.style.height = `${newHeight}px`;
}
```

### PresetSelector Component

```typescript
// web/src/components/chat/PresetSelector.tsx

import React from 'react';

const PRESETS = [
  { value: 'general', label: 'General Assistant', icon: '💬' },
  { value: 'coding', label: 'Software Engineer', icon: '💻' },
  { value: 'research', label: 'Research Assistant', icon: '🔬' },
  { value: 'quick', label: 'Quick & Efficient', icon: '⚡' },
];

export interface PresetSelectorProps {
  value: string;
  onChange: (value: string) => void;
  disabled?: boolean;
}

export function PresetSelector({ value, onChange, disabled }: PresetSelectorProps) {
  const selected = PRESETS.find((p) => p.value === value) || PRESETS[0];

  return (
    <div className="flex items-center gap-2">
      <label className="text-sm text-gray-400">Preset:</label>
      <select
        value={value}
        onChange={(e) => onChange(e.target.value)}
        disabled={disabled}
        className="
          bg-gray-800 text-gray-200 rounded px-3 py-1 text-sm
          border border-gray-600 focus:border-blue-500
          disabled:opacity-50 disabled:cursor-not-allowed
        "
      >
        {PRESETS.map((preset) => (
          <option key={preset.value} value={preset.value}>
            {preset.icon} {preset.label}
          </option>
        ))}
      </select>
    </div>
  );
}
```

### ConfigOverrides Component

```typescript
// web/src/components/chat/ConfigOverrides.tsx

import React from 'react';

export interface ChatOverrides {
  model?: string;
  creativity?: number;
  verbosity?: string;
  rounds?: number;
  top_p?: number;
  frequency_penalty?: number;
  presence_penalty?: number;
}

export interface ConfigOverridesProps {
  value: ChatOverrides;
  onChange: (overrides: ChatOverrides) => void;
  disabled?: boolean;
}

export function ConfigOverrides({ value, onChange, disabled }: ConfigOverridesProps) {
  const handleChange = (key: keyof ChatOverrides, newValue: any) => {
    onChange({ ...value, [key]: newValue });
  };

  return (
    <div className="bg-gray-800/50 border border-gray-700 rounded-lg p-4 mb-3 space-y-3">
      <h3 className="text-sm font-semibold text-gray-300 mb-2">
        Advanced Configuration
      </h3>

      {/* Model */}
      <div className="flex items-center gap-3">
        <label className="text-sm text-gray-400 w-32">Model:</label>
        <select
          value={value.model || 'gpt-5-mini'}
          onChange={(e) => handleChange('model', e.target.value)}
          disabled={disabled}
          className="flex-1 bg-gray-800 text-gray-200 rounded px-3 py-1 text-sm border border-gray-600"
        >
          <option value="gpt-5">GPT-5</option>
          <option value="gpt-5-mini">GPT-5 Mini</option>
          <option value="gpt-5-nano">GPT-5 Nano</option>
          <option value="gpt-5.2">GPT-5.2</option>
          <option value="gemini-3-flash-preview">Gemini 3 Flash</option>
          <option value="gemini-3-pro-preview">Gemini 3 Pro</option>
        </select>
      </div>

      {/* Creativity */}
      <div className="flex items-center gap-3">
        <label className="text-sm text-gray-400 w-32">
          Creativity: {(value.creativity ?? 0.5).toFixed(1)}
        </label>
        <input
          type="range"
          min="0"
          max="1"
          step="0.1"
          value={value.creativity ?? 0.5}
          onChange={(e) => handleChange('creativity', parseFloat(e.target.value))}
          disabled={disabled}
          className="flex-1"
        />
      </div>

      {/* Verbosity */}
      <div className="flex items-center gap-3">
        <label className="text-sm text-gray-400 w-32">Verbosity:</label>
        <select
          value={value.verbosity || 'normal'}
          onChange={(e) => handleChange('verbosity', e.target.value)}
          disabled={disabled}
          className="flex-1 bg-gray-800 text-gray-200 rounded px-3 py-1 text-sm border border-gray-600"
        >
          <option value="short">Short (8K tokens)</option>
          <option value="normal">Normal (16K tokens)</option>
          <option value="long">Long (32K tokens)</option>
        </select>
      </div>

      {/* Max Rounds */}
      <div className="flex items-center gap-3">
        <label className="text-sm text-gray-400 w-32">Max Rounds:</label>
        <input
          type="number"
          min="1"
          max="100"
          value={value.rounds ?? 30}
          onChange={(e) => handleChange('rounds', parseInt(e.target.value))}
          disabled={disabled}
          className="flex-1 bg-gray-800 text-gray-200 rounded px-3 py-1 text-sm border border-gray-600"
        />
      </div>

      {/* Sampling Parameters (Collapsible) */}
      <details className="text-sm">
        <summary className="text-gray-400 cursor-pointer hover:text-gray-300">
          Sampling Parameters (Optional)
        </summary>
        <div className="mt-2 space-y-2 pl-4">
          <div className="flex items-center gap-3">
            <label className="text-gray-400 w-40">Top-p:</label>
            <input
              type="number"
              min="0"
              max="1"
              step="0.1"
              value={value.top_p ?? ''}
              placeholder="0.9"
              onChange={(e) => handleChange('top_p', e.target.value ? parseFloat(e.target.value) : undefined)}
              disabled={disabled}
              className="flex-1 bg-gray-800 text-gray-200 rounded px-3 py-1 text-sm border border-gray-600"
            />
          </div>
          <div className="flex items-center gap-3">
            <label className="text-gray-400 w-40">Frequency Penalty:</label>
            <input
              type="number"
              min="-2"
              max="2"
              step="0.1"
              value={value.frequency_penalty ?? ''}
              placeholder="0.0"
              onChange={(e) => handleChange('frequency_penalty', e.target.value ? parseFloat(e.target.value) : undefined)}
              disabled={disabled}
              className="flex-1 bg-gray-800 text-gray-200 rounded px-3 py-1 text-sm border border-gray-600"
            />
          </div>
          <div className="flex items-center gap-3">
            <label className="text-gray-400 w-40">Presence Penalty:</label>
            <input
              type="number"
              min="-2"
              max="2"
              step="0.1"
              value={value.presence_penalty ?? ''}
              placeholder="0.0"
              onChange={(e) => handleChange('presence_penalty', e.target.value ? parseFloat(e.target.value) : undefined)}
              disabled={disabled}
              className="flex-1 bg-gray-800 text-gray-200 rounded px-3 py-1 text-sm border border-gray-600"
            />
          </div>
        </div>
      </details>
    </div>
  );
}
```

## 7) Acceptance Criteria

### Input Behavior
- [ ] Textarea auto-resizes from 1 to 10 lines
- [ ] Enter key sends message
- [ ] Shift+Enter inserts newline
- [ ] Ctrl/Cmd+Enter sends message
- [ ] Input clears after successful send
- [ ] Input disabled during streaming

### Validation
- [ ] Empty messages show error
- [ ] Messages >10K chars show error
- [ ] Invalid creativity range (0-1) shows error
- [ ] Invalid rounds (1-100) shows error
- [ ] Sampling params validated (-2 to 2)

### Preset Selection
- [ ] Preset dropdown shows 4 options with icons
- [ ] Changing preset updates config
- [ ] Preset selector disabled during streaming

### Advanced Config
- [ ] Advanced panel is collapsible
- [ ] Model dropdown shows all supported models
- [ ] Creativity slider updates value display
- [ ] Verbosity dropdown shows token counts
- [ ] Rounds input accepts 1-100
- [ ] Sampling params are optional and collapsible

### Visual Feedback
- [ ] Send button disabled when input empty
- [ ] Send button disabled during streaming
- [ ] Error messages display in red box
- [ ] Focus ring on textarea when focused
- [ ] Placeholder text changes based on state

## 8) Testing Checklist

- [ ] Send message with default config
- [ ] Send message with each preset
- [ ] Send message with model override
- [ ] Send message with creativity 0.0, 0.5, 1.0
- [ ] Send message with each verbosity level
- [ ] Send message with custom rounds
- [ ] Test Enter key sends message
- [ ] Test Shift+Enter adds newline
- [ ] Test empty message validation
- [ ] Test long message validation
- [ ] Test textarea auto-resize behavior
- [ ] Test input disabled during streaming
- [ ] Test advanced panel expand/collapse
- [ ] Test sampling parameters (optional)

---

**Status:** Planning  
**Dependencies:** Config API, SSE streaming hook  
**Estimated effort:** 2-3 days
