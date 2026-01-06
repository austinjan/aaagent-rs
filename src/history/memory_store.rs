use anyhow::{anyhow, Result};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::node::{Node, NodeFlags, NodeId, SessionId};
use super::session::Session;
use super::storage::{NodeFilter, TreeStore};

/// In-memory tree store implementation using HashMap
///
/// This is the simplest implementation, good for testing and development.
/// Data is not persisted and will be lost when the process exits.
#[derive(Debug, Clone)]
pub struct MemoryStore {
    nodes: Arc<RwLock<HashMap<NodeId, Node>>>,
    sessions: Arc<RwLock<HashMap<SessionId, Session>>>,
}

impl MemoryStore {
    /// Create a new empty memory store
    pub fn new() -> Self {
        Self {
            nodes: Arc::new(RwLock::new(HashMap::new())),
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
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
        let mut nodes = self.nodes.write().await;

        if nodes.contains_key(&node_id) {
            return Err(anyhow!("Node with id {} already exists", node_id));
        }

        nodes.insert(node_id.clone(), node);
        Ok(node_id)
    }

    async fn get_node(&self, node_id: NodeId) -> Result<Option<Node>> {
        let nodes = self.nodes.read().await;
        Ok(nodes.get(&node_id).cloned())
    }

    async fn update_node_flags(&self, node_id: NodeId, flags: NodeFlags) -> Result<()> {
        let mut nodes = self.nodes.write().await;

        let node = nodes
            .get_mut(&node_id)
            .ok_or_else(|| anyhow!("Node {} not found", node_id))?;

        node.flags = flags;
        Ok(())
    }

    async fn mark_node_pruned(&self, node_id: NodeId, pruned_at: i64) -> Result<()> {
        let mut nodes = self.nodes.write().await;

        let node = nodes
            .get_mut(&node_id)
            .ok_or_else(|| anyhow!("Node {} not found", node_id))?;

        node.pruned_at = Some(pruned_at);
        Ok(())
    }

    async fn delete_node(&self, node_id: NodeId) -> Result<()> {
        let mut nodes = self.nodes.write().await;
        nodes
            .remove(&node_id)
            .ok_or_else(|| anyhow!("Node {} not found", node_id))?;
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
        let mut sessions = self.sessions.write().await;

        if sessions.contains_key(&session_id) {
            return Err(anyhow!("Session with id {} already exists", session_id));
        }

        sessions.insert(session_id.clone(), session);
        Ok(session_id)
    }

    async fn get_session(&self, session_id: SessionId) -> Result<Option<Session>> {
        let sessions = self.sessions.read().await;
        Ok(sessions.get(&session_id).cloned())
    }

    async fn update_session(&self, session: &Session) -> Result<()> {
        let mut sessions = self.sessions.write().await;

        if !sessions.contains_key(&session.session_id) {
            return Err(anyhow!("Session {} not found", session.session_id));
        }

        sessions.insert(session.session_id.clone(), session.clone());
        Ok(())
    }

    async fn list_sessions(&self) -> Result<Vec<Session>> {
        let sessions = self.sessions.read().await;
        Ok(sessions.values().cloned().collect())
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
        let mut nodes = self.nodes.write().await;
        let mut ids = Vec::new();

        for node in nodes_to_insert {
            let node_id = node.node_id.clone();

            if nodes.contains_key(&node_id) {
                return Err(anyhow!("Node with id {} already exists", node_id));
            }

            nodes.insert(node_id.clone(), node);
            ids.push(node_id);
        }

        Ok(ids)
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
