"""Trace raw Anthropic usage across a real engine's loopback CCR continuation."""

import argparse
from http.server import ThreadingHTTPServer
import http.client
import json
from pathlib import Path
import platform
import re
import runpy
import sys
import threading
import uuid


HERE = Path(__file__).resolve().parent
ccr = runpy.run_path(str(HERE / "ccr_policy.py"))
surface, replay, ROOT = ccr["surface"], ccr["replay"], ccr["ROOT"]
CASES = ("complete", "missing_cache_write", "failed_continuation")
FIELDS = ("input_tokens", "cache_read_input_tokens", "cache_creation_input_tokens", "output_tokens")
MAX_COUNT = 2**53 - 1
MAX_BODY = 2 * 1024 * 1024
TIMEOUT = 30
TOOL_CALL = "toolu-fixture-ccr"


def counter(value):
    return value if type(value) is int and 0 <= value <= MAX_COUNT else None


def sum_known(values):
    return counter(sum(values)) if values and all(value is not None for value in values) else None


def usage_fields(raw):
    return {field: counter(raw.get(field)) for field in FIELDS}


def usage_sum(calls):
    result = {field: sum_known([call[field] for call in calls]) for field in FIELDS}
    result["total_input_tokens"] = sum_known([result[field] for field in FIELDS[:3]])
    result["total_tokens"] = sum_known([result["total_input_tokens"], result["output_tokens"]])
    return result


class UsageFixture(surface["SurfaceHandler"]):
    def respond(self, body, name):
        key = self.headers["X-Proof-Case"]
        case, turn = key.rsplit("-t", 1)
        if int(turn) < self.server.log_turn:
            return super().respond(body, name)
        replay.require(case in CASES and int(turn) == self.server.log_turn, "fixture_request_scope")
        replay.require(body["stream"] is False, "ccr_expected_buffered_request")
        attempts = self.server.attempts[case]
        status, stop = 200, "end_turn"
        if not attempts:
            marker = ccr["CCR_HASH"].search(ccr["log_block"](body)["content"])
            replay.require(marker, "fixture_ccr_marker_missing")
            content = [{"type": "tool_use", "id": TOOL_CALL, "name": "headroom_retrieve",
                        "input": {"hash": marker.group(1)}}]
            stop = "tool_use"
            usage = dict(zip(FIELDS, (80, 900, 100, 6), strict=True))
            if case == "missing_cache_write":
                usage.pop("cache_creation_input_tokens")
        else:
            replay.require(len(attempts) == 1, "unexpected_ccr_attempt_count")
            result = body["messages"][-1]["content"][0]
            replay.require(result["type"] == "tool_result" and result["tool_use_id"] == TOOL_CALL
                           and json.loads(result["content"])["original_content"] == self.server.original_log,
                           "ccr_continuation_original_changed")
            content = [{"type": "text", "text": "FIXTURE_OK"}]
            usage = dict(zip(FIELDS, (220, 1100, 0, 23), strict=True))
            if case == "failed_continuation":
                status, usage = 429, {}
        attempts.append({"status": status, "usage": usage_fields(usage)})
        if status == 200:
            data = replay.encoded({"id": f"msg-fixture-{len(attempts)}", "type": "message", "role": "assistant",
                                   "model": body["model"], "content": content, "stop_reason": stop,
                                   "stop_sequence": None, "usage": usage})
        else:
            data = replay.encoded({"type": "error", "error": {"type": "rate_limit_error", "message": "fixture"}})
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.send_header("Connection", "close")
        self.end_headers()
        self.wfile.write(data)


class UsageEngine(replay.Engine):
    def receive(self, request_id, timeout):
        frame = super().receive(request_id, timeout)
        if frame["event"] == "ready":
            self.control_token = frame["optimization_control"]["control_token"]
        return frame


def exchange(port, method, path, headers, body=None):
    conn = http.client.HTTPConnection("127.0.0.1", port, timeout=TIMEOUT)
    try:
        conn.request(method, path, body=body, headers=headers)
        response = conn.getresponse()
        data = response.read(MAX_BODY + 1)
        replay.require(len(data) <= MAX_BODY, "fixture_response_limit")
        return response.status, response.getheader("Content-Type", ""), data
    finally:
        conn.close()


def request_logs(engine):
    status, _, data = exchange(engine.port, "GET", "/stats?cached=1",
                               {"X-Headroom-Desktop-Control": engine.control_token})
    replay.require(status == 200, "fixture_stats_status")
    payload = json.loads(data)
    return {row["request_id"]: row for row in payload["request_logs"]}


def observe_client(engine, original, case, key):
    headers = {"Content-Type": "application/json", "x-api-key": replay.SENTINEL,
               "Authorization": "Bearer " + replay.SENTINEL,
               "anthropic-version": "2023-06-01", "X-Proof-Case": key, "x-headroom-session-id": case}
    status, media_type, data = exchange(engine.port, "POST", "/v1/messages", headers, replay.encoded(original))
    replay.require(media_type.startswith("text/event-stream"), "fixture_client_media_type")
    events = [json.loads(line[6:]) for line in data.splitlines() if line.startswith(b"data: ")]
    if case == "failed_continuation":
        replay.require(status == 502 and any(event["type"] == "error" for event in events), "fixture_ccr_failure")
        return {"status": status, "usage": None}
    replay.require(status == 200 and events[-1]["type"] == "message_stop", "fixture_client_incomplete")
    text = "".join(event["delta"]["text"] for event in events
                   if event["type"] == "content_block_delta" and event["delta"]["type"] == "text_delta")
    replay.require(text == "FIXTURE_OK" and all(event["type"] != "error" for event in events), "fixture_answer_changed")
    usage = {}
    for event in events:
        if event["type"] == "message_start":
            usage.update(event["message"].get("usage", {}))
        elif event["type"] == "message_delta":
            usage.update(event.get("usage", {}))
    return {"status": status, "usage": usage_fields(usage)}


def measure(engine, server, plan, requests, rows):
    name = plan["surface"]
    for case in CASES:
        for turn, request in enumerate(requests[:-1], 1):
            key = f"{case}-t{turn}"
            surface["http_request"](engine.port, request, key, name, TIMEOUT)
            captured = server.captured.pop(key)
            replay.require(len(captured) == 1 and captured[0]["auth_preserved"], "fixture_warmup_route")
        key = f"{case}-t{len(requests)}"
        seen = set(request_logs(engine))
        original = requests[-1]
        client = observe_client(engine, original, case, key)
        upstream = server.captured.pop(key)
        attempts = server.attempts[case]
        replay.require(len(upstream) == len(attempts) == 2, "fixture_attempt_coverage")
        for capture in upstream:
            replay.require(capture["auth_preserved"] and capture["body"]["model"] == original["model"]
                           and capture["body"]["thinking"] == original["thinking"], "fixture_route_fields_changed")
        total = usage_sum([attempt["usage"] for attempt in attempts])
        logs = [row for identity, row in request_logs(engine).items() if identity not in seen]
        replay.require(len(logs) == 1, "fixture_stats_join_not_unique")
        log = {name: counter(logs[0].get(name)) for name in
               ("cache_read_tokens", "cache_write_tokens", "uncached_input_tokens", "output_tokens")}
        rows[case] = {"client": client, "engine_request_log": log, "upstream_attempts": attempts,
                      "upstream_usage_sum": total, "original_log_restored": True}
    replay.require(not server.captured, "fixture_unattributed_upstream_calls")
    complete = rows["complete"]
    replay.require(complete["upstream_usage_sum"]["total_tokens"] == 2429, "fixture_complete_total")
    replay.require(complete["client"]["usage"]["output_tokens"] == 23
                   and complete["engine_request_log"] == {
                       "cache_read_tokens": 1100, "cache_write_tokens": 0,
                       "uncached_input_tokens": 220, "output_tokens": 23,
                   }, "fixture_stats_usage_scope")
    unknown = rows["missing_cache_write"]["upstream_usage_sum"]
    replay.require(unknown["cache_creation_input_tokens"] is None and unknown["total_tokens"] is None
                   and unknown["output_tokens"] == 29, "fixture_missing_usage_became_known")
    replay.require(all(value is None for value in rows["failed_continuation"]["upstream_usage_sum"].values()),
                   "fixture_failed_usage_became_known")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run", action="store_true", required=True)
    parser.parse_args()
    replay.require(ROOT == Path.cwd().resolve() == Path(r"D:\Projeler\Headroom"), "fixture_workspace")
    replay.require([counter(value) for value in (0, None, True, -1, "4", MAX_COUNT + 1)]
                   == [0, None, None, None, None, None]
                   and sum_known([]) is None and sum_known([MAX_COUNT, 1]) is None,
                   "fixture_usage_counter_edges")
    plan = json.loads(ccr["PLAN"].read_bytes())
    requests = ccr["workloads"](plan)["long"][:plan["short_turns"] + 1]
    manifest_path = ROOT / "runtime/manifest.json"
    controller = replay.load_module("usage_controller", ROOT / "runtime/desktop_controller.py")
    manifest, payload, _ = controller.load_manifest(manifest_path)
    runner = payload / manifest["engine"]["runner"]
    replay.require(replay.sha(Path(sys.executable)) == manifest["engine"]["python_sha256"], "fixture_python_pin")
    supervisor = replay.load_module("usage_supervisor", runner)
    cache = controller.verify_tokenizer_cache(payload)
    vendor = payload / "Lib/site-packages/headroom"
    run = ROOT / "work/proofs/optimization/anthropic-usage" / str(uuid.uuid4())
    run.mkdir(parents=True)
    overlay = replay.prepare_fixture_runtime(run, payload)
    inputs = [Path(__file__), HERE / "ccr_policy.py", ccr["PLAN"], manifest_path, runner, Path(sys.executable),
              *[HERE / name for name in ("fixtures.py", "replay.py", "surfaces.py")],
              *[ROOT / "runtime" / path.name for path in overlay], *overlay,
              *[path for path in cache.iterdir() if path.is_file()],
              *[vendor / name for name in ("proxy/server.py", "proxy/outcome.py", "proxy/request_logger.py",
                                           "proxy/handlers/anthropic.py", "proxy/handlers/streaming.py",
                                           "ccr/response_handler.py", "cache/prefix_tracker.py")],
              ROOT / "src-tauri/src/metrics/normalize.rs", ROOT / "src-tauri/src/runtime/metrics_read.rs"]
    before = replay.fingerprints(inputs)
    server = ThreadingHTTPServer(("127.0.0.1", 0), UsageFixture)
    server.capture_lock, server.captured = threading.Lock(), {}
    server.log_turn = len(requests)
    server.original_log = ccr["log_block"](requests[-1])["content"]
    server.attempts = {case: [] for case in CASES}
    threading.Thread(target=server.serve_forever, daemon=True).start()
    rows, engine, failure = {}, None, None
    try:
        engine = UsageEngine(controller, supervisor, payload, run / "engine", server.server_port, plan)
        try:
            engine.start()
            measure(engine, server, plan, requests, rows)
        finally:
            engine.close()
    except Exception as error:
        failure = type(error).__name__
        if isinstance(error, AssertionError) and re.fullmatch(r"[a-z0-9_]+", str(error)):
            failure = str(error)
    finally:
        server.shutdown()
        server.server_close()
    after = replay.fingerprints(inputs)
    network = replay.fixture_network_evidence(run)
    passed = failure is None and before == after and network["huggingface_hub_payload_bytes"] == 0
    proof = run / "result.json"
    replay.write_new(proof, {"passed": passed, "failure": failure, "rows": rows, "before": before, "after": after,
                             "shutdown": engine.stopped if engine else None, "fixture_network": network,
                             "os": platform.platform(), "python": platform.python_version(), "scope": "loopback_fixture"})
    print(json.dumps({"passed": passed, "failure": failure, "cases": len(rows), "proof": proof.relative_to(ROOT).as_posix()}))
    return 0 if passed else 1


if __name__ == "__main__":
    sys.exit(main())
