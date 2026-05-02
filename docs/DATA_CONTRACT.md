# Data Contract

Perp Radar emits LLM-ready market packets: compact, explainable snapshots that combine price action, chart state, liquidity, carry, event context, scores, and data quality into one JSON object per symbol. Packets are designed to be consumed by humans, APIs, and language models without requiring access to raw exchange streams.

## packet_schema

`packet_schema` identifies the shape and version of the emitted packet. Consumers should treat it as the compatibility key for parsing and prompt templates. A schema change can add fields, but consumers should not assume an omitted or `null` value means zero.

Current development schema: `2.1`.

The packet contains these field groups:

- `price`: latest trade or mark context, including the observable price level and recent return windows.
- `chart`: candle-derived technical features such as RSI, ATR, Bollinger Band state, ADX, MACD, and trend or volatility summaries.
- `liquidity`: order book and execution-friction features such as spread, imbalance, microprice, depth, and available liquidity.
- `carry`: funding-rate and basis context used to explain perp positioning cost or carry pressure.
- `events`: notable market events, including liquidation pressure, side, abnormal moves, feed gaps, or other explainable triggers.
- `scores`: formal indicator values for `LRI`, `TCS`, `DPI5`, `DPI10`, `CSI`, `RPI`, and `VoV`. Values may be `null` while history or trusted inputs are unavailable.
- `score_meta`: per-score audit metadata, including formulas, component snapshots, direction notes, and missing reasons.
- `legacy_scores`: Packet 2.0 score meanings retained under explicit names during migration.
- `quality`: data completeness and trust metadata for the packet.

Packet 2.1 moves the old candidate ranking and alert-style score meanings out of `scores`:

- old `scores.TCS` -> `legacy_scores.candidate_score`
- old `scores.LRI` -> `legacy_scores.liquidation_event_score`
- old `scores.CSI` -> `legacy_scores.compression_score`
- old `scores.RPI` -> `legacy_scores.momentum_abs_score`
- old `scores.VoV` -> `legacy_scores.volume_spike_z`
- old `scores.DPI5` -> `legacy_scores.notional_imbalance_i5`

Formal `scores.DPI5` and `scores.DPI10` use top-N quantity imbalance from trusted full-book state when available.

Formal `LRI`, `TCS`, `CSI`, `RPI`, and `VoV` are computed from bounded score-history windows. During warmup, these scores remain `null` with score-specific missing reasons rather than falling back to legacy meanings.

## Quality Fields

`quality.reasons` is a list of human-readable reason codes explaining why fields may be degraded, delayed, partial, or unavailable. Consumers should surface these reasons alongside generated explanations so downstream users can distinguish a true market signal from missing data.

Common quality states include complete data, partial data, stale data, missing source streams, unavailable REST backfill, or values withheld because prerequisites were not observed.

## Null Semantics

`null` means unknown, unavailable, stale, or not computable from the current inputs. It does not mean zero, neutral, false, or safe. Consumers should preserve nulls and use `quality.reasons` to explain them. If a numeric field is required for a downstream calculation, that calculation should be skipped or explicitly marked degraded when the source value is `null`.
