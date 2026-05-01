#!/usr/bin/env python3
import base64
import hashlib
import json
import os
import socket
import struct
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import parse_qs, urlparse

SYMBOLS = [
    "BTCUSDT",
    "ETHUSDT",
    "SOLUSDT",
    "BNBUSDT",
    "XRPUSDT",
    "ADAUSDT",
    "DOGEUSDT",
    "AVAXUSDT",
    "LINKUSDT",
    "TONUSDT",
    "TRXUSDT",
    "DOTUSDT",
    "MATICUSDT",
    "LTCUSDT",
    "BCHUSDT",
]


def price_for(symbol):
    return {
        "BTCUSDT": 64000.0,
        "ETHUSDT": 3200.0,
        "SOLUSDT": 150.0,
        "BNBUSDT": 600.0,
        "XRPUSDT": 0.6,
        "ADAUSDT": 0.55,
        "DOGEUSDT": 0.16,
        "AVAXUSDT": 35.0,
        "LINKUSDT": 16.0,
        "TONUSDT": 5.5,
        "TRXUSDT": 0.12,
        "DOTUSDT": 7.2,
        "MATICUSDT": 0.82,
        "LTCUSDT": 85.0,
        "BCHUSDT": 420.0,
    }.get(symbol, 100.0)


class Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def do_GET(self):
        parsed = urlparse(self.path)
        query = parse_qs(parsed.query)
        if parsed.path == "/ping":
            return self.send_json({"ok": True})
        if parsed.path == "/fapi/v1/klines":
            symbol = query.get("symbol", ["BTCUSDT"])[0].upper()
            limit = int(query.get("limit", ["64"])[0])
            return self.send_json(klines(symbol, limit))
        if parsed.path == "/fapi/v1/depth":
            symbol = query.get("symbol", ["BTCUSDT"])[0].upper()
            return self.send_json(depth(symbol))
        if parsed.path == "/fapi/v1/ticker/24hr":
            return self.send_json(tickers_24hr())
        if parsed.path == "/fapi/v1/fundingRate":
            return self.send_json([{"fundingRate": "0.0001"} for _ in range(126)])
        if parsed.path.endswith("/stream"):
            key = self.headers.get("Sec-WebSocket-Key")
            if key:
                return self.handle_websocket(key, parsed)
            return self.send_json({"error": "websocket upgrade required"}, status=426)
        return self.send_json({"error": "not found"}, status=404)

    def log_message(self, fmt, *args):
        return

    def send_json(self, payload, status=200):
        body = json.dumps(payload).encode()
        self.send_response(status)
        self.send_header("content-type", "application/json")
        self.send_header("content-length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def handle_websocket(self, key, parsed):
        accept = base64.b64encode(
            hashlib.sha1((key + "258EAFA5-E914-47DA-95CA-C5AB0DC85B11").encode()).digest()
        ).decode()
        self.send_response(101, "Switching Protocols")
        self.send_header("Upgrade", "websocket")
        self.send_header("Connection", "Upgrade")
        self.send_header("Sec-WebSocket-Accept", accept)
        self.end_headers()

        streams = parse_qs(parsed.query).get("streams", [""])[0].split("/")
        depth_sequences = {}
        try:
            while True:
                for payload in ws_payloads(streams, depth_sequences):
                    self.wfile.write(websocket_text_frame(json.dumps(payload).encode()))
                    self.wfile.flush()
                    time.sleep(0.05)
                time.sleep(0.2)
        except (BrokenPipeError, ConnectionResetError, OSError):
            return


def klines(symbol, limit):
    base = price_for(symbol)
    now = int(time.time() // 60 * 60 * 1000)
    rows = []
    for idx in range(limit):
        open_time = now - (limit - idx) * 60_000
        close = base * (1 + idx / 10000)
        rows.append([
            open_time,
            f"{close - 1:.8f}",
            f"{close + 2:.8f}",
            f"{close - 2:.8f}",
            f"{close:.8f}",
            "100.0",
            open_time + 59_999,
            f"{close * 100:.8f}",
            100,
            "60.0",
            f"{close * 60:.8f}",
            "0",
        ])
    return rows


def depth(symbol):
    mid = price_for(symbol)
    return {
        "lastUpdateId": 1000,
        "bids": [[f"{mid - 1 - idx:.8f}", "10.0"] for idx in range(20)],
        "asks": [[f"{mid + 1 + idx:.8f}", "10.0"] for idx in range(20)],
    }


def tickers_24hr():
    return [
        {
            "symbol": symbol,
            "lastPrice": f"{price_for(symbol):.8f}",
            "quoteVolume": str(100_000_000 + idx * 10_000_000),
            "priceChangePercent": str(1.0 + idx),
        }
        for idx, symbol in enumerate(SYMBOLS)
    ]


def ws_payloads(streams, depth_sequences):
    event_time = int(time.time() * 1000)
    for stream in streams:
        if stream == "!ticker@arr":
            yield {
                "stream": stream,
                "data": [
                    {
                        "e": "24hrTicker",
                        "E": event_time,
                        "s": symbol,
                        "c": f"{price_for(symbol):.8f}",
                        "q": str(100_000_000 + idx * 10_000_000),
                        "P": str(1.0 + idx),
                    }
                    for idx, symbol in enumerate(SYMBOLS)
                ],
            }
        elif stream == "!markPrice@arr":
            yield {
                "stream": stream,
                "data": [
                    {
                        "e": "markPriceUpdate",
                        "E": event_time,
                        "s": symbol,
                        "p": f"{price_for(symbol):.8f}",
                        "i": f"{price_for(symbol) * 0.999:.8f}",
                        "r": "0.0001",
                        "T": event_time + 28_800_000,
                    }
                    for symbol in SYMBOLS
                ],
            }
        elif "@kline_1m" in stream:
            symbol = stream.split("@", 1)[0].upper()
            price = price_for(symbol)
            yield {
                "stream": stream,
                "data": {
                    "e": "kline",
                    "E": event_time,
                    "s": symbol,
                    "k": {
                        "t": event_time // 60_000 * 60_000,
                        "T": event_time // 60_000 * 60_000 + 59_999,
                        "s": symbol,
                        "i": "1m",
                        "o": f"{price - 1:.8f}",
                        "c": f"{price:.8f}",
                        "h": f"{price + 2:.8f}",
                        "l": f"{price - 2:.8f}",
                        "v": "100.0",
                        "q": f"{price * 100:.8f}",
                        "n": 100,
                        "V": "60.0",
                        "Q": f"{price * 60:.8f}",
                        "x": True,
                    },
                },
            }
        elif "@depth20@500ms" in stream:
            symbol = stream.split("@", 1)[0].upper()
            book = depth(symbol)
            yield {
                "stream": stream,
                "data": {
                    "lastUpdateId": book["lastUpdateId"],
                    "bids": book["bids"][:20],
                    "asks": book["asks"][:20],
                },
            }
        elif "@depth@500ms" in stream:
            symbol = stream.split("@", 1)[0].upper()
            mid = price_for(symbol)
            previous_u = depth_sequences.get(symbol, 1000)
            if previous_u == 1000:
                first_u = 998
                final_u = 1001
                previous_final_u = 997
            else:
                first_u = previous_u + 1
                final_u = previous_u + 1
                previous_final_u = previous_u
            depth_sequences[symbol] = final_u
            yield {
                "stream": stream,
                "data": {
                    "e": "depthUpdate",
                    "E": event_time,
                    "T": event_time,
                    "s": symbol,
                    "U": first_u,
                    "u": final_u,
                    "pu": previous_final_u,
                    "b": [[f"{mid - 1:.8f}", "12.0"]],
                    "a": [],
                },
            }


def websocket_text_frame(payload):
    header = bytearray([0x81])
    length = len(payload)
    if length < 126:
        header.append(length)
    elif length < 65536:
        header.append(126)
        header.extend(struct.pack("!H", length))
    else:
        header.append(127)
        header.extend(struct.pack("!Q", length))
    return bytes(header) + payload


def main():
    bind = os.environ.get("MOCK_BINANCE_BIND", "127.0.0.1:9000")
    host, port = bind.rsplit(":", 1)
    ThreadingHTTPServer((host, int(port)), Handler).serve_forever()


if __name__ == "__main__":
    main()
