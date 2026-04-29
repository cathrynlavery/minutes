#!/usr/bin/env python3
import json
import sys


def main() -> None:
    if len(sys.argv) != 4:
        print(
            "Usage: smoke_mcpb_handshake.py stdout-path stderr-path exit-code",
            file=sys.stderr,
        )
        raise SystemExit(2)

    out_path, err_path, rc = sys.argv[1], sys.argv[2], sys.argv[3]

    with open(out_path) as f:
        stdout = f.read()
    with open(err_path) as f:
        stderr = f.read()

    response = None
    for line in stdout.splitlines():
        line = line.strip()
        if not line:
            continue
        try:
            msg = json.loads(line)
        except json.JSONDecodeError:
            continue
        if msg.get("id") == 0 and "result" in msg:
            response = msg
            break

    if response is None:
        print("No initialize response on stdout.", file=sys.stderr)
        print(f"--- stdout ({len(stdout)} bytes) ---", file=sys.stderr)
        print(stdout, file=sys.stderr)
        print(f"--- stderr ({len(stderr)} bytes) ---", file=sys.stderr)
        print(stderr, file=sys.stderr)
        print(f"--- exit code: {rc} ---", file=sys.stderr)
        raise SystemExit(1)

    result = response["result"]
    proto = result.get("protocolVersion")
    if proto != "2025-11-25":
        print(f"Expected protocolVersion=2025-11-25, got {proto!r}", file=sys.stderr)
        raise SystemExit(1)

    caps = result.get("capabilities", {})
    if "tools" not in caps or "resources" not in caps:
        print(
            f"Expected tools+resources capabilities, got keys {sorted(caps)}",
            file=sys.stderr,
        )
        raise SystemExit(1)

    ext_ui = caps.get("extensions", {}).get("io.modelcontextprotocol/ui")
    if ext_ui is None:
        print(
            "Expected extensions.io.modelcontextprotocol/ui capability.",
            file=sys.stderr,
        )
        raise SystemExit(1)

    server_info = result.get("serverInfo", {})
    print(
        f"MCPB handshake OK: server={server_info.get('name')}@"
        f"{server_info.get('version')} protocol={proto}"
    )


if __name__ == "__main__":
    main()
