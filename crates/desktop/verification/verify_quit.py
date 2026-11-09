#!/usr/bin/env python3
"""Account-free native quit check with a real, delayed loopback WS close handshake."""
import argparse
import base64
import hashlib
import json
from pathlib import Path
import shutil
import socket
import subprocess
import threading
import time


def receive_exact(connection, length):
    value = bytearray()
    while len(value) < length:
        chunk = connection.recv(length - len(value))
        if not chunk:
            raise RuntimeError("Local socket closed before its close frame")
        value.extend(chunk)
    return value


def serve(listener, result):
    try:
        connection, _ = listener.accept()
        with connection:
            connection.settimeout(90)
            headers = bytearray()
            while b"\r\n\r\n" not in headers and len(headers) < 8192:
                headers.extend(receive_exact(connection, 1))
            key = next(
                line.split(b":", 1)[1].strip()
                for line in headers.split(b"\r\n")
                if line.lower().startswith(b"sec-websocket-key:")
            )
            accept = base64.b64encode(hashlib.sha1(
                key + b"258EAFA5-E914-47DA-95CA-C5AB0DC85B11"
            ).digest())
            connection.sendall(
                b"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\n"
                b"Connection: Upgrade\r\nSec-WebSocket-Accept: " + accept + b"\r\n\r\n"
            )
            result["upgraded"] = True
            frame = receive_exact(connection, 2)
            if frame[0] != 0x88 or not frame[1] & 0x80 or frame[1] & 0x7f > 125:
                raise RuntimeError("Expected one ordinary masked WebSocket close frame")
            receive_exact(connection, 4 + (frame[1] & 0x7f))
            result["client_close_received"] = True
            # Keep an actual socket open beyond GPUI's 100 ms shutdown budget.
            time.sleep(0.4)
            connection.sendall(b"\x88\x00")
            result["server_close_replied"] = True
            connection.shutdown(socket.SHUT_WR)
            result["client_tcp_closed"] = connection.recv(1) == b""
    except (OSError, RuntimeError, StopIteration) as error:
        result["failure"] = type(error).__name__
    finally:
        listener.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("directory", type=Path)
    parser.add_argument("--route", choices=["native", "command"], default="native")
    parser.add_argument("--public-cache", type=Path)
    parser.add_argument("--expect-held", action="store_true",
                        help="Verify an unconfirmed close keeps the app alive; never counts as socket cleanup")
    args = parser.parse_args()
    args.directory.mkdir(parents=True, exist_ok=True)
    if list(args.directory.iterdir()):
        parser.error("Choose an empty verification output directory")
    if args.public_cache:
        shutil.copytree(args.public_cache, args.directory / "public-cache")
    result = {"account_used": False, "verification": "unclosed-resource-held" if args.expect_held else "socket-close-handshake"}
    listener = socket.socket()
    listener.bind(("127.0.0.1", 0))
    listener.listen(1)
    listener.settimeout(120)
    port = listener.getsockname()[1]
    worker = threading.Thread(target=serve, args=(listener, result), daemon=True)
    worker.start()
    try:
        with (args.directory / "native.log").open("wb") as log:
            process = subprocess.Popen([
                str(args.binary.resolve()), "--verify-application", "--quit-via", args.route,
                "--verification-socket", str(port), "--verification-dir", str(args.directory),
            ], stdout=log, stderr=subprocess.STDOUT)
            try:
                process.wait(timeout=90 if args.expect_held else 150)
            except subprocess.TimeoutExpired:
                result["application_still_alive"] = process.poll() is None
                process.kill()
                process.wait(timeout=5)
                if not args.expect_held:
                    raise
        result["process_exit"] = process.returncode
        listener.close()
        worker.join(timeout=5)
        report = json.loads((args.directory / "report.json").read_text())
        result["native_checks"] = len(report["checks"])
        result["page_owned_socket_count"] = report["page_owned_socket_count"]
        result["passed"] = process.returncode == 0 and report["passed"] and all(
            result.get(key) is True for key in [
                "upgraded", "client_close_received", "server_close_replied", "client_tcp_closed"
            ]
        ) and not worker.is_alive()
        if args.expect_held:
            result["passed"] = result.get("application_still_alive") is True \
                and report["failures"] == ["application-quit-cleanup-timeout"] \
                and dict(report["checks"]).get("unconfirmed_close_preserves_parent_and_child") is True \
                and not report["embedded_resource_close_fence_passed"]
            result["socket_cleanup_proven"] = False
    except (OSError, subprocess.TimeoutExpired, ValueError) as error:
        result["failure"] = type(error).__name__
        result["passed"] = False
    finally:
        listener.close()
    (args.directory / "loopback.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result))
    return 0 if result["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
