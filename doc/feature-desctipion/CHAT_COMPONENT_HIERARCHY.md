# Chat Component Hierarchy

This document describes the structure and organization of chat-related components in the frontend application.

## Architecture Overview

```
Chat (Page)
├── Header
│   ├── Session Info
│   └── New Session Button
├── SessionConfigPanel (Persistent session-level configuration)
│   ├── PresetSelector
│   └── OverrideSettings (embedded)
├── ChatContainer (Message display area)
│   └── MessageCard (per message)
│       ├── ThinkingBlock (optional)
│       └── ToolCallCard (optional)
└── Chat Input Area
    ├── TemporaryConfigPanel (Per-message overrides)
    └── ChatInput
```

## Component Descriptions

### 1. **Chat.tsx** (Page Component)
- **Location**: `web/src/pages/Chat.tsx`
- **Purpose**: Main page component that orchestrates the entire chat interface
- **Responsibilities**:
  - Session management (initialize, reset)
  - State management (messages, configs, loading states)
  - Configuration merging (session + temporary overrides)
  - Message sending coordination
  - Layout structure (header, config panels, container, input)

### 2. **ChatContainer.tsx**
- **Location**: `web/src/components/chat/ChatContainer.tsx`
- **Purpose**: Scrollable message display container
- **Features**:
  - Auto-scroll to bottom on new messages
  - Message selection handling
  - Loading indicator with animated dots
  - Empty state display
- **Props**:
  - `messages`: Array of MessageCardProps
  - `selectedMessageId`: Currently selected message
  - `onSelectMessage`: Callback for message selection
  - `autoScroll`: Enable/disable auto-scrolling
  - `isLoading`: Show loading indicator

### 3. **ChatInput.tsx**
- **Location**: `web/src/components/chat/ChatInput.tsx`
- **Purpose**: User message input field with send button
- **Features**:
  - Auto-resizing textarea (grows with content, max 200px)
  - Keyboard shortcuts (Ctrl+Enter to send)
  - Character counter
  - Focus management (auto-focus on mount)
  - Disabled state handling
- **Props**:
  - `onSend`: Callback when message is sent
  - `disabled`: Disable input during loading
  - `placeholder`: Placeholder text

### 4. **MessageCard.tsx**
- **Location**: `web/src/components/chat/MessageCard.tsx`
- **Purpose**: Individual message display with rich formatting
- **Features**:
  - Role-based styling (user, assistant, system, tool)
  - Markdown rendering for assistant messages (ReactMarkdown + remark-gfm)
  - Timestamp display
  - Streaming indicator
  - Selection state
  - Error message highlighting
  - Tool call/result display delegation
- **Props**:
  - `id`, `role`, `content`: Basic message data
  - `thinking`: Optional thinking block content
  - `toolCall`: Optional tool call data
  - `toolResult`: Optional tool result data
  - `timestamp`: Message timestamp
  - `isStreaming`: Show streaming indicator
  - `isSelected`: Highlight as selected
  - `onSelect`: Selection callback

### 5. **ThinkingBlock.tsx**
- **Location**: `web/src/components/chat/ThinkingBlock.tsx`
- **Purpose**: Display assistant's internal reasoning/thinking
- **Styling**: Purple-themed card with Brain icon
- **Props**:
  - `content`: Thinking text to display

### 6. **ToolCallCard.tsx**
- **Location**: `web/src/components/chat/ToolCallCard.tsx`
- **Purpose**: Display tool execution (input/output)
- **Features**:
  - Collapsible/expandable (default: expanded)
  - Tool icon (Wrench) with name in header
  - JSON-formatted input display
  - Success/error result display with icons
  - Collapsed state shows success/error indicator
- **Props**:
  - `id`: Tool call ID
  - `name`: Tool name
  - `input`: Tool input parameters (JSON)
  - `result`: Optional tool result data
  - `isExpanded`: Control expansion state
  - `onToggle`: Expansion toggle callback

### 7. **SessionConfigPanel.tsx**
- **Location**: `web/src/components/chat/SessionConfigPanel.tsx`
- **Purpose**: Comprehensive persistent session-level configuration
- **Features**:
  - Collapsible panel (Settings2 icon trigger)
  - Two-column layout (Basic Settings | Advanced Overrides)
  - Unsaved changes indicator
  - Save/Reset buttons
  - Real-time validation
  - Backend synchronization (PATCH `/api/sessions/:id/config`)
- **Basic Settings**:
  - Preset selector (dropdown)
  - Tools enabled (toggle switch)
  - Creativity slider (0.0-1.0)
  - Verbosity selector (short/normal/long)
  - Max tool rounds (1-100)
- **Advanced Overrides**:
  - Model override (text input)
  - Top P slider (0.0-1.0)
  - Frequency penalty slider (-2.0 to 2.0)
  - Presence penalty slider (-2.0 to 2.0)
  - Individual "Clear" buttons per override
  - "Clear All Overrides" button
- **Props**:
  - `sessionId`: Current session ID
  - `config`: SessionConfig object
  - `onConfigChanged`: Callback when config is saved
  - `disabled`: Disable all controls

### 8. **TemporaryConfigPanel.tsx**
- **Location**: `web/src/components/chat/TemporaryConfigPanel.tsx`
- **Purpose**: Simple per-message override (one-time use)
- **Features**:
  - Collapsible panel with Zap icon (indicates temporary)
  - Badge showing number of active overrides
  - Clear all button (X icon)
  - Auto-cleared after sending message
  - Currently supports model override only
- **Props**:
  - `config`: TemporaryConfig object
  - `onChange`: Callback for config changes
  - `disabled`: Disable controls

### 9. **PresetSelector.tsx**
- **Location**: `web/src/components/chat/PresetSelector.tsx`
- **Purpose**: Quick chat preset selection
- **Features**:
  - shadcn/ui Select component
  - Icon + label for each preset
  - 4 presets: General, Coding, Research, Quick
- **Presets**:
  - `general`: General Assistant (MessageCircle icon)
  - `coding`: Software Engineer (Code icon)
  - `research`: Research Assistant (FlaskConical icon)
  - `quick`: Quick & Efficient (Zap icon)
- **Props**:
  - `value`: Current preset value
  - `onChange`: Callback when preset changes
  - `disabled`: Disable selector

### 10. **OverrideSettings.tsx**
- **Location**: `web/src/components/chat/OverrideSettings.tsx`
- **Purpose**: Reusable advanced override controls
- **Features**:
  - Model override (text input)
  - Top P slider with value display
  - Frequency penalty slider with value display
  - Presence penalty slider with value display
  - Individual clear buttons per setting
  - "Clear All Overrides" button
- **Props**:
  - `overrides`: ChatOverrides object
  - `onChange`: Callback for override changes
  - `disabled`: Disable controls
- **Used By**: SessionConfigPanel (embedded in advanced settings section)

### 11. **ChatConfigPanel.tsx** (Deprecated)
- **Location**: `web/src/components/chat/ChatConfigPanel.tsx`
- **Purpose**: Legacy combined preset + override component
- **Status**: Replaced by SessionConfigPanel + TemporaryConfigPanel
- **Note**: May be removed in future cleanup

### 12. **Message.tsx** (Legacy)
- **Location**: `web/src/components/chat/Message.tsx`
- **Purpose**: Old message display component
- **Status**: Superseded by MessageCard.tsx
- **Note**: Kept for reference, may be removed

## Configuration Flow

### Session-Level (Persistent)
1. User opens SessionConfigPanel
2. Modifies settings (preset, creativity, overrides, etc.)
3. Clicks "Save to Session"
4. Config sent to backend via `PATCH /api/sessions/:id/config`
5. Backend resolves config and stores in session metadata
6. Frontend updates session config state
7. All subsequent messages use this config

### Temporary (Per-Message)
1. User opens TemporaryConfigPanel
2. Sets one-time override (e.g., model)
3. Sends message
4. Frontend merges `sessionConfig.overrides` + `tempConfig.overrides`
5. Merged config sent as `temporary_config` in chat request
6. Backend resolves temporary config (overrides session config)
7. Frontend clears `tempConfig` after sending

### Priority Order
```
temporary_config.overrides > session_config.overrides > preset defaults
```

## Styling

### Theme
- **Primary Color**: Yellow (#E8C236) - BlackBear TechHive theme
- **Background**: Black (#000000)
- **Component Library**: shadcn/ui (built on Radix UI)
- **CSS Framework**: Tailwind CSS v4

### Role Colors (CSS Custom Properties)
```css
--role-user: blue tones (user messages)
--role-assistant: gray tones (assistant messages)
--role-system: yellow tones (system messages)
--role-tool: cyan tones (tool calls/results)
--role-error: red tones (error messages)
```

### Component Patterns
- **Collapsible sections**: Use shadcn/ui Collapsible
- **Form controls**: Use shadcn/ui Input, Select, Slider, Switch
- **Buttons**: Use shadcn/ui Button with variants (default, outline, ghost)
- **Icons**: Lucide React icons

## Data Flow

### Message Rendering
```
Chat.tsx (page)
  ↓ messages array
ChatContainer.tsx
  ↓ map over messages
MessageCard.tsx
  ↓ conditional rendering
ThinkingBlock.tsx | ToolCallCard.tsx
```

### Config Management
```
Chat.tsx (state)
  ↓ props
SessionConfigPanel.tsx ← backend sync → /api/sessions/:id/config
  ↓ embedded
PresetSelector.tsx | OverrideSettings.tsx

Chat.tsx (state)
  ↓ props
TemporaryConfigPanel.tsx ← cleared after send
```

### Message Sending
```
ChatInput.tsx
  ↓ onSend callback
Chat.tsx (handleSendMessage)
  ↓ merge configs
  ↓ useChat hook
  ↓ sendMessage(content, config)
POST /api/sessions/:id/chat
  ↓ temporary_config
Backend (config resolution)
  ↓ stream_id
SSE /api/sessions/:id/stream/:stream_id
  ↓ AgentEvents
Frontend (useSSEStream)
  ↓ update messages
ChatContainer.tsx (re-render)
```

## Type Definitions

### Core Types
```typescript
// Message data
interface MessageCardProps {
  id: string;
  role: "user" | "assistant" | "system" | "tool";
  content: string;
  thinking?: string;
  toolCall?: ToolCallData;
  toolResult?: ToolResultData;
  timestamp?: Date;
  isStreaming?: boolean;
  isSelected?: boolean;
  onSelect?: (id: string) => void;
}

// Session config (persistent)
interface SessionConfig {
  preset: string;
  toolsEnabled: boolean;
  creativity: number;
  verbosity: string;
  maxRounds: number;
  overrides: ChatOverrides;
}

// Temporary config (per-message)
interface TemporaryConfig {
  overrides: ChatOverrides;
}

// Advanced overrides
interface ChatOverrides {
  model?: string;
  top_p?: number;
  frequency_penalty?: number;
  presence_penalty?: number;
}

// Tool call data
interface ToolCallData {
  id: string;
  name: string;
  arguments: Record<string, unknown>;
}

// Tool result data
interface ToolResultData {
  tool_call_id: string;
  tool_name: string;
  result: string;
  is_error: boolean;
}
```

## File Structure Summary

```
web/src/
├── pages/
│   └── Chat.tsx                      # Main page component
├── components/
│   ├── chat/
│   │   ├── ChatContainer.tsx         # Message display area
│   │   ├── ChatInput.tsx             # User input field
│   │   ├── MessageCard.tsx           # Individual message
│   │   ├── ThinkingBlock.tsx         # Thinking display
│   │   ├── ToolCallCard.tsx          # Tool execution display
│   │   ├── SessionConfigPanel.tsx    # Persistent session config
│   │   ├── TemporaryConfigPanel.tsx  # Per-message overrides
│   │   ├── PresetSelector.tsx        # Preset dropdown
│   │   ├── OverrideSettings.tsx      # Advanced override controls
│   │   ├── ChatConfigPanel.tsx       # [DEPRECATED] Combined config
│   │   └── Message.tsx               # [LEGACY] Old message component
│   └── ui/                           # shadcn/ui components
│       ├── button.tsx
│       ├── input.tsx
│       ├── select.tsx
│       ├── slider.tsx
│       ├── switch.tsx
│       ├── label.tsx
│       └── collapsible.tsx
└── hooks/
    ├── useChat.ts                    # Chat state management
    └── useSSEStream.ts               # SSE streaming
```

## Best Practices

### Component Development
1. **Use existing components first**: Check shadcn/ui library before creating custom components
2. **Keep components focused**: Each component should have a single, clear responsibility
3. **Prop drilling**: Pass only necessary props, use composition over deep nesting
4. **Type safety**: Always define TypeScript interfaces for props

### State Management
1. **Lift state up**: Shared state lives in parent (Chat.tsx)
2. **Local state**: Component-specific UI state stays local (e.g., `isOpen` in collapsibles)
3. **Sync with backend**: Session config syncs via API, temporary config is client-only

### Styling
1. **Use Tailwind utilities**: Prefer utility classes over custom CSS
2. **Follow theme**: Use CSS custom properties for colors (`hsl(var(--role-user))`)
3. **Responsive design**: Use Tailwind responsive modifiers (`md:`, `lg:`)
4. **Accessibility**: Ensure proper ARIA labels and keyboard navigation

### Error Handling
1. **Display errors inline**: Show error messages in MessageCard with red styling
2. **Validation**: Validate config inputs (ranges, required fields)
3. **User feedback**: Show loading states, save indicators, error toasts

## Future Enhancements

### Planned Features
- [ ] Session history sidebar (list of past sessions)
- [ ] Message branching UI (tree navigation)
- [ ] Export conversation (JSON, Markdown)
- [ ] Voice input/output
- [ ] Collaborative sessions (multi-user)
- [ ] Custom preset creation
- [ ] Advanced tool configuration UI

### Technical Debt
- [ ] Remove deprecated ChatConfigPanel.tsx
- [ ] Remove legacy Message.tsx
- [ ] Add comprehensive unit tests for all components
- [ ] Improve error boundary implementation
- [ ] Add loading skeletons for better UX
- [ ] Optimize re-renders with React.memo
- [ ] Add component documentation (Storybook)

---

**Document Version**: 1.0  
**Last Updated**: 2026-01-13  
**Author**: aaagent-rs project  
**Related Docs**: 
- [Frontend Guide](./front-guide.md)
- [Architecture Overview](../CLAUDE.md)
- [API Documentation](./api-guide.md)
