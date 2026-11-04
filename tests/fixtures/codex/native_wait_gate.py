"""Metadata-only stdio MCP fixture; the gate is an owned loopback HTTP service."""
import argparse
import http.client
import json
import re
import sys

MAX_FRAME = 65536


def unique_object(pairs):
    value = {}
    for key, item in pairs:
        if key in value:
            raise ValueError("duplicate_field")
        value[key] = item
    return value


def released(port, marker):
    connection = http.client.HTTPConnection("127.0.0.1", port, timeout=65)
    try:
        connection.request("GET", "/gate", headers={"X-Headroom-Probe": marker})
        response = connection.getresponse()
        if response.status != 200:
            return False
        raw = response.read(4097)
        if len(raw) > 4096:
            return False
        value = json.loads(raw, object_pairs_hook=unique_object)
        return isinstance(value, dict) and set(value) == {"released"} and value["released"] is True
    except (OSError, ValueError, http.client.HTTPException):
        return False
    finally:
        connection.close()


def serve(port, marker):
    for raw in iter(lambda: sys.stdin.buffer.readline(MAX_FRAME + 1), b""):
        if len(raw) > MAX_FRAME:
            return 2
        request_id = None
        try:
            message = json.loads(raw, object_pairs_hook=unique_object)
            if not isinstance(message, dict):
                raise ValueError("invalid_frame")
            candidate_id = message.get("id")
            if candidate_id is None:
                continue
            if isinstance(candidate_id, bool) or not isinstance(candidate_id, (str, int)) or len(str(candidate_id)) > 256:
                raise ValueError("invalid_id")
            request_id = candidate_id
            method = message.get("method")
            params = message.get("params", {})
            if not isinstance(params, dict) or message.get("jsonrpc") != "2.0":
                raise ValueError("invalid_params")
            if method == "initialize":
                protocol = params.get("protocolVersion")
                if not isinstance(protocol, str) or not re.fullmatch(r"\d{4}-\d{2}-\d{2}", protocol):
                    raise ValueError("invalid_protocol")
                result = {"protocolVersion": protocol, "capabilities": {"tools": {}}, "serverInfo": {"name": "headroom-native-wait-probe", "version": "1.0.0"}}
            elif method == "ping":
                result = {}
            elif method == "tools/list":
                result = {"tools": [{"name": "wait_for_probe", "description": "Wait for the owned local protocol probe to release its metadata gate, then return its marker. No filesystem or OS-command access.", "inputSchema": {"type": "object", "properties": {}, "additionalProperties": False}}]}
            elif method == "tools/call":
                if params.get("name") != "wait_for_probe" or params.get("arguments", {}) != {}:
                    raise ValueError("invalid_tool_call")
                passed = released(port, marker)
                result = {"content": [{"type": "text", "text": marker if passed else "probe_gate_not_released"}], "isError": not passed}
            else:
                response = {"jsonrpc": "2.0", "id": request_id, "error": {"code": -32601, "message": "method_not_found"}}
                print(json.dumps(response), flush=True)
                continue
            response = {"jsonrpc": "2.0", "id": request_id, "result": result}
        except (ValueError, TypeError, RecursionError):
            response = {"jsonrpc": "2.0", "id": request_id, "error": {"code": -32602, "message": "invalid_request"}}
        print(json.dumps(response), flush=True)
    return 0


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--port", type=int, required=True)
    parser.add_argument("--marker", required=True)
    args = parser.parse_args()
    if not 1 <= args.port <= 65535 or not re.fullmatch(r"HEADROOM_WAIT_GATE_[A-Z0-9_]{8,80}", args.marker):
        raise SystemExit(2)
    raise SystemExit(serve(args.port, args.marker))
