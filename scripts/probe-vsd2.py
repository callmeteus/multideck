#!/usr/bin/env python3
"""Probes a running VSD2 WebSocket server with candidate first messages.

Why: the desktop app closes the connection with a policy violation when the
first binary frame is not a valid vsd2 message. That gives us an oracle: a
candidate that changes the close reason, or produces a reply, is a candidate
the server understood. This is how the envelope can be found without a phone.

Usage:
    python scripts/probe-vsd2.py [--host ::1] [--port 28198]

The server is the one embedded in the official Stream Deck desktop app. Only run
this against your own machine.
"""

from __future__ import annotations

import argparse
import base64
import os
import socket
import struct
import sys
import time


def handshake(sock: socket.socket, host: str, port: int) -> None:
    key = base64.b64encode(os.urandom(16)).decode()
    host_header = f"[{host}]" if ":" in host else host
    request = (
        f"GET / HTTP/1.1\r\n"
        f"Host: {host_header}:{port}\r\n"
        f"Upgrade: websocket\r\n"
        f"Connection: Upgrade\r\n"
        f"Sec-WebSocket-Key: {key}\r\n"
        f"Sec-WebSocket-Version: 13\r\n\r\n"
    )
    sock.sendall(request.encode())
    response = b""
    while b"\r\n\r\n" not in response:
        chunk = sock.recv(4096)
        if not chunk:
            raise ConnectionError("server closed during handshake")
        response += chunk
    if b"101" not in response.split(b"\r\n", 1)[0]:
        raise ConnectionError(response.decode(errors="replace").splitlines()[0])


def send_frame(sock: socket.socket, payload: bytes, opcode: int = 2) -> None:
    mask = os.urandom(4)
    length = len(payload)
    if length < 126:
        header = bytes([0x80 | opcode, 0x80 | length])
    elif length < 65536:
        header = bytes([0x80 | opcode, 0x80 | 126]) + struct.pack(">H", length)
    else:
        header = bytes([0x80 | opcode, 0x80 | 127]) + struct.pack(">Q", length)
    masked = bytes(byte ^ mask[index % 4] for index, byte in enumerate(payload))
    sock.sendall(header + mask + masked)


def read_frame(sock: socket.socket, timeout: float) -> tuple[int, bytes]:
    sock.settimeout(timeout)
    header = sock.recv(2)
    if len(header) < 2:
        raise ConnectionError("closed")
    opcode = header[0] & 0x0F
    length = header[1] & 0x7F
    if length == 126:
        length = struct.unpack(">H", sock.recv(2))[0]
    elif length == 127:
        length = struct.unpack(">Q", sock.recv(8))[0]
    payload = b""
    while len(payload) < length:
        chunk = sock.recv(length - len(payload))
        if not chunk:
            break
        payload += chunk
    return opcode, payload


def probe(host: str, port: int, payload: bytes) -> str:
    family = socket.AF_INET6 if ":" in host else socket.AF_INET
    sock = socket.socket(family, socket.SOCK_STREAM)
    sock.settimeout(3.0)
    try:
        sock.connect((host, port))
        handshake(sock, host, port)
        send_frame(sock, payload)
        try:
            opcode, data = read_frame(sock, 1.2)
        except socket.timeout:
            return "timeout (no reply)"
        if opcode == 8:
            code = struct.unpack(">H", data[:2])[0] if len(data) >= 2 else 0
            reason = data[2:].decode(errors="replace")
            return f"close {code} {reason!r}"
        return f"opcode {opcode} reply {data.hex(' ')}"
    except Exception as error:  # noqa: BLE001 - probe tool, report anything useful
        return f"error {type(error).__name__}: {error}"
    finally:
        sock.close()


def candidates() -> list[tuple[str, bytes]]:
    items: list[tuple[str, bytes]] = []
    items.append(("empty", b""))
    for field in range(1, 17):
        # oneof style: field N, wire type 2, empty submessage
        items.append((f"field {field} empty submessage", bytes([(field << 3) | 2, 0])))
        # oneof style: field N, wire type 0, value 1
        items.append((f"field {field} varint 1", bytes([(field << 3) | 0, 1])))
    # length prefixed protobuf variants
    for prefix in (b"\x01\x00", b"\x00\x00", b"\x04\x00"):
        items.append((f"prefix {prefix.hex()}", prefix))
    return items


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--host", default="::1")
    parser.add_argument("--port", type=int, default=28198)
    args = parser.parse_args()

    interesting = 0
    for name, payload in candidates():
        result = probe(args.host, args.port, payload)
        flag = "  " if "error parsing message" in result or result.startswith("close 1008") else "->"
        print(f"{flag} {name:<32} {result}")
        if flag == "->":
            interesting += 1
        time.sleep(0.05)
    print(f"{interesting} candidate(s) produced a different outcome", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
