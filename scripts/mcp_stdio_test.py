#!/usr/bin/env python3
"""Stdio integration test for the `layra mcp` server.

Spawns the MCP server as a subprocess, speaks the JSON-RPC subset MCP uses
over stdin/stdout (initialize -> tools/list -> tools/call), and asserts the
server advertises and correctly answers the tools — with a focus on the
`list_shapes` tool added for agent shape discovery.

Usage:
    python3 scripts/mcp_stdio_test.py [path/to/layra]

Defaults to target/release/layra. Exits 0 on success, 1 on failure.
"""

import json
import os
import subprocess
import sys


class McpClient:
    """Minimal newline-delimited JSON-RPC client over a subprocess's stdio."""

    def __init__(self, argv):
        self.proc = subprocess.Popen(
            argv,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            bufsize=1,
        )
        self._id = 0

    def _send(self, obj):
        self.proc.stdin.write(json.dumps(obj) + "\n")
        self.proc.stdin.flush()

    def request(self, method, params=None):
        self._id += 1
        rid = self._id
        msg = {"jsonrpc": "2.0", "id": rid, "method": method}
        if params is not None:
            msg["params"] = params
        self._send(msg)
        # Skip any notifications (no id) until our id comes back.
        while True:
            line = self.proc.stdout.readline()
            if not line:
                raise RuntimeError(
                    f"server closed stdout waiting for id={rid}; "
                    f"stderr:\n{self.proc.stderr.read()}"
                )
            try:
                resp = json.loads(line)
            except json.JSONDecodeError:
                continue
            if resp.get("id") == rid:
                return resp

    def notify(self, method, params=None):
        msg = {"jsonrpc": "2.0", "method": method}
        if params is not None:
            msg["params"] = params
        self._send(msg)

    def close(self):
        try:
            self.proc.stdin.close()
        except Exception:
            pass
        try:
            self.proc.wait(timeout=5)
        except Exception:
            self.proc.kill()


PASSED = 0
FAILED = 0


def check(name, cond, detail=""):
    global PASSED, FAILED
    if cond:
        PASSED += 1
        print(f"  ok   {name}")
    else:
        FAILED += 1
        print(f"  FAIL {name}  {detail}")


def main():
    binary = sys.argv[1] if len(sys.argv) > 1 else os.path.join(
        "target", "release", "layra"
    )
    if not os.path.exists(binary):
        print(f"error: binary not found: {binary}\n"
              f"build it first: cargo build -p layra-cli --release",
              file=sys.stderr)
        return 1

    print(f"layra mcp stdio test ({binary})")
    client = McpClient([binary, "mcp"])
    try:
        # 1. initialize
        init = client.request("initialize", {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {"name": "stdio-test", "version": "0"},
        })
        result = init.get("result", {})
        check("initialize returns serverInfo.name == layra",
              result.get("serverInfo", {}).get("name") == "layra", str(init))
        check("initialize advertises tools capability",
              "tools" in result.get("capabilities", {}), str(init))

        client.notify("notifications/initialized")

        # 2. tools/list includes list_shapes alongside the other tools
        listed = client.request("tools/list")
        tools = {t["name"]: t for t in listed.get("result", {}).get("tools", [])}
        for expected in ("validate_diagram", "render_diagram", "list_shapes"):
            check(f"tools/list advertises {expected}", expected in tools,
                  f"got {sorted(tools)}")
        ls_schema = tools.get("list_shapes", {}).get("inputSchema", {})
        check("list_shapes takes no required args",
              not ls_schema.get("required"), str(ls_schema))

        # 3. tools/call list_shapes returns the shape vocabulary as JSON text
        called = client.request("tools/call", {"name": "list_shapes",
                                               "arguments": {}})
        cres = called.get("result", {})
        check("list_shapes call is not an error",
              not cres.get("isError"), str(called))
        content = cres.get("content", [])
        check("list_shapes returns text content",
              bool(content) and content[0].get("type") == "text", str(called))

        text = content[0]["text"] if content else "{}"
        payload = json.loads(text)  # must be valid JSON
        shapes = {s["shape"] for s in payload.get("node_shapes", [])}
        for expected in ("rect", "cylinder", "diamond", "hexagon", "stadium",
                         "circle", "rounded-rect"):
            check(f"list_shapes includes shape '{expected}'",
                  expected in shapes, f"got {sorted(shapes)}")
        styles = {e["style"] for e in payload.get("edge_styles", [])}
        for expected in ("solid", "dashed", "thick"):
            check(f"list_shapes includes edge style '{expected}'",
                  expected in styles, f"got {sorted(styles)}")
        check("list_shapes includes role classes",
              "database" in payload.get("role_classes", {}).get("roles", []),
              str(payload.get("role_classes")))
        check("list_shapes documents icon syntax",
              "{icon:" in payload.get("icons", {}).get("syntax", ""),
              str(payload.get("icons")))

        # 4. sanity: every advertised node-shape syntax actually validates
        bad = []
        for s in payload.get("node_shapes", []):
            src = "flowchart LR\n  " + s["syntax"].replace('id', 'n')
            v = client.request("tools/call", {"name": "validate_diagram",
                                              "arguments": {"source": src}})
            vtext = v.get("result", {}).get("content", [{}])[0].get("text", "")
            if v.get("result", {}).get("isError") or not vtext.startswith("ok"):
                bad.append((s["shape"], vtext))
        check("every advertised node-shape syntax validates",
              not bad, str(bad))

    finally:
        client.close()

    print(f"\n{PASSED} passed, {FAILED} failed")
    return 0 if FAILED == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
