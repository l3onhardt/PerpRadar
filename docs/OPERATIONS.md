# Operations

ClickHouse is required.

Perp Radar depends on ClickHouse for runtime storage and packet materialization. Start ClickHouse before launching the application. On startup, the app checks storage connectivity, applies migrations, and exits if ClickHouse is missing or if migration execution fails.

## Startup

Default local startup:

```bash
cargo run -p perp-radar
```

After startup, run:

```bash
curl http://127.0.0.1:8080/v1/health
```

## Binance REST Budget

Binance REST calls are budgeted and should be treated as a constrained resource. Use REST for snapshots, backfills, and recovery paths, while keeping steady-state runtime on websocket streams where possible. If the budget is exhausted or REST data is unavailable, packet fields should degrade with quality reasons instead of being silently fabricated.

## Websocket Runtime

Binance websocket connections should be treated as 24h rolling sessions. Plan for reconnects before or after the exchange-enforced session window, and expect stream gaps during reconnects.

## Lossy Streams And Retries

Runtime ingestion may coalesce high-frequency updates when downstream processing cannot keep up. Coalescing is lossy by design for fast-moving stream state such as book updates, so consumers should rely on latest-state semantics rather than every intermediate event. Retry transient failures with bounded backoff and record degraded packet quality when retries or reconnects leave missing inputs.
