import { useState } from "react";
import { MessageCard } from "../components/chat/MessageCard";

export default function MessageCardDemo() {
  const [selectedId, setSelectedId] = useState<string | null>(null);

  // Mock data for different message types
  const messages = [
    // Simple user message
    {
      id: "msg-1",
      role: "user" as const,
      content: "What is the capital of France?",
      timestamp: new Date("2024-01-12T10:30:00"),
    },

    // Simple assistant response
    {
      id: "msg-2",
      role: "assistant" as const,
      content:
        "The capital of France is Paris. It is located in the north-central part of the country and is known for its art, fashion, and culture.",
      timestamp: new Date("2024-01-12T10:30:05"),
    },

    // User message with multiple lines
    {
      id: "msg-3",
      role: "user" as const,
      content: `Can you help me understand how React hooks work?
Specifically:
1. useState
2. useEffect
3. useCallback`,
      timestamp: new Date("2024-01-12T10:31:00"),
    },

    // Assistant with thinking
    {
      id: "msg-4",
      role: "assistant" as const,
      thinking:
        "The user is asking about React hooks. I should provide a clear explanation of each hook with examples. Let me structure this response to cover all three hooks systematically.",
      content: `I'll explain each React hook:

**1. useState** - Manages component state
   \`const [count, setCount] = useState(0);\`

**2. useEffect** - Handles side effects
   \`useEffect(() => { /* effect */ }, [dependencies]);\`

**3. useCallback** - Memoizes functions
   \`const memoized = useCallback(() => { /* fn */ }, [deps]);\`

Would you like more detailed examples?`,
      timestamp: new Date("2024-01-12T10:31:10"),
    },

    // System message
    {
      id: "msg-5",
      role: "system" as const,
      content: "Session checkpoint created. 15 messages summarized.",
      timestamp: new Date("2024-01-12T10:32:00"),
    },

    // User asking for code search
    {
      id: "msg-6",
      role: "user" as const,
      content: "Find all files that use useEffect in the src directory",
      timestamp: new Date("2024-01-12T10:33:00"),
    },

    // Assistant with tool call (search)
    {
      id: "msg-7",
      role: "assistant" as const,
      thinking:
        "I need to search the codebase for files containing useEffect. Let me use the search_code tool.",
      toolCalls: [
        {
          id: "call-1",
          name: "search_code",
          input: {
            query: "useEffect",
            path: "src/",
            file_pattern: "*.tsx",
          },
        },
      ],
      toolResults: [
        {
          tool_call_id: "call-1",
          tool_name: "search_code",
          result: `Found 12 matches in 5 files:

src/components/App.tsx (3 matches)
  - Line 24: useEffect(() => {
  - Line 45: useEffect(() => {
  - Line 89: }, [data]); // useEffect dependency

src/hooks/useData.ts (4 matches)
  - Line 12: useEffect(() => {
  - Line 18: useEffect(() => {
  - Line 34: return () => {}; // useEffect cleanup
  - Line 56: }, []); // useEffect runs once

src/pages/Home.tsx (2 matches)
  - Line 67: useEffect(() => {
  - Line 78: }, [loading]);

src/utils/helpers.ts (2 matches)
  - Line 23: // Comment about useEffect
  - Line 145: // Another useEffect reference

src/components/Chat.tsx (1 match)
  - Line 156: useEffect(() => fetchMessages(), [sessionId]);`,
          is_error: false,
        },
      ],
      content: `I found **12 matches** in **5 files** that use \`useEffect\`:

The main files are:
- **App.tsx** (3 uses) - Component lifecycle management
- **useData.ts** (4 uses) - Custom hook with data fetching
- **Home.tsx** (2 uses) - Page-level effects
- **helpers.ts** (2 uses) - Comments only
- **Chat.tsx** (1 use) - Message fetching effect

Would you like me to show you the actual code from any of these files?`,
      timestamp: new Date("2024-01-12T10:33:05"),
    },

    // User requesting file content
    {
      id: "msg-8",
      role: "user" as const,
      content: "Yes, show me the code from useData.ts",
      timestamp: new Date("2024-01-12T10:34:00"),
    },

    // Assistant with tool call (read file)
    {
      id: "msg-9",
      role: "assistant" as const,
      toolCalls: [
        {
          id: "call-2",
          name: "read_file",
          input: {
            path: "src/hooks/useData.ts",
          },
        },
      ],
      toolResults: [
        {
          tool_call_id: "call-2",
          tool_name: "read_file",
          result: `import { useState, useEffect } from 'react';

export function useData(url: string) {
  const [data, setData] = useState(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState(null);

  useEffect(() => {
    setLoading(true);

    fetch(url)
      .then(res => res.json())
      .then(data => {
        setData(data);
        setLoading(false);
      })
      .catch(err => {
        setError(err);
        setLoading(false);
      });

    return () => {
      // Cleanup function
    };
  }, [url]);

  return { data, loading, error };
}`,
          is_error: false,
        },
      ],
      content: `Here's the code from \`useData.ts\`:

This is a custom hook that:
1. Fetches data from a URL
2. Manages loading and error states
3. Re-fetches when the URL changes
4. Includes cleanup function

The \`useEffect\` runs whenever the \`url\` dependency changes.`,
      timestamp: new Date("2024-01-12T10:34:03"),
    },

    // Assistant with error tool result
    {
      id: "msg-10",
      role: "assistant" as const,
      thinking:
        "Let me try to read a configuration file to help with the setup.",
      toolCalls: [
        {
          id: "call-3",
          name: "read_file",
          input: {
            path: "config.yaml",
          },
        },
      ],
      toolResults: [
        {
          tool_call_id: "call-3",
          tool_name: "read_file",
          result: `Error: File not found
Path: config.yaml
Reason: The specified file does not exist in the current directory.

Suggestion: Check if the file is located in a different directory, or create it if it doesn't exist.`,
          is_error: true,
        },
      ],
      content: `I couldn't find the \`config.yaml\` file. It appears the configuration file doesn't exist yet. Would you like me to help you create one?`,
      timestamp: new Date("2024-01-12T10:35:00"),
    },

    // Streaming message (simulated)
    {
      id: "msg-11",
      role: "assistant" as const,
      content:
        "Let me explain how streaming works in this application. The server sends events",
      isStreaming: true,
      timestamp: new Date("2024-01-12T10:36:00"),
    },
  ];

  return (
    <div className="min-h-screen bg-background p-8">
      <div className="max-w-4xl mx-auto">
        {/* Header */}
        <div className="mb-8">
          <h1 className="text-3xl font-bold mb-2">
            Message Card Component Demo
          </h1>
          <p className="text-muted-foreground">
            Interactive demo showing different message types and states
          </p>
        </div>

        {/* Controls */}
        <div className="mb-6 p-4 bg-card rounded-lg border border-border shadow-sm">
          <h2 className="text-sm font-semibold mb-2 text-foreground">
            Demo Controls
          </h2>
          <div className="flex gap-2 text-sm">
            <button
              onClick={() => setSelectedId(null)}
              className="px-3 py-1 bg-secondary hover:bg-secondary/80 text-secondary-foreground rounded border border-border"
            >
              Clear Selection
            </button>
            <span className="text-muted-foreground flex items-center">
              Selected: {selectedId || "none"}
            </span>
          </div>
        </div>

        {/* Message List */}
        <div className="space-y-4">
          {messages.map((msg) => (
            <MessageCard
              key={msg.id}
              {...msg}
              isSelected={selectedId === msg.id}
              onSelect={setSelectedId}
            />
          ))}
        </div>

        {/* Legend */}
        <div className="mt-8 p-4 bg-card rounded-lg border border-border shadow-sm">
          <h2 className="text-sm font-semibold mb-3 text-foreground">
            Card Features
          </h2>
          <ul className="space-y-2 text-sm text-muted-foreground">
            <li className="flex items-start gap-2">
              <span className="text-blue-500">●</span>
              <span>
                <strong>User messages</strong> - Blue theme
              </span>
            </li>
            <li className="flex items-start gap-2">
              <span className="text-gray-500">●</span>
              <span>
                <strong>Assistant messages</strong> - Gray theme
              </span>
            </li>
            <li className="flex items-start gap-2">
              <span className="text-yellow-500">●</span>
              <span>
                <strong>System messages</strong> - Yellow theme
              </span>
            </li>
            <li className="flex items-start gap-2">
              <span className="text-purple-500">💭</span>
              <span>
                <strong>Thinking blocks</strong> - Purple highlight showing
                reasoning
              </span>
            </li>
            <li className="flex items-start gap-2">
              <span className="text-cyan-500">🔧</span>
              <span>
                <strong>Tool calls</strong> - Cyan cards with expandable JSON
                input
              </span>
            </li>
            <li className="flex items-start gap-2">
              <span className="text-green-500">✅</span>
              <span>
                <strong>Tool results (success)</strong> - Green border
              </span>
            </li>
            <li className="flex items-start gap-2">
              <span className="text-red-500">❌</span>
              <span>
                <strong>Tool results (error)</strong> - Red border
              </span>
            </li>
            <li className="flex items-start gap-2">
              <span className="text-green-500">●</span>
              <span>
                <strong>Streaming indicator</strong> - Pulsing dot and cursor
              </span>
            </li>
            <li className="flex items-start gap-2">
              <span className="text-blue-500">◉</span>
              <span>
                <strong>Click to select</strong> - Shows blue ring highlight
              </span>
            </li>
          </ul>
        </div>
      </div>
    </div>
  );
}
