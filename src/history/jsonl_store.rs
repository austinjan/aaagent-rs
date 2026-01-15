use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::fs::{self, OpenOptions};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use super::node::{Node, NodeFlags, NodeId, SessionId};
use super::session::Session;
use super::storage::{NodeFilter, TreeStore};

/// JSONL-based tree store (simple, no cache)
///
/// **File Format:**
/// - `data/sessions/{session_id}.meta.json` - Session metadata
/// - `data/sessions/{session_id}.nodes.jsonl` - Append-only node log (one JSON per line)
///
/// **Design:**
/// - No in-memory cache - always read from disk
/// - Simple and reliable - cache coherence problems eliminated
/// - Suitable for small to medium workloads (< 10k nodes per session)
///
/// **Atomic Writes:**
/// - Nodes: Append to .jsonl using O_APPEND flag (atomic on most filesystems)
/// - Session metadata: Write to .tmp, fsync, rename (atomic)
#[derive(Clone)]
pub struct JSONLStore {
    base_path: PathBuf,
}

impl JSONLStore {
    /// Create a new JSONL store at the given path
    pub async fn new(base_path: PathBuf) -> Result<Self> {
        let sessions_dir = base_path.join("sessions");
        fs::create_dir_all(&sessions_dir).await.context(format!(
            "Failed to create sessions directory: {}",
            sessions_dir.display()
        ))?;

        crate::logger::log("[JSONLStore] Initialized (no cache)".to_string());
        Ok(Self { base_path })
    }

    /// Get session directory
    fn session_dir(&self, _session_id: &str) -> PathBuf {
        self.base_path.join("sessions")
    }

    /// Get session metadata file path
    fn session_meta_path(&self, session_id: &str) -> PathBuf {
        self.session_dir(session_id)
            .join(format!("{}.meta.json", session_id))
    }

    /// Get nodes JSONL file path
    fn nodes_jsonl_path(&self, session_id: &str) -> PathBuf {
        self.session_dir(session_id)
            .join(format!("{}.nodes.jsonl", session_id))
    }

    /// Load all nodes for a session from JSONL file
    async fn load_nodes(&self, session_id: &SessionId) -> Result<HashMap<NodeId, Node>> {
        let jsonl_path = self.nodes_jsonl_path(session_id);
        let mut nodes = HashMap::new();

        if !jsonl_path.exists() {
            return Ok(nodes);
        }

        let file = fs::File::open(&jsonl_path).await.context(format!(
            "Failed to open nodes file: {}",
            jsonl_path.display()
        ))?;

        let reader = BufReader::new(file);
        let mut lines = reader.lines();
        let mut line_number = 0;

        while let Some(line) = lines.next_line().await? {
            if line.trim().is_empty() {
                line_number += 1;
                continue;
            }

            match serde_json::from_str::<Node>(&line) {
                Ok(node) => {
                    // JSONL is append-only, so later entries override earlier ones
                    // (for update operations like update_node_flags)
                    nodes.insert(node.node_id.clone(), node);
                }
                Err(e) => {
                    crate::logger::log(format!(
                        "[JSONLStore] Warning: Failed to parse node at line {}: {}",
                        line_number, e
                    ));
                }
            }

            line_number += 1;
        }

        Ok(nodes)
    }

    /// Append a node to JSONL file (atomic)
    async fn append_node_to_jsonl(&self, node: &Node) -> Result<()> {
        let jsonl_path = self.nodes_jsonl_path(&node.session_id);

        let json = serde_json::to_string(node).context("Failed to serialize node")?;
        let line = format!("{}\n", json);

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&jsonl_path)
            .await
            .context(format!(
                "Failed to open nodes file: {}",
                jsonl_path.display()
            ))?;

        file.write_all(line.as_bytes())
            .await
            .context("Failed to write node")?;

        file.sync_all().await.context("Failed to fsync node file")?;

        Ok(())
    }

    /// Save session metadata atomically
    async fn save_session_metadata(&self, session: &Session) -> Result<()> {
        let meta_path = self.session_meta_path(&session.session_id);
        let tmp_path = meta_path.with_extension("tmp");

        let json = serde_json::to_string_pretty(session).context("Failed to serialize session")?;

        fs::write(&tmp_path, &json).await.context(format!(
            "Failed to write temp session file: {}",
            tmp_path.display()
        ))?;

        // Atomic rename
        fs::rename(&tmp_path, &meta_path).await.context(format!(
            "Failed to rename session file: {} -> {}",
            tmp_path.display(),
            meta_path.display()
        ))?;

        Ok(())
    }

    /// Load session metadata
    async fn load_session_metadata(&self, session_id: &SessionId) -> Result<Session> {
        let meta_path = self.session_meta_path(session_id);

        if !meta_path.exists() {
            return Err(anyhow!("Session {} not found", session_id));
        }

        let content = fs::read_to_string(&meta_path).await.context(format!(
            "Failed to read session metadata: {}",
            meta_path.display()
        ))?;

        let session: Session =
            serde_json::from_str(&content).context("Failed to deserialize session")?;

        Ok(session)
    }

    /// List all session metadata files
    async fn list_sessions_metadata(&self) -> Result<Vec<Session>> {
        let sessions_dir = self.session_dir("");
        if !sessions_dir.exists() {
            return Ok(Vec::new());
        }

        let mut sessions = Vec::new();
        let mut entries = fs::read_dir(&sessions_dir).await.context(format!(
            "Failed to read sessions directory: {}",
            sessions_dir.display()
        ))?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();

            // Only process .meta.json files
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }

            if let Some(file_name) = path.file_name().and_then(|s| s.to_str()) {
                if !file_name.ends_with(".meta.json") {
                    continue;
                }

                if let Ok(content) = fs::read_to_string(&path).await {
                    if let Ok(session) = serde_json::from_str::<Session>(&content) {
                        sessions.push(session);
                    }
                }
            }
        }

        sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(sessions)
    }

    /// Find a node by ID across all sessions (slow, for debugging)
    async fn find_node_in_any_session(&self, node_id: &NodeId) -> Result<Option<Node>> {
        let sessions = self.list_sessions_metadata().await?;

        for session in sessions {
            let nodes = self.load_nodes(&session.session_id).await?;
            if let Some(node) = nodes.get(node_id) {
                return Ok(Some(node.clone()));
            }
        }

        Ok(None)
    }
}

#[async_trait]
impl TreeStore for JSONLStore {
    async fn insert_node(&self, node: Node) -> Result<NodeId> {
        let node_id = node.node_id.clone();

        // Simply append to JSONL file
        self.append_node_to_jsonl(&node).await?;

        Ok(node_id)
    }

    async fn get_node(&self, node_id: NodeId) -> Result<Option<Node>> {
        // PERFORMANCE NOTE: This method searches across ALL sessions
        // In practice, Session methods use get_path_to_root_internal() which
        // already knows the session_id, so this slow path is rarely hit
        self.find_node_in_any_session(&node_id).await
    }

    async fn update_node_flags(&self, node_id: NodeId, flags: NodeFlags) -> Result<()> {
        // Load the node to get its session_id
        let node = self
            .get_node(node_id.clone())
            .await?
            .ok_or_else(|| anyhow!("Node {} not found", node_id))?;

        // Create updated node
        let mut updated_node = node.clone();
        updated_node.flags = flags;

        // Append updated version (JSONL append-only)
        // When loading, the last version wins
        self.append_node_to_jsonl(&updated_node).await?;

        Ok(())
    }

    async fn mark_node_pruned(&self, node_id: NodeId, pruned_at: i64) -> Result<()> {
        let node = self
            .get_node(node_id)
            .await?
            .ok_or_else(|| anyhow!("Node not found"))?;

        let mut updated_node = node.clone();
        updated_node.pruned_at = Some(pruned_at);

        self.append_node_to_jsonl(&updated_node).await?;

        Ok(())
    }

    async fn delete_node(&self, node_id: NodeId) -> Result<()> {
        // JSONL doesn't support deletion - just mark as pruned
        self.mark_node_pruned(node_id, super::node::now()).await
    }

    async fn get_children(&self, node_id: NodeId) -> Result<Vec<Node>> {
        // Find which session this node belongs to
        let node = self
            .get_node(node_id.clone())
            .await?
            .ok_or_else(|| anyhow!("Node {} not found", node_id))?;

        let nodes = self.load_nodes(&node.session_id).await?;

        let mut children: Vec<Node> = nodes
            .values()
            .filter(|n| n.parent_id.as_ref() == Some(&node_id))
            .cloned()
            .collect();

        children.sort_by_key(|n| n.seq);
        Ok(children)
    }

    async fn get_path_to_root_internal(&self, node_id: NodeId) -> Result<Vec<Node>> {
        // Find session that contains this node
        let node = self
            .get_node(node_id.clone())
            .await?
            .ok_or_else(|| anyhow!("Node {} not found", node_id))?;

        let nodes = self.load_nodes(&node.session_id).await?;

        // Walk path to root
        let mut path = Vec::new();
        let mut current_id = Some(node_id);

        while let Some(id) = current_id {
            let current_node = nodes
                .get(&id)
                .ok_or_else(|| anyhow!("Node {} not found in session", id))?
                .clone();

            current_id = current_node.parent_id.clone();
            path.push(current_node);
        }

        Ok(path)
    }

    async fn find_nodes(&self, session_id: SessionId, filter: NodeFilter) -> Result<Vec<Node>> {
        let nodes = self.load_nodes(&session_id).await?;

        let filtered: Vec<Node> = nodes
            .values()
            .filter(|n| {
                // Apply filters
                if let Some(ref kinds) = filter.kinds {
                    if !kinds.contains(&n.kind) {
                        return false;
                    }
                }

                if let Some(ref roles) = filter.roles {
                    if let Some(ref node_role) = n.role {
                        if !roles.contains(node_role) {
                            return false;
                        }
                    } else {
                        return false;
                    }
                }

                if let Some(after) = filter.created_after {
                    if n.created_at <= after {
                        return false;
                    }
                }

                if let Some(before) = filter.created_before {
                    if n.created_at >= before {
                        return false;
                    }
                }

                if let Some(ref flag_filter) = filter.flags {
                    if flag_filter.important && !n.flags.important {
                        return false;
                    }
                    if flag_filter.ephemeral && !n.flags.ephemeral {
                        return false;
                    }
                    if flag_filter.hidden && !n.flags.hidden {
                        return false;
                    }
                }

                if let Some(ref search) = filter.content_search {
                    if !n.content.contains(search) {
                        return false;
                    }
                }

                true
            })
            .cloned()
            .collect();

        Ok(filtered)
    }

    async fn create_session(&self, session: Session) -> Result<SessionId> {
        let session_id = session.session_id.clone();

        // Save metadata
        self.save_session_metadata(&session).await?;

        // Create JSONL file if it doesn't exist
        let jsonl_path = self.nodes_jsonl_path(&session_id);
        if !jsonl_path.exists() {
            fs::write(&jsonl_path, "").await.context(format!(
                "Failed to create nodes file: {}",
                jsonl_path.display()
            ))?;
        }

        crate::logger::log(format!("[JSONLStore] Created session {}", session_id));
        Ok(session_id)
    }

    async fn get_session(&self, session_id: SessionId) -> Result<Option<Session>> {
        match self.load_session_metadata(&session_id).await {
            Ok(session) => Ok(Some(session)),
            Err(_) => Ok(None),
        }
    }

    async fn update_session(&self, session: &Session) -> Result<()> {
        self.save_session_metadata(session).await
    }

    async fn list_sessions(&self) -> Result<Vec<Session>> {
        self.list_sessions_metadata().await
    }

    async fn get_nodes_batch(&self, node_ids: Vec<NodeId>) -> Result<Vec<Node>> {
        let mut result = Vec::new();

        for id in node_ids {
            if let Some(node) = self.get_node(id).await? {
                result.push(node);
            }
        }

        Ok(result)
    }

    async fn insert_nodes_batch(&self, nodes: Vec<Node>) -> Result<Vec<NodeId>> {
        let mut ids = Vec::new();

        for node in nodes {
            let id = self.insert_node(node).await?;
            ids.push(id);
        }

        Ok(ids)
    }
}
