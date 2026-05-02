#!/usr/bin/env python3
import json
import sys
import time
import urllib.request


BASE_URL = sys.argv[1] if len(sys.argv) > 1 else "http://127.0.0.1:18080"
SAMPLES = int(sys.argv[2]) if len(sys.argv) > 2 else 12
INTERVAL_SEC = int(sys.argv[3]) if len(sys.argv) > 3 else 10

REQUIRED_PACKET_PATHS = [
    ("price.last", lambda p: p["price"]["last"]),
    ("price.mark", lambda p: p["price"]["mark"]),
    ("price.index", lambda p: p["price"]["index"]),
    ("price.ret_1m", lambda p: p["price"]["ret_1m"]),
    ("price.ret_5m", lambda p: p["price"]["ret_5m"]),
    ("price.ret_15m", lambda p: p["price"]["ret_15m"]),
    ("chart.regime", lambda p: p["chart"]["regime"]),
    ("chart.ema_20", lambda p: p["chart"]["ema_20"]),
    ("chart.ema_50", lambda p: p["chart"]["ema_50"]),
    ("chart.rsi_14", lambda p: p["chart"]["rsi_14"]),
    ("chart.macd_histogram", lambda p: p["chart"]["macd_histogram"]),
    ("chart.atr_pct", lambda p: p["chart"]["atr_pct"]),
    ("chart.bb_width", lambda p: p["chart"]["bb_width"]),
    ("chart.adx_14", lambda p: p["chart"]["adx_14"]),
    ("chart.vwap_20", lambda p: p["chart"]["vwap_20"]),
    ("chart.cmf_20", lambda p: p["chart"]["cmf_20"]),
    ("liquidity.spread_bp", lambda p: p["liquidity"]["spread_bp"]),
    ("liquidity.i1", lambda p: p["liquidity"]["i1"]),
    ("liquidity.i5", lambda p: p["liquidity"]["i5"]),
    ("liquidity.liq_5bp_usd", lambda p: p["liquidity"]["liq_5bp_usd"]),
    ("liquidity.liq_10bp_usd", lambda p: p["liquidity"]["liq_10bp_usd"]),
    ("liquidity.slip_10000_buy_bp", lambda p: p["liquidity"]["slip_10000_buy_bp"]),
    ("liquidity.slip_10000_sell_bp", lambda p: p["liquidity"]["slip_10000_sell_bp"]),
    ("carry.funding_now", lambda p: p["carry"]["funding_now"]),
    ("carry.funding_z_7d", lambda p: p["carry"]["funding_z_7d"]),
    ("carry.next_funding_time", lambda p: p["carry"]["next_funding_time"]),
    ("events.volume_spike_z", lambda p: p["events"]["volume_spike_z"]),
    ("scores.TCS", lambda p: p["scores"]["TCS"]),
    ("scores.DPI5", lambda p: p["scores"]["DPI5"]),
    ("scores.DPI10", lambda p: p["scores"]["DPI10"]),
    ("scores.CSI", lambda p: p["scores"]["CSI"]),
    ("scores.RPI", lambda p: p["scores"]["RPI"]),
    ("scores.VoV", lambda p: p["scores"]["VoV"]),
]


def get_json(path):
    with urllib.request.urlopen(f"{BASE_URL}{path}", timeout=5) as response:
        return json.loads(response.read().decode("utf-8"))


def missing_fields(packet):
    missing = []
    for name, getter in REQUIRED_PACKET_PATHS:
        try:
            value = getter(packet)
        except (KeyError, TypeError):
            missing.append(name)
            continue
        if value is None:
            missing.append(name)
    return missing


def validate_packet(packet):
    quality = packet["quality"]
    problems = []
    if packet.get("packet_schema") == "2.1":
        if not isinstance(packet.get("score_meta"), dict):
            problems.append("score_meta_missing")
        if not isinstance(packet.get("legacy_scores"), dict):
            problems.append("legacy_scores_missing")
    if quality["stale"]:
        problems.append("quality.stale=true")
    if quality["freshness_ms"] > 15_000:
        problems.append(f"quality.freshness_ms={quality['freshness_ms']}")
    if quality["book_mode"] != "full":
        problems.append(f"quality.book_mode={quality['book_mode']}")
    if quality["book_seq_ok"] is not True:
        problems.append(f"quality.book_seq_ok={quality['book_seq_ok']}")
    missing = missing_fields(packet)
    if missing:
        problems.append("missing=" + ",".join(missing))
    return problems


def main():
    failures = []
    observed_symbols = set()

    health = get_json("/v1/health")
    if health != {"ok": True}:
        raise SystemExit(f"health failed: {health}")

    for sample in range(1, SAMPLES + 1):
        packets = get_json("/v1/packets/top?limit=3")
        if len(packets) < 3:
            failures.append(f"sample {sample}: expected 3 packets, got {len(packets)}")
        for packet in packets:
            observed_symbols.add(packet["symbol"])
            problems = validate_packet(packet)
            if problems:
                failures.append(
                    f"sample {sample} {packet['symbol']}: " + "; ".join(problems)
                )

        print(
            json.dumps(
                {
                    "sample": sample,
                    "symbols": [packet["symbol"] for packet in packets],
                    "freshness_ms": {
                        packet["symbol"]: packet["quality"]["freshness_ms"]
                        for packet in packets
                    },
                },
                separators=(",", ":"),
            )
        )
        if sample != SAMPLES:
            time.sleep(INTERVAL_SEC)

    if failures:
        print("\nFAIL", file=sys.stderr)
        for failure in failures:
            print(f"- {failure}", file=sys.stderr)
        return 1

    print(
        f"PASS live indicators stable: samples={SAMPLES} symbols={','.join(sorted(observed_symbols))}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
