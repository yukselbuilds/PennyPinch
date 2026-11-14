"""Bounded synthetic parent/child provider for the real OpenCode task tool."""
from __future__ import annotations

import hashlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import importlib.util
import json
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parents[3]
spec = importlib.util.spec_from_file_location("subagent_shared_fixture", Path(__file__).with_name("tool_fixture.py"))
shared = importlib.util.module_from_spec(spec)
spec.loader.exec_module(shared)
CASES = {
    "same": {"parent": ("chat", "lab/slot", shared.MODEL),
             "child": ("chat", "lab/slot", shared.MODEL), "agent": "headroom_inherit"},
    "switch": {"parent": ("chat", "lab/other", "headroom-opencode-switched-fixture"),
               "child": ("chat", "lab/other", "headroom-opencode-switched-fixture"), "agent": "headroom_inherit"},
    "mixed": {"parent": ("responses", "openai/slot", shared.RESPONSES_MODEL),
              "child": ("chat", "lab/child", "headroom-opencode-child-fixture"), "agent": "headroom_other"},
}


def marker(case, role):
    assert case in CASES and role in ("parent", "child"), "unknown_fixture_identity"
    return f"HEADROOM_{role.upper()}_{case}"


def identity(body):
    users = []
    for message in body["messages"]:
        if message.get("role") != "user":
            continue
        content = message["content"]
        if isinstance(content, str):
            users.append(content)
        else:
            assert isinstance(content, list) and all(isinstance(block, dict) and block.get("type") in ("text", "input_text")
                                                     and isinstance(block.get("text"), str) for block in content), "user_content_shape_changed"
            users.append("".join(block["text"] for block in content))
    assert users, "user_marker_missing"
    match = re.search(r"HEADROOM_(PARENT|CHILD)_(same|switch|mixed)\b", users[-1])
    assert match, "user_marker_missing"
    return match[2], match[1].lower()


class Conversation:
    def __init__(self, case, role):
        self.case, self.role = case, role
        self.surface, self.selector, self.model = CASES[case][role]
        self.step, self.rows, self.complete = 0, [], False
        self.content_hash = None
        self.final = f"{marker(case, role)}_OK"

    def call_id(self, kind):
        return f"call_{self.case}_{self.role}_{kind}"

    def advance(self, body):
        assert identity(body) == (self.case, self.role), "request_identity_changed"
        assert body.get("model") == self.model, "wire_model_changed"
        assert not any(key in body for key in ("reasoning", "reasoning_effort")), "unexpected_reasoning_selection"
        names = {tool.get("function", {}).get("name") for tool in body.get("tools", [])}
        assert shared.CLIENT_TOOL in names and "headroom_retrieve" in names, "fixture_tools_missing"
        row = {"step": self.step, "role": self.role, "wire_model": self.model,
               "stream": body.get("stream", False), "synthetic_auth_preserved": True}
        use = lambda kind, name, args: shared.use(self.call_id(kind), name, args)
        if self.step == 0:
            reply = {"role": "assistant", "content": None,
                     "tool_calls": [use("logs", shared.CLIENT_TOOL, {"kind": "logs"})]}
        elif self.step == 1:
            compressed = shared.result(body, self.call_id("logs"))
            match = re.search(r"(?:hash=|<<ccr:)([a-fA-F0-9]{12,24})(?=[^a-fA-F0-9]|$)", compressed)
            assert match and compressed != shared.LOGS, "compression_marker_missing"
            self.content_hash = match[1]
            row.update(original_bytes=len(shared.LOGS.encode()), compressed_bytes=len(compressed.encode()),
                       content_sha256=hashlib.sha256(shared.LOGS.encode()).hexdigest())
            reply = {"role": "assistant", "content": None,
                     "tool_calls": [use("retrieve", "headroom_retrieve", {"hash": self.content_hash})]}
        elif self.step == 2:
            restored = json.loads(shared.result(body, self.call_id("retrieve")))
            keys = {"hash", "original_content"} | ({"original_item_count"} if self.surface == "responses" else set())
            assert (isinstance(restored, dict) and set(restored) == keys and restored.get("hash") == self.content_hash
                    and restored.get("original_content") == shared.LOGS), "retrieval_changed"
            if self.surface == "responses":
                assert type(restored["original_item_count"]) is int and restored["original_item_count"] >= 0, "retrieval_count_invalid"
            row["byte_exact_retrieval"] = True
            reply = {"role": "assistant", "content": None,
                     "tool_calls": [use("witness", shared.CLIENT_TOOL, {"kind": "witness"})]}
        elif self.step == 3:
            assert shared.tool_text(shared.result(body, self.call_id("witness"))) == shared.WITNESS, "witness_changed"
            if self.role == "parent":
                assert "task" in names, "native_task_tool_missing"
                reply = {"role": "assistant", "content": None, "tool_calls": [use("task", "task", {
                    "description": "Read fixed fixture only", "prompt": marker(self.case, "child"),
                    "subagent_type": CASES[self.case]["agent"]})]}
            else:
                reply = {"role": "assistant", "content": self.final}
                self.complete = True
        elif self.step == 4 and self.role == "parent":
            task = shared.result(body, self.call_id("task"))
            assert f"{marker(self.case, 'child')}_OK" in task, "child_final_missing"
            assert re.search(r'<task id="ses_[^"]+" state="completed">', task), "child_completion_missing"
            row["child_task_completed"] = True
            reply = {"role": "assistant", "content": self.final}
            self.complete = True
        else:
            raise AssertionError("unexpected_extra_request")
        self.rows.append(row)
        self.step += 1
        return reply


class Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, *_args):
        pass

    def do_CONNECT(self):
        self.server.blocked_connects += 1
        self.send_error(502, "local_fixture_only")

    def do_POST(self):
        try:
            assert self.path in ("/v1/chat/completions", "/v1/responses"), "unexpected_route"
            assert self.headers.get("authorization") == "Bearer headroom-local-fixture-only", "synthetic_auth_changed"
            length = int(self.headers.get("content-length", "0"))
            assert 0 < length <= 4 * 1024 * 1024, "request_size_invalid"
            surface = "responses" if self.path == "/v1/responses" else "chat"
            body = shared.normalize_request(json.loads(self.rfile.read(length)), surface)
            case, role = identity(body)
            conversation = self.server.conversations[case, role]
            assert conversation.surface == surface, "protocol_changed"
            reply = conversation.advance(body)
            raw = (shared.responses_response(reply, conversation.step, body.get("stream", False)) if surface == "responses"
                   else shared.response(reply, conversation.step, body.get("stream", False), conversation.model))
        except (AssertionError, ValueError, TypeError, KeyError, OSError) as error:
            self.server.errors.append(str(error) if isinstance(error, AssertionError) else "bounded_fixture_failed")
            self.send_error(400, "fixture_rejected")
            return
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream" if body.get("stream") else "application/json")
        self.send_header("Content-Length", str(len(raw)))
        self.send_header("Connection", "close")
        self.end_headers()
        self.wfile.write(raw)


class Upstream(ThreadingHTTPServer):
    daemon_threads = True
    allow_reuse_address = False

    def __init__(self):
        super().__init__(("127.0.0.1", 0), Handler)
        self.conversations = {(case, role): Conversation(case, role) for case in CASES for role in ("parent", "child")}
        self.errors, self.blocked_connects = [], 0


def serve_mcp():
    """Static data only; the real task tool owns session creation and child dispatch."""
    calls = []
    while raw := sys.stdin.buffer.readline(65537):
        assert len(raw) <= 65536, "mcp_size_limit"
        message = json.loads(raw)
        if "id" not in message:
            continue
        method = message.get("method")
        if method == "initialize":
            value = {"protocolVersion": message["params"]["protocolVersion"], "capabilities": {"tools": {}},
                     "serverInfo": {"name": "headroom-fixed-agent-data", "version": "1"}}
        elif method == "tools/list":
            value = {"tools": [{"name": "inspect_logs", "description": "Return fixed synthetic logs or witness.",
                     "inputSchema": {"type": "object", "properties": {"kind": {"type": "string", "enum": ["logs", "witness"]}},
                                     "required": ["kind"], "additionalProperties": False}}]}
        elif method == "tools/call":
            params = message.get("params", {})
            assert params.get("name") == "inspect_logs", "unexpected_data_tool"
            args = params.get("arguments")
            assert args in ({"kind": "logs"}, {"kind": "witness"}), "unexpected_data_arguments"
            assert len(calls) < 16, "too_many_fixture_calls"
            calls.append(args["kind"])
            value = {"content": [{"type": "text", "text": shared.LOGS if args["kind"] == "logs" else shared.WITNESS}],
                     "isError": False}
        elif method == "ping":
            value = {}
        else:
            raise AssertionError("unexpected_mcp_method")
        sys.stdout.write(json.dumps({"jsonrpc": "2.0", "id": message["id"], "result": value}) + "\n")
        sys.stdout.flush()
