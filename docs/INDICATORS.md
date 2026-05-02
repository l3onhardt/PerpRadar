# Indicators

Perp Radar exposes explainable packet-facing features and Packet 2.1 formal indicators. These fields are intended to support ranking, alerting, audit, and LLM summaries without hiding the source of each value.

## Price And Returns

Return features describe recent price movement across configured windows. They explain whether a packet is being ranked because of short-term momentum, reversal, or unusually quiet price action.

## Chart Features

Chart features include RSI, ATR, Bollinger Bands, ADX, and MACD:

- RSI summarizes momentum and overbought or oversold pressure.
- ATR summarizes realized volatility.
- Bollinger Band position and width summarize relative price extension and compression.
- ADX summarizes trend strength.
- MACD summarizes moving-average momentum and signal-line behavior.

## Liquidity Features

Liquidity features describe executable market conditions. Spread, order book imbalance, microprice, and depth-based liquidity help explain whether a move is supported by book pressure or whether execution may be fragile.

## Carry Features

Funding fields describe perp carry. Positive or negative funding can explain positioning cost, crowded long or short pressure, and carry-driven ranking context.

## Events

Liquidation features describe liquidation pressure and side when available. Event fields may also explain abnormal moves, unavailable source data, or degraded packet construction.

## Null And Reason Behavior

Packet fields may be `null` when their source stream, REST input, bounded history, or prerequisite state is unavailable. A null indicator is not zero or neutral. Consumers should read `quality.reasons` and `score_meta.<score>.missing` to explain missing indicators instead of inventing values.

## Packet 2.1 Scores

Formal `scores` contains `LRI`, `TCS`, `DPI5`, `DPI10`, `CSI`, `RPI`, and `VoV`. Packet output does not convert these indicators into trade instructions.

Packet 2.1 keeps prior score meanings under `legacy_scores` during migration:

- `candidate_score`
- `liquidation_event_score`
- `compression_score`
- `momentum_abs_score`
- `volume_spike_z`
- `notional_imbalance_i5`

`DPI5` and `DPI10` are quantity imbalance indicators from trusted full-book top levels. If full-book state is unavailable or sequence quality is not trusted, these formal scores are `null` and `score_meta` explains why.

The remaining formal scores are computed from bounded in-memory history:

- `LRI` uses trusted full-book spread, visible liquidity within 5 bp, and max buy/sell slippage for the configured notional.
- `TCS` uses ADX14, trend sign versus EMA200, EMA50 slope, and bounded Bollinger width percentile.
- `CSI` uses absolute funding z and absolute basis.
- `RPI` uses RSI extreme, same-side funding pressure, and book pressure against the 1h move.
- `VoV` uses ATR percent delta ratio, not volume spike.

If a score is unavailable because the bounded window is still warming, the packet keeps the score `null` and reports a concrete `score_meta.<score>.missing` reason.
