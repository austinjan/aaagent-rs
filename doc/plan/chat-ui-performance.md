# Chat UI Performance & Lazy Loading Plan

- Feature name: `chat-ui-performance`
- Status: Draft
- Created: 2026-01-06
- Parent plan: [chat-ui-plan.md](./chat-ui-plan.md)

## 1) Overview

### Goal
Achieve <200ms initial render, 60fps scrolling, and <50MB memory usage through lazy loading and virtualization.

### Scope (In)
- Performance targets with concrete metrics
- Progressive lazy loading (4-tier strategy)
- Virtual scrolling for 100+ cards
- Memory management with card recycling
- Backend pagination API

### Non-goals (Out)
- Full history pre-loading
- Client-side history storage

## 2) Performance Targets

| Metric | Target | Measurement Point |
|--------|--------|------------------|
| Initial render | <200ms | 50 visible cards |
| Scroll frame rate | 60fps | 1000+ cards virtualized |
| Memory usage | <50MB | 1000-node session |
| SSE throughput | 100+ events/sec | No dropped frames |
| Card render budget | <4ms per card | To maintain 60fps |
| Virtualization threshold | 100 cards | When to enable virtual scroll |

## 3) Backend API Design

### Paginated Path Endpoint

```
GET /api/sessions/{session_id}/path?limit=50&offset=0&direction=newest_first

Response:
{
  "nodes": [...],           // 50 nodes
  "total_count": 1234,      // Total nodes in active path
  "has_more": true,
  "next_offset": 50,
  "estimated_heights": {    // For virtual scrolling placeholder
    "message_avg": 120,     // px
    "tool_pair_avg": 180,
    "checkpoint_avg": 150
  }
}
```

### Node Range Endpoint

```
GET /api/sessions/{session_id}/path/range?start_node_id=abc&end_node_id=xyz

Response:
{
  "nodes": [...],           // Nodes between start and end
  "start_offset": 450,      // Position in full path
  "end_offset": 500
}
```

### Metadata Endpoint

```
GET /api/sessions/{session_id}/path/metadata

Response:
{
  "total_nodes": 1234,
  "active_leaf_id": "xyz",
  "root_node_id": "abc",
  "checkpoint_positions": [100, 450, 890],  // Offsets
  "estimated_total_height": 148080          // px (for scrollbar)
}
```

## 4) Frontend Strategy

### 4-Tier Adaptive Loading

| Session Size | Strategy | Rationale |
|-------------|----------|-----------|
| <100 nodes | No virtualization, load all | Fast enough, simple |
| 100-500 nodes | Virtualization only | Smooth scroll, low overhead |
| 500-1000 nodes | Virtual + lazy load | Balance memory & requests |
| 1000+ nodes | Aggressive lazy + recycle | Keep <50MB, 60fps |

```typescript
function selectStrategy(totalNodes: number): LoadStrategy {
  if (totalNodes < 100) {
    return new LoadAllStrategy();
  } else if (totalNodes < 500) {
    return new VirtualScrollStrategy();
  } else if (totalNodes < 1000) {
    return new LazyVirtualStrategy(chunkSize: 50);
  } else {
    return new AggressiveLazyStrategy(chunkSize: 25, recyclePool: true);
  }
}
```

### Initial Load (Phase 1)

**Target: <200ms**

```typescript
async function initialLoad(sessionId: string) {
  // 1. Fetch metadata (fast, <50ms)
  const metadata = await fetch(`/api/sessions/${sessionId}/path/metadata`);
  
  // 2. Set virtual scroll total height
  setVirtualScrollHeight(metadata.estimated_total_height);
  
  // 3. Fetch only visible viewport (newest 50 cards)
  const visible = await fetch(
    `/api/sessions/${sessionId}/path?limit=50&offset=0&direction=newest_first`
  );
  
  // 4. Render immediately
  renderCards(visible.nodes);  // <200ms total
}
```

### Scroll-Triggered Loading (Phase 2)

```typescript
class VirtualizedChatContainer {
  private loadedRanges: Set<string> = new Set();
  private chunkSize = 50;
  
  onScroll(scrollTop: number) {
    const visibleRange = this.calculateVisibleRange(scrollTop);
    const chunksToLoad = this.getUnloadedChunks(visibleRange);
    
    for (const chunk of chunksToLoad) {
      this.loadChunk(chunk.offset, chunk.limit);
    }
  }
  
  async loadChunk(offset: number, limit: number) {
    const chunkKey = `${offset}-${limit}`;
    if (this.loadedRanges.has(chunkKey)) return;
    
    const nodes = await fetch(
      `/api/sessions/${sessionId}/path?offset=${offset}&limit=${limit}`
    );
    
    this.loadedRanges.add(chunkKey);
    this.insertNodesAtOffset(nodes, offset);
  }
  
  calculateVisibleRange(scrollTop: number): Range {
    const avgCardHeight = 120;
    const startIndex = Math.floor(scrollTop / avgCardHeight) - 50;  // -1 chunk buffer
    const endIndex = startIndex + 150;  // viewport + 2 chunk buffers
    
    return { start: Math.max(0, startIndex), end: endIndex };
  }
}
```

### Virtual Scrolling (Phase 3)

**For 100+ cards:**

```typescript
class VirtualScrollManager {
  private totalHeight = 0;
  private renderedCards = new Map<number, CardElement>();
  
  render() {
    const { scrollTop, viewportHeight } = this.getScrollInfo();
    const visibleIndices = this.getVisibleIndices(scrollTop, viewportHeight);
    
    // Render only visible + buffer (±25 cards)
    const toRender = this.expandWithBuffer(visibleIndices, 25);
    
    // Mount new cards
    for (const idx of toRender) {
      if (!this.renderedCards.has(idx)) {
        this.mountCard(idx);
      }
    }
    
    // Unmount far-away cards (keep memory low)
    for (const [idx, card] of this.renderedCards) {
      if (!toRender.includes(idx)) {
        this.unmountCard(idx);
      }
    }
  }
  
  mountCard(index: number) {
    const node = this.loadedNodes.get(index);
    if (!node) {
      this.triggerLazyLoad(index);
      return;
    }
    
    const card = this.createCardElement(node);
    const offset = this.calculateOffset(index);
    card.style.position = 'absolute';
    card.style.top = `${offset}px`;
    this.container.appendChild(card);
    this.renderedCards.set(index, card);
  }
}
```

## 5) Memory Management

### Card Recycling Pool

```typescript
class CardPool {
  private pool: CardElement[] = [];
  private maxPoolSize = 100;
  
  acquire(type: CardType): CardElement {
    const recycled = this.pool.find(c => c.type === type);
    if (recycled) {
      this.pool = this.pool.filter(c => c !== recycled);
      return recycled;
    }
    return this.createNew(type);
  }
  
  release(card: CardElement) {
    if (this.pool.length < this.maxPoolSize) {
      card.reset();  // Clear data, keep DOM structure
      this.pool.push(card);
    } else {
      card.destroy();  // GC will collect
    }
  }
}
```

### Aggressive Cleanup

```typescript
function cleanupOffscreenCards() {
  const { scrollTop, viewportHeight } = getScrollInfo();
  const keepRange = {
    start: scrollTop - viewportHeight * 2,  // Keep 2 viewports above
    end: scrollTop + viewportHeight * 3     // Keep 3 viewports below
  };
  
  for (const [offset, card] of renderedCards) {
    if (offset < keepRange.start || offset > keepRange.end) {
      cardPool.release(card);
      renderedCards.delete(offset);
    }
  }
}

// Run cleanup every 2 seconds during idle
setInterval(() => requestIdleCallback(cleanupOffscreenCards), 2000);
```

## 6) Backend Optimization

### Path Caching

```rust
impl Session {
    /// Get paginated slice of active path (optimized for UI)
    pub async fn get_path_slice(
        &self,
        offset: usize,
        limit: usize,
    ) -> Result<PathSlice> {
        let store = self.store()?;
        
        // Check if we have cached path
        if let Some(cached) = self.cached_path.as_ref() {
            let slice = cached.get(offset..offset + limit).unwrap_or_default();
            return Ok(PathSlice {
                nodes: slice.to_vec(),
                total_count: cached.len(),
                has_more: offset + limit < cached.len(),
            });
        }
        
        // Build path (walk tree once)
        let full_path = self.build_full_path().await?;
        
        // Cache for future requests
        self.cached_path = Some(full_path.clone());
        
        // Return slice
        let slice = full_path.get(offset..offset + limit).unwrap_or_default();
        Ok(PathSlice {
            nodes: slice.to_vec(),
            total_count: full_path.len(),
            has_more: offset + limit < full_path.len(),
        })
    }
    
    /// Invalidate path cache (call on new messages)
    pub fn invalidate_path_cache(&mut self) {
        self.cached_path = None;
    }
}
```

## 7) Performance Monitoring

```typescript
class PerformanceMonitor {
  trackInitialRender() {
    performance.mark('render-start');
    // ... render code ...
    performance.mark('render-end');
    performance.measure('initial-render', 'render-start', 'render-end');
    
    const measure = performance.getEntriesByName('initial-render')[0];
    console.log(`Initial render: ${measure.duration}ms`);  // Target: <200ms
    
    if (measure.duration > 200) {
      analytics.trackSlow('initial-render', measure.duration);
    }
  }
  
  trackMemoryUsage() {
    if ('memory' in performance) {
      const memory = (performance as any).memory;
      const usedMB = memory.usedJSHeapSize / 1024 / 1024;
      
      console.log(`Memory usage: ${usedMB.toFixed(2)}MB`);  // Target: <50MB
      
      if (usedMB > 50) {
        analytics.trackHighMemory(usedMB);
        this.triggerAggressiveCleanup();
      }
    }
  }
  
  trackScrollPerformance() {
    let frameCount = 0;
    let lastTime = performance.now();
    
    const measureFPS = () => {
      const now = performance.now();
      frameCount++;
      
      if (now - lastTime >= 1000) {
        console.log(`Scroll FPS: ${frameCount}`);  // Target: 60fps
        
        if (frameCount < 50) {
          analytics.trackLowFPS(frameCount);
        }
        
        frameCount = 0;
        lastTime = now;
      }
      
      requestAnimationFrame(measureFPS);
    };
    
    requestAnimationFrame(measureFPS);
  }
}
```

## 8) Testing Plan

**Performance Tests:**
- [ ] Initial render <200ms with 50 cards
- [ ] Scroll at 60fps with 1000+ cards
- [ ] Memory <50MB with 1000 nodes
- [ ] SSE 100+ events/sec without lag
- [ ] Card render <4ms per card
- [ ] Cleanup reduces memory by 30%+

**Lazy Loading Tests:**
- [ ] Chunks load on scroll
- [ ] No duplicate chunk loads
- [ ] Buffer prevents flickering
- [ ] Jump-to loads correct range

**Backend Tests:**
- [ ] Path slice query <50ms for 10k nodes
- [ ] Metadata endpoint <20ms
- [ ] Cache hit rate >90%
- [ ] Cache invalidates on new message

## 9) Acceptance Criteria

- [ ] Initial render <200ms
- [ ] 60fps during scroll
- [ ] Memory <50MB for 1000+ nodes
- [ ] Virtual scrolling at 100+ cards
- [ ] Lazy loading on demand
- [ ] Card recycling working
- [ ] Performance monitor logs metrics
- [ ] Adaptive strategy selects correct tier

## 10) Implementation Tasks

**Backend:**
- [ ] Implement paginated path endpoint
- [ ] Implement metadata endpoint
- [ ] Implement range endpoint
- [ ] Add path caching to Session
- [ ] Add cache invalidation

**Frontend:**
- [ ] Implement 4-tier strategy selector
- [ ] Build VirtualizedChatContainer
- [ ] Build VirtualScrollManager
- [ ] Build CardPool
- [ ] Add lazy loading on scroll
- [ ] Add cleanup scheduler
- [ ] Implement PerformanceMonitor

---

## References
- Parent plan: [chat-ui-plan.md](./chat-ui-plan.md)
- Related: [chat-ui-state-management.md](./chat-ui-state-management.md)
