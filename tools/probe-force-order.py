#!/usr/bin/env python3
import json
import socket
import ssl
import sys
import time
from base64 import b64encode
from hashlib import sha1
from os import urandom


HOST = "fstream.binance.com"
PATH = "/stream?streams=!forceOrder@arr"
TIMEOUT_SEC = int(sys.argv[1]) if len(sys.argv) > 1 else 120


def recv_exact(sock, size):
    chunks = []
    remaining = size
    while remaining:
        chunk = sock.recv(remaining)
        if not chunk:
            raise EOFError("websocket closed")
        chunks.append(chunk)
        remaining -= len(chunk)
    return b"".join(chunks)


def recv_frame(sock):
    header = recv_exact(sock, 2)
    opcode = header[0] & 0x0F
    length = header[1] & 0x7F
    if length == 126:
        length = int.from_bytes(recv_exact(sock, 2), "big")
    elif length == 127:
        length = int.from_bytes(recv_exact(sock, 8), "big")
    payload = recv_exact(sock, length)
    return opcode, payload


def main():
    key = b64encode(urandom(16)).decode("ascii")
    raw = socket.create_connection((HOST, 443), timeout=10)
    sock = ssl.create_default_context().wrap_socket(raw, server_hostname=HOST)
    request = (
        f"GET {PATH} HTTP/1.1\r\n"
        f"Host: {HOST}\r\n"
        "Upgrade: websocket\r\n"
        "Connection: Upgrade\r\n"
        f"Sec-WebSocket-Key: {key}\r\n"
        "Sec-WebSocket-Version: 13\r\n"
        "\r\n"
    )
    sock.sendall(request.encode("ascii"))
    response = b""
    while b"\r\n\r\n" not in response:
        response += sock.recv(4096)
    if b" 101 " not in response.split(b"\r\n", 1)[0]:
        raise SystemExit(response.decode("utf-8", "replace"))
    accept = response.decode("utf-8", "replace")
    expected = b64encode(sha1((key + "258EAFA5-E914-47DA-95CA-C5AB0DC85B11").encode()).digest()).decode()
    if expected not in accept:
        raise SystemExit("websocket accept key mismatch")

    deadline = time.time() + TIMEOUT_SEC
    seen = 0
    while time.time() < deadline:
        sock.settimeout(max(1, deadline - time.time()))
        try:
            opcode, payload = recv_frame(sock)
        except TimeoutError:
            break
        if opcode == 1:
            seen += 1
            print(json.dumps({"seen": seen, "payload": json.loads(payload.decode("utf-8"))}, separators=(",", ":")))
        elif opcode == 8:
            raise SystemExit("websocket close frame")
    print(json.dumps({"seen": seen, "timeout_sec": TIMEOUT_SEC}, separators=(",", ":")))


if __name__ == "__main__":
    main()
