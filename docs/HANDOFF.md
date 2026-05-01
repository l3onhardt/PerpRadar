# Perp Radar V1 Handoff

## Current State

Branch `feat/perp-radar-v1` has been fast-forward merged into `main`.

Latest commits:

- `085de54 fix: stabilize compose smoke runtime`
- `c5a2c61 feat: harden v1 market ingestion runtime`
- `4554b23 feat: wire v1 indicator ingestion pipeline`

The local Docker path runs `perp-radar`, ClickHouse, and mock Binance with the same `active_n=15` and `focus_n=3` defaults intended for VPS validation.

## Local Validation Completed

Local mock Binance long-run was completed on 2026-05-01.

- Start: `2026-05-01T22:18:50+08:00`
- End: `2026-05-01T22:49:14+08:00`
- Duration: 30 minutes
- Samples: 60, one sample every 30 seconds
- Minimum active symbols: 15
- Minimum focus symbols: 3
- Maximum top-packet `freshness_ms`: 0
- Top symbols during sampling: `BTCUSDT`, `ETHUSDT`, `SOLUSDT`
- Stale packets: none observed
- Full book sequence gaps: none observed
- Partial-book downgrade for focus symbols: none observed
- Container health failures: none observed
- App log matches for `WARN`, `ERROR`, `panic`, `WebSocket protocol error`, or `HTTP version must be 1.1`: none observed

The local result validates the mock path and runtime stability. It does not prove real Binance reachability from this machine.

## Local Startup

Use Docker Compose for local smoke and development:

```bash
docker compose up -d
```

The local compose stack intentionally points Perp Radar at mock Binance:

- REST: `http://mock-binance:9000`
- Market WS: `ws://mock-binance:9000/market`
- Public WS: `ws://mock-binance:9000/public`

Check the runtime:

```bash
curl http://127.0.0.1:8080/v1/health
curl http://127.0.0.1:8080/v1/universe
curl "http://127.0.0.1:8080/v1/packets/top?limit=3"
curl http://127.0.0.1:8080/v1/debug/ws
curl http://127.0.0.1:8080/v1/debug/rate_limits
```

Expected local smoke output:

- `/v1/health` returns `{"ok":true}`
- `/v1/universe` reports `active_n=15` and `focus_n=3`
- Top packets are non-empty
- Focus packets report `quality.book_mode="full"`
- Focus packets report `quality.book_seq_ok=true`
- Focus packets report `quality.stale=false`

## VPS Validation

Run VPS validation only from a region that can reach Binance USD-M Futures.

Use the same image and override Binance endpoints to real Binance:

```bash
PERP_RADAR__BINANCE__REST_BASE=https://fapi.binance.com
PERP_RADAR__BINANCE__MARKET_WS_BASE=wss://fstream.binance.com/market
PERP_RADAR__BINANCE__PUBLIC_WS_BASE=wss://fstream.binance.com/public
PERP_RADAR__UNIVERSE__ACTIVE_N=15
PERP_RADAR__UNIVERSE__FOCUS_N=3
```

Recommended VPS acceptance run:

```bash
docker compose up -d
docker compose logs -f perp-radar
```

Acceptance criteria for the first real Binance run:

- Run for at least 2 hours
- ClickHouse remains healthy
- `/v1/health` stays OK
- `/v1/universe` remains populated and active symbols can change over time
- `/v1/packets/top?limit=3` stays non-empty
- Top packets keep reasonable `freshness_ms`
- No persistent `stale=true` on all top packets
- Focus packets can recover to `book_seq_ok=true` after reconnect or gap
- Logs do not show repeated REST/WS reachability failures

If Binance is blocked by the VPS region, treat it as an infrastructure failure, not an application failure.

## Quick 30 Minute Local Check

This lightweight loop mirrors the completed local long-run:

```bash
for i in $(seq 1 60); do
  date -Iseconds
  curl -fsS http://127.0.0.1:8080/v1/health
  curl -fsS http://127.0.0.1:8080/v1/universe
  curl -fsS "http://127.0.0.1:8080/v1/packets/top?limit=3"
  docker compose ps
  sleep 30
done
docker compose logs --since=30m perp-radar | grep -E 'WARN|ERROR|panic|WebSocket protocol error|HTTP version must be 1.1' || true
```

Pass condition:

- Every health request succeeds
- Every universe response has `active_n=15` and `focus_n=3`
- Top packets keep refreshing
- No `stale=true`, no full-book sequence failures, and no repeated app warnings

## Operational Notes

- Local Docker Compose defaults to mock Binance. Real Binance is configured by environment override.
- ClickHouse uses the `perp_radar` database and `perp_radar` user/password in compose.
- The Docker build depends on Rust `1.95-slim` and builder packages `pkg-config` and `libssl-dev`.
- `.dockerignore` excludes `target/` so local build artifacts are not sent into Docker build context.
- Funding interval is currently marked as an estimate from config; V1 does not yet use Binance `fundingInfo` for exact intervals.
- The current runtime pins `BTCUSDT`, `ETHUSDT`, and `SOLUSDT` as focus symbols while U0 ranks the active universe.

## Troubleshooting

If Docker is running but `docker` is not on PATH in a Codex shell, use:

```bash
/Applications/Docker.app/Contents/Resources/bin/docker compose ps
```

If ClickHouse is not healthy:

```bash
docker compose logs clickhouse
curl -u perp_radar:perp_radar http://127.0.0.1:8123/ping
```

If packets are empty:

```bash
docker compose logs perp-radar
docker compose logs mock-binance
curl http://127.0.0.1:19000/fapi/v1/ticker/24hr
```

If focus symbols downgrade from full book:

```bash
curl "http://127.0.0.1:8080/v1/packets/top?limit=3"
docker compose logs --since=10m perp-radar | grep -E 'gap|resync|depth|WARN|ERROR' || true
```

## Next Owner Checklist

- Push `main` to the remote repository.
- Deploy the merged `main` branch to a Binance-reachable VPS.
- Run the 2 hour real Binance validation.
- Capture the VPS validation window, API samples, and any warning logs.
- If real Binance validation passes, tag the first V1 release candidate.
