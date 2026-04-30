# Data Contract

Perp Radar emits LLM-ready market packets: compact, explainable snapshots that combine price action, chart state, liquidity, carry, event context, scores, and data quality into one JSON object per symbol. Packets are designed to be consumed by humans, APIs, and language models without requiring access to raw exchange streams.

## packet_schema

`packet_schema` identifies the shape and version of the emitted packet. Consumers should treat it as the compatibility key for parsing and prompt templates. A schema change can add fields, but consumers should not assume an omitted or `null` value means zero.

The packet contains these field groups:

- `price`: latest trade or mark context, including the observable price level and recent return windows.
- `chart`: candle-derived technical features such as RSI, ATR, Bollinger Band state, ADX, MACD, and trend or volatility summaries.
- `liquidity`: order book and execution-friction features such as spread, imbalance, microprice, depth, and available liquidity.
- `carry`: funding-rate and basis context used to explain perp positioning cost or carry pressure.
- `events`: notable market events, including liquidation pressure, side, abnormal moves, feed gaps, or other explainable triggers.
- `scores`: normalized ranking and alert scores derived from the packet-facing features.
- `quality`: data completeness and trust metadata for the packet.

## Quality Fields

`quality.reasons` is a list of human-readable reason codes explaining why fields may be degraded, delayed, partial, or unavailable. Consumers should surface these reasons alongside generated explanations so downstream users can distinguish a true market signal from missing data.

Common quality states include complete data, partial data, stale data, missing source streams, unavailable REST backfill, or values withheld because prerequisites were not observed.

## Null Semantics

`null` means unknown, unavailable, stale, or not computable from the current inputs. It does not mean zero, neutral, false, or safe. Consumers should preserve nulls and use `quality.reasons` to explain them. If a numeric field is required for a downstream calculation, that calculation should be skipped or explicitly marked degraded when the source value is `null`.
