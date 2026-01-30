use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::node::{Node, NodeFlags, NodeId, SessionId};
use super::session::Session;
use super::storage::{NodeFilter, TreeStore};

/// In-memory tree store implementation using HashMap
///
/// Supports optional persistence to disk for production use.
#[derive(Debug, Clone)]
pub struct MemoryStore {
    nodes: Arc<RwLock<HashMap<NodeId, Node>>>,
    sessions: Arc<RwLock<HashMap<SessionId, Session>>>,
    persistence_path: Option<PathBuf>,
}

impl MemoryStore {
    /// Create a new empty memory store (no persistence)
    pub fn new() -> Self {
        Self {
            nodes: Arc::new(RwLock::new(HashMap::new())),
            sessions: Arc::new(RwLock::new(HashMap::new())),
            persistence_path: None,
        }
    }

    /// Create a new memory store with disk persistence
    pub async fn with_persistence(base_path: PathBuf) -> Result<Self> {
        // Create directory structure
        let sessions_dir = base_path.join("sessions");
        let nodes_dir = base_path.join("nodes");

        tokio::fs::create_dir_all(&sessions_dir)
            .await
            .context(format!(
                "Failed to create sessions directory: {}",
                sessions_dir.display()
            ))?;

        tokio::fs::create_dir_all(&nodes_dir)
            .await
            .context(format!(
                "Failed to create nodes directory: {}",
                nodes_dir.display()
            ))?;

        let store = Self {
            nodes: Arc::new(RwLock::new(HashMap::new())),
            sessions: Arc::new(RwLock::new(HashMap::new())),
            persistence_path: Some(base_path),
        };

        // Load existing data from disk
        store.load_from_disk().await?;

        Ok(store)
    }

    /// Load all sessions and nodes from disk
    async fn load_from_disk(&self) -> Result<()> {
        if let Some(ref base_path) = self.persistence_path {
            // Load sessions
            let sessions_dir = base_path.join("sessions");
            if sessions_dir.exists() {
                let mut entries = tokio::fs::read_dir(&sessions_dir).await.context(format!(
                    "Failed to read sessions directory: {}",
                    sessions_dir.display()
                ))?;

                let mut sessions = self.sessions.write().await;
                while let Some(entry) = entries.next_entry().await? {
                    let path = entry.path();
                    if path.extension().and_then(|s| s.to_str()) == Some("json") {
                        if let Ok(content) = tokio::fs::read_to_string(&path).await {
                            if let Ok(session) = serde_json::from_str::<Session>(&content) {
                                sessions.insert(session.session_id.clone(), session);
                            }
                        }
                    }
                }
            }

            // Load nodes
            let nodes_dir = base_path.join("nodes");
            if nodes_dir.exists() {
                let mut session_dirs = tokio::fs::read_dir(&nodes_dir).await.context(format!(
                    "Failed to read nodes directory: {}",
                    nodes_dir.display()
                ))?;

                let mut nodes = self.nodes.write().await;
                while let Some(session_dir) = session_dirs.next_entry().await? {
                    if session_dir.path().is_dir() {
                        let mut node_files = tokio::fs::read_dir(session_dir.path()).await?;

                        while let Some(node_file) = node_files.next_entry().await? {
                            let path = node_file.path();
                            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                                if let Ok(content) = tokio::fs::read_to_string(&path).await {
                                    if let Ok(node) = serde_json::from_str::<Node>(&content) {
                                        nodes.insert(node.node_id.clone(), node);
                                    }
                                }
                            }
                        }
                    }
                }
            }

            let sessions_count = self.sessions.read().await.len();
            let nodes_count = self.nodes.read().await.len();
            crate::logger::log(format!(
                "[MemoryStore] Loaded {} sessions and {} nodes from disk",
                sessions_count, nodes_count
            ));
        }

        Ok(())
    }

    /// Save a node to disk
    async fn save_node_to_disk(&self, node: &Node) -> Result<()> {
        if let Some(ref base_path) = self.persistence_path {
            let node_dir = base_path.join("nodes").join(&node.session_id);
            tokio::fs::create_dir_all(&node_dir).await.context(format!(
                "Failed to create node directory: {}",
                node_dir.display()
            ))?;

            let node_path = node_dir.join(format!("{}.json", node.node_id));
            let json = serde_json::to_string_pretty(node).context("Failed to serialize node")?;

            tokio::fs::write(&node_path, json).await.context(format!(
                "Failed to write node file: {}",
                node_path.display()
            ))?;
        }
        Ok(())
    }

    /// Save a session to disk
    async fn save_session_to_disk(&self, session: &Session) -> Result<()> {
        if let Some(ref base_path) = self.persistence_path {
            let session_path = base_path
                .join("sessions")
                .join(format!("{}.json", session.session_id));

            let json =
                serde_json::to_string_pretty(session).context("Failed to serialize session")?;

            tokio::fs::write(&session_path, json)
                .await
                .context(format!(
                    "Failed to write session file: {}",
                    session_path.display()
                ))?;
        }
        Ok(())
    }

    /// Delete a node from disk
    async fn delete_node_from_disk(&self, session_id: &str, node_id: &str) -> Result<()> {
        if let Some(ref base_path) = self.persistence_path {
            let node_path = base_path
                .join("nodes")
                .join(session_id)
                .join(format!("{}.json", node_id));

            if node_path.exists() {
                tokio::fs::remove_file(&node_path).await.context(format!(
                    "Failed to delete node file: {}",
                    node_path.display()
                ))?;
            }
        }
        Ok(())
    }
}

impl Default for MemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TreeStore for MemoryStore {
    async fn insert_node(&self, node: Node) -> Result<NodeId> {
        let node_id = node.node_id.clone();

        // 1. Insert into memory
        {
            let mut nodes = self.nodes.write().await;
            if nodes.contains_key(&node_id) {
                return Err(anyhow!("Node with id {} already exists", node_id));
            }
            nodes.insert(node_id.clone(), node.clone());
        }

        // 2. Persist to disk (if enabled)
        self.save_node_to_disk(&node).await?;

        Ok(node_id)
    }

    async fn get_node(&self, node_id: NodeId) -> Result<Option<Node>> {
        let nodes = self.nodes.read().await;
        Ok(nodes.get(&node_id).cloned())
    }

    async fn update_node_flags(&self, node_id: NodeId, flags: NodeFlags) -> Result<()> {
        let updated_node = {
            let mut nodes = self.nodes.write().await;
            let node = nodes
                .get_mut(&node_id)
                .ok_or_else(|| anyhow!("Node {} not found", node_id))?;
            node.flags = flags;
            node.clone()
        };

        // Persist to disk
        self.save_node_to_disk(&updated_node).await?;
        Ok(())
    }

    async fn mark_node_pruned(&self, node_id: NodeId, pruned_at: i64) -> Result<()> {
        let updated_node = {
            let mut nodes = self.nodes.write().await;
            let node = nodes
                .get_mut(&node_id)
                .ok_or_else(|| anyhow!("Node {} not found", node_id))?;
            node.pruned_at = Some(pruned_at);
            node.clone()
        };

        // Persist to disk
        self.save_node_to_disk(&updated_node).await?;
        Ok(())
    }

    async fn delete_node(&self, node_id: NodeId) -> Result<()> {
        let (session_id, node_id_str) = {
            let mut nodes = self.nodes.write().await;
            let node = nodes
                .remove(&node_id)
                .ok_or_else(|| anyhow!("Node {} not found", node_id))?;
            (node.session_id.clone(), node.node_id.clone())
        };

        // Delete from disk
        self.delete_node_from_disk(&session_id, &node_id_str)
            .await?;
        Ok(())
    }

    async fn get_children(&self, node_id: NodeId) -> Result<Vec<Node>> {
        let nodes = self.nodes.read().await;

        let mut children: Vec<Node> = nodes
            .values()
            .filter(|n| n.parent_id.as_ref() == Some(&node_id))
            .cloned()
            .collect();

        // Sort by seq to maintain ordering
        children.sort_by_key(|n| n.seq);

        Ok(children)
    }

    async fn get_path_to_root_internal(&self, node_id: NodeId) -> Result<Vec<Node>> {
        let nodes = self.nodes.read().await;
        let mut path = Vec::new();
        let mut current_id = Some(node_id);

        while let Some(id) = current_id {
            let node = nodes
                .get(&id)
                .ok_or_else(|| anyhow!("Node {} not found", id))?
                .clone();

            current_id = node.parent_id.clone();
            path.push(node);
        }

        Ok(path)
    }

    async fn find_nodes(&self, session_id: SessionId, filter: NodeFilter) -> Result<Vec<Node>> {
        let nodes = self.nodes.read().await;

        let filtered: Vec<Node> = nodes
            .values()
            .filter(|n| {
                // Filter by session
                if n.session_id != session_id {
                    return false;
                }

                // Filter by kind
                if let Some(ref kinds) = filter.kinds {
                    if !kinds.contains(&n.kind) {
                        return false;
                    }
                }

                // Filter by role
                if let Some(ref roles) = filter.roles {
                    if let Some(ref node_role) = n.role {
                        if !roles.contains(node_role) {
                            return false;
                        }
                    } else {
                        return false;
                    }
                }

                // Filter by created_after
                if let Some(after) = filter.created_after {
                    if n.created_at <= after {
                        return false;
                    }
                }

                // Filter by created_before
                if let Some(before) = filter.created_before {
                    if n.created_at >= before {
                        return false;
                    }
                }

                // Filter by flags
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

                // Filter by content search
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

        // 1. Insert into memory
        {
            let mut sessions = self.sessions.write().await;
            if sessions.contains_key(&session_id) {
                return Err(anyhow!("Session with id {} already exists", session_id));
            }
            sessions.insert(session_id.clone(), session.clone());
        }

        // 2. Persist to disk
        self.save_session_to_disk(&session).await?;

        Ok(session_id)
    }

    async fn get_session(&self, session_id: SessionId) -> Result<Option<Session>> {
        let sessions = self.sessions.read().await;
        Ok(sessions.get(&session_id).cloned())
    }

    async fn update_session(&self, session: &Session) -> Result<()> {
        // 1. Update memory
        {
            let mut sessions = self.sessions.write().await;
            if !sessions.contains_key(&session.session_id) {
                return Err(anyhow!("Session {} not found", session.session_id));
            }
            sessions.insert(session.session_id.clone(), session.clone());
        }

        // 2. Persist to disk
        self.save_session_to_disk(session).await?;

        Ok(())
    }

    async fn list_sessions(&self) -> Result<Vec<Session>> {
        let sessions = self.sessions.read().await;
        Ok(sessions.values().cloned().collect())
    }

    async fn archive_session(&self, session_id: SessionId) -> Result<()> {
        // Get the session and set archived flag
        let mut sessions = self.sessions.write().await;
        if let Some(session) = sessions.get_mut(&session_id) {
            session.archived = true;
            session.updated_at = crate::history::node::now();
        } else {
            return Err(anyhow::anyhow!("Session not found: {}", session_id));
        }

        Ok(())
    }

    async fn get_nodes_batch(&self, node_ids: Vec<NodeId>) -> Result<Vec<Node>> {
        let nodes = self.nodes.read().await;
        let mut result = Vec::new();

        for id in node_ids {
            if let Some(node) = nodes.get(&id) {
                result.push(node.clone());
            }
        }

        Ok(result)
    }

    async fn insert_nodes_batch(&self, nodes_to_insert: Vec<Node>) -> Result<Vec<NodeId>> {
        let mut ids = Vec::new();

        // 1. Insert all into memory
        {
            let mut nodes = self.nodes.write().await;
            for node in &nodes_to_insert {
                let node_id = node.node_id.clone();
                if nodes.contains_key(&node_id) {
                    return Err(anyhow!("Node with id {} already exists", node_id));
                }
                nodes.insert(node_id.clone(), node.clone());
                ids.push(node_id);
            }
        }

        // 2. Persist all to disk
        for node in &nodes_to_insert {
            self.save_node_to_disk(node).await?;
        }

        Ok(ids)
    }

    async fn get_node_in_session(
        &self,
        session_id: SessionId,
        node_id: NodeId,
    ) -> Result<Option<Node>> {
        // For MemoryStore, we can directly look up by node_id
        // and verify it belongs to the session
        let nodes = self.nodes.read().await;
        if let Some(node) = nodes.get(&node_id) {
            if node.session_id == session_id {
                return Ok(Some(node.clone()));
            }
        }
        Ok(None)
    }

    async fn get_nodes_batch_in_session(
        &self,
        session_id: SessionId,
        node_ids: Vec<NodeId>,
    ) -> Result<Vec<Node>> {
        let nodes = self.nodes.read().await;
        let mut result = Vec::new();

        for id in node_ids {
            if let Some(node) = nodes.get(&id) {
                if node.session_id == session_id {
                    result.push(node.clone());
                }
            }
        }

        Ok(result)
    }

    async fn get_children_in_session(
        &self,
        session_id: SessionId,
        node_id: NodeId,
    ) -> Result<Vec<Node>> {
        let nodes = self.nodes.read().await;

        let mut children: Vec<Node> = nodes
            .values()
            .filter(|n| n.session_id == session_id && n.parent_id.as_ref() == Some(&node_id))
            .cloned()
            .collect();

        // Sort by seq to maintain ordering
        children.sort_by_key(|n| n.seq);

        Ok(children)
    }

    async fn count_children_in_session(
        &self,
        session_id: SessionId,
        node_id: NodeId,
    ) -> Result<usize> {
        let nodes = self.nodes.read().await;

        let count = nodes
            .values()
            .filter(|n| n.session_id == session_id && n.parent_id.as_ref() == Some(&node_id))
            .count();

        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::node::{new_node_id, new_session_id, now, ContentType, NodeKind};
    use crate::llm::Role;

    #[tokio::test]
    async fn test_insert_and_get_node() {
        let store = MemoryStore::new();
        let session_id = new_session_id();

        let node = Node {
            node_id: new_node_id(),
            session_id: session_id.clone(),
            parent_id: None,
            kind: NodeKind::Root,
            role: None,
            content_type: ContentType::Text,
            content: "root".to_string(),
            created_at: now(),
            seq: 0,
            flags: NodeFlags::default(),
            tool_call_id: None,
            tool_calls: None,
            pruned_at: None,
            metadata: None,
        };

        let node_id = node.node_id.clone();
        store.insert_node(node).await.unwrap();

        let retrieved = store.get_node(node_id).await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().kind, NodeKind::Root);
    }

    #[tokio::test]
    async fn test_get_children() {
        let store = MemoryStore::new();
        let session_id = new_session_id();

        // Create root
        let root = Node {
            node_id: new_node_id(),
            session_id: session_id.clone(),
            parent_id: None,
            kind: NodeKind::Root,
            role: None,
            content_type: ContentType::Text,
            content: "root".to_string(),
            created_at: now(),
            seq: 0,
            flags: NodeFlags::default(),
            tool_call_id: None,
            tool_calls: None,
            pruned_at: None,
            metadata: None,
        };
        let root_id = root.node_id.clone();
        store.insert_node(root).await.unwrap();

        // Create children
        for i in 1..=3 {
            let child = Node {
                node_id: new_node_id(),
                session_id: session_id.clone(),
                parent_id: Some(root_id.clone()),
                kind: NodeKind::Message,
                role: Some(Role::User),
                content_type: ContentType::Text,
                content: format!("child {}", i),
                created_at: now(),
                seq: i,
                flags: NodeFlags::default(),
                tool_call_id: None,
                tool_calls: None,
                pruned_at: None,
                metadata: None,
            };
            store.insert_node(child).await.unwrap();
        }

        let children = store.get_children(root_id).await.unwrap();
        assert_eq!(children.len(), 3);
        assert_eq!(children[0].seq, 1);
        assert_eq!(children[1].seq, 2);
        assert_eq!(children[2].seq, 3);
    }

    #[tokio::test]
    async fn test_path_to_root() {
        let store = MemoryStore::new();
        let session_id = new_session_id();

        // Create chain: root -> A -> B -> C
        let root = Node {
            node_id: new_node_id(),
            session_id: session_id.clone(),
            parent_id: None,
            kind: NodeKind::Root,
            role: None,
            content_type: ContentType::Text,
            content: "root".to_string(),
            created_at: now(),
            seq: 0,
            flags: NodeFlags::default(),
            tool_call_id: None,
            tool_calls: None,
            pruned_at: None,
            metadata: None,
        };
        let root_id = root.node_id.clone();
        store.insert_node(root).await.unwrap();

        let node_a = Node {
            node_id: new_node_id(),
            session_id: session_id.clone(),
            parent_id: Some(root_id.clone()),
            kind: NodeKind::Message,
            role: Some(Role::User),
            content_type: ContentType::Text,
            content: "A".to_string(),
            created_at: now(),
            seq: 1,
            flags: NodeFlags::default(),
            tool_call_id: None,
            tool_calls: None,
            pruned_at: None,
            metadata: None,
        };
        let node_a_id = node_a.node_id.clone();
        store.insert_node(node_a).await.unwrap();

        let node_b = Node {
            node_id: new_node_id(),
            session_id: session_id.clone(),
            parent_id: Some(node_a_id.clone()),
            kind: NodeKind::Message,
            role: Some(Role::Assistant),
            content_type: ContentType::Text,
            content: "B".to_string(),
            created_at: now(),
            seq: 2,
            flags: NodeFlags::default(),
            tool_call_id: None,
            tool_calls: None,
            pruned_at: None,
            metadata: None,
        };
        let node_b_id = node_b.node_id.clone();
        store.insert_node(node_b).await.unwrap();

        let node_c = Node {
            node_id: new_node_id(),
            session_id: session_id.clone(),
            parent_id: Some(node_b_id.clone()),
            kind: NodeKind::Message,
            role: Some(Role::User),
            content_type: ContentType::Text,
            content: "C".to_string(),
            created_at: now(),
            seq: 3,
            flags: NodeFlags::default(),
            tool_call_id: None,
            tool_calls: None,
            pruned_at: None,
            metadata: None,
        };
        let node_c_id = node_c.node_id.clone();
        store.insert_node(node_c).await.unwrap();

        // Get path from C to root
        let path = store.get_path_to_root_internal(node_c_id).await.unwrap();
        assert_eq!(path.len(), 4); // C, B, A, root
        assert_eq!(path[0].content, "C");
        assert_eq!(path[1].content, "B");
        assert_eq!(path[2].content, "A");
        assert_eq!(path[3].content, "root");
    }

    #[tokio::test]
    async fn test_update_flags() {
        let store = MemoryStore::new();
        let session_id = new_session_id();

        let node = Node {
            node_id: new_node_id(),
            session_id: session_id.clone(),
            parent_id: None,
            kind: NodeKind::Root,
            role: None,
            content_type: ContentType::Text,
            content: "root".to_string(),
            created_at: now(),
            seq: 0,
            flags: NodeFlags::default(),
            tool_call_id: None,
            tool_calls: None,
            pruned_at: None,
            metadata: None,
        };
        let node_id = node.node_id.clone();
        store.insert_node(node).await.unwrap();

        // Update flags
        let new_flags = NodeFlags {
            important: true,
            ephemeral: false,
            hidden: false,
        };
        store
            .update_node_flags(node_id.clone(), new_flags.clone())
            .await
            .unwrap();

        // Verify
        let updated = store.get_node(node_id).await.unwrap().unwrap();
        assert!(updated.flags.important);
    }

    #[tokio::test]
    async fn test_mark_pruned() {
        let store = MemoryStore::new();
        let session_id = new_session_id();

        let node = Node {
            node_id: new_node_id(),
            session_id: session_id.clone(),
            parent_id: None,
            kind: NodeKind::Root,
            role: None,
            content_type: ContentType::Text,
            content: "root".to_string(),
            created_at: now(),
            seq: 0,
            flags: NodeFlags::default(),
            tool_call_id: None,
            tool_calls: None,
            pruned_at: None,
            metadata: None,
        };
        let node_id = node.node_id.clone();
        store.insert_node(node).await.unwrap();

        let pruned_at = now();
        store
            .mark_node_pruned(node_id.clone(), pruned_at)
            .await
            .unwrap();

        let updated = store.get_node(node_id).await.unwrap().unwrap();
        assert_eq!(updated.pruned_at, Some(pruned_at));
    }
}
