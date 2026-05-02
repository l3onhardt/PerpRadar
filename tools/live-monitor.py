#!/usr/bin/env python3
import json
import os
import subprocess
import sys
import time
import urllib.request
from datetime import datetime, timezone


BASE_URL = sys.argv[1] if len(sys.argv) > 1 else "http://127.0.0.1:18080"
INTERVAL_SEC = int(sys.argv[2]) if len(sys.argv) > 2 else 30
OUTPUT = sys.argv[3] if len(sys.argv) > 3 else "logs/live-monitor.jsonl"
LIMIT = int(sys.argv[4]) if len(sys.argv) > 4 else 3


def utc_now():
    return datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")


def get_json(path):
    with urllib.request.urlopen(f"{BASE_URL}{path}", timeout=10) as response:
        return json.loads(response.read().decode("utf-8"))


def docker_stats():
    base_cmd = ["stats", "--no-stream", "--format", "{{json .}}"]
    commands = [["docker", *base_cmd], ["sudo", "docker", *base_cmd]]
    last_error = None
    result = None
    for cmd in commands:
        try:
            result = subprocess.run(cmd, check=True, capture_output=True, text=True, timeout=10)
            break
        except Exception as exc:
            last_error = exc
    if result is None:
        return {"error": str(last_error)}
    rows = []
    for line in result.stdout.splitlines():
        try:
            rows.append(json.loads(line))
        except json.JSONDecodeError:
            rows.append({"raw": line})
    return rows


def packet_summary(packet):
    return {
        "symbol": packet["symbol"],
        "freshness_ms": packet["quality"]["freshness_ms"],
        "stale": packet["quality"]["stale"],
        "reasons": packet["quality"]["reasons"],
        "book_mode": packet["quality"]["book_mode"],
        "book_seq_ok": packet["quality"]["book_seq_ok"],
        "price_last": packet["price"]["last"],
        "rsi_14": packet["chart"]["rsi_14"],
        "spread_bp": packet["liquidity"]["spread_bp"],
        "funding_now": packet["carry"]["funding_now"],
        "liq_1m_usd": packet["events"]["liq_1m_usd"],
        "liq_5m_usd": packet["events"]["liq_5m_usd"],
        "liq_15m_usd": packet["events"]["liq_15m_usd"],
        "liq_side": packet["events"]["liq_side"],
        "tcs": packet["scores"]["TCS"],
        "lri": packet["scores"]["LRI"],
        "dpi5": packet["scores"]["DPI5"],
        "dpi10": packet["scores"].get("DPI10"),
        "score_meta": packet.get("score_meta", {}),
        "legacy_scores": packet.get("legacy_scores", {}),
    }


def sample():
    health = get_json("/v1/health")
    packets = get_json(f"/v1/packets/top?limit={LIMIT}")
    summaries = [packet_summary(packet) for packet in packets]
    problems = []
    if health != {"ok": True}:
        problems.append(f"health={health}")
    if len(summaries) < LIMIT:
        problems.append(f"packets={len(summaries)}")
    for packet in summaries:
        if packet["stale"]:
            problems.append(f"{packet['symbol']}:stale")
        if packet["freshness_ms"] > 15_000:
            problems.append(f"{packet['symbol']}:freshness_ms={packet['freshness_ms']}")
        if packet["book_mode"] != "full":
            problems.append(f"{packet['symbol']}:book_mode={packet['book_mode']}")
        if packet["book_seq_ok"] is not True:
            problems.append(f"{packet['symbol']}:book_seq_ok={packet['book_seq_ok']}")
        lri_meta = packet.get("score_meta", {}).get("LRI", {})
        if packet["lri"] is None and not lri_meta.get("missing"):
            problems.append(f"{packet['symbol']}:lri_unavailable")
    return {
        "ts": utc_now(),
        "ok": not problems,
        "problems": problems,
        "packets": summaries,
        "docker_stats": docker_stats(),
    }


def main():
    os.makedirs(os.path.dirname(OUTPUT) or ".", exist_ok=True)
    while True:
        try:
            record = sample()
        except Exception as exc:
            record = {"ts": utc_now(), "ok": False, "problems": [str(exc)]}
        with open(OUTPUT, "a", encoding="utf-8") as handle:
            handle.write(json.dumps(record, separators=(",", ":")) + "\n")
            handle.flush()
        print(json.dumps({"ts": record["ts"], "ok": record["ok"], "problems": record["problems"]}))
        time.sleep(INTERVAL_SEC)


if __name__ == "__main__":
    main()
