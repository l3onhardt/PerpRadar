# Perp Radar

**LLM-ready market intelligence packets for Binance USDⓈ-M perpetual futures.**

Perp Radar turns noisy exchange streams into compact, auditable, machine-readable market packets. Each packet combines price action, technical state, order-book liquidity, carry, liquidation pressure, open interest, order flow, formal indicators, and data-quality metadata for one symbol.

It is built for agents, dashboards, monitors, and research pipelines that need a clean market snapshot without replaying raw websocket feeds.

> Perp Radar emits indicators and explanations. It does **not** emit trade instructions.

---

## What It Does

Perp Radar runs a live futures radar loop:

1. Ingest Binance Futures websocket streams and REST backfills.
2. Maintain per-symbol state for prices, candles, books, funding, liquidations, and open interest.
3. Rank the broad market into active/focus universes.
4. Build Packet `2.1` snapshots for the top symbols.
5. Serve packets through HTTP and persist packet/features data into ClickHouse.
6. Preserve nulls and missing reasons instead of fabricating values when inputs are unavailable.

```text
Binance Futures
  ├─ websocket streams: tickers, mark price, force orders, klines, depth
  ├─ REST bootstrap: exchange info, klines, depth snapshots, funding, premium index, OI
  ↓
perp-radar-binance
  ↓ parsed events
perp-radar-state ── bounded histories / book state / quality state
  ↓
perp-radar-features ── Packet 2.1 features + formal indicators
  ↓
perp-radar-api ── /v1 packet, top, export, debug routes
  ↓
ClickHouse storage + LLM/dashboards/monitoring agents
```

---

## Workspace Architecture

This is a Rust workspace split by responsibility:

- `crates/app` — application entrypoint, config loading, runtime orchestration, ingestion tasks, API server startup, and ClickHouse writer wiring.
- `crates/binance` — Binance Futures REST client, websocket stream URL builder, stream parser, reconnect/rate-limit helpers.
- `crates/state` — per-symbol market state: candles, books, funding history, liquidation windows, open interest, freshness, and sequence quality.
- `crates/features` — packet builder, ranking model, technical indicators, robust/z-score utilities, and Packet `2.1` score computation.
- `crates/core` — shared packet schema, quality model, market types, time utilities, and public data contracts.
- `crates/api` — Axum HTTP routes, packet cache, text/JSONL export helpers, and debug endpoints.
- `crates/storage` — ClickHouse migrations, row mapping, batching, dedupe, and async storage writer.
- `config/default.yaml` — default Binance, universe, API, runtime, and storage configuration.
- `docs/` — data contract, indicator definitions, operations notes, runbook, and handoff material.
- `tools/` and `scripts/` — local monitoring and watchdog helpers.

---

## Runtime Logic

Perp Radar uses a layered universe model:

- **U0 radar layer** watches broad-market signals such as all-market mark/ticker/force-order streams.
- **Active universe** promotes the strongest ranked symbols for deeper processing.
- **Focus universe** pins the highest-priority symbols for richer backfills and higher-quality packet construction.
- **Always-focus symbols** stay pinned even when short-term ranking is volatile.

The runtime favors latest-state semantics. High-frequency book updates may be coalesced, while packets carry `quality` and `score_meta` fields so consumers can tell whether an indicator is complete, warming, stale, partial, or unavailable.

---

## Packet Schema

Current development packet schema: **`2.1`**.

A standard packet includes:

- `price` — latest/mark/index price, basis, and recent returns.
- `chart` — candle-derived state: EMA20/50/200, EMA50 slope, RSI14, MACD histogram, ATR, Bollinger width, ADX14, VWAP20, CMF20, regime, and signature.
- `liquidity` — book mode, spread, top-level imbalance, microprice, visible liquidity, and estimated buy/sell slippage.
- `carry` — funding rate, funding interval, funding z-score, and next funding time.
- `events` — liquidation notional windows, liquidation side, and volume spike context.
- `structure` — Donchian 20 high/low market-structure levels.
- `derivatives` — open interest, OI notional, OI z-score, OI 5-minute delta, and crowded-side context.
- `orderflow` — order-flow imbalance snapshots over immediate, 1-minute, and 5-minute windows.
- `scores` — formal Packet `2.1` indicators.
- `score_meta` — formula, direction, components, book source, notional assumptions, and missing reasons for each score.
- `legacy_scores` — old Packet `2.0` score meanings kept under explicit names during migration.
- `quality` — freshness, source quality, book-sequence state, warmup status, and reason codes.

`null` means unknown or not computable. It does **not** mean zero, neutral, safe, or false.

---

## Market Inputs And Features

Perp Radar combines multiple market-input families:

- **Price and returns** — 1m, 5m, 15m, and 1h return windows.
- **Chart features** — momentum, trend, compression, volatility, and candle signatures.
- **Liquidity features** — spread, book imbalance, microprice, depth liquidity, slippage, and sequence trust.
- **Carry features** — funding, basis, and normalized carry pressure.
- **Event features** — liquidation pressure, liquidation side, volume spike, stream gaps, and freshness degradation.
- **Structure features** — Donchian-style breakout/range context.
- **Derivatives features** — open interest level, notional exposure, OI z-score, OI delta, and crowding hints.
- **Orderflow features** — immediate and rolling order-flow imbalance.

---

## Five Unique Indicators

Packet `2.1` exposes seven formal scores: `LRI`, `TCS`, `DPI5`, `DPI10`, `CSI`, `RPI`, and `VoV`. These are not opaque signals; every score has metadata explaining formula, direction, components, and missing prerequisites.

### 1. `LRI` — Liquidity Risk Index

Measures immediate execution friction using trusted full-book data:

- spread pressure
- visible liquidity within 5 basis points
- estimated buy/sell slippage for the configured notional

Higher `LRI` means stronger observed liquidity / lower immediate execution friction under the defined formula. If full-book data is not trusted, the value is `null` with a missing reason.

### 2. `TCS` — Trend-Compression Score

Combines trend strength and compression context:

- `ADX14`
- trend sign versus `EMA200`
- `EMA50` slope
- Bollinger width percentile

It helps identify symbols where trend structure and volatility compression/expansion are meaningful enough to inspect.

### 3. `CSI` — Carry Stress Index

Captures perp carry stress from:

- absolute funding z-score
- absolute basis pressure

It highlights symbols where positioning cost or basis dislocation may be unusually important.

### 4. `RPI` — Reversal Pressure Index

Estimates reversal pressure by combining:

- RSI extreme
- same-side funding pressure
- order-book pressure against the 1h move

It is designed to explain when a directional move may be crowded, stretched, or facing book resistance.

### 5. `VoV` — Volatility Of Volatility

Tracks volatility regime change using ATR percent delta ratio, not volume spike. It focuses on whether realized volatility itself is expanding or contracting unusually fast.

> `DPI5` and `DPI10` are also formal Packet `2.1` indicators. They measure trusted full-book top-5/top-10 quantity imbalance and are especially useful for order-book pressure analysis.

---

## HTTP API

Default local bind in examples: `127.0.0.1:18080`.

- `GET /v1/health` — process health.
- `GET /v1/schema` — packet schema version and available routes.
- `GET /v1/universe` — current active/focus universe.
- `GET /v1/symbols` — cached packet symbols.
- `GET /v1/packet/:symbol` — latest packet for one symbol.
- `GET /v1/packets/top?limit=10` — top-ranked packets.
- `GET /v1/export/top.txt?limit=5` — compact LLM-readable packet summaries.
- `GET /v1/export/top.jsonl?limit=5` — JSONL export for agents and pipelines.
- `GET /v1/debug/ws` — websocket runtime policy/status.
- `GET /v1/debug/rate_limits` — configured runtime rate-limit posture.

Example:

```bash
curl http://127.0.0.1:18080/v1/health
curl "http://127.0.0.1:18080/v1/export/top.txt?limit=3"
```

---

## Quick Start

### Docker Compose smoke run

The recommended local path starts ClickHouse, mock Binance fixtures, and Perp Radar:

```bash
docker compose up --build
```

Then check:

```bash
curl http://127.0.0.1:18080/v1/health
curl http://127.0.0.1:18080/v1/schema
curl "http://127.0.0.1:18080/v1/export/top.txt?limit=1"
```

### Direct cargo run

Use this when ClickHouse is already available:

```bash
PERP_RADAR__API__BIND=127.0.0.1:18080 cargo run -p perp-radar
```

The binary verifies storage connectivity, applies migrations, logs configured websocket URLs, starts ingestion, and serves the HTTP API.

---

## Validation

Common development checks:

```bash
cargo fmt --all -- --check
cargo test --workspace
```

The test suite covers config contracts, runtime ingestion behavior, API routes, Binance parsers/URLs, packet schema, indicator behavior, robust statistics, state transitions, and storage mapping/migrations.

---

## Design Principles

- **Explainability over magic** — packets expose components and formulas, not just scores.
- **Nulls over fake certainty** — unavailable inputs remain `null` with concrete missing reasons.
- **Latest-state semantics** — suitable for live monitoring where every intermediate book update is less important than trusted current state.
- **LLM-friendly output** — text and JSONL exports are designed for agents and summaries.
- **Operational safety** — REST budget, reconnect behavior, ClickHouse migrations, and debug routes are first-class runtime concerns.

---

## Documentation

- `docs/DATA_CONTRACT.md` — packet schema and null semantics.
- `docs/INDICATORS.md` — indicator families and Packet `2.1` formal scores.
- `docs/OPERATIONS.md` — startup, Binance budget, runtime behavior, and validation notes.
- `docs/RUNBOOK.md` — local commands and API smoke checks.
- `docs/HANDOFF.md` — current branch/runtime handoff notes.
