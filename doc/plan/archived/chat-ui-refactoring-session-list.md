# Chat UI Refactoring: Session List & Tree Separation

- Feature name: `chat-ui-refactoring-session-list`
- Status: **✅ Completed & Archived**
- Created: 2026-01-30
- Completed: 2026-01-30
- Parent plan: [chat-ui-plan.md](../chat-ui-plan.md)

## Completion Summary

**Achieved on**: 2026-01-30

**Implementation completed with enhancements beyond original plan:**
- ✅ Session list sidebar with message count, timestamps, and archive functionality
- ✅ Session management: Archive (instead of delete), rename (manual + AI-powered), auto-naming
- ✅ Tree view as inline toggle (Chat/Tree) with node details panel
- ✅ Session toolbar with conditional display based on view
- ✅ Loading/error/empty states for better UX
- ✅ Copy session ID functionality
- ✅ Backward compatible session storage with archived field

**Key improvements over plan:**
- Archive instead of delete for better data preservation
- AI-powered session naming using conversation context
- Inline tree toggle instead of modal for smoother UX
- Node details panel for rich tree exploration
- Relative timestamps (2h ago, 3d ago) for better time context

## 1) Overview

### Goal
Refactor the Chat UI to separate concerns: move tree visualization to a standalone view for history analysis, replace the left sidebar with a session list for session management, and add session ID copy functionality.

### Scope (In)
- **Session List Sidebar**: Replace tree navigation with session list in left sidebar
  - Display all sessions with metadata (name, date, message count)
  - Support session selection (load session)
  - Support session deletion
  - Add "New Session" button
- **Session ID Copy Button**: Add copy-to-clipboard button in chat header
- **Tree View Separation**: Move tree visualization to standalone view
  - Tree becomes analysis/visualization tool only
  - Tree no longer controls message selection or scrolling
  - Accessible via modal/separate page

### Non-goals (Out)
- Session search/filtering (defer to future enhancement)
- Session tagging/organization (defer to future enhancement)
- Multi-session comparison (defer to future enhancement)
- Tree editing/manipulation (keep read-only)

## 2) Requirements

### Functional Requirements

**Session List Sidebar:**
- [ ] Display list of all sessions sorted by most recent
- [ ] Show session metadata: name, created date, message count
- [ ] Each session has action menu/tools:
  - [ ] "Display Chat" - Load session into chat view
  - [ ] "Display Tree" - Open tree modal for this session
  - [ ] "Rename" - Edit session name inline or in modal
  - [ ] "Delete" - Delete session (with confirmation)
- [ ] "New Session" button at top
- [ ] Active session highlighted
- [ ] Empty state message when no sessions

**Session ID Copy:**
- [x] Copy button in chat header next to session name
- [x] Click → copy session ID to clipboard
- [x] Show toast/feedback message after copy
- [x] Icon: clipboard or copy icon

**Tree Visualization:**
- [x] Remove from left sidebar
- [x] Move to separate modal/drawer
- [x] Access via button in chat header (e.g., "View Tree")
- [x] Tree remains read-only visualization
- [x] No selection/scrolling coupling with messages
- [x] Can still show active path highlighting

### Non-functional Requirements
- **Performance**: Session list should load <500ms for 100 sessions
- **UX**: Session deletion requires confirmation (prevent accidents)
- **Accessibility**: Keyboard navigation for session list
- **Responsive**: Session list collapses on mobile

## 3) Design

### Current Layout (Before)
```
┌─────────────────────────────────────────────────┐
│ Header                                          │
├──────────────┬──────────────────────────────────┤
│              │                                  │
│  Tree Nav    │  Chat Messages                   │
│  (select +   │                                  │
│   scroll)    │                                  │
│              │                                  │
│  Summary     │  Chat Input                      │
└──────────────┴──────────────────────────────────┘
```

### New Layout (After)
```
┌─────────────────────────────────────────────────┐
│ Header [Session Name] [Copy ID] [View Tree]    │
├──────────────┬──────────────────────────────────┤
│              │                                  │
│ [New Session]│  Chat Messages                   │
│              │                                  │
│ Session List │                                  │
│ • Session 1  │                                  │
│   [💬📊✏️🗑] │  Chat Input                      │
│ • Session 2  │                                  │
│   [💬📊✏️🗑] │                                  │
│              │                                  │
└──────────────┴──────────────────────────────────┘
   💬 = Display Chat
   📊 = Display Tree
   ✏️ = Rename
   🗑 = Delete

         [📊 Display Tree] → Opens Modal/Drawer
         ┌──────────────────────────────┐
         │ Tree Visualization           │
         │   (Session: "My Chat")       │
         │                              │
         │   (Read-only, for analysis)  │
         │                              │
         └──────────────────────────────┘
```

### Component Structure

**Before:**
```
Chat.tsx
├── TreeNavigationPanel (left sidebar)
│   ├── TreeVisualization
│   └── SummaryPanel
└── ChatContainer (right main)
    ├── MessageCard[]
    └── ChatInput
```

**After:**
```
Chat.tsx
├── SessionListSidebar (left sidebar) [NEW]
│   ├── NewSessionButton
│   └── SessionListItem[]
│       ├── SessionName
│       ├── SessionMetadata (date, message count)
│       └── SessionActions [NEW]
│           ├── DisplayChatButton
│           ├── DisplayTreeButton
│           ├── RenameButton
│           └── DeleteButton
├── ChatHeader [UPDATED]
│   ├── SessionName
│   ├── CopySessionIdButton [NEW]
│   └── ViewTreeButton [NEW]
└── ChatContainer (right main)
    ├── MessageCard[]
    └── ChatInput

TreeModal (separate) [NEW]
├── TreeVisualization (moved from sidebar)
└── CloseButton
```

## 4) Implementation Plan

### Phase 1: Backend API Support

**Endpoints needed:**
- `GET /api/sessions` - List all sessions ✅ (already exists)
- `DELETE /api/sessions/{id}` - Delete session ❌ (needs implementation)
- `PATCH /api/sessions/{id}` - Update session name ❌ (needs implementation)

**Tasks:**
- [ ] Implement `DELETE /api/sessions/{id}` endpoint
- [ ] Implement `PATCH /api/sessions/{id}` endpoint (for rename)
- [ ] Update session list API to include metadata (name, created_at, message_count)

### Phase 2: Session List Sidebar

**New Component: `SessionListSidebar.tsx`**
```typescript
interface SessionListSidebarProps {
  sessions: SessionSummary[];
  activeSessionId: string | null;
  onDisplayChat: (sessionId: string) => void;
  onDisplayTree: (sessionId: string) => void;
  onSessionRename: (sessionId: string, newName: string) => void;
  onSessionDelete: (sessionId: string) => void;
  onNewSession: () => void;
}

interface SessionSummary {
  session_id: string;
  name: string;
  created_at: number;
  updated_at: number;
  message_count: number;
}
```

**New Component: `SessionActions.tsx`**
```typescript
interface SessionActionsProps {
  sessionId: string;
  sessionName: string;
  isActive: boolean;
  onDisplayChat: () => void;
  onDisplayTree: () => void;
  onRename: (newName: string) => void;
  onDelete: () => void;
}

// Actions shown as icon buttons or dropdown menu
// Icons: MessageSquare, GitBranch, Edit, Trash
```

**Tasks:**
- [ ] Create `SessionListSidebar.tsx` component
- [ ] Create `SessionListItem.tsx` sub-component
- [ ] Create `SessionActions.tsx` component with 4 actions
- [ ] Add "New Session" button
- [ ] Add inline rename functionality (or rename dialog)
- [ ] Add delete confirmation dialog
- [ ] Fetch sessions on mount
- [ ] Handle "Display Chat" action
- [ ] Handle "Display Tree" action (open tree modal for specific session)
- [ ] Handle rename action
- [ ] Handle delete action
- [ ] Show loading/empty states

### Phase 3: Session ID Copy Button

**New Component: `CopySessionIdButton.tsx`**
```typescript
interface CopySessionIdButtonProps {
  sessionId: string;
}

function CopySessionIdButton({ sessionId }: CopySessionIdButtonProps) {
  const [copied, setCopied] = useState(false);
  
  const handleCopy = () => {
    navigator.clipboard.writeText(sessionId);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };
  
  return (
    <button onClick={handleCopy}>
      {copied ? <Check /> : <Copy />}
      {copied ? "Copied!" : "Copy ID"}
    </button>
  );
}
```

**Tasks:**
- [ ] Create `CopySessionIdButton.tsx`
- [ ] Add to chat header
- [ ] Use clipboard API
- [ ] Show toast/feedback on copy

### Phase 4: Tree View Separation

**New Component: `TreeModal.tsx`**
```typescript
interface TreeModalProps {
  isOpen: boolean;
  onClose: () => void;
  sessionId: string;  // Load tree data for specific session
  sessionName: string; // Display in modal header
}

// Modal will fetch tree data internally using sessionId
// No longer coupled to main chat view's tree state
```

**Tasks:**
- [ ] Create `TreeModal.tsx` (or `TreeDrawer.tsx`)
- [ ] Move `TreeVisualization` component into modal
- [ ] Add sessionId prop to load tree for any session
- [ ] Add session name to modal header
- [ ] Add "View Tree" button to chat header (for current session)
- [ ] Remove tree from left sidebar
- [ ] Remove selection/scrolling coupling
- [ ] Keep active path highlighting (read-only)
- [ ] Tree modal can display tree for non-active sessions

### Phase 5: Layout Updates

**Update `Chat.tsx`:**
```typescript
// Before
<TreeNavigationPanel onNodeSelect={handleSelectMessage} />

// After
<SessionListSidebar 
  sessions={sessions}
  activeSessionId={sessionId}
  onSessionSelect={loadHistory}
  onSessionDelete={handleDeleteSession}
  onNewSession={handleNewSession}
/>
```

**Tasks:**
- [ ] Replace `TreeNavigationPanel` with `SessionListSidebar` in `Chat.tsx`
- [ ] Update chat header to include copy button and tree button
- [ ] Remove `SummaryPanel` from sidebar (or move elsewhere)
- [ ] Update layout CSS for new sidebar content

### Phase 6: State Management

**Session List State:**
```typescript
// Add to useChat hook or create new useSessions hook
const [sessions, setSessions] = useState<SessionSummary[]>([]);

const loadSessions = async () => {
  const response = await fetch('/api/sessions');
  const data = await response.json();
  setSessions(data.sessions);
};

const deleteSession = async (sessionId: string) => {
  await fetch(`/api/sessions/${sessionId}`, { method: 'DELETE' });
  setSessions(sessions.filter(s => s.session_id !== sessionId));
};
```

**Tasks:**
- [ ] Add session list state management
- [ ] Implement `loadSessions` function
- [ ] Implement `deleteSession` function
- [ ] Update session list after new session created
- [ ] Handle errors gracefully

## 5) Files to Create/Modify

### New Files
- `web/src/components/sessions/SessionListSidebar.tsx`
- `web/src/components/sessions/SessionListItem.tsx`
- `web/src/components/sessions/SessionActions.tsx` ⭐ (4 action buttons)
- `web/src/components/sessions/CopySessionIdButton.tsx`
- `web/src/components/tree/TreeModal.tsx` (or `TreeDrawer.tsx`)
- `web/src/hooks/useSessions.ts` (optional)

### Modified Files
- `web/src/pages/Chat.tsx` - Replace sidebar, update header
- `web/src/hooks/useChat.ts` - Remove tree selection coupling
- `src/api/mod.rs` - Add DELETE endpoint
- `web/src/services/api.ts` - Add deleteSession function

## 6) API Changes

### New Endpoint: DELETE /api/sessions/{id}

```rust
// src/api/mod.rs
pub async fn delete_session(
    Path(session_id): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<Value>, ApiError> {
    // Delete session file
    state.store.delete_session(session_id.clone()).await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    
    Ok(Json(json!({
        "success": true,
        "session_id": session_id
    })))
}
```

### Update: GET /api/sessions

Ensure it returns:
```json
{
  "sessions": [
    {
      "session_id": "01KG...",
      "name": "My Chat",
      "created_at": 1769740012,
      "updated_at": 1769740190,
      "message_count": 10
    }
  ]
}
```

## 7) UX Considerations

### Session Deletion Confirmation
```typescript
const handleDeleteSession = async (sessionId: string) => {
  const confirmed = window.confirm(
    "Are you sure you want to delete this session? This cannot be undone."
  );
  
  if (confirmed) {
    await deleteSession(sessionId);
    // If deleted active session, create new one
    if (sessionId === activeSessionId) {
      await handleNewSession();
    }
  }
};
```

### Session List Empty State
```tsx
{sessions.length === 0 ? (
  <div className="p-4 text-center text-muted-foreground">
    <p>No sessions yet</p>
    <button onClick={onNewSession}>Create your first session</button>
  </div>
) : (
  <SessionList sessions={sessions} />
)}
```

### Copy Feedback
```tsx
{copied && (
  <Toast message="Session ID copied to clipboard!" />
)}
```

## 8) Testing Plan

### Manual Testing

**Session List:**
- [ ] Load page → sessions list appears
- [ ] Click session → loads into chat view
- [ ] Click "New Session" → creates new session
- [ ] Click delete → shows confirmation
- [ ] Confirm delete → session removed from list
- [ ] Delete active session → creates new session automatically

**Copy Button:**
- [ ] Click copy button → session ID copied to clipboard
- [ ] Paste in notepad → verify correct ID
- [ ] Visual feedback shows "Copied!"

**Tree Modal:**
- [ ] Click "View Tree" → modal opens
- [ ] Tree displays correctly
- [ ] Clicking nodes does NOT scroll messages
- [ ] Close modal → returns to chat

**Responsive:**
- [ ] Session list looks good on desktop
- [ ] Session list collapses/adapts on mobile
- [ ] Tree modal works on mobile

### Unit Tests
- [ ] SessionListSidebar renders correctly
- [ ] CopySessionIdButton copies to clipboard
- [ ] Session deletion calls API correctly
- [ ] Session selection loads session

## 9) Acceptance Criteria

**Session List:**
- [ ] Sessions displayed in left sidebar (replacing tree navigation)
- [ ] Sessions sorted by most recent first
- [ ] Active session highlighted
- [ ] Click session loads it into chat
- [ ] Delete button works with confirmation
- [ ] "New Session" button creates new session

**Copy Button:**
- [ ] Copy button visible in chat header
- [ ] Clicking copies session ID to clipboard
- [ ] Visual feedback shown after copy
- [ ] Works across browsers (Chrome, Firefox, Safari)

**Tree Separation:**
- [ ] Tree removed from left sidebar
- [ ] "View Tree" button in chat header
- [ ] Tree opens in modal/drawer
- [ ] Tree is read-only (no message selection/scrolling)
- [ ] Active path still highlighted
- [ ] Modal can be closed

**Overall:**
- [ ] No breaking changes to existing functionality
- [ ] Chat messages still display correctly
- [ ] Session persistence still works
- [ ] Performance acceptable (<500ms session list load)

## 10) Migration Notes

**For Users:**
- Tree navigation moved to "View Tree" button
- Sessions now managed in left sidebar
- Can now easily delete old sessions
- Can copy session ID for sharing/debugging

**For Developers:**
- TreeNavigationPanel component no longer used in main layout
- Session list requires DELETE endpoint
- Tree visualization is now standalone component
- Message selection decoupled from tree

## 11) Future Enhancements

**Session List Improvements:**
- [ ] Search/filter sessions by name or date
- [ ] Session tagging/categorization
- [ ] Bulk delete sessions
- [ ] Export session to JSON
- [ ] Session rename inline editing
- [ ] Folder/workspace organization

**Tree Improvements:**
- [ ] Diff view between branches
- [ ] Export tree as image
- [ ] Tree statistics (depth, branches, tokens)
- [ ] Timeline view (alternative to tree)

**Copy Features:**
- [ ] Copy entire conversation as markdown
- [ ] Copy selected messages
- [ ] Share session via link

## 12) Dependencies

- Clipboard API (browser support: all modern browsers)
- Modal/Dialog component (can use existing UI library)
- DELETE session backend endpoint (needs implementation)

## 13) Timeline Estimate

- **Phase 1** (Backend API): 2 hours
- **Phase 2** (Session List): 4 hours
- **Phase 3** (Copy Button): 1 hour
- **Phase 4** (Tree Separation): 3 hours
- **Phase 5** (Layout Updates): 2 hours
- **Phase 6** (State Management): 2 hours
- **Testing**: 2 hours

**Total**: ~16 hours (2 days)

---

## References
- Parent plan: [chat-ui-plan.md](./chat-ui-plan.md)
- Related: [chat-ui-session-management.md](./chat-ui-session-management.md)
- Tree component: `web/src/components/tree/TreeVisualization.tsx`
- Current layout: `web/src/pages/Chat.tsx`
