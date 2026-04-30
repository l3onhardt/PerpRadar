# Perp Radar V1 Design

## Goal

Perp Radar V1 is a real-time Binance USD-M perpetual market data service that exposes accurate, low-latency, LLM-ready packets through HTTP. It does not generate trading strategies, place orders, or call an LLM. Strategy, meeting, and execution modules will integrate later as external consumers of the API.

## Selected Approach

Use one Rust binary with ClickHouse as a required dependency. The binary contains Binance ingestion, normalization, hot state, feature calculation, packet cache, ClickHouse writer, and HTTP API.

The API reads from in-memory packet and ranking caches. ClickHouse is used for history, audit, research, and latest packet persistence, not as the primary request path for real-time packet endpoints.

Rejected alternatives:

- ClickHouse-centered packet generation: easier to audit but too slow and too dependent on query-time calculation.
- Multi-service architecture: cleaner deployment boundaries later, but too much operational surface for V1.

## Scope

V1 includes:

- Binance REST bootstrap and WebSocket ingestion.
- U0 whole-market radar over USDT perpetual symbols.
- U1 active pool with 15 symbols.
- U2 focus pool with 3 symbols.
- In-memory hot state and ring buffers.
- Incremental technical, liquidity, funding, event, and score features.
- Required ClickHouse migrations and batch writes.
- LLM-ready JSON and TXT HTTP endpoints.
- Debug and health endpoints.
- Quality flags and explicit null reasons for incomplete data.

V1 excludes:

- Built-in LLM strategy generation.
- Real or paper trading.
- Cross-exchange data.
- All-market aggTrade.
- All-market full order books.
- Kafka, Redis, Flink, or separate microservices.

## Binance Data Sources

The service uses the current Binance USD-M Futures WebSocket structure:

- Market streams: `wss://fstream.binance.com/market/stream?streams=...`
- Public streams: `wss://fstream.binance.com/public/stream?streams=...`
- REST base: `https://fapi.binance.com`

U0 whole-market streams:

- `!markPrice@arr`
- `!ticker@arr`
- `!forceOrder@arr`

U1 active pool streams:

- `<symbol>@kline_1m`
- `<symbol>@depth20@500ms`

U2 focus pool streams:

- `<symbol>@depth@500ms`
- REST `/fapi/v1/depth?limit=1000` snapshots

Default U2 symbols are `BTCUSDT`, `ETHUSDT`, and `SOLUSDT`. U1 symbols are selected from the whole-market universe using quote volume, recent movement, volatility, liquidation events, funding stress, and basic liquidity filters.

## Runtime Data Flow

```text
Binance REST/WS
  -> ingest/parser
  -> normalize
  -> symbol hot state + ring buffers
  -> feature engine
  -> packet/ranking cache
  -> HTTP API for LLM consumers

normalized events/features
  -> ClickHouse batch writer
```

Startup flow:

1. Load configuration.
2. Connect to ClickHouse.
3. Run migrations.
4. Fetch `exchangeInfo`.
5. Build the USDT perpetual universe.
6. Bootstrap U1 klines and funding history.
7. Bootstrap U2 order book snapshots after stream buffering starts.
8. Start WebSocket supervisors.
9. Build packet caches.
10. Mark the service ready when minimum warmup conditions are met.

## Repository Structure

```text
perp-radar/
  Cargo.toml
  crates/
    core/
      src/
        types.rs
        time.rs
        units.rs
        symbol.rs
        quality.rs
        packet.rs
    binance/
      src/
        ws_client.rs
        rest_client.rs
        streams.rs
        parser.rs
        rate_limiter.rs
        reconnect.rs
    state/
      src/
        symbol_state.rs
        candle_ring.rs
        book_partial.rs
        book_full.rs
        funding.rs
        events.rs
    features/
      src/
        ta.rs
        liquidity.rs
        funding.rs
        scores.rs
        ranking.rs
    storage/
      src/
        clickhouse.rs
        batcher.rs
        migrations.rs
    api/
      src/
        routes.rs
        packet.rs
        export.rs
        debug.rs
    app/
      src/
        main.rs
        config.rs
        supervisor.rs
  migrations/
    001_symbols.sql
    002_klines_1m.sql
    003_mark_funding_sample.sql
    004_depth_features_1s.sql
    005_features_1m.sql
    006_latest_packets.sql
  config/
    default.yaml
  docs/
    DATA_CONTRACT.md
    RUNBOOK.md
    INDICATORS.md
    OPERATIONS.md
```

Module responsibilities:

- `core`: stable internal types, units, time helpers, quality model, and packet schema.
- `binance`: REST/WS clients, parsers, stream routing, rate limiting, and reconnect policy.
- `state`: hot symbol state, ring buffers, partial books, full books, funding history, and event windows.
- `features`: technical indicators, liquidity features, funding features, composite scores, and ranking.
- `storage`: ClickHouse migrations, repositories, and batch writer.
- `api`: axum routes, JSON packet output, TXT/JSONL export, and debug endpoints.
- `app`: configuration, task supervision, startup warmup, and graceful shutdown.

## Hot State

Each symbol has a `SymbolState` containing:

- Metadata from `exchangeInfo`.
- Latest price, mark, index, basis, ticker, and 24h volume.
- 1m closed candle ring buffer.
- Locally resampled 5m, 15m, 1h, and 4h candle buffers.
- Indicator state.
- Optional U1 partial book state.
- Optional U2 full book state.
- Funding history ring buffer.
- Liquidation and volume event windows.
- Quality state.
- Last compact, standard, and full packet bytes.

Rules:

- Only kline events with `x=true` enter the closed candle buffer.
- Open kline updates can inform nowcast fields but must be quality-marked as partial.
- 5m, 15m, 1h, and 4h candles are resampled from local 1m candles.
- Partial depth is lossy current state.
- Full book sequence gaps immediately mark full-book liquidity stale and trigger resync.
- Missing or insufficient data produces `null` plus a quality reason.

## Features

Chart features:

- Returns over 1m, 5m, 15m, and 1h.
- EMA state.
- RSI14.
- ATR percentage.
- Bollinger Band width.
- ADX.
- MACD histogram.
- Volume z-score.
- Compressed candle signature.

Liquidity features:

- Spread in basis points.
- I1 and I5 imbalance.
- Microprice basis points.
- Top20 pressure.
- U2-only 5bp and 10bp visible liquidity.
- U2-only buy/sell slippage for configured notional sizes.
- U2 full-book sequence quality.

Carry features:

- Current funding.
- Next funding time.
- Funding interval hours.
- 7-day funding z-score.
- Basis in basis points.

Event features:

- 1m, 5m, and 15m liquidation event totals.
- Dominant liquidation side.
- Volume spike z-score.

Composite scores:

- TCS.
- LRI.
- DPI5.
- CSI.
- RPI.
- VoV.

Each score has explicit prerequisites. If prerequisites are not met, the score is `null` and the packet includes a reason.

## Packet API

Primary endpoints:

```text
GET /v1/health
GET /v1/schema
GET /v1/universe
GET /v1/symbols
GET /v1/packet/{symbol}?profile=compact|standard|full
GET /v1/packets/top?limit=20&profile=standard
GET /v1/export/top.txt?limit=20
GET /v1/export/top.jsonl?limit=20
GET /v1/debug/symbol/{symbol}/quality
GET /v1/debug/ws
GET /v1/debug/rate_limits
```

The packet schema version is `2.0`. A standard packet contains:

- `packet_schema`
- `ts`
- `symbol`
- `rank`
- `universe`
- `price`
- `chart`
- `liquidity`
- `carry`
- `events`
- `scores`
- `quality`

Example shape:

```json
{
  "packet_schema": "2.0",
  "ts": "2026-05-01T00:00:00Z",
  "symbol": "BTCUSDT",
  "rank": 1,
  "universe": {
    "tier": "U2",
    "active_n": 15,
    "focus_n": 3
  },
  "price": {
    "last": 64210.5,
    "mark": 64208.9,
    "index": 64193.2,
    "basis_bp": 2.45,
    "ret_1m": 0.0012,
    "ret_5m": 0.0048,
    "ret_15m": 0.0091,
    "ret_1h": 0.0184
  },
  "chart": {
    "regime": "trend_up",
    "tf": {
      "1m": {
        "rsi": 67.1,
        "atr_pct": 0.0016,
        "bb_width": 0.012
      },
      "5m": {
        "trend": "up",
        "vol_z": 2.1
      },
      "15m": {
        "trend": "up",
        "adx": 31.4
      },
      "1h": {
        "trend": "up",
        "dist_ema200_bp": 214.0
      }
    },
    "signature": "1m:GGGRGGUGRRGG; wick:upper_mild; structure:HHHL"
  },
  "liquidity": {
    "book_mode": "full",
    "spread_bp": 0.62,
    "i1": 0.16,
    "i5": 0.09,
    "microprice_bp": 0.31,
    "liq_5bp_usd": 1850000.0,
    "liq_10bp_usd": 4200000.0,
    "slip_10000_buy_bp": 0.8,
    "slip_10000_sell_bp": 1.0
  },
  "carry": {
    "funding_now": 0.00019,
    "funding_unit": "per_interval",
    "funding_interval_hours": 8,
    "funding_z_7d": 1.74,
    "next_funding_time": "2026-05-01T08:00:00Z"
  },
  "events": {
    "liq_1m_usd": 0.0,
    "liq_5m_usd": 1260000.0,
    "liq_side": "short_liq_dominant",
    "volume_spike_z": 2.4
  },
  "scores": {
    "TCS": 1.52,
    "LRI": 0.94,
    "DPI5": 0.09,
    "CSI": 1.22,
    "RPI": 0.48,
    "VoV": 1.66
  },
  "quality": {
    "freshness_ms": 384,
    "warm": true,
    "kline_gap_1m": 0,
    "book_mode": "full",
    "book_seq_ok": true,
    "book_depth_coverage_bp": 14.8,
    "funding_history_points": 128,
    "stale": false,
    "reasons": []
  }
}
```

`/v1/export/top.txt` emits a compact text format for low-token LLM scanning. `/v1/export/top.jsonl` emits one packet per line.

## Quality Model

Quality is part of the contract, not an internal implementation detail.

Tracked quality fields:

- `freshness_ms`
- `warm`
- `kline_gap_1m`
- `book_mode`
- `book_seq_ok`
- `book_depth_coverage_bp`
- `funding_history_points`
- `stale`
- `reasons`

Quality rules:

- Never use zero as a substitute for missing data.
- Use `null` for unavailable fields.
- Add a reason for every important null field.
- Mark packet stale when event freshness exceeds configured thresholds.
- Mark U2 full-book liquidity unavailable when sequence validation fails.
- Mark funding z-score unavailable until enough funding history exists.

## ClickHouse Schema

Required V1 tables:

```text
symbols
klines_1m
mark_funding_sample
depth_features_1s
features_1m
latest_packets
```

`symbols` stores exchange metadata and trading rules.

`klines_1m` stores confirmed 1m candles and repair/backfill source metadata.

`mark_funding_sample` stores sampled mark, index, basis, funding, and next funding time.

`depth_features_1s` stores derived depth features, not raw order book deltas.

`features_1m` stores the primary research and replay feature record.

`latest_packets` stores latest JSON packets for audit and restart inspection.

Raw depth updates are not stored by default in V1.

## Configuration

Default config:

```yaml
binance:
  market_ws_base: "wss://fstream.binance.com/market"
  public_ws_base: "wss://fstream.binance.com/public"
  rest_base: "https://fapi.binance.com"

universe:
  quote_assets: ["USDT"]
  contract_type: "PERPETUAL"
  include_status: ["TRADING"]
  active_n: 15
  focus_n: 3
  refresh_sec: 300
  hysteresis_rank_buffer: 5
  always_focus: ["BTCUSDT", "ETHUSDT", "SOLUSDT"]

streams:
  mark_all:
    enabled: true
  ticker_all:
    enabled: true
  liquidation_all:
    enabled: true
  active_kline:
    interval: "1m"
  active_depth:
    levels: 20
    speed: "500ms"
  focus_full_depth:
    speed: "500ms"
    snapshot_limit: 1000
    resync_cooldown_sec: 3
  focus_agg_trade:
    enabled: false

rest_scheduler:
  max_weight_per_min: 900
  bootstrap_concurrency: 4
  repair_concurrency: 2
  funding_history_refresh_min: 30
  oi_refresh_min: 5

storage:
  clickhouse_url: "http://localhost:8123"
  database: "perp_radar"
  batch_rows: 2000
  batch_interval_ms: 1000
  depth_features_interval_ms: 1000
  mark_sample_interval_ms: 3000

packets:
  compact_interval_ms: 1000
  standard_interval_ms: 1000
  full_interval_ms: 5000
  topk_refresh_ms: 1000

api:
  bind: "127.0.0.1:8080"
```

## Reliability

WebSocket reliability:

- Roll connections before 24h expiry, defaulting to 23h30m.
- Reconnect with exponential backoff.
- Warm new connections before closing old connections when feasible.
- Route combined stream events by stream name.
- Respect control-message budgets when changing subscriptions.

REST reliability:

- Use a global token bucket.
- Track Binance weight headers.
- Back off on `429`.
- Circuit-break on `418`.
- Keep bootstrap, repair, and slow metrics in separate queues.

Data repair:

- Detect missing 1m candle open times and repair via REST klines.
- Recalculate affected features after kline repair.
- Detect full book sequence gaps via `U/u/pu`.
- Mark full book stale and resync on sequence gaps.
- Remove symbols from active/focus pools when `exchangeInfo` status is no longer `TRADING`.

Backpressure:

- P0 data is retried and not voluntarily dropped: closed klines, funding history, full-book resync, liquidation events.
- P1 data can be coalesced: mark price, ticker, slow REST metrics.
- P2 data can be dropped or reduced to latest-per-symbol: partial depth, open kline updates, book ticker.

## Monitoring

V1 exposes health/debug JSON. Prometheus can be added after the core API is stable.

Metrics in `/v1/health` and debug endpoints:

- WebSocket connected state by connection.
- WebSocket reconnect counts.
- Message rates by connection.
- WebSocket lag percentiles.
- Parser error count.
- Queue depths.
- Dropped lossy update counts.
- Kline gap count.
- Book resync count.
- REST 429 and 418 counts.
- REST used weight.
- ClickHouse insert latency.
- Packet build latency.
- Packet cache age.
- Warm and stale symbol counts.

## Testing

Implementation will follow TDD.

Test groups:

- Parser tests: Binance payloads become normalized events.
- State tests: kline close handling, gap detection, ring buffers, partial books, and full-book sequence updates.
- Feature tests: TA indicators, liquidity, funding z-score, and quality null reasons.
- Packet tests: missing inputs produce `null` plus explicit reasons.
- Storage tests: migration SQL and ClickHouse repositories.
- API tests: packet, top, export, and debug routes read cache state.
- Integration smoke test: replay a small mock stream, start the service, and request `/v1/packets/top`.

## Acceptance Criteria

V1 is complete when:

- The service starts only when ClickHouse is reachable and migrations succeed.
- The service can bootstrap a light universe from Binance.
- U0, U1, and U2 streams run concurrently.
- At least BTCUSDT, ETHUSDT, and SOLUSDT produce standard packets.
- `/v1/packet/{symbol}` returns from memory cache.
- `/v1/packets/top` returns ranked packet summaries.
- `/v1/export/top.txt` returns an LLM-readable overview.
- Missing indicators are represented as `null` with reasons.
- Full-book gaps mark liquidity stale and trigger resync.
- Core parser, state, feature, packet, storage, and API tests pass.

