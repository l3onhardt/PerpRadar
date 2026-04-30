# Indicators

Perp Radar V1 exposes explainable packet-facing features. These fields are intended to support ranking, alerting, and LLM summaries without hiding the source of each signal.

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

Packet fields may be `null` when their source stream, REST input, or prerequisite state is unavailable. A null indicator is not zero or neutral. Consumers should read the packet quality metadata and reason list to explain missing indicators instead of inventing values.
