use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::fs::{self, OpenOptions};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::RwLock;

use super::node::{Node, NodeFlags, NodeId, SessionId};
use super::session::Session;
use super::storage::{NodeFilter, TreeStore};

/// JSONL-based tree store with lazy in-memory cache
///
/// **File Format:**
/// - `data/sessions/{session_id}.meta.json` - Session metadata
/// - `data/sessions/{session_id}.nodes.jsonl` - Append-only node log (one JSON per line)
///
/// **Design:**
/// - Lazy in-memory cache per session to avoid full scans
/// - Simple and reliable - cache can be rebuilt from disk on restart
/// - Suitable for small to medium workloads (< 10k nodes per session)
///
/// **Atomic Writes:**
/// - Nodes: Append to .jsonl using O_APPEND flag (atomic on most filesystems)
/// - Session metadata: Write to .tmp, fsync, rename (atomic)
#[derive(Clone)]
pub struct JSONLStore {
    base_path: PathBuf,
    cache: Arc<RwLock<HashMap<SessionId, Arc<RwLock<SessionCache>>>>>,
    sync_every_writes: u64,
    write_counter: Arc<AtomicU64>,
}

#[derive(Debug, Default)]
struct SessionCache {
    nodes: HashMap<NodeId, Node>,
    child_map: HashMap<Option<NodeId>, Vec<NodeId>>,
    next_seq: HashMap<Option<NodeId>, u32>,
}

impl SessionCache {
    fn from_nodes(nodes: HashMap<NodeId, Node>) -> Self {
        let mut cache = Self {
            nodes,
            child_map: HashMap::new(),
            next_seq: HashMap::new(),
        };

        let node_ids: Vec<NodeId> = cache.nodes.keys().cloned().collect();
        for node_id in node_ids {
            if let Some(node) = cache.nodes.get(&node_id) {
                if let Some(parent_id) = node.parent_id.clone() {
                    cache
                        .child_map
                        .entry(Some(parent_id.clone()))
                        .or_default()
                        .push(node_id.clone());

                    let next_entry = cache.next_seq.entry(Some(parent_id)).or_insert(1);
                    let candidate = node.seq + 1;
                    if candidate > *next_entry {
                        *next_entry = candidate;
                    }
                }
            }
        }

        for child_ids in cache.child_map.values_mut() {
            child_ids.sort_by_key(|id| cache.nodes.get(id).map(|n| n.seq).unwrap_or(0));
        }

        cache
    }

    fn insert_node(&mut self, node: Node) {
        let node_id = node.node_id.clone();
        let parent_id = node.parent_id.clone();
        let seq = node.seq;

        self.nodes.insert(node_id.clone(), node);

        if let Some(parent_id) = parent_id {
            let children = self.child_map.entry(Some(parent_id.clone())).or_default();
            if !children.contains(&node_id) {
                children.push(node_id.clone());
            }

            children.sort_by_key(|id| self.nodes.get(id).map(|n| n.seq).unwrap_or(0));

            let next_entry = self.next_seq.entry(Some(parent_id)).or_insert(1);
            let candidate = seq + 1;
            if candidate > *next_entry {
                *next_entry = candidate;
            }
        }
    }

    fn update_node(&mut self, node: Node) {
        self.nodes.insert(node.node_id.clone(), node);
    }
}

impl JSONLStore {
    /// Create a new JSONL store at the given path
    pub async fn new(base_path: PathBuf) -> Result<Self> {
        Self::new_with_sync_every(base_path, 1).await
    }

    /// Create a new JSONL store with batched fsync
    ///
    /// `sync_every_writes` controls how often writes call `sync_all`.
    /// Use 1 for durability on every write (default behavior).
    pub async fn new_with_sync_every(base_path: PathBuf, sync_every_writes: u64) -> Result<Self> {
        let sessions_dir = base_path.join("sessions");
        fs::create_dir_all(&sessions_dir).await.context(format!(
            "Failed to create sessions directory: {}",
            sessions_dir.display()
        ))?;

        crate::logger::log(format!(
            "[JSONLStore] Initialized (lazy cache, fsync every {} writes)",
            if sync_every_writes == 0 {
                1
            } else {
                sync_every_writes
            }
        ));
        Ok(Self {
            base_path,
            cache: Arc::new(RwLock::new(HashMap::new())),
            sync_every_writes: if sync_every_writes == 0 {
                1
            } else {
                sync_every_writes
            },
            write_counter: Arc::new(AtomicU64::new(0)),
        })
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

    /// Load all nodes for a session from JSONL file (disk only)
    async fn load_nodes_from_disk(&self, session_id: &SessionId) -> Result<HashMap<NodeId, Node>> {
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

    async fn get_session_cache(&self, session_id: &SessionId) -> Result<Arc<RwLock<SessionCache>>> {
        if let Some(cache) = self.get_session_cache_if_loaded(session_id).await {
            return Ok(cache);
        }

        let nodes = self.load_nodes_from_disk(session_id).await?;
        let cache = Arc::new(RwLock::new(SessionCache::from_nodes(nodes)));

        let mut caches = self.cache.write().await;
        let entry = caches
            .entry(session_id.clone())
            .or_insert_with(|| cache.clone());
        Ok(entry.clone())
    }

    async fn get_session_cache_if_loaded(
        &self,
        session_id: &SessionId,
    ) -> Option<Arc<RwLock<SessionCache>>> {
        let caches = self.cache.read().await;
        caches.get(session_id).cloned()
    }

    async fn find_node_in_cache(&self, node_id: &NodeId) -> Option<(SessionId, Node)> {
        let cache_entries: Vec<(SessionId, Arc<RwLock<SessionCache>>)> = {
            let caches = self.cache.read().await;
            caches
                .iter()
                .map(|(session_id, cache)| (session_id.clone(), cache.clone()))
                .collect()
        };

        for (session_id, cache) in cache_entries {
            let cache_guard = cache.read().await;
            if let Some(node) = cache_guard.nodes.get(node_id) {
                return Some((session_id, node.clone()));
            }
        }

        None
    }

    async fn update_cache_node(&self, session_id: &SessionId, node: Node) -> Result<()> {
        if let Some(cache) = self.get_session_cache_if_loaded(session_id).await {
            let mut cache_guard = cache.write().await;
            cache_guard.update_node(node);
        }
        Ok(())
    }

    async fn insert_cache_node(&self, session_id: &SessionId, node: Node) -> Result<()> {
        if let Some(cache) = self.get_session_cache_if_loaded(session_id).await {
            let mut cache_guard = cache.write().await;
            cache_guard.insert_node(node);
        }
        Ok(())
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

        let write_count = self.write_counter.fetch_add(1, Ordering::Relaxed) + 1;
        if write_count % self.sync_every_writes == 0 {
            file.sync_all().await.context("Failed to fsync node file")?;
        }

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
        let mut sessions = Vec::new();

        // Scan active sessions directory
        let sessions_dir = self.session_dir("");
        if sessions_dir.exists() {
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
        }

        // Scan archived sessions directory
        let archived_dir = self.base_path.join("archived");
        if archived_dir.exists() {
            let mut entries = fs::read_dir(&archived_dir).await.context(format!(
                "Failed to read archived directory: {}",
                archived_dir.display()
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
        }

        sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(sessions)
    }

    /// Find a node by ID across all sessions (slow, for debugging)
    async fn find_node_in_any_session(&self, node_id: &NodeId) -> Result<Option<Node>> {
        if let Some((_session_id, node)) = self.find_node_in_cache(node_id).await {
            return Ok(Some(node));
        }

        let sessions = self.list_sessions_metadata().await?;

        for session in sessions {
            let nodes = self.load_nodes_from_disk(&session.session_id).await?;
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
        let session_id = node.session_id.clone();

        // Simply append to JSONL file
        self.append_node_to_jsonl(&node).await?;
        self.insert_cache_node(&session_id, node).await?;

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
        let session_id = updated_node.session_id.clone();

        // Append updated version (JSONL append-only)
        // When loading, the last version wins
        self.append_node_to_jsonl(&updated_node).await?;
        self.update_cache_node(&session_id, updated_node).await?;

        Ok(())
    }

    async fn mark_node_pruned(&self, node_id: NodeId, pruned_at: i64) -> Result<()> {
        let node = self
            .get_node(node_id)
            .await?
            .ok_or_else(|| anyhow!("Node not found"))?;

        let mut updated_node = node.clone();
        updated_node.pruned_at = Some(pruned_at);
        let session_id = updated_node.session_id.clone();

        self.append_node_to_jsonl(&updated_node).await?;
        self.update_cache_node(&session_id, updated_node).await?;

        Ok(())
    }

    async fn delete_node(&self, node_id: NodeId) -> Result<()> {
        // JSONL doesn't support deletion - just mark as pruned
        self.mark_node_pruned(node_id, super::node::now()).await
    }

    async fn get_children(&self, node_id: NodeId) -> Result<Vec<Node>> {
        let (session_id, _node) = match self.find_node_in_cache(&node_id).await {
            Some((session_id, node)) => (session_id, node),
            None => {
                let node = self
                    .get_node(node_id.clone())
                    .await?
                    .ok_or_else(|| anyhow!("Node {} not found", node_id))?;
                (node.session_id.clone(), node)
            }
        };

        let cache = self.get_session_cache(&session_id).await?;
        let cache_guard = cache.read().await;
        let child_ids = cache_guard
            .child_map
            .get(&Some(node_id.clone()))
            .cloned()
            .unwrap_or_default();

        let mut children = Vec::with_capacity(child_ids.len());
        for child_id in child_ids {
            if let Some(child) = cache_guard.nodes.get(&child_id) {
                children.push(child.clone());
            }
        }

        children.sort_by_key(|n| n.seq);
        Ok(children)
    }

    async fn get_path_to_root_internal(&self, node_id: NodeId) -> Result<Vec<Node>> {
        let (session_id, _node) = match self.find_node_in_cache(&node_id).await {
            Some((session_id, node)) => (session_id, node),
            None => {
                let node = self
                    .get_node(node_id.clone())
                    .await?
                    .ok_or_else(|| anyhow!("Node {} not found", node_id))?;
                (node.session_id.clone(), node)
            }
        };

        let cache = self.get_session_cache(&session_id).await?;
        let cache_guard = cache.read().await;

        // Walk path to root
        let mut path = Vec::new();
        let mut current_id = Some(node_id);

        while let Some(id) = current_id {
            let current_node = cache_guard
                .nodes
                .get(&id)
                .ok_or_else(|| anyhow!("Node {} not found in session", id))?
                .clone();

            current_id = current_node.parent_id.clone();
            path.push(current_node);
        }

        Ok(path)
    }

    async fn find_nodes(&self, session_id: SessionId, filter: NodeFilter) -> Result<Vec<Node>> {
        let cache = self.get_session_cache(&session_id).await?;
        let cache_guard = cache.read().await;

        let filtered: Vec<Node> = cache_guard
            .nodes
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

    async fn archive_session(&self, session_id: SessionId) -> Result<()> {
        // Load the session
        let mut session = self
            .get_session(session_id.clone())
            .await?
            .ok_or_else(|| anyhow::anyhow!("Session not found: {}", session_id))?;

        // Set archived flag
        session.archived = true;
        session.updated_at = crate::history::node::now();

        // Save the updated session first
        self.update_session(&session).await?;

        // Create archived directory if it doesn't exist (data/archived)
        let archived_dir = self.base_path.join("archived");
        fs::create_dir_all(&archived_dir).await.context(format!(
            "Failed to create archived directory: {}",
            archived_dir.display()
        ))?;

        // Move metadata file
        let source_meta = self.session_meta_path(&session_id);
        let dest_meta = archived_dir.join(format!("{}.meta.json", session_id));
        if source_meta.exists() {
            fs::rename(&source_meta, &dest_meta).await.context(format!(
                "Failed to move metadata file from {} to {}",
                source_meta.display(),
                dest_meta.display()
            ))?;
        }

        // Move nodes JSONL file
        let source_nodes = self.nodes_jsonl_path(&session_id);
        let dest_nodes = archived_dir.join(format!("{}.nodes.jsonl", session_id));
        if source_nodes.exists() {
            fs::rename(&source_nodes, &dest_nodes)
                .await
                .context(format!(
                    "Failed to move nodes file from {} to {}",
                    source_nodes.display(),
                    dest_nodes.display()
                ))?;
        }

        // Remove from cache after moving files
        let mut cache = self.cache.write().await;
        cache.remove(&session_id);

        crate::logger::log(format!(
            "[JSONLStore] Archived session {} to archived directory",
            session_id
        ));

        Ok(())
    }

    async fn get_nodes_batch(&self, node_ids: Vec<NodeId>) -> Result<Vec<Node>> {
        let mut result = Vec::new();

        for id in node_ids {
            if let Some((_session_id, node)) = self.find_node_in_cache(&id).await {
                result.push(node);
                continue;
            }

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

    async fn get_node_in_session(
        &self,
        session_id: SessionId,
        node_id: NodeId,
    ) -> Result<Option<Node>> {
        let cache = self.get_session_cache(&session_id).await?;
        let cache_guard = cache.read().await;
        Ok(cache_guard.nodes.get(&node_id).cloned())
    }

    async fn get_nodes_batch_in_session(
        &self,
        session_id: SessionId,
        node_ids: Vec<NodeId>,
    ) -> Result<Vec<Node>> {
        let cache = self.get_session_cache(&session_id).await?;
        let cache_guard = cache.read().await;
        let mut result = Vec::new();
        for id in node_ids {
            if let Some(node) = cache_guard.nodes.get(&id) {
                result.push(node.clone());
            }
        }

        Ok(result)
    }

    async fn get_children_in_session(
        &self,
        session_id: SessionId,
        node_id: NodeId,
    ) -> Result<Vec<Node>> {
        let cache = self.get_session_cache(&session_id).await?;
        let cache_guard = cache.read().await;
        let child_ids = cache_guard
            .child_map
            .get(&Some(node_id))
            .cloned()
            .unwrap_or_default();

        let mut children = Vec::with_capacity(child_ids.len());
        for child_id in child_ids {
            if let Some(child) = cache_guard.nodes.get(&child_id) {
                children.push(child.clone());
            }
        }

        children.sort_by_key(|n| n.seq);
        Ok(children)
    }

    async fn count_children_in_session(
        &self,
        session_id: SessionId,
        node_id: NodeId,
    ) -> Result<usize> {
        let cache = self.get_session_cache(&session_id).await?;
        let cache_guard = cache.read().await;
        let count = cache_guard
            .child_map
            .get(&Some(node_id))
            .map(|children| children.len())
            .unwrap_or(0);
        Ok(count)
    }
}
