# Bug: `current_l1` Not Set to Derivation Cursor

## Summary

The `optimism_syncStatus` RPC endpoint returns `current_l1` that is incorrectly set to the latest L1 head instead of the actual derivation cursor (where the derivation pipeline last idled at).

Impact: Affects observability and external services that rely on knowing where the node is in the derivation process.

## Discovery

While implementing follow mode (Step 1-A), we needed to inject external `current_l1` values when derivation is disabled. During investigation of how `current_l1` is populated, we discovered the current implementation doesn't match the specification.

## The Specification

From `crates/protocol/protocol/src/sync.rs:15-22`:

```rust
pub struct SyncStatus {
    /// The current L1 block.
    ///
    /// This is the L1 block that the derivation process is last idled at.
    /// This may not be fully derived into L2 data yet.
    /// The safe L2 blocks were produced/included fully from the L1 chain up to _but excluding_
    /// this L1 block. If the node is synced, this matches the `head_l1`, minus the verifier
    /// confirmation distance.
    pub current_l1: BlockInfo,

    /// The L1 head block ref.
    ///
    /// The head is not guaranteed to build on the other L1 sync status fields,
    /// as the node may be in progress of resetting to adapt to a L1 reorg.
    pub head_l1: BlockInfo,
    // ...
}
```

**Key point**: `current_l1` should be "the L1 block that the derivation process is last idled at" (the derivation cursor), NOT the latest L1 head.

## Current (Incorrect) Implementation

### How It Currently Works

**L1WatcherActor** (`crates/node/service/src/actors/l1_watcher/actor.rs:158-191`):
```rust
L1WatcherQueries::L1State(sender) => {
    let current_l1 = *latest_head.borrow();  // ← BUG: Uses L1 head, not derivation cursor
    // ...
    sender.send(L1State {
        current_l1,
        head_l1,
        // ...
    })
}
```

**What `latest_head` is** (`crates/node/service/src/service/node.rs:231-251`):
```rust
let head_stream = BlockStream::new_as_stream(
    self.l1_config.engine_provider.clone(),
    BlockNumberOrTag::Latest,  // ← Polls actual L1 latest head
    Duration::from_secs(HEAD_STREAM_POLL_INTERVAL),
);

let l1_watcher = L1WatcherActor::new(
    // ...
    l1_head_updates_tx.clone(),  // ← This becomes latest_head
    // ...
    head_stream,  // ← Fed by Latest tag
    finalized_stream,
);
```

**L1WatcherActor updates** (`l1_watcher/actor.rs:110-116`):
```rust
new_head = self.head_stream.next() => {
    Some(head_block_info) => {
        // Send the head update event to all consumers.
        self.latest_head.send_replace(Some(head_block_info));  // ← Latest L1 head
    }
}
```

### Where the Real Derivation Cursor Lives

**DerivationPipeline** (`crates/protocol/derive/src/pipeline/core.rs:53-55`):
```rust
impl<S, P> OriginProvider for DerivationPipeline<S, P> {
    fn origin(&self) -> Option<BlockInfo> {
        self.attributes.origin()  // ← This is the actual derivation cursor
    }
}
```

**DerivationActor uses it** (`crates/node/service/src/actors/derivation.rs:234, 265`):
```rust
self.pipeline.origin()  // ← Returns current L1 origin being processed
```

**But**: This value is **never communicated back** to L1WatcherActor for RPC queries.

## The Impact

### When the Bug Manifests

1. **Node is synced and idle**: ✅ Works okay
   - `head_l1`: Block 1000
   - `current_l1`: Block 995 (close enough due to confirmation distance)
   - Derivation cursor: ~Block 995
   - **Impact**: Minimal, values are close

2. **Node is syncing/catching up**: ❌ Completely wrong
   - `head_l1`: Block 1000
   - `current_l1`: Block 1000 ← **BUG: Should be 500!**
   - Derivation cursor: Block 500 (way behind)
   - **Impact**: External services think node is at L1 1000 when it's really at 500

3. **Node is offline/stalled**: ❌ Misleading
   - `head_l1`: Block 1000 (stale from last poll)
   - `current_l1`: Block 1000 ← **BUG: Should show where derivation stopped**
   - Derivation cursor: Block 800 (stopped here)
   - **Impact**: Doesn't reflect where derivation actually is

### Who This Affects

- **External monitoring services**: Get wrong information about derivation progress
- **Dependent microservices**: May make incorrect decisions based on `current_l1`
- **Operators**: Can't accurately monitor actual derivation lag
- **Metrics/dashboards**: Show incorrect sync status

## Data Flow Diagram

### Current (Incorrect) Flow
```
BlockStream(Latest) ─┐
  polls every 4s     │
                     ▼
            L1WatcherActor.latest_head
                     │
                     ▼
              Used as current_l1 in RPC
                     │
                     ▼
              External Services
              (wrong info!)

DerivationPipeline.origin() ─► Used internally only
  (actual cursor)               ❌ Never exposed!
```

### Correct Flow (Proposed)
```
BlockStream(Latest) ─┐
  polls every 4s     │
                     ▼
            L1WatcherActor.latest_head
                     │
                     ▼
              Used as head_l1 in RPC ✓

DerivationPipeline.origin() ─► DerivationActor
  (actual cursor)                     │
                                      ▼
                              derivation_origin_tx
                                      │
                                      ▼
                            L1WatcherActor.current_l1
                                      │
                                      ▼
                              Used in RPC ✓
                                      │
                                      ▼
                              External Services
                              (correct info!)
```

## Proposed Fix (General Case)

### Changes Needed

1. **Add new channel for derivation origin**:
   ```rust
   // In RollupNode::start()
   let (derivation_origin_tx, derivation_origin_rx) = watch::channel(None);
   ```

2. **DerivationActor sends origin updates**:
   ```rust
   // In DerivationActor
   pub struct DerivationActor {
       // ...
       derivation_origin_tx: watch::Sender<Option<BlockInfo>>,
   }

   // After processing/advancing origin:
   if let Some(origin) = self.pipeline.origin() {
       self.derivation_origin_tx.send_replace(Some(origin));
   }
   ```

3. **L1WatcherActor uses derivation origin**:
   ```rust
   pub struct L1WatcherActor {
       latest_head: watch::Sender<Option<BlockInfo>>,      // For head_l1
       derivation_origin: watch::Receiver<Option<BlockInfo>>, // For current_l1 ✓
       // ...
   }

   L1WatcherQueries::L1State(sender) => {
       let current_l1 = *self.derivation_origin.borrow();  // ✓ Use derivation cursor
       let head_l1 = /* query L1 provider */;
       // ...
   }
   ```

### Files to Modify

- `crates/node/service/src/service/node.rs` - Create derivation_origin channel
- `crates/node/service/src/actors/derivation.rs` - Send origin updates
- `crates/node/service/src/actors/l1_watcher/actor.rs` - Use derivation origin for current_l1

## Step 1-A: Follow Mode (Partial Fix)

### Context

When implementing follow mode (`--l2.follow.source`), derivation is completely disabled. The node relies on an external L2 CL source for safe/finalized heads. This external source also provides its `current_l1` (where it's at in derivation).

### The Follow Mode Fix

When follow mode is enabled:
1. **No local derivation cursor exists** (DerivationActor not spawned)
2. **Must use external `current_l1`** from FollowStatus
3. **Inject into L1WatcherActor** so RPC returns correct value

This is actually the **correct** behavior for follow mode - we report where the external source is at, not our non-existent local cursor.

### Implementation (Step 1-A)

```rust
// FollowActor sends external current_l1 to L1WatcherActor
pub struct FollowActor {
    follow_client: FC,
    follow_status_tx: mpsc::Sender<FollowStatus>,
    external_l1_origin_tx: mpsc::Sender<BlockInfo>,  // ← New channel to L1WatcherActor
    // ...
}

// When receiving FollowStatus:
match self.follow_client.get_follow_status().await {
    Ok(status) => {
        // Send external current_l1 to L1WatcherActor
        self.external_l1_origin_tx.send(status.current_l1).await;

        // Also send full status to EngineActor for safe/finalized injection
        self.follow_status_tx.send(status).await;
    }
}

// L1WatcherActor uses external origin when available:
pub struct L1WatcherActor {
    latest_head: watch::Sender<Option<BlockInfo>>,
    external_l1_origin_rx: Option<mpsc::Receiver<BlockInfo>>,  // ← From FollowActor
    // ...
}

L1WatcherQueries::L1State(sender) => {
    let current_l1 = if let Some(rx) = &mut self.external_l1_origin_rx {
        // Follow mode: use external source's current_l1 ✓
        rx.try_recv().ok().or_else(|| *self.latest_head.borrow())
    } else {
        // Normal mode: still has the bug (uses latest_head)
        *self.latest_head.borrow()
    };
    // ...
}
```

## Resolution Status

- [ ] **General bug fix**: Not yet implemented (derivation mode still has bug)
- [ ] **Follow mode fix (Step 1-A)**: Implementing next (uses external current_l1)
- [ ] **Full fix**: Should implement both to have consistent, correct behavior

## References

- OP Stack Sync Status Spec: https://github.com/ethereum-optimism/optimism/blob/develop/op-service/eth/sync_status.go#L5
- Kona Protocol SyncStatus: `crates/protocol/protocol/src/sync.rs`
- DerivationPipeline OriginProvider: `crates/protocol/derive/src/traits/stages.rs:17-20`

## Timeline

- **Discovered**: 2025-12-16 (during follow mode implementation)
- **Documented**: 2025-12-16
- **Fix planned**: Step 1-A (follow mode) in progress; general fix TBD

---

**Note**: This bug has likely existed since the initial RPC implementation. It works "well enough" when nodes are synced, which is why it may not have been noticed until now when implementing follow mode (where there's no local derivation at all).
