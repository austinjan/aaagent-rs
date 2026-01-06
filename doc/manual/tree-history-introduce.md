---
marp: true
theme: default
paginate: true
---

# The Problem with Traditional AI Memory

### Linear conversations do not scale

---

## Problem #1 — Cost Explodes

```text
User
 ↓
AI (10K tokens)
 ↓
AI (20K tokens)
 ↓
AI (40K tokens)
 ↓
AI (80K tokens)
```

**Every step costs more than the previous one**

- AI rereads everything every time
- Cost grows exponentially, not linearly
- This quickly becomes unaffordable

---

## Problem #2 — One Mistake, Restart Everything

```text
Start
  ↓
Decision A
  ↓
Decision B
  ↓
Decision C ❌
  ↓
❗ Restart from zero
```

**No retry from earlier decisions**
- No way to compare alternatives
- Every retry means paying again
- AI experimentation becomes expensive and slow

---

## Problem #3 — No Control, No Audit

```text
AI Conversation
──────────────────────────────▶

✔ expensive
✔ slow
✖ no retry
✖ no audit
✖ no control
```

**History only grows longer**
- No structure, no checkpoints
- Impossible to manage long-running AI
- This is fragile infrastructure

---

# Our Breakthrough

### AI conversations should work like **decision trees**, not chat logs

---

## Tree-Based AI History

```text
            ┌─ Option B
Start ── Decision ── Option A ── Result
            └─ Option C
```

**Every decision creates a branch**
- No history is overwritten
- Retry and compare without restarting
- Explore alternatives safely

---

## Immutable Tree: The Audit Foundation

```text
Node A ──▶ Node B ──▶ Node C
  ↓          ↓          ↓
[Append] [Append] [Append]

Never Delete. Never Modify. Only Append.
```

**Why immutability matters:**
- **Complete audit trail**: Every AI decision is permanently recorded
- **Reproducible debugging**: Replay exact conversation paths
- **Behavioral analysis**: Trace why AI made specific decisions
- **Compliance ready**: Tamper-proof history for regulations

---

## Checkpoints Control Cost

```text
Start ── Conversation ── [Checkpoint] ── New Work
```

**Safe memory snapshots**
- Old context compressed
- Same understanding, far fewer tokens
- Like saving progress in a game

---

## What the AI Actually Sees

```text
Complex Tree History
        ↓
Clean, Short Context
        ↓
     AI Model
```

**AI remains simple**
- No special model required
- Works with any provider
- Complexity stays in our system

---

## Example: Tool Result Compression

**3-Layer Strategy**

| Layer | Age | Strategy |
|-------|-----|----------|
| Layer 1 | Last 2 turns | Keep full content |
| Layer 2 | 2-10 turns | Truncate large results (>500 chars) |
| Layer 3 | >10 turns | Summarize everything |

**LLM can recall full content on-demand via `recall_tool_result` tool**

---

## Real Impact: Token Savings

**Before (Turn 4):** Would send 9 messages (all history)
**After (Turn 4):** Only sends 2 messages (checkpoint summary + recent)

**~78% reduction in messages sent**

- Same conversation quality
- Much lower API costs
- Scalable to hundreds of turns

---

## Business Impact

```text
Tree History
   ↓
Lower Cost
Faster Decisions
Safe Retries
Auditability
```

**Concrete benefits:**
- Predictable AI operating cost
- Long-running agents become feasible
- Enterprise-ready by design

---

## Why This Is Hard to Copy

- Most systems built around linear chat logs
- Retrofitting branching is complex and risky
- Our architecture designed for scale from day one

> **This is infrastructure, not a feature**

---

## Why Now? The Strategic Window

**We're past the "linear conversation" threshold**

Modern agents are not simple chatbots:
- Long-running tasks (hours/days)
- Hundreds of tool calls per session
- Single tool output = thousands of tokens
- Retry/what-if scenarios are required

**Linear history is now a bottleneck, not a choice**

---

## The Technical Debt Trap

```text
Now: Implement tree-based history
  ↓
✓ Schema defined correctly
✓ No customer data migration
✓ Low refactor cost

Later: Retrofit branching
  ↓
✗ Break existing APIs
✗ Migrate production data
✗ Rewrite provider integrations
```

**Do it now = investment. Do it later = technical debt.**

---

## Competitive Landscape

**Most agent frameworks still use linear history:**
- LangChain, AutoGPT, CrewAI: Linear message arrays
- Summary strategies destroy replayability
- Branching/retry is unreliable or missing

**Market gap = Strategic opportunity**

We're building the foundation others will need in 6-12 months

---

## ROI Timeline

**Short-term (1-2 months)**
- 40-80% token cost reduction
- Faster debugging with replay
- Stable long-running tasks

**Mid-term (3-6 months)**
- History visualization & branching UI
- Audit/compliance capabilities
- Agent behavior analytics

**Long-term (Strategic asset)**
- AI self-improvement via A/B reasoning
- Regulatory-ready audit trails
- Core product differentiator

---

## What This Enables

**Long-running autonomous agents**
- Multi-day or multi-week workflows
- Parallel strategy exploration
- Smart context management

**Enterprise capabilities**
- Audit trails for compliance
- Reproducible decision paths
- Cost control and prediction

---

## Technical Architecture

```rust
Session {
  tree: ConversationTree,
  checkpoints: HashMap<NodeId, Checkpoint>,
  archived_results: HashMap<ToolCallId, Content>,
  config: CompressionConfig
}
```

**Key innovations:**
- Append-only tree structure
- Metadata-based checkpoints (not tree nodes)
- Three-layer compression strategy
- On-demand content recall

---

# Closing

### We turned AI memory into reusable decision infrastructure

**Cheaper.** Token optimization reduces costs by 40-80%

**Safer.** Immutable history, branching support, audit trails

**Scalable.** Handles conversations of any length

---

## Thank You

### Questions?

**Tree-based history = The future of AI memory**

