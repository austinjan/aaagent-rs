import { useState } from "react";
import { MessageCard } from "@/components/chat/MessageCard";
import { BranchIndicator } from "@/components/branch/BranchIndicator";
import { CheckpointBadge } from "@/components/branch/CheckpointBadge";
import type { BranchInfo } from "@/components/branch/BranchIndicator";
import type { CheckpointData } from "@/types";

// Fake data for demonstration
const FAKE_BRANCHES: BranchInfo[] = [
  {
    leafId: "branch-1-leaf",
    depth: 5,
    preview: "I can help you implement that feature using React hooks...",
    lastRole: "assistant",
    lastUpdated: new Date(Date.now() - 1000 * 60 * 5), // 5 min ago
    isActive: true,
    branchPointId: "msg-2",
  },
  {
    leafId: "branch-2-leaf",
    depth: 3,
    preview: "Let me try a different approach using Redux...",
    lastRole: "assistant",
    lastUpdated: new Date(Date.now() - 1000 * 60 * 30), // 30 min ago
    isActive: false,
    branchPointId: "msg-2",
  },
  {
    leafId: "branch-3-leaf",
    depth: 8,
    preview: "How about we use Zustand for state management?",
    lastRole: "user",
    lastUpdated: new Date(Date.now() - 1000 * 60 * 60), // 1 hour ago
    isActive: false,
    branchPointId: "msg-2",
  },
];

const FAKE_CHECKPOINT: CheckpointData = {
  summary:
    "Discussion about implementing a real-time chat feature with WebSocket support. User requested help with message persistence and offline sync. Assistant suggested using IndexedDB for local storage and optimistic updates for better UX.",
  created_at: Math.floor(Date.now() / 1000),
  strategy: "manual",
  stats: {
    nodes_covered: 15,
    total_tokens: 12500,
    summary_tokens: 800,
    compression_ratio: 0.936,
    covered_time_range: [
      Math.floor((Date.now() - 1000 * 60 * 60 * 2) / 1000),
      Math.floor(Date.now() / 1000),
    ],
  },
};

interface DemoMessage {
  id: string;
  role: "user" | "assistant" | "system" | "tool";
  content: string;
  thinking?: string;
  timestamp: Date;
}

export function BranchDemo() {
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [currentBranch, setCurrentBranch] = useState(1);
  const [checkpointExpanded, setCheckpointExpanded] = useState(false);

  const handleBranchNavigate = (direction: "prev" | "next") => {
    setCurrentBranch((prev) => {
      if (direction === "prev" && prev > 1) return prev - 1;
      if (direction === "next" && prev < FAKE_BRANCHES.length) return prev + 1;
      return prev;
    });
  };

  const handleBranchSelect = (leafId: string) => {
    const index = FAKE_BRANCHES.findIndex((b) => b.leafId === leafId);
    if (index !== -1) {
      setCurrentBranch(index + 1);
    }
  };

  const handleCreateBranch = (messageId: string) => {
    console.log("Create branch from message:", messageId);
    alert(`Branch created from message: ${messageId}`);
  };

  const messages: DemoMessage[] = [
    {
      id: "msg-1",
      role: "user",
      content:
        "Help me implement a real-time chat feature with offline support.",
      timestamp: new Date(Date.now() - 1000 * 60 * 60 * 2),
    },
    {
      id: "msg-2",
      role: "assistant",
      content:
        "I'd be happy to help you implement a real-time chat feature! Let me outline the key components we'll need:\n\n1. **WebSocket Connection** - For real-time messaging\n2. **Message Queue** - For handling offline messages\n3. **Local Storage** - Using IndexedDB for persistence\n4. **Sync Engine** - For reconciling local and server state\n\nWould you like me to start with any specific part?",
      timestamp: new Date(Date.now() - 1000 * 60 * 60 * 1.5),
    },
    {
      id: "msg-3",
      role: "user",
      content: "Start with the WebSocket connection setup.",
      timestamp: new Date(Date.now() - 1000 * 60 * 60),
    },
    {
      id: "msg-4",
      role: "assistant",
      content: `Here's a basic WebSocket setup with reconnection logic:

\`\`\`typescript
class ChatWebSocket {
  private ws: WebSocket | null = null;
  private reconnectAttempts = 0;
  private maxReconnectAttempts = 5;

  connect(url: string) {
    this.ws = new WebSocket(url);

    this.ws.onopen = () => {
      console.log('Connected');
      this.reconnectAttempts = 0;
    };

    this.ws.onclose = () => {
      this.handleReconnect(url);
    };
  }

  private handleReconnect(url: string) {
    if (this.reconnectAttempts < this.maxReconnectAttempts) {
      setTimeout(() => {
        this.reconnectAttempts++;
        this.connect(url);
      }, 1000 * Math.pow(2, this.reconnectAttempts));
    }
  }
}
\`\`\`

This includes exponential backoff for reconnection. Want me to add message handling next?`,
      thinking:
        "The user wants to start with WebSocket. I should provide a clean, production-ready implementation with reconnection logic since they mentioned offline support earlier.",
      timestamp: new Date(Date.now() - 1000 * 60 * 30),
    },
    {
      id: "msg-5",
      role: "user",
      content: "Yes, add message handling and the offline queue.",
      timestamp: new Date(Date.now() - 1000 * 60 * 15),
    },
    {
      id: "msg-6",
      role: "assistant",
      content: "Currently implementing message queue and offline support...",
      timestamp: new Date(Date.now() - 1000 * 60 * 5),
      thinking:
        "I need to design a robust message queue that handles both online and offline scenarios...",
    },
  ];

  return (
    <div className="min-h-screen bg-background">
      {/* Header */}
      <header className="sticky top-0 z-10 border-b border-border bg-background/95 backdrop-blur supports-[backdrop-filter]:bg-background/60">
        <div className="max-w-4xl mx-auto px-4 py-4">
          <h1 className="text-2xl font-bold tracking-tight text-foreground">
            Branch Navigation Demo
          </h1>
          <p className="text-sm text-muted-foreground mt-1">
            Testing BranchIndicator, CheckpointBadge, and MessageCard with
            branch button
          </p>
        </div>
      </header>

      {/* Main content */}
      <main className="max-w-4xl mx-auto px-4 py-6 space-y-8">
        {/* Standalone Components Section */}
        <section className="space-y-4">
          <h2 className="text-xl font-semibold text-foreground">
            Standalone Components
          </h2>

          <div className="grid gap-4 md:grid-cols-2">
            {/* BranchIndicator Demo */}
            <div className="p-4 rounded-lg border border-border bg-card">
              <h3 className="text-sm font-medium text-foreground mb-3">
                BranchIndicator
              </h3>
              <div className="flex items-center gap-4">
                <BranchIndicator
                  currentIndex={currentBranch}
                  totalBranches={3}
                  branches={FAKE_BRANCHES}
                  onPrev={() => handleBranchNavigate("prev")}
                  onNext={() => handleBranchNavigate("next")}
                  onSelectBranch={handleBranchSelect}
                />
                <span className="text-xs text-muted-foreground">
                  Click arrows or dropdown
                </span>
              </div>
            </div>

            {/* CheckpointBadge Demo */}
            <div className="p-4 rounded-lg border border-border bg-card">
              <h3 className="text-sm font-medium text-foreground mb-3">
                CheckpointBadge
              </h3>
              <CheckpointBadge
                summary={FAKE_CHECKPOINT.summary}
                stats={
                  FAKE_CHECKPOINT.stats
                    ? {
                        nodesCovered: FAKE_CHECKPOINT.stats.nodes_covered,
                        originalTokens: FAKE_CHECKPOINT.stats.total_tokens,
                        compressedTokens: FAKE_CHECKPOINT.stats.summary_tokens,
                        compressionRatio:
                          FAKE_CHECKPOINT.stats.compression_ratio,
                      }
                    : undefined
                }
                isExpanded={checkpointExpanded}
                onToggle={setCheckpointExpanded}
              />
            </div>
          </div>
        </section>

        {/* Message Cards Section */}
        <section className="space-y-4">
          <h2 className="text-xl font-semibold text-foreground">
            Message Cards with Branch Button
          </h2>
          <p className="text-sm text-muted-foreground">
            Hover over user/assistant messages to see the branch button. Click
            it to create a new branch. Press Ctrl+B when a message is selected.
          </p>

          <div className="space-y-4">
            {messages.map((msg) => (
              <MessageCard
                key={msg.id}
                id={msg.id}
                role={msg.role}
                content={msg.content}
                thinking={msg.thinking}
                timestamp={msg.timestamp}
                isSelected={selectedId === msg.id}
                onSelect={setSelectedId}
                onBranchAfter={handleCreateBranch}
                onBranchAlternative={handleCreateBranch}
              />
            ))}
          </div>
        </section>

        {/* State Debug Section */}
        <section className="space-y-4">
          <h2 className="text-xl font-semibold text-foreground">Debug State</h2>
          <div className="p-4 rounded-lg border border-border bg-muted/50 font-mono text-xs">
            <pre>
              {JSON.stringify(
                {
                  selectedId,
                  currentBranch,
                  currentBranchLeaf: FAKE_BRANCHES[currentBranch - 1]?.leafId,
                  checkpointExpanded,
                },
                null,
                2,
              )}
            </pre>
          </div>
        </section>
      </main>
    </div>
  );
}

export default BranchDemo;
