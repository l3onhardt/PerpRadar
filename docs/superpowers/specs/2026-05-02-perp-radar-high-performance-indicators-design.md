# PerpRadar High Performance Indicators Design

## 0. Purpose

This document replaces a normal "score iteration" framing with a high-performance indicator pipeline design.

The goal is to make PerpRadar continuously compute unusual, audit-friendly derivatives-market indicators without the resource blowups and silent degradation that are common in Python implementations that repeatedly rescan large arrays.

The first implementation target is Packet `2.1` with formal indicators:

- `LRI` - Liquidity Risk Index, using the user-defined formula and direction.
- `TCS` - Trend-Compression Score.
- `DPI5` / `DPI10` - Depth Pressure Imbalance.
- `CSI` - Carry Stress Index.
- `RPI` - Reversal Pressure Index.
- `VoV` - Volatility of Volatility.

Packet output must not tell a trader whether to trade. It only emits indicator values, audit metadata, missing reasons, and source quality.

## 1. Current State

The repository is stable enough to use as a runtime base. `cargo test --workspace` passes, local mock Binance support is present, and ClickHouse migrations exist.

The indicator layer is not yet correct for the target definitions:

- Packet schema is `2.0`.
- `scores.TCS` is a candidate ranking score, not trend-compression.
- `scores.LRI` is based on liquidation notional, not liquidity condition.
- `scores.CSI` is compression, not carry stress.
- `scores.RPI` is absolute return, not reversal pressure.
- `scores.VoV` is volume spike, not volatility-of-volatility.
- `scores.DPI5` reuses notional top-5 imbalance, not user-defined qty imbalance.
- `DPI10`, `score_meta`, and `legacy_scores` are missing.
- Runtime currently refreshes packets and recomputes universe ranking on many events, which is acceptable for V1 but risky for high-frequency unique indicators.
- ClickHouse is required and migrations exist, but the runtime does not yet insert feature rows or latest packets.

## 2. Design Principles

### 2.1 Formula Fidelity

Do not "improve" the user formulas by changing signs, replacing inputs, or filling missing values with zero.

If a score cannot be computed, output `null` and a concrete `score_meta.<score>.missing` list.

### 2.2 High-Performance Computation

Indicators must be maintained incrementally where possible:

- Fixed-size ring buffers for rolling windows.
- No long-lived unbounded vectors for score histories.
- No full candle scans on every depth update.
- No packet rebuild on every raw event when a fixed output cadence is enough.
- No REST calls in steady-state computation except bootstrap and recovery.

For a 120-sample window, exact median/MAD/percentile calculation by copying and sorting a small fixed window is acceptable. This is bounded, deterministic, and much cheaper than Python dataframe-style recomputation.

### 2.3 Auditability

Every formal score must be reproducible from `score_meta`:

- source fields
- component values
- window length
- robust z statistics
- formula string
- missing reasons
- direction notes where the name can be misread

### 2.4 Packet Output Discipline

Packet output is not a signal engine. It must not say "trade", "do not trade", "long", "short", "safe", or "unsafe".

It may expose the indicator state and data quality needed by downstream users.

## 3. Binance Constraints

Binance USD-M Futures constraints shape the implementation.

- Market streams and public streams are separated in the current codebase: mark price, ticker, and liquidation events use market streams; depth uses public streams.
- WebSocket sessions should be treated as 24-hour rolling sessions, with planned reconnect and gap handling.
- Binance documents a 10 incoming-messages-per-second control limit and a combined-stream scale limit. The runtime must avoid subscription churn and excessive ping/subscription traffic.
- REST depth snapshots are weighted and should be used for bootstrap and full-book resync, not as the normal indicator feed.
- Partial book streams expose only top `5`, `10`, or `20` levels; full depth requires REST snapshot plus diff book depth stream.

Official references:

- Binance USD-M Futures WebSocket connect rules: https://developers.binance.com/docs/derivatives/usds-margined-futures/websocket-market-streams/Connect
- Binance REST order book: https://developers.binance.com/docs/derivatives/usds-margined-futures/market-data/rest-api/Order-Book
- Binance partial book depth streams: https://developers.binance.com/docs/derivatives/usds-margined-futures/websocket-market-streams/Partial-Book-Depth-Streams
- Binance diff book depth streams: https://developers.binance.com/docs/derivatives/usds-margined-futures/websocket-market-streams/Diff-Book-Depth-Streams
- Binance funding history: https://developers.binance.com/docs/derivatives/usds-margined-futures/market-data/rest-api/Get-Funding-Rate-History

## 4. Target Runtime Shape

### 4.1 Data Planes

Use four separate planes:

1. **Ingest plane**
   Parse Binance events, validate numeric fields, update latest state.

2. **Feature plane**
   Maintain incremental features and rolling component histories per symbol.

3. **Score plane**
   Compute formal scores from feature snapshots and fixed windows.

4. **Output plane**
   Emit packet/cache/storage updates on configured cadence.

The score plane must not call Binance APIs. It consumes local state only.

### 4.2 Cadence

Recommended cadences:

- Raw WS ingest: as received.
- Book feature update: on depth event, but only update bounded book-derived state.
- Candle feature update: on closed 1m candle.
- Score evaluation: every `scores.interval_ms`, default `1000`.
- Packet emission: every `packets.standard_interval_ms`, default `1000`.
- ClickHouse `latest_packets`: every packet emission.
- ClickHouse score/features table: every score emission or every 1m depending on storage budget.

This prevents full packet rebuilds from being driven directly by depth event rate.

### 4.3 State Additions

Add a score history state in `crates/state` or `crates/features` with explicit bounded buffers:

```rust
pub struct ScoreHistoryState {
    pub neg_spread_bp: RingWindow,
    pub liq_5bp_usd: RingWindow,
    pub neg_slip_bp: RingWindow,
    pub lri_raw: RingWindow,
    pub adx14: RingWindow,
    pub ema50_slope: RingWindow,
    pub bb_width_pctile: RingWindow,
    pub abs_fundz_7d: RingWindow,
    pub abs_basis_bp: RingWindow,
    pub rsi_extreme: RingWindow,
    pub funding_same_side: RingWindow,
    pub book_against_move: RingWindow,
    pub atr_delta_ratio: RingWindow,
}
```

`RingWindow` should store only finite values and expose:

- `push(value)`
- `len()`
- `values_recent()`
- `stats(current, min_samples, z_clip)`
- `percentile_rank(current)`

Use dynamic maps only if the code remains simple. Typed fields are preferred because these indicators are audit-sensitive.

## 5. Robust Z-Score

All z-score components use the same robust calculation:

```text
median = median(history)
mad = median(abs(x_i - median))
scale = 1.4826 * mad

if history_n < min_samples:
    return null

if scale == 0:
    fallback to sample stddev

if stddev == 0:
    return null

z = (current - median) / scale
z = clamp(z, -z_clip, z_clip)
```

Default config:

```yaml
scores:
  robust_window: 120
  min_samples: 30
  z_clip: 5.0
  interval_ms: 1000
  lri_slip_notional_usd: 10000
  ema50_slope_lookback_bars: 10
```

Important null rule:

```text
0 = real computed zero
null = missing, insufficient history, stale source, failed prerequisite, or untrusted quality
```

## 6. Legacy Score Renaming

Move existing score meanings to `legacy_scores`. Do not delete them in the first migration.

```json
"legacy_scores": {
  "candidate_score": null,
  "liquidation_event_score": null,
  "compression_score": null,
  "momentum_abs_score": null,
  "volume_spike_z": null,
  "notional_imbalance_i5": null
}
```

Mapping:

- old `TCS` -> `candidate_score`
- old `LRI` -> `liquidation_event_score`
- old `CSI` -> `compression_score`
- old `RPI` -> `momentum_abs_score`
- old `VoV` -> `volume_spike_z`
- old `DPI5` -> `notional_imbalance_i5`

Formal `scores` must contain only Score V2 definitions.

## 7. Packet Schema 2.1

Add:

```json
"scores": {
  "LRI": null,
  "TCS": null,
  "DPI5": null,
  "DPI10": null,
  "CSI": null,
  "RPI": null,
  "VoV": null
},
"score_meta": {},
"legacy_scores": {}
```

Add chart fields:

```json
"chart": {
  "ema_200": null,
  "ema50_slope": null,
  "bb_width_pctile": null,
  "atr_1h_pct": null,
  "atr_1h_pct_prev": null,
  "atr_1h_pct_delta_ratio": null
}
```

Do not output trade decisions.

## 8. Indicator Details

## 8.1 LRI - Liquidity Risk Index

LRI is the most sensitive indicator in this design.

The user-defined formula is:

```text
LRI_raw =
  0.4 * z(-spread_bp)
+ 0.3 * z(liq_5bp_usd)
+ 0.3 * z(-slip_bp)

LRI = robust_zscore(LRI_raw)
```

Do not invert the formula.

### 8.1.1 Direction

Although the name contains "Risk", the specified formula makes higher values correspond to tighter spread, deeper visible liquidity, and lower modeled slippage relative to recent history.

Therefore Packet `score_meta.LRI.direction` must state:

```json
"direction": "higher means stronger observed liquidity / lower immediate execution friction under the defined formula"
```

Do not reinterpret high LRI as high risk inside Packet output.

### 8.1.2 Source Discipline

LRI must be computed from a consistent book source.

Preferred source:

- full book snapshot + diff book stream
- `quality.book_mode == "full"`
- `quality.book_seq_ok == true`

Required fields:

- full-book spread in bp
- `liq_5bp_usd`
- buy slippage for configured notional
- sell slippage for configured notional

`slip_bp`:

```text
slip_bp = max(slip_10000_buy_bp, slip_10000_sell_bp)
```

If full book is missing or sequence quality is false, LRI is `null`.

Do not compute LRI from liquidation events. Liquidations belong in `events` and `legacy_scores.liquidation_event_score`.

### 8.1.3 Why LRI Is Hard

LRI can degrade silently if any of these are mishandled:

- spread comes from partial depth but liquidity/slippage comes from full depth
- REST snapshot is stale but diff stream still updates another sequence
- slippage notional is too small for large symbols or too large for illiquid symbols
- `liq_5bp_usd` is treated as "risk" without considering formula direction
- z-score history mixes cold-start, partial-book, and full-book periods
- missing slippage is replaced by zero
- full book is rebuilt by allocating large vectors on every event

The implementation must keep LRI component histories only when all source prerequisites are trusted. If the book downgrades, do not push degraded component values into the normal LRI history.

### 8.1.4 Performance Plan

Extend `FullBook` with bounded top-level and within-bp iterators:

- `spread_bp()`
- `top_qty_imbalance(n)`
- `visible_liquidity_usd(max_distance_bp)`
- `slippage_bp_for_notional(notional, side)`

Avoid allocating full depth arrays. Iterate the `BTreeMap` ranges already held by `FullBook`.

On each score tick:

1. Read current full-book features.
2. Validate full-book quality.
3. Compute `neg_spread_bp`, `liq_5bp_usd`, `neg_slip_bp`.
4. Push only valid components into their windows.
5. Compute robust z for each component.
6. Compute `LRI_raw`.
7. Push `LRI_raw` into its own window only if all component z values exist.
8. Compute final robust z of `LRI_raw`.

### 8.1.5 LRI Meta

```json
"score_meta": {
  "LRI": {
    "available": true,
    "formula": "robust_z(0.4*z(-spread_bp)+0.3*z(liq_5bp_usd)+0.3*z(-slip_bp))",
    "direction": "higher means stronger observed liquidity / lower immediate execution friction under the defined formula",
    "book_source": "full",
    "slip_notional_usd": 10000,
    "raw": 0.73,
    "z": 0.88,
    "components": {
      "neg_spread_bp": {},
      "liq_5bp_usd": {},
      "neg_slip_bp": {}
    },
    "missing": []
  }
}
```

Required missing reasons:

- `book_not_full`
- `book_seq_not_ok`
- `spread_bp_missing`
- `liq_5bp_usd_missing`
- `slip_buy_missing`
- `slip_sell_missing`
- `component_window_insufficient`
- `lri_raw_window_insufficient`

## 8.2 TCS - Trend-Compression Score

Formula:

```text
TCS =
  z(ADX14) * sign(close - EMA200)
+ 0.5 * z(ema50_slope)
- 0.5 * z(BB_width_pctile)
```

No second z-score in Packet `2.1`.

Required chart fields:

- `close`
- `ema_50`
- `ema_200`
- `ema50_slope`
- `bb_width_pctile`
- `adx_14`

`ema50_slope`:

```text
ema50_slope = (ema50_now - ema50_N_bars_ago) / ema50_N_bars_ago
```

`bb_width_pctile` should be computed from a bounded rolling window of Bollinger widths, not from all historical candles.

Performance note:

- EMA200 should be maintained incrementally or computed only on closed candles.
- Do not rescan 1500 candles on every packet tick.

Missing reasons:

- `close_missing`
- `ema200_missing`
- `ema50_slope_missing`
- `bb_width_pctile_missing`
- `adx14_missing`
- `component_window_insufficient`

## 8.3 DPI5 / DPI10 - Depth Pressure Imbalance

Formula:

```text
DPI_N =
  (sum_bid_qty_top_N - sum_ask_qty_top_N)
  / (sum_bid_qty_top_N + sum_ask_qty_top_N)
```

`N = 5` and `N = 10`.

This is qty imbalance, not notional imbalance.

Preferred source:

- full book for focus symbols
- partial depth20 can be an allowed degraded source only if `score_meta` marks `book_source = "partial20"`

For Packet `2.1`, formal scoring uses full book only. Partial-depth fallback can be added in a future schema only with explicit config and meta.

Required `FullBook` API:

```rust
pub fn qty_imbalance_top_n(&self, n: usize) -> Option<DepthQtyImbalance>
```

Meta components:

- `bid_qty_topN`
- `ask_qty_topN`
- `all_qty_topN`
- `book_source`

Missing reasons:

- `depth_array_missing`
- `bid_depth_lt_N`
- `ask_depth_lt_N`
- `all_qty_zero`

## 8.4 CSI - Carry Stress Index

Formula:

```text
CSI = z(abs(funding_z_7d)) + 0.5 * z(abs(basis_bp))
```

Inputs:

- `carry.funding_z_7d`
- `price.basis_bp`

Performance note:

- Funding history changes slowly; do not recompute funding z on every depth event.
- Update funding z when mark price/funding updates or funding history changes.

Missing reasons:

- `funding_z_7d_missing`
- `basis_bp_missing`
- `component_window_insufficient`

## 8.5 RPI - Reversal Pressure Index

Formula:

```text
rsi_extreme = abs(RSI14 - 50)

funding_same_side =
  max(0, sign(RSI14 - 50) * funding_z_7d)

book_against_move =
  max(0, -sign(ret_1h) * I1)

RPI =
  z(rsi_extreme)
+ z(funding_same_side)
+ z(book_against_move)
```

Inputs:

- `chart.rsi_14`
- `carry.funding_z_7d`
- `price.ret_1h`
- `liquidity.i1`

Important:

- RPI is a pressure index only.
- Do not output a reversal signal or trade direction.

Missing reasons:

- `rsi14_missing`
- `funding_z_7d_missing`
- `ret_1h_missing`
- `i1_missing`
- `component_window_insufficient`

## 8.6 VoV - Volatility of Volatility

Formula:

```text
atr_delta_ratio = (atr_1h_pct_now - atr_1h_pct_prev) / atr_1h_pct_prev
VoV = z(atr_delta_ratio)
```

Use ATR pct consistently. Do not mix raw ATR and ATR pct in the same formula.

Inputs:

- `chart.atr_1h_pct`
- `chart.atr_1h_pct_prev`
- `chart.atr_1h_pct_delta_ratio`

Implementation:

- Derive 1h ATR from 1m candles using a stable rolling representation or closed 1h aggregation.
- Store previous ATR pct explicitly.
- Do not use `events.volume_spike_z`.

Missing reasons:

- `atr_1h_pct_missing`
- `atr_1h_pct_prev_missing`
- `atr_1h_pct_prev_non_positive`
- `atr_delta_ratio_window_insufficient`

## 9. Storage Plan

Storage must support replay and audit without storing every raw depth delta.

P0:

- Write `latest_packets` on packet emission.
- Add a `score_features_1s` or evolve `features_1m` after schema 2.1 is finalized.

P1:

- Store score component snapshots with meta JSON.
- Store enough chart/liquidity/carry fields to reproduce scores.

Do not block score computation on ClickHouse insert latency. Use a bounded channel and batch writer.

If writer backpressure appears:

- keep latest packet cache live
- drop or coalesce storage rows according to config
- add quality/health counters

## 10. Implementation Units

Suggested files:

- `crates/features/src/robust.rs`
- `crates/features/src/score_meta.rs`
- `crates/features/src/scores_v2.rs`
- `crates/features/src/legacy_scores.rs`
- `crates/state/src/score_history.rs`
- `crates/state/src/book_full.rs` extensions for qty imbalance and full-book spread
- `crates/core/src/packet.rs` schema 2.1 structs
- `crates/features/src/packet_builder.rs` split into feature snapshot, legacy scores, formal scores, and meta
- `tools/live-monitor.py` updated for schema 2.1 null/meta rules

## 11. Testing Requirements

### 11.1 Robust Stats

- normal robust z
- MAD zero fallback to stddev
- stddev zero returns null
- insufficient samples returns null
- clipping works
- percentile works

### 11.2 LRI

- uses `-spread_bp`
- uses `liq_5bp_usd`
- uses `-slip_bp`
- uses max buy/sell slippage
- computes `LRI_raw`
- computes final robust z from `LRI_raw`
- does not use liquidation events
- null when full book missing
- null when sequence not ok
- does not push degraded book values into LRI history
- meta includes direction, raw, components, book source, missing reasons

### 11.3 TCS

- trend sign positive above EMA200
- trend sign negative below EMA200
- `ema50_slope` required
- `bb_width_pctile` required
- no second z-score

### 11.4 DPI

- DPI5 uses qty, not notional
- DPI10 uses qty, not notional
- zero total qty returns null
- missing depth returns null
- source is marked in meta

### 11.5 CSI

- uses absolute funding z
- uses absolute basis
- basis weight is `0.5`
- missing funding or basis returns null

### 11.6 RPI

- RSI extreme is `abs(rsi - 50)`
- funding same-side formula is exact
- book against move formula is exact
- missing `ret_1h` or `i1` returns null
- meta includes source fields

### 11.7 VoV

- uses ATR pct delta ratio
- previous ATR required
- previous ATR <= 0 returns null
- does not use volume spike

### 11.8 Performance

Add focused tests or benchmarks for:

- score tick does not allocate unbounded histories
- packet emission can run without rescanning full candle history for every depth event
- full-book DPI and LRI helpers iterate bounded top/range levels
- bounded channels do not block packet cache updates

## 12. Acceptance

Required command:

```bash
cargo test --workspace
```

Required API checks after runtime starts:

```bash
curl http://127.0.0.1:18080/v1/health
curl "http://127.0.0.1:18080/v1/packets/top?limit=3"
curl "http://127.0.0.1:18080/v1/packet/BTCUSDT?profile=full"
```

Schema checks:

- `packet_schema == "2.1"`
- formal `scores` contains `DPI10`
- `legacy_scores` exists
- `score_meta` exists
- every null formal score has non-empty `missing`
- every non-null formal score has `available == true`
- `scores.LRI` is not liquidation-based
- `scores.CSI` is not compression-based
- `scores.RPI` is not absolute return
- `scores.VoV` is not volume spike
- `scores.DPI5` is not notional `liquidity.i5`

## 13. P0 Decisions

Use these decisions for Packet `2.1` implementation:

1. DPI partial-depth fallback is disabled for formal `scores`.
2. `score_meta` component stats are present in standard packets; full timeseries appears only for `profile=full`.
3. Score V2 snapshots are stored at 1s cadence if storage writer is implemented in this cycle; otherwise `latest_packets` is the required first storage target.
4. LRI notional remains fixed at `$10,000` in Packet `2.1` through `scores.lri_slip_notional_usd`.

P0 should keep the formula fixed and the implementation conservative.
