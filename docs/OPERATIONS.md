# Operations

ClickHouse is required.

Perp Radar depends on ClickHouse for runtime storage and migrations. Start ClickHouse before launching the application. On startup, the current binary checks storage connectivity, applies migrations, logs configured websocket URLs, serves the API, and exits if ClickHouse is missing or if migration execution fails.

## Startup

Default local startup with Docker Compose:

```bash
docker compose up --build
```

The compose stack starts ClickHouse, a local mock Binance REST fixture, and Perp Radar with environment overrides. This is the recommended local smoke path because some development networks cannot access Binance Futures directly.

Direct cargo startup remains available when ClickHouse is already running:

```bash
cargo run -p perp-radar
```

After startup succeeds, run:

```bash
curl http://127.0.0.1:8080/v1/health
```

Use this API check to confirm the HTTP runtime is serving requests.

## Binance REST Budget

Binance REST calls are budgeted and should be treated as a constrained resource. Use REST for snapshots, backfills, and recovery paths, while keeping steady-state runtime on websocket streams where possible. If the budget is exhausted or REST data is unavailable, packet fields should degrade with quality reasons instead of being silently fabricated.

## Websocket Runtime

Binance websocket connections should be treated as 24h rolling sessions. Plan for reconnects before or after the exchange-enforced session window, and expect stream gaps during reconnects.

V1 keeps all-market mark/ticker/forceOrder as the U0 radar layer, then promotes ranked symbols into the active and focus pools. The default stable local/VPS configuration is `active_n=15` and `focus_n=3`; `always_focus` symbols stay pinned in the active pool even when U0 ranking is volatile.

## Local And VPS Validation

Local validation uses the mock Binance service in compose to verify ClickHouse startup, migrations, API health, packet generation, quality metadata, and recovery behavior without depending on Binance reachability. VPS validation should run the same image with real Binance base URLs from a Binance Futures reachable region and confirm packets refresh continuously for at least two hours.

## Lossy Streams And Retries

Runtime ingestion may coalesce high-frequency updates when downstream processing cannot keep up. Coalescing is lossy by design for fast-moving stream state such as book updates, so consumers should rely on latest-state semantics rather than every intermediate event. Retry transient failures with bounded backoff and record degraded packet quality when retries or reconnects leave missing inputs.
