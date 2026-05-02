# PerpRadar 仓库审计与 AI 接手文档

审计时间：2026-05-02 UTC  
当前分支：`main`  
项目定位：Binance USD-M 永续合约实时雷达。服务从 Binance REST/WebSocket 或本地 mock Binance 获取行情，维护内存热状态，计算 LLM-ready market packet，并通过 HTTP API 暴露。

本文的目标是让新接手的 AI 即使不能查看仓库，也能理解项目工作逻辑、架构边界、指标计算方式，并快速定位核心问题。

## 当前运行状态

本机 API 当前可访问：

```bash
curl http://127.0.0.1:18080/v1/health
# {"ok":true}
```

当前 top packets 有 `BTCUSDT`、`ETHUSDT`、`SOLUSDT` 三个 symbol，均为 `quality.book_mode="full"`、`quality.book_seq_ok=true`、`quality.stale=false`，`freshness_ms` 约百毫秒级。包内配置值显示 `universe.active_n=15`、`universe.focus_n=3`。

注意：`docker compose ps` 在当前 shell 中因为 `/var/run/docker.sock` 权限失败，不能直接确认容器状态；但 API 可用，且 `logs/live-monitor.jsonl` 记录了先前运行中的 `perp-radar`、`mock-binance`、`clickhouse` 容器 stats。

全量测试已通过：

```bash
cargo test --workspace
```

结果覆盖 `app/api/binance/core/features/state/storage` 的 contract tests、integration tests 和 doc-tests，合计 127 个测试通过，未发现失败。

当前工作区有未提交改动，审计时不能假设这些改动已经入库。`git status --short` 显示业务代码、Docker/compose、文档、工具脚本和 `logs/` 均有改动或新增。

## 已实现与未实现边界

已实现：

- Rust workspace，主二进制为 `perp-radar`。
- 启动时读取 `config/default.yaml`，支持 `PERP_RADAR__...` 环境变量覆盖。
- 启动时强依赖 ClickHouse：先检查连接，再运行 SQL migrations。
- 本地 compose 默认运行 ClickHouse、mock Binance、perp-radar。
- 支持真实 Binance endpoint override，见 `docker-compose.real.yml`。
- Binance REST bootstrap：focus symbols 的 1m K 线、depth snapshot、funding history、premium index。
- Binance WebSocket ingestion：全市场 mark price、ticker、force order；focus symbols 的 kline、partial depth、full depth delta。
- 内存热状态 `SymbolState`：每个 symbol 聚合 K 线、mark/index/funding、ticker、order book、liquidation、quality。
- packet 构建：价格、技术指标、流动性、资金费率、强平事件、综合分数、质量信息。
- 内存 `PacketCache`：按 symbol 保存最新 packet，API 请求直接读取缓存。
- HTTP API：health/schema/universe/symbols/single packet/top packets/text export/jsonl export/debug routes。
- ClickHouse migrations 已定义 6 张表。
- 单元和契约测试覆盖 parser、runtime、packet builder、指标、book、migration、API。

未完全实现或需要注意：

- ClickHouse 目前只作为启动依赖和 migration target；主 runtime 没有把实时 K 线、features、latest packets 批量写入 ClickHouse。`crates/storage/src/batcher.rs` 只有 flush 判定，没有 writer 接入。
- `/v1/universe` 当前由 packet cache 推导，返回的是缓存中 packet 数量。当前接口返回 `active_n=3`，但 packet 内 `universe.active_n=15`。这是接口语义不一致，定位在 `crates/api/src/routes.rs` 的 `universe()`。
- `universe.refresh_sec` 和 `hysteresis_rank_buffer` 存在配置字段，但当前 runtime recompute 是每条 WS payload 后立即执行，没有真正按 refresh interval 或 hysteresis 实现。
- WebSocket reconnect policy 有结构和 debug 文案，但 `stream_text_messages()` 本身只读到断开；外层固定 2 秒 sleep 重连，未使用 `ReconnectPolicy` 的指数退避。
- `active_n=15` 的设计是 U0 全市场选 15 个活跃 symbol，但当前 WebSocket 的 kline/depth/full-book stream 只为 `always_focus` 构建。全市场 ticker/mark/forceOrder 可以创建其他 symbol state，但没有为动态 active/focus symbols 动态重订阅 K 线和深度流。
- `focus_symbols` 在 recompute 中来自 U0 ranking 的 top N，不强制等于 `always_focus`；但 full book stream 仍只订阅 `always_focus`。若 ranking 选出非 pinned symbol，它可能缺少 K 线/full book 指标。
- `funding_interval_hours` 固定传 8，`funding_unit` 标记为 `8h_estimate`，未接 Binance `fundingInfo` 获取真实 interval。
- `logs/live-monitor.py` 会把 `LRI` 为空视为问题；mock Binance 通常没有 forceOrder，所以本地长期监控会 `ok=false` 且问题为 `lri_unavailable`。这不等价于 runtime 故障。

## 启动方式

本地推荐：

```bash
docker compose up --build
```

本地 compose endpoint：

- REST: `http://mock-binance:9000`
- market WS: `ws://mock-binance:9000/market`
- public WS: `ws://mock-binance:9000/public`
- API: `http://127.0.0.1:18080`
- ClickHouse HTTP: `http://127.0.0.1:8123`

直跑 cargo 需要先有 ClickHouse：

```bash
PERP_RADAR__API__BIND=127.0.0.1:18080 cargo run -p perp-radar
```

真实 Binance 验证：

```bash
docker compose -f docker-compose.yml -f docker-compose.real.yml up --build
```

真实环境需要从可访问 Binance USD-M Futures 的 VPS 运行。

## 仓库架构

workspace crates：

- `crates/core`：公共类型、packet schema、quality、时间工具。
- `crates/binance`：REST client、WebSocket stream URL、WebSocket text reader、Binance payload parser、rate limiter。
- `crates/state`：每个 symbol 的热状态，K 线 ring buffer，partial/full order book，book 序列校验。
- `crates/features`：技术指标、流动性质量、funding z-score、U0 ranking、packet builder、综合分数。
- `crates/storage`：ClickHouse client、migration renderer/runner、batch config。
- `crates/api`：PacketCache、axum routes、debug route、文本/JSONL export。
- `crates/app`：配置、supervisor、runtime engine、ingestion tasks、API server、main binary。

关键配置：

- `config/default.yaml`：默认 Binance endpoint、universe、storage、api、packets。
- `docker-compose.yml`：本地 mock stack。
- `docker-compose.real.yml`：真实 Binance endpoint override。
- `migrations/*.sql`：ClickHouse 表结构。
- `tools/live-monitor.py`：周期采样 API 和 docker stats。
- `tools/validate-live-indicators.py`：校验 top packets 指标完整性。

## 主流程

1. `crates/app/src/main.rs`
   - 初始化 tracing。
   - `AppConfig::from_path("config/default.yaml")` 读取配置和 env override。
   - `verify_required_storage()` 检查 ClickHouse 并执行 migrations。
   - `build_ws_urls()` 生成 4 类 combined stream URL。
   - 创建 `PacketCache`。
   - `start_ingestion_tasks()` 启动 ingestion。
   - `serve_api()` 启动 axum API。

2. `start_ingestion_tasks()`
   - 创建 mpsc channel，WebSocket tasks 将原始 text payload 发到 channel。
   - 创建 `RuntimeEngine`，初始 symbols 来自 `config.universe.always_focus`。
   - bootstrap 顺序：
     - REST klines：每个 focus symbol 取 500 条 1m K 线。
     - REST depth snapshot：每个 focus symbol 取 1000 档深度。
     - REST funding history：每个 focus symbol 取 126 条 funding rate。
     - REST premium index：mark/index/funding/next funding。
   - 事件循环：
     - 收到 WS payload 后 `engine.apply_json()`。
     - 每条 payload 后 `engine.recompute_universe()`。
     - 如果 full book sequence gap，REST 重新 bootstrap depth。
     - 每 1 秒 `engine.age_all()` 更新 stale/freshness。

3. `RuntimeEngine.apply_event()`
   - Binance parser 输出 `BinanceEvent`。
   - 按事件类型更新对应 `SymbolState`。
   - 每次状态接受更新后调用 `refresh_symbol()` 重新构建 packet 并 upsert 到 `PacketCache`。

4. API 请求
   - `crates/api/src/routes.rs` 直接读取 `PacketCache`。
   - `/v1/packets/top` 按 packet.rank 升序返回。
   - `/v1/packet/:symbol` 支持大小写 symbol。
   - export endpoints 把 packet 转成 text 或 JSONL。

## WebSocket 订阅

`build_ws_urls()` 生成：

- global market streams：`!markPrice@arr`、`!ticker@arr`、`!forceOrder@arr`。
- U1 kline streams：`{symbol}@kline_1m`，symbols 来自 `always_focus`。
- U1 depth20 streams：`{symbol}@depth20@500ms`，symbols 来自 `always_focus`。
- U2 full depth streams：`{symbol}@depth@500ms`，symbols 来自 `always_focus`。

默认 `always_focus=["BTCUSDT","ETHUSDT","SOLUSDT"]`，因此本地实际深度/K 线完整的通常只有三大币。

## API 契约

主要 endpoints：

- `GET /v1/health` -> `{"ok":true}`
- `GET /v1/schema` -> packet schema 和 route 列表
- `GET /v1/universe` -> active/focus symbols，当前由 cache 推导
- `GET /v1/symbols` -> cache 中 symbol 列表
- `GET /v1/packet/:symbol` -> 单 symbol latest packet
- `GET /v1/packets/top?limit=N` -> rank 排序 packets，默认 20，最大 100
- `GET /v1/export/top.txt?limit=N` -> LLM-readable 文本
- `GET /v1/export/top.jsonl?limit=N` -> JSONL
- `GET /v1/debug/ws`
- `GET /v1/debug/rate_limits`

packet schema 版本：`packet_schema="2.0"`。字段组：

- `price`
- `chart`
- `liquidity`
- `carry`
- `events`
- `scores`
- `quality`

`null` 语义：未知、不可用、数据不足、源缺失或计算前置条件不满足；不能当成 0 或中性值。

## ClickHouse 表

migrations 会创建：

- `symbols`：交易对静态信息。
- `klines_1m`：1m K 线。
- `mark_funding_sample`：mark/index/basis/funding/next funding。
- `depth_features_1s`：1 秒深度特征。
- `features_1m`：1 分钟聚合特征和分数。
- `latest_packets`：最新 packet JSON。

当前主流程没有实际插入这些表。若排查“为什么 ClickHouse 没数据”，优先看 `crates/app/src/runtime.rs` 是否接入 writer；当前没有。

## 状态模型

`SymbolState` 维护：

- `candles_1m: CandleRing`，容量 runtime 默认 1500。
- `mark_price`、`index_price`、`funding_rate`、`funding_history`、`next_funding_time`。
- `last_price`、`quote_volume_24h`、`price_change_percent_24h`。
- `partial_book`。
- `full_book`。
- `liquidations`，最多保留 512 条。
- `quality`。
- `last_event_time_ms`。

K 线只接受已闭合 candle。相同 open time 会替换最后一根；旧 candle 忽略；如果新 candle open time 大于上一根 close 后的预期下一分钟，则累加 `quality.kline_gap_1m`。

full book 从 REST snapshot 初始化，WebSocket delta 校验 sequence：

- 首个 delta：`first_update_id <= snapshot.last_update_id && final_update_id >= snapshot.last_update_id`。
- bootstrapped 后：`previous_final_update_id == current.last_update_id`。
- gap 时 `quality.book_seq_ok=false` 并添加 `FullBookSequenceGap`，runtime 会触发 REST depth resync。

## 指标计算方式

以下公式来自当前代码实现，不是理论目标。

### Price

`price.last`：

- 优先使用 ticker 的 `last_price`。
- 如果没有 ticker，则使用最新 1m candle close。

`price.mark`：mark price stream 或 premium index REST。

`price.index`：index price stream 或 premium index REST。

`price.basis_bp`：

```text
(mark - index) / index * 10000
```

如果 mark/index 缺失、非有限数或 index 为 0，则为 `null`。

`ret_1m`、`ret_5m`、`ret_15m`、`ret_1h`：

```text
(end_close - start_close) / start_close
```

其中 `end_close` 是最新 closed candle close，`start_close` 是 tail 中 `minutes + 1` 根之前的 close。缺少足够 K 线、非有限数或 start 为 0 时为 `null`。

### Chart

技术指标要求至少 50 根合法 candle；否则整个 technical snapshot 为 `null`，只可能保留 `chart.signature`。

合法 candle 条件：

- open/high/low/close/volume 均为有限数。
- high >= low。
- high、low、close > 0。
- volume_base、volume_quote >= 0。

`chart.signature`：

```text
1m:<last up to 12 candle colors joined by comma>
```

每根 candle：

- `G`：close > open
- `R`：close < open
- `DOJI`：close == open

`ema_20`、`ema_50`：

```text
seed = first period values average
multiplier = 2 / (period + 1)
ema_next = (value - ema_prev) * multiplier + ema_prev
```

`rsi_14`：

使用最近 `period + 1` 个 close，简单 RSI，不是 Wilder 平滑：

```text
gain_sum = sum(max(delta, 0))
loss_sum = sum(abs(min(delta, 0)))
RS = gain_sum / loss_sum
RSI = 100 - 100 / (1 + RS)
```

特殊值：

- gain=0 且 loss=0 -> 50
- loss=0 -> 100

`macd_histogram`：

```text
MACD = EMA12(close) - EMA26(close)
signal = EMA9(MACD series)
histogram = MACD - signal
```

需要至少 35 个 close。

`atr_pct`：

True Range：

```text
TR = max(high - low, abs(high - previous_close), abs(low - previous_close))
ATR = average(last 14 TR)
atr_pct = ATR / latest_close
```

`bb_width`：

使用最近 20 个 close：

```text
mean = average(close)
stddev = sqrt(sum((close - mean)^2) / 20)
bb_width = 4 * stddev / mean
```

这里的 4 表示上下 2 sigma 总宽度。

`adx_14`：

当前实现是简化版，只用最近 14 个 DM/TR 聚合：

```text
up_move = current_high - previous_high
down_move = previous_low - current_low
+DM = up_move if up_move > down_move and up_move > 0 else 0
-DM = down_move if down_move > up_move and down_move > 0 else 0
ATR_sum = sum(last 14 TR)
+DI = 100 * sum(last 14 +DM) / ATR_sum
-DI = 100 * sum(last 14 -DM) / ATR_sum
ADX = abs(+DI - -DI) / (+DI + -DI) * 100
```

如果分母为 0，则 ADX 为 0；如果 ATR_sum 为 0，则为 `null`。

`vwap_20`：

```text
typical_price = (high + low + close) / 3
vwap = sum(typical_price * volume_base) / sum(volume_base)
```

使用最近 20 根 candle，volume 总和为 0 时为 `null`。

`cmf_20`：

```text
money_flow_multiplier = ((close - low) - (high - close)) / (high - low)
money_flow_volume = money_flow_multiplier * volume_base
cmf = sum(money_flow_volume) / sum(volume_base)
```

如果 high==low，则该 candle 的 money flow volume 记 0。使用最近 20 根。

`chart.regime`：

```text
direction =
  trend_up   if ema_20 > ema_50
  trend_down if ema_20 < ema_50
  range      otherwise

if adx_14 >= 20:
  regime = direction
else if bb_width < 0.03:
  regime = compression
else:
  regime = range
```

### Liquidity

Partial book 指标来自 `PartialBook`，通常是 depth20。

`best_bid`：bids 第一档。  
`best_ask`：asks 第一档。  
`mid`：

```text
(best_bid + best_ask) / 2
```

`spread_bp`：

```text
(best_ask - best_bid) / mid * 10000
```

`i1`、`i5`：

```text
bid_notional = sum(price * qty for top N bids)
ask_notional = sum(price * qty for top N asks)
imbalance = (bid_notional - ask_notional) / (bid_notional + ask_notional)
```

N=1 得 `i1`，N=5 得 `i5`。分母为 0 时为 `null`。

`microprice_bp`：

```text
microprice = (ask_price * bid_qty + bid_price * ask_qty) / (bid_qty + ask_qty)
microprice_bp = (microprice - mid) / mid * 10000
```

top qty 总和为 0 时为 `null`。

Full book 指标来自 REST snapshot + full depth delta。

`liq_5bp_usd`、`liq_10bp_usd`：

```text
bid_floor = mid * (1 - max_distance_bp / 10000)
ask_ceiling = mid * (1 + max_distance_bp / 10000)
bid_notional = sum(price * qty for bids with price >= bid_floor)
ask_notional = sum(price * qty for asks with price <= ask_ceiling)
visible_liquidity_usd = bid_notional + ask_notional
```

`slip_10000_buy_bp`、`slip_10000_sell_bp`：

- 买入：从 asks 最优价开始吃单，直到累计 notional 达到 10000 USD。
- 卖出：从 bids 最优价开始吃单，直到累计 notional 达到 10000 USD。

```text
average_price = spent_notional / acquired_qty
slippage_bp = abs(average_price - mid) / mid * 10000
```

深度不足以成交 10000 USD 时为 `null`。

`book_depth_coverage_bp`：

- partial book 时取 bid/ask 最后一档相对 mid 的覆盖 bp，并取两边最小值。
- full book snapshot 接受后当前直接设为 10.0，只要能计算 10bp visible liquidity。

### Carry

`funding_now`：当前 funding rate。

`funding_unit`：当前实现为 `"8h_estimate"`。

`funding_interval_hours`：当前实现固定 8。

`funding_z_7d`：

```text
mean = average(funding_history)
variance = sum((rate - mean)^2) / (n - 1)
stddev = sqrt(variance)
z = (current_funding_rate - mean) / stddev
```

需要 funding history 至少 2 个点且 stddev > 0。

`next_funding_time`：来自 mark price stream 或 premium index REST 的 timestamp。

### Events

`liq_1m_usd`、`liq_5m_usd`、`liq_15m_usd`：

```text
latest = max(liquidation.event_time_ms)
sum(notional_usd for events where latest - event_time_ms <= window_ms)
```

window 分别是 60000、300000、900000 ms。没有 liquidation event 时为 `null`。

`notional_usd`：

```text
price * qty
```

`liq_side`：

统计最近 5 分钟：

- Binance force order side `SELL` 计入 long liquidation。
- side `BUY` 计入 short liquidation。
- long notional 大于 short -> `"long"`。
- short notional 大于 long -> `"short"`。
- 相等或无数据 -> `null`。

`volume_spike_z`：

```text
history = previous up to 20 candles' volume_quote
current = latest candle volume_quote
volume_spike_z = z_score(history, current)
```

z-score 与 funding z-score 同一公式。需要 history 至少 2 个点且 stddev > 0。

### Scores

`DPI5`：

```text
DPI5 = liquidity.i5
```

`LRI`：

```text
LRI = min(liq_5m_usd / 1_000_000, 3.0)
```

没有 5 分钟 liquidation 数据时为 `null`。

`CSI`：

```text
CSI = max(0.1 - bb_width, 0) * 10
```

如果 `bb_width` 不可计算，则为 `null`。packet builder 给 TCS 输入时会用 0 fallback。

`RPI`：

```text
RPI = abs(ret_15m)
```

如果 15m 不足，则 fallback 到 `abs(ret_5m)`。

`VoV`：

```text
VoV = volume_spike_z
```

`liquidity_quality`：

```text
spread_component = clamp(1 - spread_bp / 20, 0, 1)
coverage_component = clamp(book_depth_coverage_bp / 10, 0, 1)
liquidity_quality = (spread_component + coverage_component) / 2
```

`TCS`：

TCS 是 composite candidate score：

```text
TCS =
  0.25 * volume_accel_z
+ 0.20 * ret_15m_z_abs
+ 0.15 * atr_pctile
+ 0.15 * funding_z_abs
+ 0.10 * liquidation_event_score
+ 0.10 * squeeze_or_breakout_score
+ 0.05 * liquidity_quality
```

当前传入值并不完全是标准化 percentile/z：

- `volume_accel_z` = `volume_spike_z`，没有时 fallback 0。
- `ret_15m_z_abs` = `abs(ret_15m)`，不足则 `abs(ret_5m)`。
- `atr_pctile` = `atr_pct`，没有则 fallback `abs(ret_5m)`。
- `funding_z_abs` = `min(abs(funding_rate) / 0.0001, 5.0)`。
- `liquidation_event_score` = `LRI`，没有时 fallback 0。
- `squeeze_or_breakout_score` = `CSI`，没有时 fallback 0。
- `liquidity_quality` 必须可计算。

所有 TCS 输入都必须非空且 finite，否则 TCS 为 `null`。

### U0 Universe Ranking

`rank_u0_universe()` 输入来自所有 `SymbolState`：

- `quote_volume_24h`
- `price_change_percent_24h`
- `funding_rate`
- `liquidation_5m_usd`
- `ret_15m`

只有 `quote_volume_24h` 和 `price_change_percent_24h` 是必须值。

分数：

```text
volume_score = clamp(ln(1 + quote_volume_24h) / 20, 0, 2)
price_score = clamp(abs(price_change_percent_24h) / 5, 0, 2)
funding_stress = min(abs(funding_rate) / 0.0001, 6), missing -> 0
liquidation_score = clamp(ln(1 + liquidation_5m_usd) / 14, 0, 2), missing -> 0
momentum = min(abs(ret_15m) * 100, 6), missing -> 0

U0 score =
  0.45 * volume_score
+ 0.20 * price_score
+ 0.15 * funding_stress
+ 0.15 * liquidation_score
+ 0.05 * momentum
```

排序按 score 降序，同分按 symbol 字母升序。取 `active_n` 个。

active symbols 构建：

1. 先放入 pinned symbols，也就是构造 engine 时传入的初始 symbols，最多 `active_n` 个。
2. 再从 U0 ranked candidates 补足到 `active_n`。

focus symbols 当前直接取 U0 ranked top `focus_n`，不额外 pin `always_focus`。

packet rank 来自 active symbols 中的位置；找不到时 rank 默认为 1。

## Quality 规则

`QualityState` 字段：

- `freshness_ms`
- `warm`
- `kline_gap_1m`
- `book_mode`
- `book_seq_ok`
- `book_depth_coverage_bp`
- `funding_history_points`
- `stale`
- `reasons`

初始化：

- `freshness_ms = u64::MAX`
- `warm = false`
- `book_mode = "none"`
- `stale = true`

warm：

- 接受至少 2 根 closed 1m candle 后为 true。

stale：

```text
freshness_ms = now_ms - last_event_time_ms
stale = freshness_ms > stale_after_ms
```

runtime 中：

```text
stale_after_ms = packets.standard_interval_ms * 15
```

默认 `standard_interval_ms=1000`，所以 stale 阈值 15000 ms。

quality reasons：

- `InsufficientKlineHistory`：packet 构建时 candles 长度 <= 5。
- `InsufficientFundingHistory`：当前 funding_rate 缺失。
- `depth_coverage_lt_5bp`：book depth coverage 小于 5bp。
- `FullBookSequenceGap`：full book delta sequence gap。
- `StaleMarketData`：没有 event time 或 freshness 超阈值。
- `MissingMarkPrice`：mark price 缺失。
- `MissingIndexPrice`：index price 缺失。

## 快速定位核心问题

服务无法启动：

- 看 `crates/app/src/main.rs` 和 `crates/app/src/supervisor.rs`。
- 多数是 ClickHouse 不可达或 migrations 失败。
- 检查 `PERP_RADAR__STORAGE__CLICKHOUSE_URL` 和 ClickHouse health。

API health OK 但 packets 为空：

- 看 `crates/app/src/runtime.rs` bootstrap 是否失败。
- 看 REST endpoint 是否可达。
- 看 WebSocket 是否连上。
- 本地看 mock Binance 是否启动。

packet 有数据但大量 `null`：

- 技术指标 null：K 线少于 50 或 candle 非法，查 `crates/features/src/ta.rs`。
- funding null：mark/premium index 或 funding history 缺失，查 `bootstrap_focus_funding_history()` 和 `bootstrap_focus_premium_index()`。
- liquidity full-book null：depth snapshot/delta 未接入或 seq gap，查 `crates/state/src/book_full.rs`。
- liquidation/LRI null：没有 forceOrder 事件；mock 本地常见，不一定是故障。

full book sequence gap：

- 查 `FullBook::apply_delta()`。
- runtime 会调用 `resync_focus_depths()` 重新拉 REST depth snapshot。
- 如果持续 gap，重点查 Binance stream URL、mock stream shape、REST snapshot/delta sequence 对齐。

排行不符合预期：

- U0 排名公式在 `crates/features/src/ranking.rs`。
- active/focus 重算在 `RuntimeEngine::recompute_universe()`。
- 当前 dynamic focus 与实际订阅不完全一致，是关键设计风险。

`/v1/universe` 数字不对：

- 该接口当前不是读 `RuntimeEngine.debug_snapshot()`，而是读 `PacketCache.top()` 推导。
- 修复点在 `crates/api/src/routes.rs`，需要把真实 universe 状态传给 API，或按 packet 内 universe config 返回。

ClickHouse 没有行情数据：

- 这是当前实现边界，不是单纯 bug。
- migrations 已跑，但 runtime 没有 insert writer。
- 需要设计并接入 storage writer，把 accepted state/packet/features 写入对应表。

真实 Binance 不通：

- 本地 compose 默认 mock，不证明真实 Binance 可达。
- 用 Binance 可达地区 VPS 跑 `docker-compose.real.yml`。
- 如果区域封锁，属于基础设施问题。

## 后续优先级建议

1. 修复 `/v1/universe` 语义，让 API 返回真实 configured active/focus 和实际 runtime universe。
2. 明确 dynamic universe 设计：active/focus 变动后是否要动态重订阅 kline/depth/full book。
3. 接入 ClickHouse writer，至少写 `latest_packets`，再写 K 线和 features。
4. 让 `hysteresis_rank_buffer`、`refresh_sec` 真正参与 universe recompute。
5. 使用 `ReconnectPolicy` 的 bounded exponential backoff，并记录 reconnect/gap metrics。
6. 把 `live-monitor.py` 对 `LRI` 的校验改成可配置；mock 环境不应因为没有强平事件持续失败。
7. 增加真实 Binance 2 小时 VPS 验证记录。

## 最小接手检查清单

```bash
git status --short
cargo test --workspace
curl http://127.0.0.1:18080/v1/health
curl http://127.0.0.1:18080/v1/universe
curl "http://127.0.0.1:18080/v1/packets/top?limit=3"
```

重点观察：

- health 是否 OK。
- top packets 是否非空。
- `quality.stale` 是否 false。
- `quality.book_mode` 是否 full。
- `quality.book_seq_ok` 是否 true。
- `quality.reasons` 是否为空或可解释。
- packet 内 `universe.active_n/focus_n` 与 `/v1/universe` 是否一致。
- `LRI` 为 null 时先确认是否确实没有 forceOrder 数据。

