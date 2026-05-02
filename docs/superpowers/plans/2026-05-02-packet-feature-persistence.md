# Packet Feature Persistence Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Persist live `latest_packets` and minute-level `features_1m` rows to ClickHouse without blocking real-time packet cache updates.

**Architecture:** Add row-mapping and writer code to `crates/storage`, expose a small non-blocking `StorageSink`, and wire it into `RuntimeEngine` after `PacketCache` updates. API reads stay in memory; ClickHouse write failures degrade persistence only.

**Tech Stack:** Rust 2021, Tokio bounded channels, ClickHouse HTTP client, serde JSON, existing packet schema 2.1, `cargo test`.

---

## File Structure

- `crates/storage/src/rows.rs`: Pure conversion from `StandardPacket` to persistable row structs. No ClickHouse network access.
- `crates/storage/src/sink.rs`: Runtime-facing `StorageSink` and `PersistEvent`.
- `crates/storage/src/writer.rs`: Async batch writer and feature-minute dedupe.
- `crates/storage/src/lib.rs`: Export new modules.
- `crates/storage/Cargo.toml`: Add `serde_json`.
- `crates/storage/tests/persistence_contract.rs`: Unit/contract tests for row mapping, dedupe, and sink behavior.
- `crates/app/src/runtime.rs`: Add optional storage sink to `RuntimeEngine` and emit persistence events after cache updates.
- `crates/app/src/main.rs`: Start writer and pass sink into ingestion.
- `crates/app/tests/runtime_contract.rs`: Runtime emits persistence event; disabled sink preserves current behavior.

## Task 1: Packet Row Mapping

**Files:**
- Create: `crates/storage/src/rows.rs`
- Modify: `crates/storage/src/lib.rs`
- Modify: `crates/storage/Cargo.toml`
- Create: `crates/storage/tests/persistence_contract.rs`

- [ ] **Step 1: Write failing row mapping tests**

Add `serde_json.workspace = true` to `crates/storage/Cargo.toml`.

Create `crates/storage/tests/persistence_contract.rs` with tests that build a `StandardPacket`, call `LatestPacketRow::from_packet()` and `Feature1mRow::from_packet()`, and assert:

```rust
assert_eq!(row.symbol, "BTCUSDT");
assert_eq!(row.profile, "standard");
assert_eq!(row.rank, 1);
assert!(row.packet_json.contains("\"packet_schema\":\"2.1\""));
assert_eq!(feature.price, Some(64000.0));
assert_eq!(feature.rsi14, Some(55.0));
assert_eq!(feature.lri, None);
assert!(feature.quality_json.contains("\"warm\":true"));
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p perp-radar-storage --test persistence_contract
```

Expected: FAIL because `rows` module and row types do not exist.

- [ ] **Step 3: Implement row mapping**

Create `LatestPacketRow` and `Feature1mRow` with public fields matching existing ClickHouse tables. Implement:

```rust
impl LatestPacketRow {
    pub fn from_packet(packet: &StandardPacket) -> anyhow::Result<Self>
}

impl Feature1mRow {
    pub fn from_packet(packet: &StandardPacket) -> anyhow::Result<Self>
}
```

Use `packet.profile` serialized with serde and trim quotes to get `standard`. Use `serde_json::to_string()` for `packet_json` and `quality_json`. Truncate packet timestamp to minute for `Feature1mRow.ts`.

- [ ] **Step 4: Run test to verify it passes**

Run:

```bash
cargo test -p perp-radar-storage --test persistence_contract
```

Expected: PASS.

## Task 2: Sink And Minute Dedupe

**Files:**
- Create: `crates/storage/src/sink.rs`
- Create: `crates/storage/src/writer.rs`
- Modify: `crates/storage/src/lib.rs`
- Modify: `crates/storage/tests/persistence_contract.rs`

- [ ] **Step 1: Write failing sink/dedupe tests**

Add tests that:

- Create a `tokio::sync::mpsc::channel(1)`, wrap it in `StorageSink::channel`, call `persist_packet(packet)`, and receive `PersistEvent::Packet`.
- Create a `FeatureMinuteDedupe`, call `should_write(&packet)` twice for the same symbol and same minute, and assert first is `true`, second is `false`.
- Change the packet timestamp by one minute and assert `should_write()` is `true`.

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p perp-radar-storage --test persistence_contract
```

Expected: FAIL because `sink` and `writer::FeatureMinuteDedupe` do not exist.

- [ ] **Step 3: Implement sink and dedupe**

Implement:

```rust
pub enum PersistEvent {
    Packet(StandardPacket),
}

#[derive(Clone, Default)]
pub struct StorageSink {
    sender: Option<tokio::sync::mpsc::Sender<PersistEvent>>,
}
```

`StorageSink::disabled()` returns no sender. `StorageSink::channel(sender)` stores sender. `persist_packet()` uses `try_send`; on full/closed it logs with `tracing::warn!` and returns without panic.

Implement `FeatureMinuteDedupe` with `HashMap<String, DateTime<Utc>>`.

- [ ] **Step 4: Run test to verify it passes**

Run:

```bash
cargo test -p perp-radar-storage --test persistence_contract
```

Expected: PASS.

## Task 3: ClickHouse Writer

**Files:**
- Modify: `crates/storage/src/writer.rs`
- Modify: `crates/storage/tests/persistence_contract.rs`

- [ ] **Step 1: Write failing writer batching test**

Add a pure unit test for `PendingRows`:

- Push two packet events for same symbol and same minute.
- Assert `latest_packets.len() == 2`.
- Assert `features_1m.len() == 1`.
- Push one packet for the next minute.
- Assert `features_1m.len() == 2`.

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p perp-radar-storage --test persistence_contract
```

Expected: FAIL because `PendingRows` does not exist.

- [ ] **Step 3: Implement writer batching**

Implement `PendingRows` as an in-memory accumulator over `LatestPacketRow`, `Feature1mRow`, and `FeatureMinuteDedupe`.

Implement async writer entrypoint:

```rust
pub fn spawn_clickhouse_writer(
    client: clickhouse::Client,
    config: BatchConfig,
    receiver: tokio::sync::mpsc::Receiver<PersistEvent>,
) -> tokio::task::JoinHandle<()>
```

The writer drains events, flushes when `BatchConfig::should_flush()` reaches the combined row count, and flushes on interval. Insert failures are logged and do not panic.

- [ ] **Step 4: Run test to verify it passes**

Run:

```bash
cargo test -p perp-radar-storage --test persistence_contract
```

Expected: PASS.

## Task 4: Runtime Wiring

**Files:**
- Modify: `crates/app/src/runtime.rs`
- Modify: `crates/app/src/main.rs`
- Modify: `crates/app/tests/runtime_contract.rs`

- [ ] **Step 1: Write failing runtime persistence test**

Add a runtime contract test that creates an mpsc channel, builds `RuntimeEngine::with_config_and_storage(...)`, applies enough state to generate a packet using existing helper patterns, and asserts a `PersistEvent::Packet` is received after the cache receives the packet.

Add a second test that uses the existing `RuntimeEngine::new(...)` constructor to verify disabled storage still updates cache and does not require a receiver.

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p perp-radar --test runtime_contract runtime_engine_emits_persistence_event
```

Expected: FAIL because runtime has no storage sink constructor.

- [ ] **Step 3: Implement runtime wiring**

Add `storage_sink: StorageSink` to `RuntimeEngine`.

Keep existing constructors and add:

```rust
pub fn with_config_and_storage(
    symbols: Vec<String>,
    cache: PacketCache,
    config: RuntimeEngineConfig,
    storage_sink: StorageSink,
) -> Self
```

Existing constructors call this with `StorageSink::disabled()`. In `refresh_symbol_with_rank_at()`, call `self.cache.upsert(packet.clone())` first, then `self.storage_sink.persist_packet(packet)`.

In `main.rs`, after migrations, create a ClickHouse client, bounded channel, spawn writer, pass sink into ingestion.

- [ ] **Step 4: Run test to verify it passes**

Run:

```bash
cargo test -p perp-radar --test runtime_contract runtime_engine_emits_persistence_event
cargo test --workspace
```

Expected: PASS.

## Task 5: Live Verification

**Files:**
- No required code files.

- [ ] **Step 1: Build and test**

Run:

```bash
cargo test --workspace
```

Expected: PASS.

- [ ] **Step 2: Restart local app if needed**

If an old `perp-radar` process is still running, stop it gracefully if it belongs to this workspace, then start:

```bash
PERP_RADAR__API__BIND=127.0.0.1:18080 \
PERP_RADAR__STORAGE__CLICKHOUSE_URL=http://perp_radar:perp_radar@127.0.0.1:8123 \
cargo run -p perp-radar
```

Expected: service starts and `/v1/health` returns `{"ok":true}`.

- [ ] **Step 3: Verify persisted rows**

After packets are visible, run:

```bash
curl -u perp_radar:perp_radar --data-binary "SELECT count() FROM perp_radar.latest_packets" http://127.0.0.1:8123/
curl -u perp_radar:perp_radar --data-binary "SELECT count() FROM perp_radar.features_1m" http://127.0.0.1:8123/
```

Expected: both counts are greater than zero after runtime has emitted packets.

## Self-Review

- Spec coverage: latest packet persistence, minute feature persistence, non-blocking runtime path, quality JSON, null preservation, and dedupe are covered.
- Deferred scope: `depth_features_1s`, historical APIs, writer health endpoint, and replay recovery remain out of this plan.
- Placeholder scan: no open-ended implementation placeholders remain.
- Type consistency: planned names are `LatestPacketRow`, `Feature1mRow`, `PersistEvent`, `StorageSink`, `FeatureMinuteDedupe`, and `PendingRows`.
