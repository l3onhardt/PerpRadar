# Runbook

## Start Perp Radar

Start ClickHouse before Perp Radar. The application applies migrations on startup and exits if ClickHouse is unavailable or migrations fail.

Default local startup:

```bash
docker compose up --build
```

This starts ClickHouse, mock Binance, and Perp Radar with local-safe environment overrides.

Direct cargo startup:

```bash
cargo run -p perp-radar
```

After ClickHouse verification succeeds, this command serves the HTTP API on the configured bind address.

## Health Check

```bash
curl http://127.0.0.1:8080/v1/health
```

Use this to confirm the API process is accepting requests.

## Top Packet Text

Fetch the top ranked packet in text form:

```bash
curl "http://127.0.0.1:8080/v1/export/top.txt?limit=1"
```

Use this endpoint for a quick LLM-ready smoke check. The response should include the leading market packet summary and quality context.

With the minimal runtime, empty output is expected until packets have been ingested and stored in the in-memory cache.

## Universe And Debug

```bash
curl http://127.0.0.1:8080/v1/universe
curl http://127.0.0.1:8080/v1/debug/ws
curl http://127.0.0.1:8080/v1/debug/rate_limits
```

Use these endpoints to inspect active/focus symbols, websocket policy, and rate-limit posture during local smoke or VPS validation.

## Handoff

See [HANDOFF.md](HANDOFF.md) for the current V1 branch state, completed 30 minute local long-run result, and VPS acceptance checklist.
