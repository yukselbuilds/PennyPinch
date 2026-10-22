"""In-memory local Responses gateway fixture for the H008 Codex CLI proof."""

from __future__ import annotations

import hashlib
import json
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from typing import Any
from urllib.parse import urlsplit


STREAM_PARTIAL = "H008_PLAN_MESSAGE_BEFORE_COMPLETION_6D2"
PLAN_STEP_MARKER = "H008_PLAN_STEP_A2_9C6"
FINAL_MARKER = "H008_FINAL_RESPONSE_CLOSED_91B"
GOAL_FINAL_MARKER = "H008_GOAL_FINAL_A3_5C2"
CALL_ID = "call_h008_plan_update"
GOAL_CALL_ID = "call_h008_get_goal"
MODEL = "gpt-5-codex"
REASONING_EFFORT = "high"
MAX_GOAL_OUTPUT_BYTES = 64 * 1024


def _event(name: str, payload: dict[str, Any]) -> bytes:
    data = json.dumps(payload, ensure_ascii=False, separators=(",", ":"))
    return f"event: {name}\ndata: {data}\n\n".encode("utf-8")


class _State:
    def __init__(self, scenario: str) -> None:
        self.lock = threading.Lock()
        self.scenario = scenario
        self.requests: list[dict[str, Any]] = []
        self.transport_attempts: list[dict[str, Any]] = []
        self.partial_sent = threading.Event()
        self.message_item_completed_sent = threading.Event()
        self.release_first_response = threading.Event()
        self.first_completion_sent = threading.Event()
        self.tool_name: str | None = None
        self.tool_arguments: str | None = None
        self.tool_shape: str | None = None
        self.plan_tool_available = False
        self.native_tool_available = False
        self.goal_output_summary: dict[str, Any] | None = None

    def add(self, value: dict[str, Any]) -> int:
        with self.lock:
            self.requests.append(value)
            return len(self.requests)

    def all_requests(self) -> list[dict[str, Any]]:
        with self.lock:
            return list(self.requests)

    def add_transport_attempt(self, value: dict[str, Any]) -> None:
        with self.lock:
            self.transport_attempts.append(value)

    def all_transport_attempts(self) -> list[dict[str, Any]]:
        with self.lock:
            return list(self.transport_attempts)


def _tool_inventory(tools: Any) -> list[dict[str, str]]:
    """Keep only safe type/name metadata for bounded diagnostics."""
    if not isinstance(tools, list):
        return []
    inventory: list[dict[str, str]] = []
    for tool in tools[:128]:
        if not isinstance(tool, dict):
            continue
        function = tool.get("function") if isinstance(tool.get("function"), dict) else tool
        kind, name = tool.get("type"), function.get("name")
        inventory.append(
            {
                "type": kind[:64] if isinstance(kind, str) else "",
                "name": name[:96] if isinstance(name, str) else "",
            }
        )
    return inventory


def _select_update_plan_function(tools: Any) -> tuple[str, str, str] | None:
    """Use only the actual advertised update_plan function schema."""
    if not isinstance(tools, list):
        return None
    for tool in tools:
        if not isinstance(tool, dict) or tool.get("type") != "function":
            continue
        function = tool.get("function") if isinstance(tool.get("function"), dict) else tool
        name = function.get("name")
        if name != "update_plan":
            continue
        parameters = function.get("parameters", {})
        properties = parameters.get("properties", {}) if isinstance(parameters, dict) else {}
        required = parameters.get("required", []) if isinstance(parameters, dict) else []
        plan_schema = properties.get("plan") if isinstance(properties, dict) else None
        if not isinstance(required, list) or "plan" not in required or not isinstance(plan_schema, dict):
            return None
        item_schema = plan_schema.get("items")
        item_properties = item_schema.get("properties") if isinstance(item_schema, dict) else None
        item_required = item_schema.get("required", []) if isinstance(item_schema, dict) else []
        if plan_schema.get("type") != "array" or not isinstance(item_properties, dict) or not isinstance(item_required, list):
            return None

        marker_key = next(
            (
                key
                for key in ("step", "title")
                if isinstance(item_properties.get(key), dict)
                and item_properties[key].get("type") == "string"
            ),
            None,
        )
        if marker_key is None:
            return None
        item: dict[str, Any] = {marker_key: PLAN_STEP_MARKER}
        fields = list(item_required)
        if "status" in item_properties and "status" not in fields:
            fields.append("status")
        for key in fields:
            if key in item:
                continue
            if key == "status":
                status_schema = item_properties.get(key, {})
                allowed = status_schema.get("enum", []) if isinstance(status_schema, dict) else []
                if "in_progress" not in allowed:
                    return None
                item[key] = "in_progress"
            elif key in {"description", "details"}:
                item[key] = "Isolated H008 native plan-update proof."
            else:
                return None

        args: dict[str, Any] = {"plan": [item]}
        for key in required:
            if key == "plan":
                continue
            if key == "explanation":
                explanation_schema = properties.get(key, {})
                if not isinstance(explanation_schema, dict) or explanation_schema.get("type") != "string":
                    return None
                args[key] = "Update the isolated H008 proof plan."
            else:
                return None
        return str(name), json.dumps(args, ensure_ascii=False, separators=(",", ":")), "function_call"
    return None


def _select_get_goal_function(tools: Any) -> tuple[str, str, str] | None:
    """Select only a zero-argument get_goal function actually advertised by the CLI."""
    if not isinstance(tools, list):
        return None
    for tool in tools:
        if not isinstance(tool, dict) or tool.get("type") != "function":
            continue
        function = tool.get("function") if isinstance(tool.get("function"), dict) else tool
        if function.get("name") != "get_goal":
            continue
        parameters = function.get("parameters")
        if not isinstance(parameters, dict) or parameters.get("type") != "object":
            return None
        properties = parameters.get("properties", {})
        required = parameters.get("required", [])
        if not isinstance(properties, dict) or properties or required != []:
            return None
        return "get_goal", "{}", "function_call"
    return None


def _summarize_goal_output(value: Any) -> dict[str, Any]:
    """Return bounded, content-free metadata for the native get_goal result."""
    if isinstance(value, str):
        raw = value.encode("utf-8", errors="replace")
        parsed_json = False
        parsed: Any = None
        if len(raw) <= MAX_GOAL_OUTPUT_BYTES:
            try:
                parsed = json.loads(value)
                parsed_json = True
            except (json.JSONDecodeError, RecursionError):
                pass
        output_type = "string"
    else:
        try:
            raw = json.dumps(value, ensure_ascii=True, sort_keys=True, separators=(",", ":"), allow_nan=False).encode("utf-8")
        except (TypeError, ValueError, RecursionError):
            raw = type(value).__name__.encode("ascii", errors="replace")
            parsed = None
            parsed_json = False
        else:
            parsed = value
            parsed_json = len(raw) <= MAX_GOAL_OUTPUT_BYTES
        output_type = "null" if value is None else "object" if isinstance(value, dict) else "array" if isinstance(value, list) else type(value).__name__[:32]

    goal_field_present = parsed is None and parsed_json
    goal_value = parsed
    if isinstance(parsed, dict) and "goal" in parsed:
        goal_field_present = True
        goal_value = parsed["goal"]
    return {
        "output_type": output_type,
        "parsed_json": bool(parsed_json),
        "goal_field_present": bool(goal_field_present),
        "goal_is_null": bool(goal_field_present and goal_value is None),
        "sha256": hashlib.sha256(raw).hexdigest(),
    }


class _Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    @property
    def state(self) -> _State:
        return self.server.state  # type: ignore[attr-defined]

    def log_message(self, _format: str, *_args: object) -> None:
        return

    def _record_transport_attempt(self) -> str:
        path = urlsplit(self.path).path[:160]
        self.state.add_transport_attempt(
            {
                "method": str(self.command)[:16],
                "path": path,
                "websocket_upgrade": self.headers.get("Upgrade", "").lower() == "websocket",
            }
        )
        return path

    def do_GET(self) -> None:
        path = self._record_transport_attempt()
        if path.endswith("/models"):
            self._json(
                200,
                {"object": "list", "data": [{"id": MODEL, "object": "model", "created": 0, "owned_by": "h008-proof"}]},
            )
        else:
            self._json(404, {"error": {"message": "fixture route not found"}})

    def do_POST(self) -> None:
        path = self._record_transport_attempt()
        length = int(self.headers.get("content-length", "0"))
        if length < 0 or length > 8_000_000:
            self._json(413, {"error": {"message": "fixture request too large"}})
            return
        raw = self.rfile.read(length)
        try:
            body = json.loads(raw)
        except (json.JSONDecodeError, UnicodeDecodeError):
            self._json(400, {"error": {"message": "invalid fixture request"}})
            return
        if not isinstance(body, dict) or not path.endswith("/responses"):
            self._json(404, {"error": {"message": "fixture route not found"}})
            return

        inputs = body.get("input", [])
        outputs = [
            item
            for item in inputs
            if isinstance(item, dict) and item.get("type") == "function_call_output"
        ] if isinstance(inputs, list) else []
        request = {
            "path": path,
            "method": "POST",
            "websocket_upgrade": self.headers.get("Upgrade", "").lower() == "websocket",
            "body": body,
            "raw_sha256": hashlib.sha256(raw).hexdigest(),
            "authorization": self.headers.get("authorization"),
            "tool_outputs": outputs,
            "tool_inventory": _tool_inventory(body.get("tools")),
        }
        ordinal = self.state.add(request)
        if ordinal == 1:
            self._first_response(body)
        else:
            if self.state.scenario == "goal-query":
                matching = [item for item in outputs if isinstance(item, dict) and item.get("call_id") == GOAL_CALL_ID]
                if len(matching) == 1 and "output" in matching[0]:
                    self.state.goal_output_summary = _summarize_goal_output(matching[0]["output"])
            self._final_response(body)

    def _first_response(self, body: dict[str, Any]) -> None:
        model = str(body.get("model") or MODEL)
        tools = body.get("tools")
        tools = tools if isinstance(tools, list) else []
        expected_name = "get_goal" if self.state.scenario == "goal-query" else "update_plan"
        self.state.native_tool_available = any(
            isinstance(tool, dict)
            and tool.get("type") == "function"
            and (
                (tool.get("function") if isinstance(tool.get("function"), dict) else tool).get("name")
                == expected_name
            )
            for tool in tools
        )
        self.state.plan_tool_available = self.state.native_tool_available if self.state.scenario == "plan-update" else False
        selected = (
            _select_get_goal_function(tools)
            if self.state.scenario == "goal-query"
            else _select_update_plan_function(tools)
        )
        if selected is None:
            self.state.tool_shape = (
                f"unsupported-{expected_name}-schema"
                if self.state.native_tool_available
                else f"{expected_name}-not-advertised"
            )
        else:
            self.state.tool_name, self.state.tool_arguments, self.state.tool_shape = selected

        self._start_sse()
        sequence = 0

        def send(name: str, payload: dict[str, Any]) -> None:
            nonlocal sequence
            self._chunk(_event(name, {"sequence_number": sequence, **payload}))
            sequence += 1

        response = {
            "id": "resp_h008_first",
            "object": "response",
            "created_at": int(time.time()),
            "status": "in_progress",
            "model": model,
            "output": [],
        }
        send("response.created", {"type": "response.created", "response": response})
        send("response.in_progress", {"type": "response.in_progress", "response": response})
        message = {"id": "msg_h008_partial", "type": "message", "role": "assistant", "status": "in_progress", "content": []}
        send("response.output_item.added", {"type": "response.output_item.added", "output_index": 0, "item": message})
        send(
            "response.content_part.added",
            {"type": "response.content_part.added", "item_id": message["id"], "output_index": 0, "content_index": 0, "part": {"type": "output_text", "text": "", "annotations": []}},
        )
        send(
            "response.output_text.delta",
            {"type": "response.output_text.delta", "item_id": message["id"], "output_index": 0, "content_index": 0, "delta": STREAM_PARTIAL},
        )
        self.wfile.flush()
        self.state.partial_sent.set()
        text = STREAM_PARTIAL
        send("response.output_text.done", {"type": "response.output_text.done", "item_id": message["id"], "output_index": 0, "content_index": 0, "text": text})
        send("response.content_part.done", {"type": "response.content_part.done", "item_id": message["id"], "output_index": 0, "content_index": 0, "part": {"type": "output_text", "text": text, "annotations": []}})
        message = {**message, "status": "completed", "content": [{"type": "output_text", "text": text, "annotations": []}]}
        send("response.output_item.done", {"type": "response.output_item.done", "output_index": 0, "item": message})
        self.wfile.flush()
        self.state.message_item_completed_sent.set()
        if not self.state.release_first_response.wait(timeout=45):
            self._end_sse()
            return

        output_items: list[dict[str, Any]] = [message]

        if selected is not None:
            name, arguments, shape = selected
            is_goal_query = self.state.scenario == "goal-query"
            call = {
                "id": "fc_h008_goal" if is_goal_query else "fc_h008_plan",
                "type": shape,
                "call_id": GOAL_CALL_ID if is_goal_query else CALL_ID,
                "name": name,
                "arguments": "",
            }
            send("response.output_item.added", {"type": "response.output_item.added", "output_index": 1, "item": call})
            first, second = arguments[: max(1, len(arguments) // 2)], arguments[max(1, len(arguments) // 2) :]
            for fragment in (first, second):
                if fragment:
                    send("response.function_call_arguments.delta", {"type": "response.function_call_arguments.delta", "item_id": call["id"], "output_index": 1, "delta": fragment})
            send("response.function_call_arguments.done", {"type": "response.function_call_arguments.done", "item_id": call["id"], "output_index": 1, "arguments": arguments})
            call["arguments"] = arguments
            send("response.output_item.done", {"type": "response.output_item.done", "output_index": 1, "item": call})
            output_items.append(call)

        completed = {
            **response,
            "status": "completed",
            "output": output_items,
            "usage": {"input_tokens": 42, "output_tokens": 11, "total_tokens": 53},
        }
        send("response.completed", {"type": "response.completed", "response": completed})
        self.state.first_completion_sent.set()
        self._end_sse()

    def _final_response(self, body: dict[str, Any]) -> None:
        model = str(body.get("model") or MODEL)
        self._start_sse()
        sequence = 0
        response_id = "resp_h008_final"
        item_id = "msg_h008_final"
        is_goal_query = self.state.scenario == "goal-query"
        text = (
            f"Goal query completed. {GOAL_FINAL_MARKER}"
            if is_goal_query
            else f"Plan update completed. {FINAL_MARKER}"
        )

        def send(name: str, payload: dict[str, Any]) -> None:
            nonlocal sequence
            self._chunk(_event(name, {"sequence_number": sequence, **payload}))
            sequence += 1

        response = {"id": response_id, "object": "response", "created_at": int(time.time()), "status": "in_progress", "model": model, "output": []}
        send("response.created", {"type": "response.created", "response": response})
        send("response.in_progress", {"type": "response.in_progress", "response": response})
        message = {"id": item_id, "type": "message", "role": "assistant", "status": "in_progress", "content": []}
        send("response.output_item.added", {"type": "response.output_item.added", "output_index": 0, "item": message})
        send("response.content_part.added", {"type": "response.content_part.added", "item_id": item_id, "output_index": 0, "content_index": 0, "part": {"type": "output_text", "text": "", "annotations": []}})
        send("response.output_text.delta", {"type": "response.output_text.delta", "item_id": item_id, "output_index": 0, "content_index": 0, "delta": text})
        send("response.output_text.done", {"type": "response.output_text.done", "item_id": item_id, "output_index": 0, "content_index": 0, "text": text})
        send("response.content_part.done", {"type": "response.content_part.done", "item_id": item_id, "output_index": 0, "content_index": 0, "part": {"type": "output_text", "text": text, "annotations": []}})
        message = {**message, "status": "completed", "content": [{"type": "output_text", "text": text, "annotations": []}]}
        send("response.output_item.done", {"type": "response.output_item.done", "output_index": 0, "item": message})
        completed = {
            **response,
            "status": "completed",
            "output": [message],
            "usage": {"input_tokens": 51, "output_tokens": 16, "total_tokens": 67},
        }
        send("response.completed", {"type": "response.completed", "response": completed})
        self._end_sse()

    def _start_sse(self) -> None:
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Cache-Control", "no-cache")
        self.send_header("Transfer-Encoding", "chunked")
        self.send_header("Connection", "close")
        self.end_headers()

    def _chunk(self, data: bytes) -> None:
        self.wfile.write(f"{len(data):X}\r\n".encode("ascii"))
        self.wfile.write(data + b"\r\n")
        self.wfile.flush()

    def _end_sse(self) -> None:
        self.wfile.write(b"0\r\n\r\n")
        self.wfile.flush()

    def _json(self, status: int, value: dict[str, Any]) -> None:
        data = json.dumps(value, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.send_header("Connection", "close")
        self.end_headers()
        self.wfile.write(data)


class LocalCodexGatewayFixture:
    """Loopback-only fake API gateway with a gated first Responses stream."""

    def __init__(self, scenario: str = "plan-update") -> None:
        if scenario not in {"plan-update", "goal-query"}:
            raise ValueError("unsupported-codex-proof-scenario")
        self.state = _State(scenario)
        self.server = ThreadingHTTPServer(("127.0.0.1", 0), _Handler)
        self.server.daemon_threads = True
        self.server.state = self.state  # type: ignore[attr-defined]
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)

    @property
    def port(self) -> int:
        return int(self.server.server_address[1])

    def start(self) -> None:
        self.thread.start()

    def release(self) -> None:
        self.state.release_first_response.set()

    def close(self) -> None:
        self.release()
        self.server.shutdown()
        self.server.server_close()
        self.thread.join(timeout=5)
        if self.thread.is_alive():
            raise RuntimeError("codex-fixture-thread-did-not-stop")

    def requests(self) -> list[dict[str, Any]]:
        return self.state.all_requests()

    def transport_attempts(self) -> list[dict[str, Any]]:
        return self.state.all_transport_attempts()
