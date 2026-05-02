# Packet And Feature Persistence Design

## Purpose

PerpRadar needs durable, queryable market data for long-running operation. The current runtime computes useful real-time packets, but API consumers read them from an in-memory cache and ClickHouse tables remain empty. This design adds the first reliable persistence slice: latest packet snapshots plus minute-level feature history.

The goal is not to promise profitable trades. The goal is to preserve trustworthy, auditable, LLM-consumable market state so downstream analysis can distinguish usable signals from stale, missing, or degraded data.

## Scope

This slice persists two existing tables:

- `latest_packets`: latest packet JSON per `(symbol, profile)`.
- `features_1m`: one feature row per symbol per minute bucket.

This slice does not persist raw WebSocket messages, every depth delta, `depth_features_1s`, or replay recovery from ClickHouse. Those remain follow-up work after this storage path is stable.

## Current State

The runtime currently does this:

```text
Binance REST/WS -> SymbolState -> StandardPacket -> PacketCache -> HTTP API
```

ClickHouse is required at startup and migrations create the target tables, but runtime writes are not connected. `PacketCache` is the only live packet store.

## Target Architecture

Add an asynchronous storage writer behind a small runtime-facing interface:

```text
RuntimeEngine refreshes packet
  -> PacketCache upsert, always first
  -> StorageSink enqueue, best effort
  -> ClickHouse writer batches rows in background
```

The storage path must never block packet cache updates or HTTP API serving. ClickHouse latency, errors, or backpressure may degrade persistence, but they must not stop live packet output.

## Components

### StorageSink

`StorageSink` is the runtime-facing handle. It accepts a cloned `StandardPacket` and records it for persistence. Runtime code only depends on this small interface, so tests can use an in-memory sink without ClickHouse.

Expected behavior:

- `StorageSink::disabled()` preserves existing behavior.
- `StorageSink::channel(sender)` enqueues persistence events without awaiting ClickHouse.
- If the queue is full or closed, the sink drops the event and emits a warning or counter hook. It does not panic.

### Persistence Event

Use one event type for packet emission:

```text
PersistPacket(StandardPacket)
```

The writer derives both `latest_packets` and `features_1m` rows from the packet. This keeps runtime wiring simple and prevents separate code paths from disagreeing about values.

### ClickHouse Writer

The writer owns the ClickHouse client and receives persistence events from a bounded channel. It batches `latest_packets` rows and `features_1m` rows separately.

Flush triggers:

- `storage.batch_rows` reached.
- `storage.batch_interval_ms` elapsed.
- Shutdown handling if practical in the current runtime structure.

Insert errors are logged and counted for future health reporting. They do not bubble into runtime packet generation.

## Row Mapping

### latest_packets

Each packet emission writes one row:

- `ts`: packet timestamp.
- `symbol`: packet symbol.
- `profile`: packet profile, currently `standard`.
- `rank`: packet rank as `UInt32`.
- `packet_json`: full serialized `StandardPacket` JSON.

`latest_packets` uses `ReplacingMergeTree(ts)` with `(symbol, profile)`, so readers should query with `FINAL` or order by `ts DESC` when they require the newest version immediately.

### features_1m

Write at most one row per `(symbol, minute_bucket)` from emitted packets. The minute bucket is `packet.ts` truncated to the minute.

Field mapping:

- `price`: `packet.price.last`.
- `ret_1m`, `ret_5m`, `ret_15m`: matching packet price returns.
- `atr_pct`, `rsi14`, `macd_hist`, `adx14`, `bb_width`: matching chart fields.
- `funding_z_7d`, `basis_bp`: carry and price basis fields.
- `spread_bp`, `i1`, `i5`: liquidity fields.
- `tcs`, `lri`, `dpi5`, `csi`, `rpi`, `vov`: formal score fields.
- `quality_json`: serialized packet quality object.

`null` values must remain ClickHouse `NULL`. Missing indicators are not converted to zero.

## Deduplication

The runtime may refresh packets more frequently than once per minute. The writer should avoid writing repeated `features_1m` rows for the same symbol and minute bucket.

A small in-memory dedupe map is sufficient:

```text
last_feature_minute_by_symbol: HashMap<String, DateTime<Utc minute>>
```

If a packet belongs to the same minute already written for that symbol, write `latest_packets` but skip `features_1m`.

This intentionally favors stable minute history over high-frequency feature spam. Later work can add `score_features_1s` for sub-minute analysis.

## Data Quality Rules

Quality metadata is part of the data, not a side note.

- Persist packets even when `quality.warm=false`; downstream consumers need to know why a symbol is cold.
- Persist `features_1m` rows even when many numeric fields are null, as long as the packet timestamp and symbol are valid.
- Preserve `quality.reasons`, `stale`, `warm`, `book_seq_ok`, `book_mode`, and `freshness_ms` in `quality_json`.
- Do not fabricate values for LLM consumption. A null with a reason is better than a false numeric value.

## Backpressure And Failure Behavior

The live API is higher priority than storage completeness.

If ClickHouse is slow or unavailable after startup:

- `PacketCache` continues to update.
- The bounded storage queue may drop events.
- Dropped persistence events should be observable through logs now and a health/debug endpoint later.

If the queue is full, `latest_packets` and `features_1m` events are both represented by the same packet event, so dropping an event drops both durable outputs for that emission. This is acceptable for this slice because the next packet emission will refresh `latest_packets`, and `features_1m` is best-effort until writer health metrics are added.

## API And Runtime Compatibility

HTTP API behavior remains unchanged in this slice. API routes continue to read from `PacketCache`, not ClickHouse. This avoids adding query latency to the hot path.

Startup remains ClickHouse-gated. If ClickHouse is unavailable at startup, the service exits as it does now. After startup, writer failures degrade persistence only.

## Tests

Use TDD for implementation.

Required unit and contract coverage:

- Convert a `StandardPacket` into a `latest_packets` row with full JSON payload.
- Convert a `StandardPacket` into a `features_1m` row preserving nullable numeric fields.
- Serialize `quality_json` with warm/stale/reasons/book metadata.
- Skip duplicate `features_1m` rows for the same symbol and minute bucket while still writing `latest_packets`.
- `RuntimeEngine` sends persistence events when a packet is refreshed.
- Disabled storage sink preserves existing runtime behavior.
- Existing `cargo test --workspace` remains green.

Manual/runtime verification:

```bash
cargo test --workspace
curl http://127.0.0.1:18080/v1/health
curl "http://127.0.0.1:18080/v1/packets/top?limit=3"
curl -u perp_radar:perp_radar --data-binary "SELECT count() FROM perp_radar.latest_packets" http://127.0.0.1:8123/
curl -u perp_radar:perp_radar --data-binary "SELECT count() FROM perp_radar.features_1m" http://127.0.0.1:8123/
```

## Follow-Up Work

- Add `depth_features_1s` or `score_features_1s` for sub-minute score replay.
- Add writer health/debug endpoint with queue depth, dropped events, successful inserts, and last error.
- Add ClickHouse-backed API or export endpoints for historical feature windows.
- Add replay/recovery from ClickHouse on startup if runtime cache should warm from durable state.
