# PerpRadar Indicators P0 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the first Packet 2.1 vertical slice for audit-friendly high-performance indicators: schema 2.1 output, formal score placeholders/meta, legacy score relocation, robust bounded stats, and full-book quantity imbalance/spread helpers.

**Architecture:** Keep existing runtime flow intact and add a bounded indicator foundation that is safe to consume from `packet_builder`. The first slice preserves old score meanings under `legacy_scores`, makes formal `scores` follow Packet 2.1 names including `DPI10`, and emits `score_meta` missing reasons instead of silently treating unavailable formulas as zero.

**Tech Stack:** Rust 2021 workspace, serde JSON packet contracts, existing `perp-radar-core`, `perp-radar-state`, and `perp-radar-features` crates, `cargo test`.

---

## File Structure

- `crates/core/src/packet.rs`: Add Packet 2.1 fields and serializable score metadata structures.
- `crates/core/tests/packet_contract.rs`: Contract tests for Packet 2.1 JSON shape.
- `crates/state/src/book_full.rs`: Add non-allocating full-book `spread_bp` and qty imbalance helpers.
- `crates/state/tests/book_state.rs`: Tests for full-book spread and `DPI5`/`DPI10` qty imbalance prerequisites.
- `crates/features/src/robust.rs`: Add fixed-size `RingWindow` and robust z/percentile helpers.
- `crates/features/src/lib.rs`: Export `robust`.
- `crates/features/src/packet_builder.rs`: Emit Packet 2.1, formal score/meta/null discipline, legacy score mapping, full-book DPI values, and new chart fields where currently derivable.
- `crates/features/tests/packet_builder_contract.rs`: Contract tests for Packet 2.1 builder behavior.
- `crates/api/src/cache.rs`, `crates/api/src/routes.rs`, `crates/api/tests/api_contract.rs`: Update default/schema examples to Packet 2.1.
- `crates/api/src/export.rs`: Keep export compatible with `DPI10` absent from text output or add it explicitly.
- `tools/live-monitor.py`, `tools/validate-live-indicators.py`: Accept Packet 2.1 `score_meta`, `legacy_scores`, and `DPI10`.
- `docs/DATA_CONTRACT.md`, `docs/INDICATORS.md`: Document schema 2.1 packet fields and legacy score relocation.

## Task 1: Core Packet 2.1 JSON Shape

**Files:**
- Modify: `crates/core/src/packet.rs`
- Modify: `crates/core/tests/packet_contract.rs`

- [ ] **Step 1: Write the failing Packet 2.1 serialization test**

Add this test to `crates/core/tests/packet_contract.rs`:

```rust
#[test]
fn packet_21_serializes_formal_scores_meta_and_legacy_scores() {
    let mut score_meta = std::collections::BTreeMap::new();
    score_meta.insert(
        "LRI".to_string(),
        perp_radar_core::packet::ScoreMeta {
            available: false,
            formula: Some("robust_z(0.4*z(-spread_bp)+0.3*z(liq_5bp_usd)+0.3*z(-slip_bp))".to_string()),
            direction: Some("higher means stronger observed liquidity / lower immediate execution friction under the defined formula".to_string()),
            book_source: Some("full".to_string()),
            slip_notional_usd: Some(10_000.0),
            raw: None,
            z: None,
            components: serde_json::json!({}),
            missing: vec!["book_not_full".to_string()],
        },
    );

    let packet = StandardPacket {
        packet_schema: "2.1".to_string(),
        ts: Utc.with_ymd_and_hms(2026, 5, 2, 0, 0, 0).unwrap(),
        symbol: "BTCUSDT".to_string(),
        rank: 1,
        profile: PacketProfile::Standard,
        universe: UniverseBlock {
            tier: UniverseTier::U2,
            active_n: 15,
            focus_n: 3,
        },
        price: PriceBlock::default(),
        chart: ChartBlock {
            ema_200: Some(64100.0),
            ema50_slope: Some(0.012),
            bb_width_pctile: Some(0.42),
            atr_1h_pct: Some(0.018),
            atr_1h_pct_prev: Some(0.017),
            atr_1h_pct_delta_ratio: Some((0.018 - 0.017) / 0.017),
            ..ChartBlock::default()
        },
        liquidity: LiquidityBlock::default(),
        carry: CarryBlock::default(),
        events: EventsBlock::default(),
        scores: ScoresBlock {
            dpi10: Some(0.12),
            ..ScoresBlock::default()
        },
        score_meta,
        legacy_scores: LegacyScoresBlock {
            candidate_score: Some(0.81),
            liquidation_event_score: Some(1.2),
            compression_score: Some(0.3),
            momentum_abs_score: Some(0.04),
            volume_spike_z: Some(1.42),
            notional_imbalance_i5: Some(0.09),
        },
        quality: QualityState::cold("full"),
    };

    let json = serde_json::to_value(&packet).unwrap();

    assert_eq!(json["packet_schema"], "2.1");
    assert!(json["scores"]["DPI10"].is_number());
    assert!(json["scores"]["LRI"].is_null());
    assert_eq!(json["score_meta"]["LRI"]["missing"][0], "book_not_full");
    assert_eq!(json["legacy_scores"]["candidate_score"], 0.81);
    assert_eq!(json["chart"]["ema_200"], 64100.0);
    assert!(json["chart"]["atr_1h_pct_delta_ratio"].is_number());
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run:

```bash
cargo test -p perp-radar-core packet_21_serializes_formal_scores_meta_and_legacy_scores
```

Expected: FAIL because `ScoreMeta`, `LegacyScoresBlock`, `ScoresBlock::dpi10`, chart fields, and packet fields do not exist.

- [ ] **Step 3: Add Packet 2.1 structs and fields**

In `crates/core/src/packet.rs`:

```rust
use std::collections::BTreeMap;
```

Extend `ChartBlock`:

```rust
    pub ema_200: Option<f64>,
    pub ema50_slope: Option<f64>,
    pub bb_width_pctile: Option<f64>,
    pub atr_1h_pct: Option<f64>,
    pub atr_1h_pct_prev: Option<f64>,
    pub atr_1h_pct_delta_ratio: Option<f64>,
```

Extend `ScoresBlock`:

```rust
    #[serde(rename = "DPI10")]
    pub dpi10: Option<f64>,
```

Add:

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ScoreMeta {
    pub available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub formula: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direction: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub book_source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slip_notional_usd: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub z: Option<f64>,
    #[serde(default)]
    pub components: serde_json::Value,
    #[serde(default)]
    pub missing: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct LegacyScoresBlock {
    pub candidate_score: Option<f64>,
    pub liquidation_event_score: Option<f64>,
    pub compression_score: Option<f64>,
    pub momentum_abs_score: Option<f64>,
    pub volume_spike_z: Option<f64>,
    pub notional_imbalance_i5: Option<f64>,
}
```

Extend `StandardPacket`:

```rust
    pub score_meta: BTreeMap<String, ScoreMeta>,
    pub legacy_scores: LegacyScoresBlock,
```

- [ ] **Step 4: Run the core contract test**

Run:

```bash
cargo test -p perp-radar-core packet_21_serializes_formal_scores_meta_and_legacy_scores
```

Expected: PASS.

## Task 2: FullBook Formal Depth Helpers

**Files:**
- Modify: `crates/state/src/book_full.rs`
- Modify: `crates/state/tests/book_state.rs`

- [ ] **Step 1: Write failing tests for full-book spread and qty imbalance**

Add to `crates/state/tests/book_state.rs`:

```rust
#[test]
fn full_book_calculates_spread_bp_from_top_of_book() {
    let book = FullBook::from_snapshot(
        "BTCUSDT",
        10,
        vec![BookLevel { price: 100.0, qty: 10.0 }],
        vec![BookLevel { price: 100.1, qty: 6.0 }],
    );

    let expected = (100.1 - 100.0) / 100.05 * 10_000.0;

    assert!((book.spread_bp().unwrap() - expected).abs() < 0.0001);
}

#[test]
fn full_book_calculates_qty_imbalance_top_n_for_dpi() {
    let book = FullBook::from_snapshot(
        "BTCUSDT",
        10,
        vec![
            BookLevel { price: 100.0, qty: 10.0 },
            BookLevel { price: 99.9, qty: 5.0 },
            BookLevel { price: 99.8, qty: 1.0 },
        ],
        vec![
            BookLevel { price: 100.1, qty: 6.0 },
            BookLevel { price: 100.2, qty: 2.0 },
            BookLevel { price: 100.3, qty: 2.0 },
        ],
    );

    let dpi2 = book.qty_imbalance_top_n(2).unwrap();

    assert_eq!(dpi2.bid_qty_top_n, 15.0);
    assert_eq!(dpi2.ask_qty_top_n, 8.0);
    assert_eq!(dpi2.all_qty_top_n, 23.0);
    assert!((dpi2.imbalance - ((15.0 - 8.0) / 23.0)).abs() < 0.0001);
}

#[test]
fn full_book_qty_imbalance_is_none_when_depth_is_insufficient_or_zero() {
    let shallow = FullBook::from_snapshot(
        "BTCUSDT",
        10,
        vec![BookLevel { price: 100.0, qty: 10.0 }],
        vec![BookLevel { price: 100.1, qty: 6.0 }],
    );
    let zero = FullBook::from_snapshot(
        "BTCUSDT",
        10,
        vec![BookLevel { price: 100.0, qty: 0.0 }, BookLevel { price: 99.9, qty: 0.0 }],
        vec![BookLevel { price: 100.1, qty: 0.0 }, BookLevel { price: 100.2, qty: 0.0 }],
    );

    assert_eq!(shallow.qty_imbalance_top_n(2), None);
    assert_eq!(zero.qty_imbalance_top_n(2), None);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test -p perp-radar-state full_book_calculates_spread_bp_from_top_of_book full_book_calculates_qty_imbalance_top_n_for_dpi full_book_qty_imbalance_is_none_when_depth_is_insufficient_or_zero
```

Expected: FAIL because `spread_bp`, `DepthQtyImbalance`, and `qty_imbalance_top_n` do not exist.

- [ ] **Step 3: Implement helpers**

In `crates/state/src/book_full.rs`, add:

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DepthQtyImbalance {
    pub bid_qty_top_n: f64,
    pub ask_qty_top_n: f64,
    pub all_qty_top_n: f64,
    pub imbalance: f64,
}
```

Add methods in `impl FullBook`:

```rust
    pub fn spread_bp(&self) -> Option<f64> {
        let bid = self.best_bid()?;
        let ask = self.best_ask()?;
        let mid = (bid + ask) / 2.0;
        if mid <= 0.0 {
            return None;
        }
        Some((ask - bid) / mid * 10_000.0)
    }

    pub fn qty_imbalance_top_n(&self, n: usize) -> Option<DepthQtyImbalance> {
        if n == 0 || self.bids.len() < n || self.asks.len() < n {
            return None;
        }

        let bid_qty_top_n = self.bids.iter().rev().take(n).map(|(_, qty)| *qty).sum::<f64>();
        let ask_qty_top_n = self.asks.iter().take(n).map(|(_, qty)| *qty).sum::<f64>();
        let all_qty_top_n = bid_qty_top_n + ask_qty_top_n;
        if !all_qty_top_n.is_finite() || all_qty_top_n <= 0.0 {
            return None;
        }

        Some(DepthQtyImbalance {
            bid_qty_top_n,
            ask_qty_top_n,
            all_qty_top_n,
            imbalance: (bid_qty_top_n - ask_qty_top_n) / all_qty_top_n,
        })
    }
```

- [ ] **Step 4: Run state tests**

Run:

```bash
cargo test -p perp-radar-state book_state
```

Expected: PASS.

## Task 3: Robust Ring Window Foundation

**Files:**
- Create: `crates/features/src/robust.rs`
- Modify: `crates/features/src/lib.rs`
- Add: `crates/features/tests/robust_contract.rs`

- [ ] **Step 1: Write failing robust stats tests**

Create `crates/features/tests/robust_contract.rs`:

```rust
use perp_radar_features::robust::RingWindow;

#[test]
fn robust_stats_returns_none_until_min_samples() {
    let mut window = RingWindow::new(5);
    window.push(1.0);
    window.push(2.0);

    assert_eq!(window.stats(3.0, 3, 5.0), None);
}

#[test]
fn robust_stats_uses_mad_and_clips_z_score() {
    let mut window = RingWindow::new(5);
    for value in [1.0, 2.0, 3.0, 4.0, 5.0] {
        window.push(value);
    }

    let stats = window.stats(100.0, 5, 5.0).unwrap();

    assert_eq!(stats.n, 5);
    assert_eq!(stats.median, 3.0);
    assert_eq!(stats.z, 5.0);
}

#[test]
fn robust_stats_falls_back_to_stddev_when_mad_is_zero() {
    let mut window = RingWindow::new(5);
    for value in [1.0, 1.0, 1.0, 2.0, 3.0] {
        window.push(value);
    }

    let stats = window.stats(2.0, 5, 5.0).unwrap();

    assert!(stats.scale > 0.0);
    assert!(stats.z.is_finite());
}

#[test]
fn robust_stats_returns_none_when_all_history_values_are_equal() {
    let mut window = RingWindow::new(5);
    for _ in 0..5 {
        window.push(1.0);
    }

    assert_eq!(window.stats(2.0, 5, 5.0), None);
}

#[test]
fn ring_window_keeps_only_finite_recent_values() {
    let mut window = RingWindow::new(3);
    window.push(1.0);
    window.push(f64::NAN);
    window.push(2.0);
    window.push(3.0);
    window.push(4.0);

    assert_eq!(window.values_recent(), vec![2.0, 3.0, 4.0]);
}

#[test]
fn percentile_rank_counts_values_less_than_or_equal_current() {
    let mut window = RingWindow::new(5);
    for value in [10.0, 20.0, 30.0, 40.0] {
        window.push(value);
    }

    assert_eq!(window.percentile_rank(25.0), Some(0.5));
    assert_eq!(window.percentile_rank(40.0), Some(1.0));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test -p perp-radar-features --test robust_contract
```

Expected: FAIL because `robust` module does not exist.

- [ ] **Step 3: Implement `RingWindow` and robust stats**

Create `crates/features/src/robust.rs`:

```rust
use std::collections::VecDeque;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RobustStats {
    pub n: usize,
    pub median: f64,
    pub mad: f64,
    pub scale: f64,
    pub z: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RingWindow {
    capacity: usize,
    values: VecDeque<f64>,
}

impl RingWindow {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            values: VecDeque::with_capacity(capacity),
        }
    }

    pub fn push(&mut self, value: f64) {
        if self.capacity == 0 || !value.is_finite() {
            return;
        }
        if self.values.len() == self.capacity {
            self.values.pop_front();
        }
        self.values.push_back(value);
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn values_recent(&self) -> Vec<f64> {
        self.values.iter().copied().collect()
    }

    pub fn stats(&self, current: f64, min_samples: usize, z_clip: f64) -> Option<RobustStats> {
        if !current.is_finite() || self.values.len() < min_samples || min_samples == 0 {
            return None;
        }
        let values = self.values_recent();
        let median_value = median(values.clone())?;
        let deviations = values
            .iter()
            .map(|value| (value - median_value).abs())
            .collect::<Vec<_>>();
        let mad = median(deviations)?;
        let mut scale = 1.4826 * mad;

        if scale == 0.0 {
            scale = sample_stddev(&values)?;
        }
        if scale == 0.0 || !scale.is_finite() {
            return None;
        }

        let clip = if z_clip.is_finite() && z_clip > 0.0 {
            z_clip
        } else {
            f64::INFINITY
        };
        let z = ((current - median_value) / scale).clamp(-clip, clip);
        Some(RobustStats {
            n: values.len(),
            median: median_value,
            mad,
            scale,
            z,
        })
    }

    pub fn percentile_rank(&self, current: f64) -> Option<f64> {
        if !current.is_finite() || self.values.is_empty() {
            return None;
        }
        let count = self.values.iter().filter(|value| **value <= current).count();
        Some(count as f64 / self.values.len() as f64)
    }
}

fn median(mut values: Vec<f64>) -> Option<f64> {
    if values.is_empty() || values.iter().any(|value| !value.is_finite()) {
        return None;
    }
    values.sort_by(|a, b| a.total_cmp(b));
    let mid = values.len() / 2;
    if values.len() % 2 == 0 {
        Some((values[mid - 1] + values[mid]) / 2.0)
    } else {
        Some(values[mid])
    }
}

fn sample_stddev(values: &[f64]) -> Option<f64> {
    if values.len() < 2 {
        return None;
    }
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let variance = values
        .iter()
        .map(|value| {
            let diff = value - mean;
            diff * diff
        })
        .sum::<f64>()
        / (values.len() - 1) as f64;
    Some(variance.sqrt())
}
```

Add to `crates/features/src/lib.rs`:

```rust
pub mod robust;
```

- [ ] **Step 4: Run robust tests**

Run:

```bash
cargo test -p perp-radar-features --test robust_contract
```

Expected: PASS.

## Task 4: Packet Builder 2.1 Formal/Legacy Score Split

**Files:**
- Modify: `crates/features/src/packet_builder.rs`
- Modify: `crates/features/tests/packet_builder_contract.rs`

- [ ] **Step 1: Write failing builder contract tests**

Update `standard_packet_uses_symbol_price_and_quality_from_state` in `crates/features/tests/packet_builder_contract.rs`:

```rust
    assert_eq!(packet.packet_schema, "2.1");
    assert!(packet.scores.lri.is_none());
    assert!(packet.scores.dpi10.is_none());
    assert!(packet.score_meta.contains_key("LRI"));
    assert!(packet.score_meta["LRI"].missing.contains(&"book_not_full".to_string()));
```

Add:

```rust
#[test]
fn packet_builder_moves_old_score_meanings_to_legacy_scores() {
    let mut state = SymbolState::new("BTCUSDT", 100);
    for idx in 0..64 {
        let close = 100.0 + idx as f64;
        state.apply_kline(KlineUpdate {
            candle: Candle {
                symbol: "BTCUSDT".to_string(),
                open_time_ms: 1_700_000_000_000 + (idx as i64 * 60_000),
                close_time_ms: 1_700_000_059_999 + (idx as i64 * 60_000),
                open: close - 0.5,
                high: close + 1.0,
                low: close - 1.0,
                close,
                volume_base: 100.0 + idx as f64,
                volume_quote: (100.0 + idx as f64) * close,
                trades: 100 + idx as u64,
                taker_buy_base: (100.0 + idx as f64) * 0.5,
                taker_buy_quote: (100.0 + idx as f64) * close * 0.5,
                is_closed: true,
                source: "test".to_string(),
            },
        });
    }
    state.apply_mark_price(MarkPriceUpdate {
        symbol: "BTCUSDT".to_string(),
        mark_price: 164.0,
        index_price: 163.5,
        funding_rate: 0.0001,
        next_funding_time_ms: 1_714_550_400_000,
        event_time_ms: 1_714_521_600_000,
    });
    state.apply_partial_depth(PartialDepthUpdate {
        symbol: "BTCUSDT".to_string(),
        last_update_id: 42,
        bids: vec![BookLevel { price: 163.9, qty: 10.0 }],
        asks: vec![BookLevel { price: 164.1, qty: 8.0 }],
        event_time_ms: 1_714_521_602_000,
    });

    let packet = build_standard_packet(&state, 1, 15, 3);

    assert_eq!(packet.scores.tcs, None);
    assert!(packet.legacy_scores.candidate_score.is_some());
    assert_eq!(packet.legacy_scores.notional_imbalance_i5, packet.liquidity.i5);
    assert_eq!(packet.legacy_scores.volume_spike_z, packet.events.volume_spike_z);
}

#[test]
fn packet_builder_uses_full_book_qty_imbalance_for_dpi5_and_dpi10() {
    let mut state = SymbolState::new("BTCUSDT", 100);
    state.apply_full_depth_snapshot(FullDepthSnapshotUpdate {
        symbol: "BTCUSDT".to_string(),
        last_update_id: 123,
        bids: (0..10)
            .map(|idx| BookLevel { price: 100.0 - idx as f64 * 0.01, qty: 10.0 })
            .collect(),
        asks: (0..10)
            .map(|idx| BookLevel { price: 100.1 + idx as f64 * 0.01, qty: 5.0 })
            .collect(),
    });

    let packet = build_standard_packet(&state, 1, 15, 3);

    assert_eq!(packet.scores.dpi5, Some((50.0 - 25.0) / 75.0));
    assert_eq!(packet.scores.dpi10, Some((100.0 - 50.0) / 150.0));
    assert_eq!(packet.score_meta["DPI5"].book_source.as_deref(), Some("full"));
    assert!(packet.score_meta["DPI5"].missing.is_empty());
    assert!(packet.score_meta["DPI10"].missing.is_empty());
}
```

- [ ] **Step 2: Run builder tests to verify they fail**

Run:

```bash
cargo test -p perp-radar-features --test packet_builder_contract
```

Expected: FAIL because Packet 2.1 fields and formal/legacy split are not wired.

- [ ] **Step 3: Update packet builder imports and schema**

In `crates/features/src/packet_builder.rs`, import:

```rust
use std::collections::BTreeMap;
use perp_radar_core::packet::{LegacyScoresBlock, ScoreMeta};
```

Set:

```rust
packet_schema: "2.1".to_string(),
```

- [ ] **Step 4: Fill new chart fields conservatively**

In the `ChartBlock` construction:

```rust
            ema_200: ema_last_from_candles(&candles, 200),
            ema50_slope: ema_slope_from_candles(&candles, 50, 10),
            bb_width_pctile: None,
            atr_1h_pct: None,
            atr_1h_pct_prev: None,
            atr_1h_pct_delta_ratio: None,
```

Add private helpers:

```rust
fn ema_last_from_candles(candles: &[Candle], period: usize) -> Option<f64> {
    if candles.len() < period || period == 0 {
        return None;
    }
    let closes = candles.iter().map(|candle| candle.close).collect::<Vec<_>>();
    ema_last(&closes, period)
}

fn ema_slope_from_candles(candles: &[Candle], period: usize, lookback: usize) -> Option<f64> {
    if lookback == 0 || candles.len() < period + lookback {
        return None;
    }
    let closes = candles.iter().map(|candle| candle.close).collect::<Vec<_>>();
    let now = ema_last(&closes, period)?;
    let past_end = closes.len().checked_sub(lookback)?;
    let past = ema_last(&closes[..past_end], period)?;
    if past == 0.0 {
        return None;
    }
    Some((now - past) / past)
}

fn ema_last(values: &[f64], period: usize) -> Option<f64> {
    if values.len() < period || period == 0 || values.iter().any(|value| !value.is_finite()) {
        return None;
    }
    let seed = values[..period].iter().sum::<f64>() / period as f64;
    let multiplier = 2.0 / (period as f64 + 1.0);
    Some(values[period..].iter().fold(seed, |ema, value| ((value - ema) * multiplier) + ema))
}
```

- [ ] **Step 5: Split legacy and formal scores**

Replace `scores: scores_block(state, &candles),` with:

```rust
        scores: formal_scores_block(state),
        score_meta: score_meta_block(state),
        legacy_scores: legacy_scores_block(state, &candles),
```

Add:

```rust
fn formal_scores_block(state: &SymbolState) -> ScoresBlock {
    ScoresBlock {
        tcs: None,
        lri: None,
        dpi5: state
            .full_book
            .as_ref()
            .filter(|_| state.quality.book_mode == "full" && state.quality.book_seq_ok == Some(true))
            .and_then(|book| book.qty_imbalance_top_n(5))
            .map(|imbalance| imbalance.imbalance),
        dpi10: state
            .full_book
            .as_ref()
            .filter(|_| state.quality.book_mode == "full" && state.quality.book_seq_ok == Some(true))
            .and_then(|book| book.qty_imbalance_top_n(10))
            .map(|imbalance| imbalance.imbalance),
        csi: None,
        rpi: None,
        vov: None,
    }
}

fn legacy_scores_block(state: &SymbolState, candles: &[Candle]) -> LegacyScoresBlock {
    let legacy = legacy_candidate_components(state, candles);
    LegacyScoresBlock {
        candidate_score: legacy.candidate_score,
        liquidation_event_score: legacy.liquidation_event_score,
        compression_score: legacy.compression_score,
        momentum_abs_score: legacy.momentum_abs_score,
        volume_spike_z: legacy.volume_spike_z,
        notional_imbalance_i5: state.partial_book.as_ref().and_then(|book| book.imbalance_top_n(5)),
    }
}
```

Move the old `scores_block` calculation into a small private `LegacyCandidateComponents` helper so the old values are preserved under their new names.

- [ ] **Step 6: Add score meta**

Add:

```rust
fn score_meta_block(state: &SymbolState) -> BTreeMap<String, ScoreMeta> {
    let mut meta = BTreeMap::new();
    meta.insert("LRI".to_string(), lri_meta(state));
    meta.insert("TCS".to_string(), unavailable_meta("z(ADX14)*sign(close-EMA200)+0.5*z(ema50_slope)-0.5*z(BB_width_pctile)", vec!["component_window_insufficient"]));
    meta.insert("DPI5".to_string(), dpi_meta(state, 5));
    meta.insert("DPI10".to_string(), dpi_meta(state, 10));
    meta.insert("CSI".to_string(), unavailable_meta("z(abs(funding_z_7d))+0.5*z(abs(basis_bp))", vec!["component_window_insufficient"]));
    meta.insert("RPI".to_string(), unavailable_meta("z(rsi_extreme)+z(funding_same_side)+z(book_against_move)", vec!["component_window_insufficient"]));
    meta.insert("VoV".to_string(), unavailable_meta("z(atr_delta_ratio)", vec!["atr_delta_ratio_window_insufficient"]));
    meta
}
```

Ensure `lri_meta` reports `book_not_full` when `quality.book_mode != "full"`, `book_seq_not_ok` when `book_seq_ok != Some(true)`, and formula/direction/slip notional exactly from the design. Ensure `dpi_meta` reports `bid_depth_lt_N`, `ask_depth_lt_N`, or `all_qty_zero` through available information where possible; use `depth_array_missing` if no full book is trusted.

- [ ] **Step 7: Run builder tests**

Run:

```bash
cargo test -p perp-radar-features --test packet_builder_contract
```

Expected: PASS.

## Task 5: API, Tools, and Docs Compatibility

**Files:**
- Modify: `crates/api/src/cache.rs`
- Modify: `crates/api/src/routes.rs`
- Modify: `crates/api/src/export.rs`
- Modify: `crates/api/tests/api_contract.rs`
- Modify: `crates/app/tests/runtime_contract.rs`
- Modify: `tools/live-monitor.py`
- Modify: `tools/validate-live-indicators.py`
- Modify: `docs/DATA_CONTRACT.md`
- Modify: `docs/INDICATORS.md`

- [ ] **Step 1: Run workspace tests to discover Packet 2.1 compile failures**

Run:

```bash
cargo test --workspace
```

Expected: FAIL in crates that construct `StandardPacket` literals without `score_meta`, `legacy_scores`, or `dpi10`, and tests still expecting schema `2.0`.

- [ ] **Step 2: Update Rust packet literals and schema routes**

For each `StandardPacket` literal, add:

```rust
score_meta: std::collections::BTreeMap::new(),
legacy_scores: LegacyScoresBlock::default(),
```

For each `ScoresBlock` literal, add `dpi10: None` or use `..ScoresBlock::default()`.

Update schema route output from `"2.0"` to `"2.1"`.

- [ ] **Step 3: Update text export**

In `crates/api/src/export.rs`, either add `DPI10={dpi10}` to the output format or intentionally leave it out while keeping compilation correct. If adding it, use:

```rust
dpi10 = fmt_opt(packet.scores.dpi10),
```

- [ ] **Step 4: Update Python tools**

In `tools/live-monitor.py`, read:

```python
"dpi10": packet["scores"].get("DPI10"),
"score_meta": packet.get("score_meta", {}),
"legacy_scores": packet.get("legacy_scores", {}),
```

In `tools/validate-live-indicators.py`, add `scores.DPI10` to required checks and confirm `score_meta` exists for Packet `2.1`.

- [ ] **Step 5: Update docs**

In `docs/DATA_CONTRACT.md`, document `packet_schema: "2.1"`, `scores.DPI10`, `score_meta`, and `legacy_scores`.

In `docs/INDICATORS.md`, document that former score meanings now live under `legacy_scores` and formal `scores` may be null with reasons in `score_meta`.

- [ ] **Step 6: Run workspace tests**

Run:

```bash
cargo test --workspace
```

Expected: PASS.

## Self-Review

- Spec coverage in this P0 plan: Packet 2.1 shape, `DPI10`, `score_meta`, `legacy_scores`, robust z foundation, full-book spread/qty imbalance helpers, and null/missing discipline are covered.
- Explicitly deferred from full design: complete incremental score histories inside state, final LRI/TCS/CSI/RPI/VoV numeric computation, ClickHouse score feature storage, and runtime score emission cadence changes. These should be separate follow-up plans after this P0 contract slice lands.
- Placeholder scan: No `TBD`/`TODO` steps remain; deferred work is named explicitly as out of scope.
- Type consistency: `ScoreMeta`, `LegacyScoresBlock`, `RingWindow`, `DepthQtyImbalance`, and `ScoresBlock::dpi10` are introduced before dependent tasks use them.

## Continuation: Formal Score Computation Slice

The next development slice completes formal Packet 2.1 score computation without changing runtime transport or ClickHouse storage.

### Task 6: Add Bounded Score History State

**Files:**
- Create: `crates/state/src/score_history.rs`
- Modify: `crates/state/src/lib.rs`
- Modify: `crates/state/src/symbol_state.rs`
- Add: `crates/state/tests/score_history.rs`

**Behavior:**
- Add typed bounded windows for LRI, TCS, CSI, RPI, and VoV components.
- Keep only finite samples.
- Do not push LRI book components unless full-book source is trusted.
- Update score history from accepted closed candles, mark price updates, partial/full book events, and funding history updates.

### Task 7: Compute Formal Scores From State

**Files:**
- Modify: `crates/features/src/packet_builder.rs`
- Modify: `crates/features/tests/packet_builder_contract.rs`

**Behavior:**
- `LRI = robust_z(0.4*z(-spread_bp)+0.3*z(liq_5bp_usd)+0.3*z(-slip_bp))`.
- `TCS = z(ADX14) * sign(close - EMA200) + 0.5*z(ema50_slope) - 0.5*z(BB_width_pctile)`.
- `CSI = z(abs(funding_z_7d)) + 0.5*z(abs(basis_bp))`.
- `RPI = z(abs(RSI14 - 50)) + z(max(0, sign(RSI14 - 50) * funding_z_7d)) + z(max(0, -sign(ret_1h) * I1))`.
- `VoV = z((atr_1h_pct_now - atr_1h_pct_prev) / atr_1h_pct_prev)`.
- Every null formal score has concrete `score_meta.<score>.missing`.
- Every non-null formal score has `score_meta.<score>.available == true`.
- Old meanings remain only under `legacy_scores`.

### Task 8: Runtime And Documentation Acceptance

**Files:**
- Modify: runtime/API/tool/docs tests only as required.

**Behavior:**
- Preserve `cargo test --workspace`.
- Keep `latest_packets` behavior unchanged.
- Keep live monitor tolerant of formal null scores when missing reasons exist.
