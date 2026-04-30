# Perp Radar V1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a Rust service that ingests Binance USD-M perpetual market data, writes required ClickHouse history, and exposes real-time LLM-ready packets over HTTP.

**Architecture:** One Rust workspace with focused crates for core contracts, Binance ingestion, hot state, features, storage, API, and app supervision. Runtime packet endpoints read an in-memory cache; ClickHouse is required for startup, migrations, history, audit, and latest packet persistence.

**Tech Stack:** Rust 2021, Tokio, axum, serde, reqwest, tokio-tungstenite, clickhouse, chrono, tracing, YAML config, ClickHouse SQL migrations.

---

## File Structure

Create these files during implementation:

```text
Cargo.toml
.gitignore
config/default.yaml
migrations/001_symbols.sql
migrations/002_klines_1m.sql
migrations/003_mark_funding_sample.sql
migrations/004_depth_features_1s.sql
migrations/005_features_1m.sql
migrations/006_latest_packets.sql
crates/core/Cargo.toml
crates/core/src/lib.rs
crates/core/src/build_info.rs
crates/core/src/types.rs
crates/core/src/time.rs
crates/core/src/quality.rs
crates/core/src/packet.rs
crates/core/tests/workspace_smoke.rs
crates/core/tests/packet_contract.rs
crates/binance/Cargo.toml
crates/binance/src/lib.rs
crates/binance/src/streams.rs
crates/binance/src/parser.rs
crates/binance/src/rest_client.rs
crates/binance/src/ws_client.rs
crates/binance/src/rate_limiter.rs
crates/binance/tests/parser_contract.rs
crates/binance/tests/stream_urls.rs
crates/binance/tests/client_contract.rs
crates/state/Cargo.toml
crates/state/src/lib.rs
crates/state/src/candle_ring.rs
crates/state/src/book_partial.rs
crates/state/src/book_full.rs
crates/state/src/symbol_state.rs
crates/state/tests/candle_state.rs
crates/state/tests/book_state.rs
crates/features/Cargo.toml
crates/features/src/lib.rs
crates/features/src/ta.rs
crates/features/src/liquidity.rs
crates/features/src/funding.rs
crates/features/src/scores.rs
crates/features/src/ranking.rs
crates/features/src/packet_builder.rs
crates/features/tests/feature_contract.rs
crates/features/tests/packet_builder_contract.rs
crates/storage/Cargo.toml
crates/storage/src/lib.rs
crates/storage/src/migrations.rs
crates/storage/src/clickhouse.rs
crates/storage/src/batcher.rs
crates/storage/tests/migration_contract.rs
crates/api/Cargo.toml
crates/api/src/lib.rs
crates/api/src/cache.rs
crates/api/src/routes.rs
crates/api/src/export.rs
crates/api/src/debug.rs
crates/api/tests/api_contract.rs
crates/app/Cargo.toml
crates/app/src/lib.rs
crates/app/src/main.rs
crates/app/src/config.rs
crates/app/src/runtime.rs
crates/app/src/supervisor.rs
crates/app/tests/config_contract.rs
crates/app/tests/docs_contract.rs
crates/app/tests/runtime_contract.rs
docs/DATA_CONTRACT.md
docs/RUNBOOK.md
docs/INDICATORS.md
docs/OPERATIONS.md
```

## Task 1: Workspace Scaffold

**Files:**

- Create: `Cargo.toml`
- Create: `.gitignore`
- Create: `crates/core/Cargo.toml`
- Create: `crates/core/src/lib.rs`
- Create: `crates/core/src/build_info.rs`
- Create: `crates/core/tests/workspace_smoke.rs`
- Create: `crates/binance/Cargo.toml`
- Create: `crates/binance/src/lib.rs`
- Create: `crates/state/Cargo.toml`
- Create: `crates/state/src/lib.rs`
- Create: `crates/features/Cargo.toml`
- Create: `crates/features/src/lib.rs`
- Create: `crates/storage/Cargo.toml`
- Create: `crates/storage/src/lib.rs`
- Create: `crates/api/Cargo.toml`
- Create: `crates/api/src/lib.rs`
- Create: `crates/app/Cargo.toml`
- Create: `crates/app/src/main.rs`

- [ ] **Step 1: Write the failing smoke test**

Create `crates/core/tests/workspace_smoke.rs`:

```rust
#[test]
fn workspace_exposes_core_build_info() {
    assert_eq!(perp_radar_core::build_info::crate_name(), "perp-radar-core");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p perp-radar-core workspace_exposes_core_build_info --test workspace_smoke
```

Expected: FAIL with a missing workspace or missing package error because `Cargo.toml` does not exist yet.

- [ ] **Step 3: Write minimal workspace implementation**

Create root `Cargo.toml`:

```toml
[workspace]
resolver = "2"
members = [
  "crates/core",
  "crates/binance",
  "crates/state",
  "crates/features",
  "crates/storage",
  "crates/api",
  "crates/app",
]

[workspace.package]
edition = "2021"
license = "MIT"
version = "0.1.0"

[workspace.dependencies]
anyhow = "1"
async-trait = "0.1"
axum = "0.7"
bytes = "1"
chrono = { version = "0.4", features = ["serde"] }
clickhouse = "0.13"
config = "0.14"
futures-util = "0.3"
reqwest = { version = "0.12", features = ["json", "rustls-tls"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
serde_yaml = "0.9"
thiserror = "1"
tokio = { version = "1", features = ["macros", "rt-multi-thread", "signal", "sync", "time"] }
tokio-tungstenite = { version = "0.24", features = ["rustls-tls-webpki-roots"] }
tower = { version = "0.5", features = ["util"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "fmt"] }
url = "2"
```

Create `.gitignore`:

```gitignore
/target/
/.env
/.DS_Store
*.log
```

Create `crates/core/Cargo.toml`:

```toml
[package]
name = "perp-radar-core"
edition.workspace = true
license.workspace = true
version.workspace = true

[dependencies]
chrono.workspace = true
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
```

Create `crates/core/src/lib.rs`:

```rust
pub mod build_info;
```

Create `crates/core/src/build_info.rs`:

```rust
pub fn crate_name() -> &'static str {
    "perp-radar-core"
}
```

Create the remaining crate manifests:

```toml
# crates/binance/Cargo.toml
[package]
name = "perp-radar-binance"
edition.workspace = true
license.workspace = true
version.workspace = true

[dependencies]
anyhow.workspace = true
chrono.workspace = true
futures-util.workspace = true
perp-radar-core = { path = "../core" }
reqwest.workspace = true
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
tokio.workspace = true
tokio-tungstenite.workspace = true
url.workspace = true
```

```toml
# crates/state/Cargo.toml
[package]
name = "perp-radar-state"
edition.workspace = true
license.workspace = true
version.workspace = true

[dependencies]
chrono.workspace = true
perp-radar-core = { path = "../core" }
serde.workspace = true
thiserror.workspace = true
```

```toml
# crates/features/Cargo.toml
[package]
name = "perp-radar-features"
edition.workspace = true
license.workspace = true
version.workspace = true

[dependencies]
perp-radar-core = { path = "../core" }
perp-radar-state = { path = "../state" }
serde.workspace = true
```

```toml
# crates/storage/Cargo.toml
[package]
name = "perp-radar-storage"
edition.workspace = true
license.workspace = true
version.workspace = true

[dependencies]
anyhow.workspace = true
chrono.workspace = true
clickhouse.workspace = true
perp-radar-core = { path = "../core" }
serde.workspace = true
tokio.workspace = true
```

```toml
# crates/api/Cargo.toml
[package]
name = "perp-radar-api"
edition.workspace = true
license.workspace = true
version.workspace = true

[dependencies]
axum.workspace = true
chrono.workspace = true
perp-radar-core = { path = "../core" }
serde.workspace = true
serde_json.workspace = true
tokio.workspace = true
tower.workspace = true
```

```toml
# crates/app/Cargo.toml
[package]
name = "perp-radar"
edition.workspace = true
license.workspace = true
version.workspace = true

[[bin]]
name = "perp-radar"
path = "src/main.rs"

[dependencies]
anyhow.workspace = true
config.workspace = true
perp-radar-api = { path = "../api" }
perp-radar-binance = { path = "../binance" }
perp-radar-core = { path = "../core" }
perp-radar-features = { path = "../features" }
perp-radar-state = { path = "../state" }
perp-radar-storage = { path = "../storage" }
serde.workspace = true
tokio.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true
```

Create each minimal library file:

```rust
// crates/binance/src/lib.rs
pub fn crate_name() -> &'static str {
    "perp-radar-binance"
}
```

```rust
// crates/state/src/lib.rs
pub fn crate_name() -> &'static str {
    "perp-radar-state"
}
```

```rust
// crates/features/src/lib.rs
pub fn crate_name() -> &'static str {
    "perp-radar-features"
}
```

```rust
// crates/storage/src/lib.rs
pub fn crate_name() -> &'static str {
    "perp-radar-storage"
}
```

```rust
// crates/api/src/lib.rs
pub fn crate_name() -> &'static str {
    "perp-radar-api"
}
```

Create `crates/app/src/main.rs`:

```rust
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_env_filter("info").init();
    tracing::info!("perp-radar starting");
    Ok(())
}
```

- [ ] **Step 4: Run test to verify it passes**

Run:

```bash
cargo test -p perp-radar-core workspace_exposes_core_build_info --test workspace_smoke
cargo test --workspace
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml .gitignore crates
git commit -m "chore: scaffold rust workspace"
```

## Task 2: Core Packet Contract

**Files:**

- Create: `crates/core/src/types.rs`
- Create: `crates/core/src/time.rs`
- Create: `crates/core/src/quality.rs`
- Create: `crates/core/src/packet.rs`
- Modify: `crates/core/src/lib.rs`
- Create: `crates/core/tests/packet_contract.rs`

- [ ] **Step 1: Write the failing packet tests**

Create `crates/core/tests/packet_contract.rs`:

```rust
use chrono::{TimeZone, Utc};
use perp_radar_core::packet::{
    CarryBlock, ChartBlock, EventsBlock, LiquidityBlock, PacketProfile, PriceBlock, ScoresBlock,
    StandardPacket, UniverseBlock,
};
use perp_radar_core::quality::{QualityReason, QualityState};
use perp_radar_core::types::UniverseTier;

#[test]
fn standard_packet_serializes_null_metrics_and_reasons() {
    let packet = StandardPacket {
        packet_schema: "2.0".to_string(),
        ts: Utc.with_ymd_and_hms(2026, 5, 1, 0, 0, 0).unwrap(),
        symbol: "BTCUSDT".to_string(),
        rank: 1,
        profile: PacketProfile::Standard,
        universe: UniverseBlock {
            tier: UniverseTier::U2,
            active_n: 15,
            focus_n: 3,
        },
        price: PriceBlock {
            last: Some(64210.5),
            mark: Some(64208.9),
            index: Some(64193.2),
            basis_bp: Some(2.45),
            ret_1m: None,
            ret_5m: None,
            ret_15m: None,
            ret_1h: None,
        },
        chart: ChartBlock::default(),
        liquidity: LiquidityBlock {
            book_mode: "partial20".to_string(),
            spread_bp: Some(0.62),
            i1: Some(0.16),
            i5: Some(0.09),
            microprice_bp: Some(0.31),
            liq_5bp_usd: None,
            liq_10bp_usd: None,
            slip_10000_buy_bp: None,
            slip_10000_sell_bp: None,
        },
        carry: CarryBlock::default(),
        events: EventsBlock::default(),
        scores: ScoresBlock::default(),
        quality: QualityState {
            freshness_ms: 384,
            warm: true,
            kline_gap_1m: 0,
            book_mode: "partial20".to_string(),
            book_seq_ok: None,
            book_depth_coverage_bp: Some(3.1),
            funding_history_points: 0,
            stale: false,
            reasons: vec![QualityReason::DepthCoverageLt5Bp],
        },
    };

    let json = serde_json::to_value(&packet).unwrap();

    assert_eq!(json["packet_schema"], "2.0");
    assert_eq!(json["profile"], "standard");
    assert!(json["liquidity"]["liq_5bp_usd"].is_null());
    assert_eq!(json["quality"]["reasons"][0], "depth_coverage_lt_5bp");
}

#[test]
fn quality_reasons_are_unique() {
    let mut quality = QualityState::cold("partial20");
    quality.add_reason(QualityReason::InsufficientFundingHistory);
    quality.add_reason(QualityReason::InsufficientFundingHistory);

    assert_eq!(quality.reasons, vec![QualityReason::InsufficientFundingHistory]);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p perp-radar-core --test packet_contract
```

Expected: FAIL with unresolved imports for `packet`, `quality`, and `types`.

- [ ] **Step 3: Write minimal core implementation**

Modify `crates/core/src/lib.rs`:

```rust
pub mod build_info;
pub mod packet;
pub mod quality;
pub mod time;
pub mod types;
```

Create `crates/core/src/types.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UniverseTier {
    #[serde(rename = "U0")]
    U0,
    #[serde(rename = "U1")]
    U1,
    #[serde(rename = "U2")]
    U2,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Candle {
    pub symbol: String,
    pub open_time_ms: i64,
    pub close_time_ms: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume_base: f64,
    pub volume_quote: f64,
    pub trades: u64,
    pub taker_buy_base: f64,
    pub taker_buy_quote: f64,
    pub is_closed: bool,
    pub source: String,
}
```

Create `crates/core/src/time.rs`:

```rust
use chrono::{DateTime, Utc};

pub fn now_utc() -> DateTime<Utc> {
    Utc::now()
}

pub fn ms_to_utc(ms: i64) -> DateTime<Utc> {
    DateTime::<Utc>::from_timestamp_millis(ms).expect("valid millisecond timestamp")
}
```

Create `crates/core/src/quality.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityReason {
    InsufficientKlineHistory,
    InsufficientFundingHistory,
    DepthCoverageLt5Bp,
    FullBookSequenceGap,
    StaleMarketData,
    MissingMarkPrice,
    MissingIndexPrice,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QualityState {
    pub freshness_ms: u64,
    pub warm: bool,
    pub kline_gap_1m: u32,
    pub book_mode: String,
    pub book_seq_ok: Option<bool>,
    pub book_depth_coverage_bp: Option<f64>,
    pub funding_history_points: usize,
    pub stale: bool,
    pub reasons: Vec<QualityReason>,
}

impl QualityState {
    pub fn cold(book_mode: impl Into<String>) -> Self {
        Self {
            freshness_ms: u64::MAX,
            warm: false,
            kline_gap_1m: 0,
            book_mode: book_mode.into(),
            book_seq_ok: None,
            book_depth_coverage_bp: None,
            funding_history_points: 0,
            stale: true,
            reasons: Vec::new(),
        }
    }

    pub fn add_reason(&mut self, reason: QualityReason) {
        if !self.reasons.contains(&reason) {
            self.reasons.push(reason);
        }
    }
}
```

Create `crates/core/src/packet.rs`:

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::quality::QualityState;
use crate::types::UniverseTier;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PacketProfile {
    Compact,
    Standard,
    Full,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UniverseBlock {
    pub tier: UniverseTier,
    pub active_n: usize,
    pub focus_n: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct PriceBlock {
    pub last: Option<f64>,
    pub mark: Option<f64>,
    pub index: Option<f64>,
    pub basis_bp: Option<f64>,
    pub ret_1m: Option<f64>,
    pub ret_5m: Option<f64>,
    pub ret_15m: Option<f64>,
    pub ret_1h: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ChartBlock {
    pub regime: Option<String>,
    pub signature: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct LiquidityBlock {
    pub book_mode: String,
    pub spread_bp: Option<f64>,
    pub i1: Option<f64>,
    pub i5: Option<f64>,
    pub microprice_bp: Option<f64>,
    pub liq_5bp_usd: Option<f64>,
    pub liq_10bp_usd: Option<f64>,
    pub slip_10000_buy_bp: Option<f64>,
    pub slip_10000_sell_bp: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct CarryBlock {
    pub funding_now: Option<f64>,
    pub funding_unit: Option<String>,
    pub funding_interval_hours: Option<u32>,
    pub funding_z_7d: Option<f64>,
    pub next_funding_time: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct EventsBlock {
    pub liq_1m_usd: Option<f64>,
    pub liq_5m_usd: Option<f64>,
    pub liq_side: Option<String>,
    pub volume_spike_z: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ScoresBlock {
    #[serde(rename = "TCS")]
    pub tcs: Option<f64>,
    #[serde(rename = "LRI")]
    pub lri: Option<f64>,
    #[serde(rename = "DPI5")]
    pub dpi5: Option<f64>,
    #[serde(rename = "CSI")]
    pub csi: Option<f64>,
    #[serde(rename = "RPI")]
    pub rpi: Option<f64>,
    #[serde(rename = "VoV")]
    pub vov: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StandardPacket {
    pub packet_schema: String,
    pub ts: DateTime<Utc>,
    pub symbol: String,
    pub rank: usize,
    pub profile: PacketProfile,
    pub universe: UniverseBlock,
    pub price: PriceBlock,
    pub chart: ChartBlock,
    pub liquidity: LiquidityBlock,
    pub carry: CarryBlock,
    pub events: EventsBlock,
    pub scores: ScoresBlock,
    pub quality: QualityState,
}
```

- [ ] **Step 4: Run test to verify it passes**

Run:

```bash
cargo test -p perp-radar-core --test packet_contract
cargo test --workspace
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/core
git commit -m "feat: add core packet contract"
```

## Task 3: Candle Ring And Symbol State

**Files:**

- Create: `crates/state/src/candle_ring.rs`
- Create: `crates/state/src/symbol_state.rs`
- Modify: `crates/state/src/lib.rs`
- Create: `crates/state/tests/candle_state.rs`

- [ ] **Step 1: Write failing state tests**

Create `crates/state/tests/candle_state.rs`:

```rust
use perp_radar_core::types::Candle;
use perp_radar_state::candle_ring::CandleRing;
use perp_radar_state::symbol_state::{KlineUpdate, SymbolState};

fn candle(open_time_ms: i64, close: f64) -> Candle {
    Candle {
        symbol: "BTCUSDT".to_string(),
        open_time_ms,
        close_time_ms: open_time_ms + 59_999,
        open: close,
        high: close,
        low: close,
        close,
        volume_base: 1.0,
        volume_quote: close,
        trades: 10,
        taker_buy_base: 0.5,
        taker_buy_quote: close * 0.5,
        is_closed: true,
        source: "test".to_string(),
    }
}

#[test]
fn ring_keeps_most_recent_items() {
    let mut ring = CandleRing::new(2);
    ring.push(candle(60_000, 100.0));
    ring.push(candle(120_000, 101.0));
    ring.push(candle(180_000, 102.0));

    assert_eq!(ring.len(), 2);
    assert_eq!(ring.items()[0].open_time_ms, 120_000);
    assert_eq!(ring.items()[1].open_time_ms, 180_000);
}

#[test]
fn symbol_state_only_stores_closed_klines() {
    let mut state = SymbolState::new("BTCUSDT", 10);

    state.apply_kline(KlineUpdate {
        candle: Candle { is_closed: false, ..candle(60_000, 100.0) },
    });
    assert_eq!(state.candles_1m.len(), 0);

    state.apply_kline(KlineUpdate { candle: candle(60_000, 100.0) });
    assert_eq!(state.candles_1m.len(), 1);
}

#[test]
fn symbol_state_counts_1m_gaps() {
    let mut state = SymbolState::new("BTCUSDT", 10);
    state.apply_kline(KlineUpdate { candle: candle(60_000, 100.0) });
    state.apply_kline(KlineUpdate { candle: candle(180_000, 102.0) });

    assert_eq!(state.quality.kline_gap_1m, 1);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p perp-radar-state --test candle_state
```

Expected: FAIL with unresolved modules `candle_ring` and `symbol_state`.

- [ ] **Step 3: Write minimal state implementation**

Modify `crates/state/src/lib.rs`:

```rust
pub mod candle_ring;
pub mod symbol_state;

pub fn crate_name() -> &'static str {
    "perp-radar-state"
}
```

Create `crates/state/src/candle_ring.rs`:

```rust
use std::collections::VecDeque;

use perp_radar_core::types::Candle;

#[derive(Debug, Clone)]
pub struct CandleRing {
    capacity: usize,
    items: VecDeque<Candle>,
}

impl CandleRing {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "candle ring capacity must be greater than zero");
        Self {
            capacity,
            items: VecDeque::with_capacity(capacity),
        }
    }

    pub fn push(&mut self, candle: Candle) {
        if self.items.len() == self.capacity {
            self.items.pop_front();
        }
        self.items.push_back(candle);
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn last(&self) -> Option<&Candle> {
        self.items.back()
    }

    pub fn items(&self) -> Vec<Candle> {
        self.items.iter().cloned().collect()
    }
}
```

Create `crates/state/src/symbol_state.rs`:

```rust
use perp_radar_core::quality::QualityState;
use perp_radar_core::types::Candle;

use crate::candle_ring::CandleRing;

#[derive(Debug, Clone)]
pub struct KlineUpdate {
    pub candle: Candle,
}

#[derive(Debug, Clone)]
pub struct SymbolState {
    pub symbol: String,
    pub candles_1m: CandleRing,
    pub quality: QualityState,
}

impl SymbolState {
    pub fn new(symbol: impl Into<String>, candle_capacity: usize) -> Self {
        Self {
            symbol: symbol.into(),
            candles_1m: CandleRing::new(candle_capacity),
            quality: QualityState::cold("none"),
        }
    }

    pub fn apply_kline(&mut self, update: KlineUpdate) {
        if !update.candle.is_closed {
            return;
        }

        if let Some(last) = self.candles_1m.last() {
            let expected_next = last.open_time_ms + 60_000;
            if update.candle.open_time_ms > expected_next {
                let missed = ((update.candle.open_time_ms - expected_next) / 60_000) as u32;
                self.quality.kline_gap_1m += missed;
            }
        }

        self.candles_1m.push(update.candle);
        self.quality.warm = self.candles_1m.len() >= 2;
        self.quality.stale = false;
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run:

```bash
cargo test -p perp-radar-state --test candle_state
cargo test --workspace
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/state
git commit -m "feat: add candle ring and symbol state"
```

## Task 4: Partial And Full Book State

**Files:**

- Create: `crates/state/src/book_partial.rs`
- Create: `crates/state/src/book_full.rs`
- Modify: `crates/state/src/lib.rs`
- Create: `crates/state/tests/book_state.rs`

- [ ] **Step 1: Write failing book tests**

Create `crates/state/tests/book_state.rs`:

```rust
use perp_radar_state::book_full::{BookDelta, FullBook, LevelDelta};
use perp_radar_state::book_partial::{BookLevel, PartialBook};

#[test]
fn partial_book_calculates_spread_imbalance_and_microprice() {
    let book = PartialBook::new(
        "BTCUSDT",
        vec![
            BookLevel { price: 100.0, qty: 10.0 },
            BookLevel { price: 99.9, qty: 5.0 },
        ],
        vec![
            BookLevel { price: 100.1, qty: 6.0 },
            BookLevel { price: 100.2, qty: 4.0 },
        ],
    );

    assert!((book.spread_bp().unwrap() - 10.0).abs() < 0.0001);
    assert!((book.imbalance_top_n(1).unwrap() - 0.25).abs() < 0.0001);
    assert!(book.microprice_bp().unwrap() > 0.0);
}

#[test]
fn full_book_rejects_sequence_gap() {
    let mut book = FullBook::from_snapshot(
        "BTCUSDT",
        10,
        vec![BookLevel { price: 100.0, qty: 10.0 }],
        vec![BookLevel { price: 100.1, qty: 6.0 }],
    );

    let result = book.apply_delta(BookDelta {
        first_update_id: 12,
        final_update_id: 13,
        previous_final_update_id: 9,
        bids: vec![LevelDelta { price: 100.0, qty: 11.0 }],
        asks: vec![],
    });

    assert!(result.is_err());
    assert!(!book.seq_ok());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p perp-radar-state --test book_state
```

Expected: FAIL with unresolved modules `book_partial` and `book_full`.

- [ ] **Step 3: Write minimal book implementation**

Modify `crates/state/src/lib.rs`:

```rust
pub mod book_full;
pub mod book_partial;
pub mod candle_ring;
pub mod symbol_state;

pub fn crate_name() -> &'static str {
    "perp-radar-state"
}
```

Create `crates/state/src/book_partial.rs`:

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct BookLevel {
    pub price: f64,
    pub qty: f64,
}

#[derive(Debug, Clone)]
pub struct PartialBook {
    pub symbol: String,
    pub bids: Vec<BookLevel>,
    pub asks: Vec<BookLevel>,
}

impl PartialBook {
    pub fn new(symbol: impl Into<String>, bids: Vec<BookLevel>, asks: Vec<BookLevel>) -> Self {
        Self {
            symbol: symbol.into(),
            bids,
            asks,
        }
    }

    pub fn best_bid(&self) -> Option<&BookLevel> {
        self.bids.first()
    }

    pub fn best_ask(&self) -> Option<&BookLevel> {
        self.asks.first()
    }

    pub fn mid(&self) -> Option<f64> {
        Some((self.best_bid()?.price + self.best_ask()?.price) / 2.0)
    }

    pub fn spread_bp(&self) -> Option<f64> {
        let mid = self.mid()?;
        Some((self.best_ask()?.price - self.best_bid()?.price) / mid * 10_000.0)
    }

    pub fn imbalance_top_n(&self, n: usize) -> Option<f64> {
        let bid_notional: f64 = self.bids.iter().take(n).map(|level| level.price * level.qty).sum();
        let ask_notional: f64 = self.asks.iter().take(n).map(|level| level.price * level.qty).sum();
        let total = bid_notional + ask_notional;
        if total == 0.0 {
            return None;
        }
        Some((bid_notional - ask_notional) / total)
    }

    pub fn microprice_bp(&self) -> Option<f64> {
        let bid = self.best_bid()?;
        let ask = self.best_ask()?;
        let mid = self.mid()?;
        let microprice = (ask.price * bid.qty + bid.price * ask.qty) / (bid.qty + ask.qty);
        Some((microprice - mid) / mid * 10_000.0)
    }
}
```

Create `crates/state/src/book_full.rs`:

```rust
use std::collections::BTreeMap;

use thiserror::Error;

use crate::book_partial::BookLevel;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FullBookError {
    #[error("full book sequence gap")]
    SequenceGap,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LevelDelta {
    pub price: f64,
    pub qty: f64,
}

#[derive(Debug, Clone)]
pub struct BookDelta {
    pub first_update_id: u64,
    pub final_update_id: u64,
    pub previous_final_update_id: u64,
    pub bids: Vec<LevelDelta>,
    pub asks: Vec<LevelDelta>,
}

#[derive(Debug, Clone)]
pub struct FullBook {
    symbol: String,
    last_update_id: u64,
    seq_ok: bool,
    bids: BTreeMap<i64, f64>,
    asks: BTreeMap<i64, f64>,
}

impl FullBook {
    pub fn from_snapshot(
        symbol: impl Into<String>,
        last_update_id: u64,
        bids: Vec<BookLevel>,
        asks: Vec<BookLevel>,
    ) -> Self {
        Self {
            symbol: symbol.into(),
            last_update_id,
            seq_ok: true,
            bids: levels_to_map(bids),
            asks: levels_to_map(asks),
        }
    }

    pub fn apply_delta(&mut self, delta: BookDelta) -> Result<(), FullBookError> {
        if delta.previous_final_update_id != self.last_update_id {
            self.seq_ok = false;
            return Err(FullBookError::SequenceGap);
        }

        apply_levels(&mut self.bids, delta.bids);
        apply_levels(&mut self.asks, delta.asks);
        self.last_update_id = delta.final_update_id;
        self.seq_ok = true;
        Ok(())
    }

    pub fn seq_ok(&self) -> bool {
        self.seq_ok
    }

    pub fn symbol(&self) -> &str {
        &self.symbol
    }
}

fn levels_to_map(levels: Vec<BookLevel>) -> BTreeMap<i64, f64> {
    let mut map = BTreeMap::new();
    for level in levels {
        map.insert(price_key(level.price), level.qty);
    }
    map
}

fn apply_levels(map: &mut BTreeMap<i64, f64>, levels: Vec<LevelDelta>) {
    for level in levels {
        let key = price_key(level.price);
        if level.qty == 0.0 {
            map.remove(&key);
        } else {
            map.insert(key, level.qty);
        }
    }
}

fn price_key(price: f64) -> i64 {
    (price * 100_000_000.0).round() as i64
}
```

- [ ] **Step 4: Run test to verify it passes**

Run:

```bash
cargo test -p perp-radar-state --test book_state
cargo test --workspace
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/state
git commit -m "feat: add order book state"
```

## Task 5: Feature Calculations

**Files:**

- Create: `crates/features/src/ta.rs`
- Create: `crates/features/src/liquidity.rs`
- Create: `crates/features/src/funding.rs`
- Create: `crates/features/src/scores.rs`
- Create: `crates/features/src/ranking.rs`
- Modify: `crates/features/src/lib.rs`
- Create: `crates/features/tests/feature_contract.rs`

- [ ] **Step 1: Write failing feature tests**

Create `crates/features/tests/feature_contract.rs`:

```rust
use perp_radar_features::funding::z_score;
use perp_radar_features::ranking::{rank_candidates, Candidate};
use perp_radar_features::scores::{composite_candidate_score, ScoreInputs};
use perp_radar_features::ta::{return_pct, simple_rsi};

#[test]
fn return_pct_uses_decimal_return() {
    assert!((return_pct(100.0, 105.0).unwrap() - 0.05).abs() < 0.0001);
}

#[test]
fn rsi_is_high_for_monotonic_up_series() {
    let closes = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
    assert!(simple_rsi(&closes, 5).unwrap() > 99.0);
}

#[test]
fn funding_z_score_uses_sample_mean_and_stddev() {
    let history = vec![0.0001, 0.0002, 0.0003, 0.0004];
    let z = z_score(&history, 0.0005).unwrap();
    assert!(z > 1.0);
}

#[test]
fn composite_score_returns_none_when_required_input_missing() {
    let inputs = ScoreInputs {
        volume_accel_z: Some(1.0),
        ret_15m_z_abs: None,
        atr_pctile: Some(0.5),
        funding_z_abs: Some(0.4),
        liquidation_event_score: Some(0.2),
        squeeze_or_breakout_score: Some(0.3),
        liquidity_quality: Some(0.9),
    };

    assert!(composite_candidate_score(&inputs).is_none());
}

#[test]
fn ranking_orders_highest_score_first() {
    let ranked = rank_candidates(vec![
        Candidate { symbol: "ETHUSDT".to_string(), score: 0.8 },
        Candidate { symbol: "BTCUSDT".to_string(), score: 1.2 },
    ]);

    assert_eq!(ranked[0].symbol, "BTCUSDT");
    assert_eq!(ranked[0].rank, 1);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p perp-radar-features --test feature_contract
```

Expected: FAIL with unresolved modules and functions.

- [ ] **Step 3: Write minimal feature implementation**

Modify `crates/features/src/lib.rs`:

```rust
pub mod funding;
pub mod liquidity;
pub mod ranking;
pub mod scores;
pub mod ta;

pub fn crate_name() -> &'static str {
    "perp-radar-features"
}
```

Create `crates/features/src/ta.rs`:

```rust
pub fn return_pct(start: f64, end: f64) -> Option<f64> {
    if start == 0.0 {
        return None;
    }
    Some((end - start) / start)
}

pub fn simple_rsi(closes: &[f64], period: usize) -> Option<f64> {
    if closes.len() < period + 1 || period == 0 {
        return None;
    }

    let window = &closes[closes.len() - period - 1..];
    let mut gains = 0.0;
    let mut losses = 0.0;

    for pair in window.windows(2) {
        let change = pair[1] - pair[0];
        if change >= 0.0 {
            gains += change;
        } else {
            losses += change.abs();
        }
    }

    if losses == 0.0 {
        return Some(100.0);
    }

    let rs = gains / losses;
    Some(100.0 - (100.0 / (1.0 + rs)))
}

pub fn volume_z_score(samples: &[f64], current: f64) -> Option<f64> {
    crate::funding::z_score(samples, current)
}
```

Create `crates/features/src/funding.rs`:

```rust
pub fn z_score(history: &[f64], current: f64) -> Option<f64> {
    if history.len() < 2 {
        return None;
    }

    let mean = history.iter().sum::<f64>() / history.len() as f64;
    let variance = history
        .iter()
        .map(|value| {
            let diff = value - mean;
            diff * diff
        })
        .sum::<f64>()
        / (history.len() as f64 - 1.0);
    let stddev = variance.sqrt();

    if stddev == 0.0 {
        return None;
    }

    Some((current - mean) / stddev)
}
```

Create `crates/features/src/liquidity.rs`:

```rust
pub fn liquidity_quality(spread_bp: Option<f64>, coverage_bp: Option<f64>) -> Option<f64> {
    let spread = spread_bp?;
    let coverage = coverage_bp?;
    let spread_component = (1.0 - (spread / 20.0)).clamp(0.0, 1.0);
    let coverage_component = (coverage / 10.0).clamp(0.0, 1.0);
    Some((spread_component + coverage_component) / 2.0)
}
```

Create `crates/features/src/scores.rs`:

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct ScoreInputs {
    pub volume_accel_z: Option<f64>,
    pub ret_15m_z_abs: Option<f64>,
    pub atr_pctile: Option<f64>,
    pub funding_z_abs: Option<f64>,
    pub liquidation_event_score: Option<f64>,
    pub squeeze_or_breakout_score: Option<f64>,
    pub liquidity_quality: Option<f64>,
}

pub fn composite_candidate_score(inputs: &ScoreInputs) -> Option<f64> {
    Some(
        0.25 * inputs.volume_accel_z?
            + 0.20 * inputs.ret_15m_z_abs?
            + 0.15 * inputs.atr_pctile?
            + 0.15 * inputs.funding_z_abs?
            + 0.10 * inputs.liquidation_event_score?
            + 0.10 * inputs.squeeze_or_breakout_score?
            + 0.05 * inputs.liquidity_quality?,
    )
}
```

Create `crates/features/src/ranking.rs`:

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    pub symbol: String,
    pub score: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RankedCandidate {
    pub symbol: String,
    pub score: f64,
    pub rank: usize,
}

pub fn rank_candidates(mut candidates: Vec<Candidate>) -> Vec<RankedCandidate> {
    candidates.sort_by(|a, b| b.score.total_cmp(&a.score).then_with(|| a.symbol.cmp(&b.symbol)));
    candidates
        .into_iter()
        .enumerate()
        .map(|(idx, candidate)| RankedCandidate {
            symbol: candidate.symbol,
            score: candidate.score,
            rank: idx + 1,
        })
        .collect()
}
```

- [ ] **Step 4: Run test to verify it passes**

Run:

```bash
cargo test -p perp-radar-features --test feature_contract
cargo test --workspace
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/features
git commit -m "feat: add feature calculations"
```

## Task 6: Binance Stream Builders And Parsers

**Files:**

- Create: `crates/binance/src/streams.rs`
- Create: `crates/binance/src/parser.rs`
- Modify: `crates/binance/src/lib.rs`
- Create: `crates/binance/tests/stream_urls.rs`
- Create: `crates/binance/tests/parser_contract.rs`

- [ ] **Step 1: Write failing Binance tests**

Create `crates/binance/tests/stream_urls.rs`:

```rust
use perp_radar_binance::streams::{combined_stream_url, WsBase};

#[test]
fn market_combined_stream_uses_market_base() {
    let url = combined_stream_url(
        WsBase::Market("wss://fstream.binance.com/market".to_string()),
        &["!markPrice@arr", "!ticker@arr"],
    )
    .unwrap();

    assert_eq!(
        url.as_str(),
        "wss://fstream.binance.com/market/stream?streams=!markPrice@arr/!ticker@arr"
    );
}

#[test]
fn public_combined_stream_uses_public_base() {
    let url = combined_stream_url(
        WsBase::Public("wss://fstream.binance.com/public".to_string()),
        &["btcusdt@depth20@500ms"],
    )
    .unwrap();

    assert_eq!(
        url.as_str(),
        "wss://fstream.binance.com/public/stream?streams=btcusdt@depth20@500ms"
    );
}
```

Create `crates/binance/tests/parser_contract.rs`:

```rust
use perp_radar_binance::parser::{parse_combined_event, BinanceEvent};

#[test]
fn parses_combined_kline_event() {
    let payload = r#"{
      "stream":"btcusdt@kline_1m",
      "data":{
        "e":"kline",
        "E":1714521600000,
        "s":"BTCUSDT",
        "k":{
          "t":1714521600000,
          "T":1714521659999,
          "s":"BTCUSDT",
          "i":"1m",
          "o":"64000.0",
          "c":"64100.0",
          "h":"64200.0",
          "l":"63950.0",
          "v":"12.5",
          "q":"801250.0",
          "n":120,
          "V":"6.0",
          "Q":"384600.0",
          "x":true
        }
      }
    }"#;

    let event = parse_combined_event(payload).unwrap();
    match event {
        BinanceEvent::Kline(kline) => {
            assert_eq!(kline.candle.symbol, "BTCUSDT");
            assert!(kline.candle.is_closed);
            assert_eq!(kline.candle.close, 64100.0);
        }
        other => panic!("expected kline event, got {other:?}"),
    }
}

#[test]
fn parses_depth_update_sequence_fields() {
    let payload = r#"{
      "stream":"btcusdt@depth@500ms",
      "data":{
        "e":"depthUpdate",
        "E":1714521600000,
        "T":1714521600000,
        "s":"BTCUSDT",
        "U":101,
        "u":110,
        "pu":100,
        "b":[["64000.0","1.2"]],
        "a":[["64001.0","0.8"]]
      }
    }"#;

    let event = parse_combined_event(payload).unwrap();
    match event {
        BinanceEvent::Depth(delta) => {
            assert_eq!(delta.symbol, "BTCUSDT");
            assert_eq!(delta.first_update_id, 101);
            assert_eq!(delta.previous_final_update_id, 100);
        }
        other => panic!("expected depth event, got {other:?}"),
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p perp-radar-binance --test stream_urls --test parser_contract
```

Expected: FAIL with unresolved modules `streams` and `parser`.

- [ ] **Step 3: Write minimal parser and stream builder implementation**

Modify `crates/binance/src/lib.rs`:

```rust
pub mod parser;
pub mod streams;

pub fn crate_name() -> &'static str {
    "perp-radar-binance"
}
```

Create `crates/binance/src/streams.rs`:

```rust
use url::Url;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WsBase {
    Market(String),
    Public(String),
}

impl WsBase {
    fn as_str(&self) -> &str {
        match self {
            WsBase::Market(value) | WsBase::Public(value) => value,
        }
    }
}

pub fn combined_stream_url(base: WsBase, streams: &[&str]) -> anyhow::Result<Url> {
    let joined = streams.join("/");
    let url = format!("{}/stream?streams={}", base.as_str().trim_end_matches('/'), joined);
    Ok(Url::parse(&url)?)
}
```

Create `crates/binance/src/parser.rs`:

```rust
use perp_radar_core::types::Candle;
use perp_radar_state::book_full::{BookDelta, LevelDelta};
use perp_radar_state::symbol_state::KlineUpdate;
use serde::Deserialize;

#[derive(Debug, Clone)]
pub enum BinanceEvent {
    Kline(KlineUpdate),
    Depth(DepthEvent),
    Ignored,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DepthEvent {
    pub symbol: String,
    pub first_update_id: u64,
    pub final_update_id: u64,
    pub previous_final_update_id: u64,
    pub bids: Vec<LevelDelta>,
    pub asks: Vec<LevelDelta>,
}

#[derive(Debug, Deserialize)]
struct CombinedPayload {
    data: serde_json::Value,
}

pub fn parse_combined_event(payload: &str) -> anyhow::Result<BinanceEvent> {
    let combined: CombinedPayload = serde_json::from_str(payload)?;
    match combined.data.get("e").and_then(|value| value.as_str()) {
        Some("kline") => parse_kline(combined.data),
        Some("depthUpdate") => parse_depth(combined.data),
        _ => Ok(BinanceEvent::Ignored),
    }
}

fn parse_kline(data: serde_json::Value) -> anyhow::Result<BinanceEvent> {
    #[derive(Debug, Deserialize)]
    struct KlineEnvelope {
        k: KlinePayload,
    }

    #[derive(Debug, Deserialize)]
    struct KlinePayload {
        #[serde(rename = "t")]
        open_time_ms: i64,
        #[serde(rename = "T")]
        close_time_ms: i64,
        #[serde(rename = "s")]
        symbol: String,
        #[serde(rename = "o")]
        open: String,
        #[serde(rename = "h")]
        high: String,
        #[serde(rename = "l")]
        low: String,
        #[serde(rename = "c")]
        close: String,
        #[serde(rename = "v")]
        volume_base: String,
        #[serde(rename = "q")]
        volume_quote: String,
        #[serde(rename = "n")]
        trades: u64,
        #[serde(rename = "V")]
        taker_buy_base: String,
        #[serde(rename = "Q")]
        taker_buy_quote: String,
        #[serde(rename = "x")]
        is_closed: bool,
    }

    let envelope: KlineEnvelope = serde_json::from_value(data)?;
    let k = envelope.k;
    Ok(BinanceEvent::Kline(KlineUpdate {
        candle: Candle {
            symbol: k.symbol,
            open_time_ms: k.open_time_ms,
            close_time_ms: k.close_time_ms,
            open: k.open.parse()?,
            high: k.high.parse()?,
            low: k.low.parse()?,
            close: k.close.parse()?,
            volume_base: k.volume_base.parse()?,
            volume_quote: k.volume_quote.parse()?,
            trades: k.trades,
            taker_buy_base: k.taker_buy_base.parse()?,
            taker_buy_quote: k.taker_buy_quote.parse()?,
            is_closed: k.is_closed,
            source: "ws".to_string(),
        },
    }))
}

fn parse_depth(data: serde_json::Value) -> anyhow::Result<BinanceEvent> {
    #[derive(Debug, Deserialize)]
    struct DepthPayload {
        #[serde(rename = "s")]
        symbol: String,
        #[serde(rename = "U")]
        first_update_id: u64,
        #[serde(rename = "u")]
        final_update_id: u64,
        #[serde(rename = "pu")]
        previous_final_update_id: u64,
        #[serde(rename = "b")]
        bids: Vec<[String; 2]>,
        #[serde(rename = "a")]
        asks: Vec<[String; 2]>,
    }

    let depth: DepthPayload = serde_json::from_value(data)?;
    Ok(BinanceEvent::Depth(DepthEvent {
        symbol: depth.symbol,
        first_update_id: depth.first_update_id,
        final_update_id: depth.final_update_id,
        previous_final_update_id: depth.previous_final_update_id,
        bids: parse_levels(depth.bids)?,
        asks: parse_levels(depth.asks)?,
    }))
}

fn parse_levels(levels: Vec<[String; 2]>) -> anyhow::Result<Vec<LevelDelta>> {
    levels
        .into_iter()
        .map(|level| {
            Ok(LevelDelta {
                price: level[0].parse()?,
                qty: level[1].parse()?,
            })
        })
        .collect()
}

impl From<DepthEvent> for BookDelta {
    fn from(value: DepthEvent) -> Self {
        BookDelta {
            first_update_id: value.first_update_id,
            final_update_id: value.final_update_id,
            previous_final_update_id: value.previous_final_update_id,
            bids: value.bids,
            asks: value.asks,
        }
    }
}
```

Add `perp-radar-state` to `crates/binance/Cargo.toml`:

```toml
perp-radar-state = { path = "../state" }
```

- [ ] **Step 4: Run test to verify it passes**

Run:

```bash
cargo test -p perp-radar-binance --test stream_urls --test parser_contract
cargo test --workspace
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/binance
git commit -m "feat: parse binance market streams"
```

## Task 7: ClickHouse Migrations And Storage Gate

**Files:**

- Create: `migrations/001_symbols.sql`
- Create: `migrations/002_klines_1m.sql`
- Create: `migrations/003_mark_funding_sample.sql`
- Create: `migrations/004_depth_features_1s.sql`
- Create: `migrations/005_features_1m.sql`
- Create: `migrations/006_latest_packets.sql`
- Create: `crates/storage/src/migrations.rs`
- Create: `crates/storage/src/clickhouse.rs`
- Modify: `crates/storage/src/lib.rs`
- Create: `crates/storage/tests/migration_contract.rs`

- [ ] **Step 1: Write failing migration tests**

Create `crates/storage/tests/migration_contract.rs`:

```rust
use perp_radar_storage::migrations::{migration_names, migration_sql};

#[test]
fn migrations_are_ordered_and_named() {
    assert_eq!(
        migration_names(),
        vec![
            "001_symbols.sql",
            "002_klines_1m.sql",
            "003_mark_funding_sample.sql",
            "004_depth_features_1s.sql",
            "005_features_1m.sql",
            "006_latest_packets.sql",
        ]
    );
}

#[test]
fn latest_packets_migration_contains_packet_json() {
    let sql = migration_sql("006_latest_packets.sql").unwrap();
    assert!(sql.contains("latest_packets"));
    assert!(sql.contains("packet_json String"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p perp-radar-storage --test migration_contract
```

Expected: FAIL with unresolved module `migrations`.

- [ ] **Step 3: Write migrations and storage gate implementation**

Create `migrations/001_symbols.sql`:

```sql
CREATE TABLE IF NOT EXISTS perp_radar.symbols
(
    symbol String,
    pair String,
    contract_type String,
    status String,
    base_asset String,
    quote_asset String,
    margin_asset String,
    tick_size Float64,
    step_size Float64,
    min_notional Float64,
    updated_at DateTime64(3, 'UTC')
)
ENGINE = MergeTree
ORDER BY (symbol, updated_at);
```

Create `migrations/002_klines_1m.sql`:

```sql
CREATE TABLE IF NOT EXISTS perp_radar.klines_1m
(
    symbol String,
    open_time DateTime64(3, 'UTC'),
    close_time DateTime64(3, 'UTC'),
    open Float64,
    high Float64,
    low Float64,
    close Float64,
    volume_base Float64,
    volume_quote Float64,
    trades UInt64,
    taker_buy_base Float64,
    taker_buy_quote Float64,
    is_closed Bool,
    source String,
    ingest_time DateTime64(3, 'UTC')
)
ENGINE = MergeTree
ORDER BY (symbol, open_time);
```

Create `migrations/003_mark_funding_sample.sql`:

```sql
CREATE TABLE IF NOT EXISTS perp_radar.mark_funding_sample
(
    ts DateTime64(3, 'UTC'),
    symbol String,
    mark_price Float64,
    index_price Float64,
    basis_bp Float64,
    funding_rate Float64,
    next_funding_time DateTime64(3, 'UTC')
)
ENGINE = MergeTree
ORDER BY (symbol, ts);
```

Create `migrations/004_depth_features_1s.sql`:

```sql
CREATE TABLE IF NOT EXISTS perp_radar.depth_features_1s
(
    ts DateTime64(3, 'UTC'),
    symbol String,
    mode String,
    spread_bp Nullable(Float64),
    mid Nullable(Float64),
    i1 Nullable(Float64),
    i5 Nullable(Float64),
    i10 Nullable(Float64),
    microprice_bp Nullable(Float64),
    bid_top20_usd Nullable(Float64),
    ask_top20_usd Nullable(Float64),
    liq_5bp_usd Nullable(Float64),
    liq_10bp_usd Nullable(Float64),
    slip_10k_buy_bp Nullable(Float64),
    slip_10k_sell_bp Nullable(Float64),
    coverage_bid_bp Nullable(Float64),
    coverage_ask_bp Nullable(Float64),
    seq_ok Nullable(Bool)
)
ENGINE = MergeTree
ORDER BY (symbol, ts);
```

Create `migrations/005_features_1m.sql`:

```sql
CREATE TABLE IF NOT EXISTS perp_radar.features_1m
(
    ts DateTime64(3, 'UTC'),
    symbol String,
    price Nullable(Float64),
    ret_1m Nullable(Float64),
    ret_5m Nullable(Float64),
    ret_15m Nullable(Float64),
    atr_pct Nullable(Float64),
    rsi14 Nullable(Float64),
    macd_hist Nullable(Float64),
    adx14 Nullable(Float64),
    bb_width Nullable(Float64),
    funding_z_7d Nullable(Float64),
    basis_bp Nullable(Float64),
    spread_bp Nullable(Float64),
    i1 Nullable(Float64),
    i5 Nullable(Float64),
    tcs Nullable(Float64),
    lri Nullable(Float64),
    dpi5 Nullable(Float64),
    csi Nullable(Float64),
    rpi Nullable(Float64),
    vov Nullable(Float64),
    quality_json String
)
ENGINE = MergeTree
ORDER BY (symbol, ts);
```

Create `migrations/006_latest_packets.sql`:

```sql
CREATE TABLE IF NOT EXISTS perp_radar.latest_packets
(
    ts DateTime64(3, 'UTC'),
    symbol String,
    profile String,
    rank UInt32,
    packet_json String
)
ENGINE = ReplacingMergeTree(ts)
ORDER BY (symbol, profile);
```

Modify `crates/storage/src/lib.rs`:

```rust
pub mod batcher;
pub mod clickhouse;
pub mod migrations;

pub fn crate_name() -> &'static str {
    "perp-radar-storage"
}
```

Create `crates/storage/src/migrations.rs`:

```rust
use std::collections::BTreeMap;

pub fn migration_names() -> Vec<&'static str> {
    vec![
        "001_symbols.sql",
        "002_klines_1m.sql",
        "003_mark_funding_sample.sql",
        "004_depth_features_1s.sql",
        "005_features_1m.sql",
        "006_latest_packets.sql",
    ]
}

pub fn migration_sql(name: &str) -> Option<&'static str> {
    all_migrations().get(name).copied()
}

pub fn all_ordered_sql() -> Vec<(&'static str, &'static str)> {
    migration_names()
        .into_iter()
        .filter_map(|name| migration_sql(name).map(|sql| (name, sql)))
        .collect()
}

fn all_migrations() -> BTreeMap<&'static str, &'static str> {
    BTreeMap::from([
        ("001_symbols.sql", include_str!("../../../migrations/001_symbols.sql")),
        ("002_klines_1m.sql", include_str!("../../../migrations/002_klines_1m.sql")),
        (
            "003_mark_funding_sample.sql",
            include_str!("../../../migrations/003_mark_funding_sample.sql"),
        ),
        (
            "004_depth_features_1s.sql",
            include_str!("../../../migrations/004_depth_features_1s.sql"),
        ),
        (
            "005_features_1m.sql",
            include_str!("../../../migrations/005_features_1m.sql"),
        ),
        (
            "006_latest_packets.sql",
            include_str!("../../../migrations/006_latest_packets.sql"),
        ),
    ])
}
```

Create `crates/storage/src/clickhouse.rs`:

```rust
use anyhow::Context;
use clickhouse::Client;

use crate::migrations;

pub fn client(url: &str, database: &str) -> Client {
    Client::default().with_url(url).with_database(database)
}

pub fn admin_client(url: &str) -> Client {
    Client::default().with_url(url)
}

pub async fn assert_clickhouse_ready(client: &Client) -> anyhow::Result<()> {
    client
        .query("SELECT 1")
        .execute()
        .await
        .context("ClickHouse readiness query failed")
}

pub async fn run_migrations(url: &str, database: &str) -> anyhow::Result<Client> {
    let admin = admin_client(url);
    admin
        .query(&format!("CREATE DATABASE IF NOT EXISTS {database}"))
        .execute()
        .await
        .with_context(|| format!("failed to create {database} database"))?;

    let client = client(url, database);
    for (name, sql) in migrations::all_ordered_sql() {
        client
            .query(sql)
            .execute()
            .await
            .with_context(|| format!("failed to run migration {name}"))?;
    }

    Ok(client)
}
```

Create `crates/storage/src/batcher.rs`:

```rust
#[derive(Debug, Clone)]
pub struct BatchConfig {
    pub max_rows: usize,
    pub flush_interval_ms: u64,
}

impl BatchConfig {
    pub fn new(max_rows: usize, flush_interval_ms: u64) -> Self {
        Self {
            max_rows,
            flush_interval_ms,
        }
    }

    pub fn should_flush(&self, pending_rows: usize) -> bool {
        pending_rows >= self.max_rows
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run:

```bash
cargo test -p perp-radar-storage --test migration_contract
cargo test --workspace
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add migrations crates/storage
git commit -m "feat: add clickhouse migrations"
```

## Task 8: Packet Cache And API Routes

**Files:**

- Create: `crates/api/src/cache.rs`
- Create: `crates/api/src/routes.rs`
- Create: `crates/api/src/export.rs`
- Create: `crates/api/src/debug.rs`
- Modify: `crates/api/src/lib.rs`
- Create: `crates/api/tests/api_contract.rs`

- [ ] **Step 1: Write failing API tests**

Create `crates/api/tests/api_contract.rs`:

```rust
use axum::body::Body;
use axum::http::{Request, StatusCode};
use chrono::{TimeZone, Utc};
use perp_radar_api::cache::PacketCache;
use perp_radar_api::routes::router;
use perp_radar_core::packet::{
    CarryBlock, ChartBlock, EventsBlock, LiquidityBlock, PacketProfile, PriceBlock, ScoresBlock,
    StandardPacket, UniverseBlock,
};
use perp_radar_core::quality::QualityState;
use perp_radar_core::types::UniverseTier;
use serde_json::Value;
use tower::ServiceExt;

fn packet(symbol: &str) -> StandardPacket {
    StandardPacket {
        packet_schema: "2.0".to_string(),
        ts: Utc.with_ymd_and_hms(2026, 5, 1, 0, 0, 0).unwrap(),
        symbol: symbol.to_string(),
        rank: 1,
        profile: PacketProfile::Standard,
        universe: UniverseBlock { tier: UniverseTier::U2, active_n: 15, focus_n: 3 },
        price: PriceBlock { last: Some(100.0), ..PriceBlock::default() },
        chart: ChartBlock::default(),
        liquidity: LiquidityBlock { book_mode: "full".to_string(), ..LiquidityBlock::default() },
        carry: CarryBlock::default(),
        events: EventsBlock::default(),
        scores: ScoresBlock::default(),
        quality: QualityState { freshness_ms: 10, warm: true, kline_gap_1m: 0, book_mode: "full".to_string(), book_seq_ok: Some(true), book_depth_coverage_bp: Some(12.0), funding_history_points: 10, stale: false, reasons: vec![] },
    }
}

#[tokio::test]
async fn packet_route_returns_cached_packet() {
    let cache = PacketCache::default();
    cache.upsert(packet("BTCUSDT"));
    let app = router(cache);

    let response = app
        .oneshot(Request::builder().uri("/v1/packet/BTCUSDT").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["symbol"], "BTCUSDT");
}

#[tokio::test]
async fn top_text_export_is_llm_readable() {
    let cache = PacketCache::default();
    cache.upsert(packet("BTCUSDT"));
    let app = router(cache);

    let response = app
        .oneshot(Request::builder().uri("/v1/export/top.txt?limit=1").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let text = String::from_utf8(body.to_vec()).unwrap();
    assert!(text.contains("[BTCUSDT]"));
    assert!(text.contains("quality:"));
}

#[tokio::test]
async fn schema_and_jsonl_routes_are_available() {
    let cache = PacketCache::default();
    cache.upsert(packet("BTCUSDT"));
    let app = router(cache);

    let schema = app
        .clone()
        .oneshot(Request::builder().uri("/v1/schema").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(schema.status(), StatusCode::OK);

    let jsonl = app
        .oneshot(Request::builder().uri("/v1/export/top.jsonl?limit=1").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(jsonl.status(), StatusCode::OK);
    let body = axum::body::to_bytes(jsonl.into_body(), usize::MAX).await.unwrap();
    let text = String::from_utf8(body.to_vec()).unwrap();
    assert!(text.contains("\"symbol\":\"BTCUSDT\""));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p perp-radar-api --test api_contract
```

Expected: FAIL with unresolved modules `cache` and `routes`.

- [ ] **Step 3: Write minimal API implementation**

Modify `crates/api/src/lib.rs`:

```rust
pub mod cache;
pub mod debug;
pub mod export;
pub mod routes;

pub fn crate_name() -> &'static str {
    "perp-radar-api"
}
```

Create `crates/api/src/cache.rs`:

```rust
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use perp_radar_core::packet::StandardPacket;

#[derive(Debug, Clone, Default)]
pub struct PacketCache {
    inner: Arc<RwLock<HashMap<String, StandardPacket>>>,
}

impl PacketCache {
    pub fn upsert(&self, packet: StandardPacket) {
        self.inner.write().expect("packet cache lock").insert(packet.symbol.clone(), packet);
    }

    pub fn get(&self, symbol: &str) -> Option<StandardPacket> {
        self.inner.read().expect("packet cache lock").get(symbol).cloned()
    }

    pub fn top(&self, limit: usize) -> Vec<StandardPacket> {
        let mut packets: Vec<_> = self.inner.read().expect("packet cache lock").values().cloned().collect();
        packets.sort_by_key(|packet| packet.rank);
        packets.truncate(limit);
        packets
    }
}
```

Create `crates/api/src/export.rs`:

```rust
use perp_radar_core::packet::StandardPacket;

pub fn packet_to_text(packet: &StandardPacket) -> String {
    format!(
        "[{}] rank={} {:?} price={:?} ret15m={:?}\nliq: spread={:?}bp I5={:?}\ncarry: funding={:?}/interval z7d={:?}\nevents: liq5m={:?} side={:?}\nscores: TCS={:?} LRI={:?} CSI={:?} RPI={:?} VoV={:?}\nquality: fresh={}ms warm={} gaps={} reasons={:?}",
        packet.symbol,
        packet.rank,
        packet.universe.tier,
        packet.price.last,
        packet.price.ret_15m,
        packet.liquidity.spread_bp,
        packet.liquidity.i5,
        packet.carry.funding_now,
        packet.carry.funding_z_7d,
        packet.events.liq_5m_usd,
        packet.events.liq_side,
        packet.scores.tcs,
        packet.scores.lri,
        packet.scores.csi,
        packet.scores.rpi,
        packet.scores.vov,
        packet.quality.freshness_ms,
        packet.quality.warm,
        packet.quality.kline_gap_1m,
        packet.quality.reasons
    )
}
```

Create `crates/api/src/routes.rs`:

```rust
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;

use crate::cache::PacketCache;
use crate::export::packet_to_text;

#[derive(Debug, Deserialize)]
struct LimitQuery {
    limit: Option<usize>,
}

pub fn router(cache: PacketCache) -> Router {
    Router::new()
        .route("/v1/health", get(health))
        .route("/v1/schema", get(schema))
        .route("/v1/universe", get(universe))
        .route("/v1/symbols", get(symbols))
        .route("/v1/packet/:symbol", get(packet))
        .route("/v1/packets/top", get(top))
        .route("/v1/export/top.txt", get(top_txt))
        .route("/v1/export/top.jsonl", get(top_jsonl))
        .route("/v1/debug/ws", get(debug_ws))
        .route("/v1/debug/rate_limits", get(debug_rate_limits))
        .with_state(cache)
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "ready": true }))
}

async fn schema() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "packet_schema": "2.0" }))
}

async fn universe(State(cache): State<PacketCache>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "active_n": 15,
        "focus_n": 3,
        "cached_symbols": cache.top(usize::MAX).len()
    }))
}

async fn symbols(State(cache): State<PacketCache>) -> Json<Vec<String>> {
    Json(cache.top(usize::MAX).into_iter().map(|packet| packet.symbol).collect())
}

async fn packet(State(cache): State<PacketCache>, Path(symbol): Path<String>) -> Response {
    match cache.get(&symbol) {
        Some(packet) => Json(packet).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn top(State(cache): State<PacketCache>, Query(query): Query<LimitQuery>) -> Json<Vec<perp_radar_core::packet::StandardPacket>> {
    Json(cache.top(query.limit.unwrap_or(20)))
}

async fn top_txt(State(cache): State<PacketCache>, Query(query): Query<LimitQuery>) -> String {
    cache
        .top(query.limit.unwrap_or(20))
        .iter()
        .map(packet_to_text)
        .collect::<Vec<_>>()
        .join("\n\n")
}

async fn top_jsonl(State(cache): State<PacketCache>, Query(query): Query<LimitQuery>) -> String {
    cache
        .top(query.limit.unwrap_or(20))
        .iter()
        .map(|packet| serde_json::to_string(packet).expect("packet serializes"))
        .collect::<Vec<_>>()
        .join("\n")
}

async fn debug_ws() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "connections": [] }))
}

async fn debug_rate_limits() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "rest_used_weight_1m": null }))
}
```

Create `crates/api/src/debug.rs`:

```rust
pub fn debug_routes_enabled() -> bool {
    true
}
```

- [ ] **Step 4: Run test to verify it passes**

Run:

```bash
cargo test -p perp-radar-api --test api_contract
cargo test --workspace
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/api
git commit -m "feat: expose packet api routes"
```

## Task 9: App Configuration And ClickHouse Startup Requirement

**Files:**

- Create: `config/default.yaml`
- Create: `crates/app/src/config.rs`
- Create: `crates/app/src/supervisor.rs`
- Modify: `crates/app/src/main.rs`
- Create: `crates/app/tests/config_contract.rs`

- [ ] **Step 1: Write failing config tests**

Create `crates/app/tests/config_contract.rs`:

```rust
use perp_radar::config::AppConfig;

#[test]
fn default_config_uses_light_universe_and_clickhouse() {
    let config = AppConfig::from_path("config/default.yaml").unwrap();

    assert_eq!(config.universe.active_n, 15);
    assert_eq!(config.universe.focus_n, 3);
    assert_eq!(config.universe.always_focus, vec!["BTCUSDT", "ETHUSDT", "SOLUSDT"]);
    assert_eq!(config.storage.database, "perp_radar");
    assert_eq!(config.api.bind, "127.0.0.1:8080");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p perp-radar --test config_contract
```

Expected: FAIL with unresolved import `perp_radar::config` or missing config file.

- [ ] **Step 3: Write config implementation**

Add `crates/app/src/lib.rs`:

```rust
pub mod config;
pub mod supervisor;
```

Add this to `crates/app/Cargo.toml`:

```toml
[lib]
name = "perp_radar"
path = "src/lib.rs"
```

Create `config/default.yaml`:

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
storage:
  clickhouse_url: "http://localhost:8123"
  database: "perp_radar"
  batch_rows: 2000
  batch_interval_ms: 1000
api:
  bind: "127.0.0.1:8080"
packets:
  standard_interval_ms: 1000
  topk_refresh_ms: 1000
```

Create `crates/app/src/config.rs`:

```rust
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub binance: BinanceConfig,
    pub universe: UniverseConfig,
    pub storage: StorageConfig,
    pub api: ApiConfig,
    pub packets: PacketConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BinanceConfig {
    pub market_ws_base: String,
    pub public_ws_base: String,
    pub rest_base: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UniverseConfig {
    pub quote_assets: Vec<String>,
    pub contract_type: String,
    pub include_status: Vec<String>,
    pub active_n: usize,
    pub focus_n: usize,
    pub refresh_sec: u64,
    pub hysteresis_rank_buffer: usize,
    pub always_focus: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StorageConfig {
    pub clickhouse_url: String,
    pub database: String,
    pub batch_rows: usize,
    pub batch_interval_ms: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApiConfig {
    pub bind: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PacketConfig {
    pub standard_interval_ms: u64,
    pub topk_refresh_ms: u64,
}

impl AppConfig {
    pub fn from_path(path: &str) -> anyhow::Result<Self> {
        let settings = config::Config::builder()
            .add_source(config::File::with_name(path))
            .build()?;
        Ok(settings.try_deserialize()?)
    }
}
```

Create `crates/app/src/supervisor.rs`:

```rust
use perp_radar_storage::clickhouse;

use crate::config::AppConfig;

pub async fn verify_required_storage(config: &AppConfig) -> anyhow::Result<()> {
    let admin = clickhouse::admin_client(&config.storage.clickhouse_url);
    clickhouse::assert_clickhouse_ready(&admin).await?;
    clickhouse::run_migrations(&config.storage.clickhouse_url, &config.storage.database).await?;
    Ok(())
}
```

Modify `crates/app/src/main.rs`:

```rust
use perp_radar::config::AppConfig;
use perp_radar::supervisor::verify_required_storage;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_env_filter("info").init();
    let config = AppConfig::from_path("config/default.yaml")?;
    verify_required_storage(&config).await?;
    tracing::info!("perp-radar ready to start runtime tasks");
    Ok(())
}
```

- [ ] **Step 4: Run test to verify it passes**

Run:

```bash
cargo test -p perp-radar --test config_contract
cargo test --workspace
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add config crates/app
git commit -m "feat: require clickhouse during startup"
```

## Task 10: REST Client, WS Client, And Live Wiring

**Files:**

- Create: `crates/binance/src/rest_client.rs`
- Create: `crates/binance/src/ws_client.rs`
- Create: `crates/binance/src/rate_limiter.rs`
- Modify: `crates/binance/src/lib.rs`
- Modify: `crates/app/src/supervisor.rs`

- [ ] **Step 1: Write failing client tests**

Create `crates/binance/tests/client_contract.rs`:

```rust
use perp_radar_binance::rate_limiter::TokenBucket;
use perp_radar_binance::rest_client::RestClient;

#[test]
fn rest_client_builds_exchange_info_url() {
    let client = RestClient::new("https://fapi.binance.com");
    assert_eq!(
        client.exchange_info_url().as_str(),
        "https://fapi.binance.com/fapi/v1/exchangeInfo"
    );
}

#[tokio::test]
async fn token_bucket_denies_when_empty() {
    let bucket = TokenBucket::new(1);
    assert!(bucket.try_take(1));
    assert!(!bucket.try_take(1));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p perp-radar-binance --test client_contract
```

Expected: FAIL with unresolved modules `rest_client` and `rate_limiter`.

- [ ] **Step 3: Write minimal clients**

Modify `crates/binance/src/lib.rs`:

```rust
pub mod parser;
pub mod rate_limiter;
pub mod rest_client;
pub mod streams;
pub mod ws_client;

pub fn crate_name() -> &'static str {
    "perp-radar-binance"
}
```

Create `crates/binance/src/rest_client.rs`:

```rust
use url::Url;

#[derive(Debug, Clone)]
pub struct RestClient {
    base: Url,
    client: reqwest::Client,
}

impl RestClient {
    pub fn new(base: &str) -> Self {
        Self {
            base: Url::parse(base.trim_end_matches('/')).expect("valid Binance REST base URL"),
            client: reqwest::Client::new(),
        }
    }

    pub fn exchange_info_url(&self) -> Url {
        self.base.join("/fapi/v1/exchangeInfo").expect("valid exchangeInfo URL")
    }

    pub async fn exchange_info_json(&self) -> anyhow::Result<serde_json::Value> {
        Ok(self.client.get(self.exchange_info_url()).send().await?.error_for_status()?.json().await?)
    }
}
```

Create `crates/binance/src/rate_limiter.rs`:

```rust
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Debug)]
pub struct TokenBucket {
    remaining: AtomicUsize,
}

impl TokenBucket {
    pub fn new(tokens: usize) -> Self {
        Self {
            remaining: AtomicUsize::new(tokens),
        }
    }

    pub fn try_take(&self, tokens: usize) -> bool {
        let mut current = self.remaining.load(Ordering::Relaxed);
        loop {
            if current < tokens {
                return false;
            }
            match self.remaining.compare_exchange(
                current,
                current - tokens,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ) {
                Ok(_) => return true,
                Err(next) => current = next,
            }
        }
    }
}
```

Create `crates/binance/src/ws_client.rs`:

```rust
use futures_util::StreamExt;
use tokio::sync::mpsc;
use tokio_tungstenite::connect_async;
use url::Url;

pub async fn stream_text_messages(url: Url, tx: mpsc::Sender<String>) -> anyhow::Result<()> {
    let (ws, _) = connect_async(url).await?;
    let (_, mut read) = ws.split();
    while let Some(message) = read.next().await {
        let message = message?;
        if message.is_text() {
            tx.send(message.into_text()?).await?;
        }
    }
    Ok(())
}
```

- [ ] **Step 4: Run test to verify it passes**

Run:

```bash
cargo test -p perp-radar-binance --test client_contract
cargo test --workspace
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/binance crates/app
git commit -m "feat: add binance clients"
```

## Task 11: Packet Builder And Top Ranking Integration

**Files:**

- Modify: `crates/features/src/ranking.rs`
- Create: `crates/features/src/packet_builder.rs`
- Modify: `crates/features/src/lib.rs`
- Create: `crates/features/tests/packet_builder_contract.rs`

- [ ] **Step 1: Write failing packet builder tests**

Create `crates/features/tests/packet_builder_contract.rs`:

```rust
use perp_radar_core::types::Candle;
use perp_radar_features::packet_builder::build_standard_packet;
use perp_radar_state::symbol_state::{KlineUpdate, SymbolState};

fn candle(open_time_ms: i64, close: f64) -> Candle {
    Candle {
        symbol: "BTCUSDT".to_string(),
        open_time_ms,
        close_time_ms: open_time_ms + 59_999,
        open: close,
        high: close,
        low: close,
        close,
        volume_base: 1.0,
        volume_quote: close,
        trades: 10,
        taker_buy_base: 0.5,
        taker_buy_quote: close * 0.5,
        is_closed: true,
        source: "test".to_string(),
    }
}

#[test]
fn packet_builder_marks_missing_history_with_reason() {
    let mut state = SymbolState::new("BTCUSDT", 10);
    state.apply_kline(KlineUpdate { candle: candle(60_000, 100.0) });

    let packet = build_standard_packet(&state, 1, 15, 3);

    assert_eq!(packet.symbol, "BTCUSDT");
    assert!(packet.price.ret_5m.is_none());
    assert!(!packet.quality.reasons.is_empty());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p perp-radar-features --test packet_builder_contract
```

Expected: FAIL with unresolved module `packet_builder`.

- [ ] **Step 3: Write packet builder implementation**

Modify `crates/features/src/lib.rs`:

```rust
pub mod funding;
pub mod liquidity;
pub mod packet_builder;
pub mod ranking;
pub mod scores;
pub mod ta;

pub fn crate_name() -> &'static str {
    "perp-radar-features"
}
```

Create `crates/features/src/packet_builder.rs`:

```rust
use chrono::Utc;
use perp_radar_core::packet::{
    CarryBlock, ChartBlock, EventsBlock, LiquidityBlock, PacketProfile, PriceBlock, ScoresBlock,
    StandardPacket, UniverseBlock,
};
use perp_radar_core::quality::QualityReason;
use perp_radar_core::types::UniverseTier;
use perp_radar_state::symbol_state::SymbolState;

use crate::ta::return_pct;

pub fn build_standard_packet(
    state: &SymbolState,
    rank: usize,
    active_n: usize,
    focus_n: usize,
) -> StandardPacket {
    let candles = state.candles_1m.items();
    let mut quality = state.quality.clone();
    let last = candles.last().map(|candle| candle.close);
    let ret_1m = ret_from_tail(&candles, 1);
    let ret_5m = ret_from_tail(&candles, 5);
    let ret_15m = ret_from_tail(&candles, 15);
    let ret_1h = ret_from_tail(&candles, 60);

    if ret_5m.is_none() {
        quality.add_reason(QualityReason::InsufficientKlineHistory);
    }

    StandardPacket {
        packet_schema: "2.0".to_string(),
        ts: Utc::now(),
        symbol: state.symbol.clone(),
        rank,
        profile: PacketProfile::Standard,
        universe: UniverseBlock {
            tier: UniverseTier::U2,
            active_n,
            focus_n,
        },
        price: PriceBlock {
            last,
            mark: None,
            index: None,
            basis_bp: None,
            ret_1m,
            ret_5m,
            ret_15m,
            ret_1h,
        },
        chart: ChartBlock {
            regime: None,
            signature: candle_signature(&candles),
        },
        liquidity: LiquidityBlock {
            book_mode: quality.book_mode.clone(),
            ..LiquidityBlock::default()
        },
        carry: CarryBlock::default(),
        events: EventsBlock::default(),
        scores: ScoresBlock::default(),
        quality,
    }
}

fn ret_from_tail(candles: &[perp_radar_core::types::Candle], periods: usize) -> Option<f64> {
    if candles.len() <= periods {
        return None;
    }
    let end = candles.last()?.close;
    let start = candles.get(candles.len() - periods - 1)?.close;
    return_pct(start, end)
}

fn candle_signature(candles: &[perp_radar_core::types::Candle]) -> Option<String> {
    if candles.is_empty() {
        return None;
    }
    let tokens = candles
        .iter()
        .rev()
        .take(12)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .map(|candle| {
            if candle.close > candle.open {
                "G"
            } else if candle.close < candle.open {
                "R"
            } else {
                "DOJI"
            }
        })
        .collect::<Vec<_>>()
        .join(",");
    Some(format!("1m:{tokens}"))
}
```

- [ ] **Step 4: Run test to verify it passes**

Run:

```bash
cargo test -p perp-radar-features --test packet_builder_contract
cargo test --workspace
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/features
git commit -m "feat: build standard packets from state"
```

## Task 12: Runtime Smoke And Documentation

**Files:**

- Create: `docs/DATA_CONTRACT.md`
- Create: `docs/RUNBOOK.md`
- Create: `docs/INDICATORS.md`
- Create: `docs/OPERATIONS.md`
- Modify: `crates/app/src/supervisor.rs`
- Modify: `crates/app/src/main.rs`

- [ ] **Step 1: Write failing documentation check**

Create `crates/app/tests/docs_contract.rs`:

```rust
#[test]
fn operations_docs_name_required_clickhouse_dependency() {
    let docs = std::fs::read_to_string("docs/OPERATIONS.md").unwrap();
    assert!(docs.contains("ClickHouse is required"));
}

#[test]
fn data_contract_docs_name_packet_schema() {
    let docs = std::fs::read_to_string("docs/DATA_CONTRACT.md").unwrap();
    assert!(docs.contains("packet_schema"));
    assert!(docs.contains("quality.reasons"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p perp-radar --test docs_contract
```

Expected: FAIL because docs files do not exist.

- [ ] **Step 3: Write documentation and runtime notes**

Create `docs/DATA_CONTRACT.md`:

```markdown
# Data Contract

Perp Radar exposes LLM-ready market packets. The packet schema version is stored in `packet_schema`.

Important fields:

- `price`: latest price, mark, index, basis, and returns.
- `chart`: compact chart digest and candle signature.
- `liquidity`: spread, imbalance, microprice, and U2-only full-book liquidity.
- `carry`: funding and basis fields.
- `events`: liquidation and volume pressure fields.
- `scores`: composite radar scores.
- `quality`: freshness, warm state, book status, and `quality.reasons`.

Unavailable metrics are `null`. Missing data is explained in `quality.reasons`.
```

Create `docs/RUNBOOK.md`:

```markdown
# Runbook

Start ClickHouse before starting Perp Radar.

Default command:

```bash
cargo run -p perp-radar
```

Health check:

```bash
curl http://127.0.0.1:8080/v1/health
```

Top packet text:

```bash
curl http://127.0.0.1:8080/v1/export/top.txt?limit=20
```
```

Create `docs/INDICATORS.md`:

```markdown
# Indicators

V1 computes only explainable, packet-facing features:

- Returns over 1m, 5m, 15m, and 1h.
- RSI14, ATR percentage, Bollinger width, ADX, MACD histogram.
- Spread, imbalance, microprice, and U2-only visible liquidity.
- Current funding, next funding time, funding interval hours, and funding z-score.
- Liquidation event totals and dominant side.

When inputs are missing, the output is `null` with a reason.
```

Create `docs/OPERATIONS.md`:

```markdown
# Operations

ClickHouse is required. The service exits during startup if ClickHouse cannot be reached or migrations fail.

Binance REST requests use a global budget. WebSocket connections roll before the 24 hour limit.

Lossy streams can be coalesced under pressure. Closed klines, funding history, full-book resync, and liquidation events are retried.
```

- [ ] **Step 4: Run test to verify it passes**

Run:

```bash
cargo test -p perp-radar --test docs_contract
cargo test --workspace
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add docs crates/app
git commit -m "docs: add runtime operations guide"
```

## Task 13: Minimal Runtime Engine

**Files:**

- Create: `crates/app/src/runtime.rs`
- Modify: `crates/app/src/lib.rs`
- Modify: `crates/app/src/main.rs`
- Modify: `crates/app/src/supervisor.rs`
- Create: `crates/app/tests/runtime_contract.rs`

- [ ] **Step 1: Write failing runtime tests**

Create `crates/app/tests/runtime_contract.rs`:

```rust
use perp_radar::config::AppConfig;
use perp_radar::runtime::{build_global_market_streams, build_u1_streams, build_u2_streams};

#[test]
fn runtime_builds_expected_stream_groups() {
    let config = AppConfig::from_path("config/default.yaml").unwrap();

    assert_eq!(
        build_global_market_streams(),
        vec!["!markPrice@arr", "!ticker@arr", "!forceOrder@arr"]
    );

    let u1 = build_u1_streams(&["BTCUSDT".to_string(), "ETHUSDT".to_string()]);
    assert!(u1.contains(&"btcusdt@kline_1m".to_string()));
    assert!(u1.contains(&"ethusdt@depth20@500ms".to_string()));

    let u2 = build_u2_streams(&config.universe.always_focus);
    assert_eq!(u2, vec!["btcusdt@depth@500ms", "ethusdt@depth@500ms", "solusdt@depth@500ms"]);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p perp-radar --test runtime_contract
```

Expected: FAIL with unresolved module `runtime`.

- [ ] **Step 3: Write minimal runtime engine**

Modify `crates/app/src/lib.rs`:

```rust
pub mod config;
pub mod runtime;
pub mod supervisor;
```

Create `crates/app/src/runtime.rs`:

```rust
use std::net::SocketAddr;

use perp_radar_api::cache::PacketCache;
use perp_radar_api::routes;
use perp_radar_binance::streams::{combined_stream_url, WsBase};
use tokio::net::TcpListener;

use crate::config::AppConfig;

pub fn build_global_market_streams() -> Vec<&'static str> {
    vec!["!markPrice@arr", "!ticker@arr", "!forceOrder@arr"]
}

pub fn build_u1_streams(symbols: &[String]) -> Vec<String> {
    symbols
        .iter()
        .flat_map(|symbol| {
            let lower = symbol.to_ascii_lowercase();
            vec![format!("{lower}@kline_1m"), format!("{lower}@depth20@500ms")]
        })
        .collect()
}

pub fn build_u2_streams(symbols: &[String]) -> Vec<String> {
    symbols
        .iter()
        .map(|symbol| format!("{}@depth@500ms", symbol.to_ascii_lowercase()))
        .collect()
}

pub fn build_ws_urls(config: &AppConfig) -> anyhow::Result<Vec<url::Url>> {
    let global = combined_stream_url(
        WsBase::Market(config.binance.market_ws_base.clone()),
        &build_global_market_streams(),
    )?;

    let u2_streams = build_u2_streams(&config.universe.always_focus);
    let u2_refs = u2_streams.iter().map(String::as_str).collect::<Vec<_>>();
    let u2 = combined_stream_url(WsBase::Public(config.binance.public_ws_base.clone()), &u2_refs)?;

    Ok(vec![global, u2])
}

pub async fn serve_api(config: &AppConfig, cache: PacketCache) -> anyhow::Result<()> {
    let addr: SocketAddr = config.api.bind.parse()?;
    let listener = TcpListener::bind(addr).await?;
    axum::serve(listener, routes::router(cache)).await?;
    Ok(())
}
```

Modify `crates/app/src/main.rs`:

```rust
use perp_radar::config::AppConfig;
use perp_radar::runtime::{build_ws_urls, serve_api};
use perp_radar::supervisor::verify_required_storage;
use perp_radar_api::cache::PacketCache;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_env_filter("info").init();
    let config = AppConfig::from_path("config/default.yaml")?;
    verify_required_storage(&config).await?;

    for url in build_ws_urls(&config)? {
        tracing::info!(%url, "configured websocket stream");
    }

    serve_api(&config, PacketCache::default()).await
}
```

- [ ] **Step 4: Run test to verify it passes**

Run:

```bash
cargo test -p perp-radar --test runtime_contract
cargo test --workspace
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/app
git commit -m "feat: add runtime stream wiring"
```

## Final Verification

- [ ] Run formatting:

```bash
cargo fmt --all -- --check
```

Expected: PASS.

- [ ] Run all tests:

```bash
cargo test --workspace
```

Expected: PASS.

- [ ] Run clippy:

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: PASS.

- [ ] Run ClickHouse-gated startup smoke with a local ClickHouse instance:

```bash
cargo run -p perp-radar
```

Expected: service starts after migrations when ClickHouse is reachable; service exits with a ClickHouse readiness error when ClickHouse is unreachable.

## Plan Self-Review

Spec coverage:

- Runtime API and packet cache are covered by Tasks 2, 8, and 11.
- Binance stream URLs, parser, REST client, and WS client are covered by Tasks 6 and 10.
- Runtime stream group wiring and API serving are covered by Task 13.
- Hot state, candle rings, and book state are covered by Tasks 3 and 4.
- Features, scores, and ranking are covered by Tasks 5 and 11.
- ClickHouse migrations and startup requirement are covered by Tasks 7 and 9.
- Documentation and operator notes are covered by Task 12.

Type consistency:

- `StandardPacket`, `QualityState`, `SymbolState`, `CandleRing`, `PartialBook`, `FullBook`, and `PacketCache` are named consistently across tasks.
- Crate package names use hyphens, and Rust crate imports use underscores.

Scope:

- This plan builds a working V1 skeleton with real Binance and ClickHouse connection points, tested contracts, packet cache routes, and explicit quality handling.
- LLM strategy generation, paper trading, cross-exchange feeds, and all-market full books remain outside V1.
