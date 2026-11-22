"""Real owned WebSocket usage, continuation and HTTP fallback; loopback only."""
import argparse
import base64
from concurrent.futures import ThreadPoolExecutor
import hashlib
from http.server import ThreadingHTTPServer
import json
from pathlib import Path
import platform
import runpy
import subprocess
import sys
import threading
import time
import uuid

from websockets.sync.client import connect
from websockets.exceptions import ConnectionClosed

ROOT = Path(__file__).resolve().parents[2]
http = runpy.run_path(str(ROOT / "tests/runtime/responses_usage_capture.py"))
capture, probe, replay, ccr = (http[name] for name in ("capture", "probe", "replay", "ccr"))
surface = ccr["surface"]
require = replay.require
WS_SURFACE = "openai_responses_ws"


def body(model="gpt-6.1-sol", effort="xhigh", **changes):
    return {"type": "response.create", "model": model, "reasoning": {"effort": effort},
            "input": [{"role": "user", "content": "Report FIXTURE_OK."}], **changes}


class Fixture(surface["SurfaceHandler"]):
    def read_create(self):
        while True:
            opcode, raw = surface["ws_read"](self.rfile)
            if opcode == 8:
                surface["ws_write"](self.wfile, 8, raw)
                raise EOFError()
            if opcode == 9:
                surface["ws_write"](self.wfile, 10, raw)
                continue
            require(opcode == 1, "ws_fixture_text_frame")
            value = json.loads(raw)
            require(value.get("type") == "response.create", "ws_fixture_create_frame")
            with self.server.capture_lock:
                self.server.calls.setdefault(self.case, []).append(value)
            return value

    def emit(self, request, identity, counters, *, failed=False, ccr_hash=None, created=True):
        response = http["response"](request, counters, output=[] if counters is not None and counters[-1] == 0 else None)
        response["id"] = identity
        if ccr_hash:
            response["output"] = [{"type": "function_call", "id": "fc-fixture", "call_id": "call-retrieve",
                "name": "headroom_retrieve", "arguments": json.dumps({"hash": ccr_hash})}]
        common = {"stream_id": request["stream_id"]} if "stream_id" in request else {}
        if created:
            surface["ws_write"](self.wfile, 1, {"type": "response.created", "response": {
                **response, "status": "in_progress", "usage": None}, **common})
        if ccr_hash:
            surface["ws_write"](self.wfile, 1, {"type": "response.output_item.done", "item": response["output"][0], **common})
        elif not failed and counters is not None and counters[-1]:
            surface["ws_write"](self.wfile, 1, {"type": "response.output_text.delta", "item_id": "msg-fixture",
                "output_index": 0, "content_index": 0, "delta": "FIXTURE_OK", **common})
        if failed:
            response["status"] = "failed"
        surface["ws_write"](self.wfile, 1, {"type": "response.failed" if failed else "response.completed",
            "response": response, **common})

    def do_GET(self):
        require(self.path == "/v1/responses", "ws_fixture_path")
        self.case = self.headers["X-Proof-Case"]
        require(self.headers.get("Authorization") == "Bearer " + replay.SENTINEL, "ws_auth_changed")
        with self.server.capture_lock:
            count = self.server.handshakes.get(self.case, 0) + 1
            self.server.handshakes[self.case] = count
        if self.case == "fallback" or self.case == "handshake_retry" and count == 1:
            self.send_response(503)
            self.send_header("Content-Length", "0")
            self.send_header("Connection", "close")
            self.end_headers()
            self.close_connection = True
            return
        accept = base64.b64encode(hashlib.sha1((self.headers["Sec-WebSocket-Key"]
            + "258EAFA5-E914-47DA-95CA-C5AB0DC85B11").encode()).digest()).decode()
        self.send_response(101)
        self.send_header("Upgrade", "websocket")
        self.send_header("Connection", "Upgrade")
        self.send_header("Sec-WebSocket-Accept", accept)
        self.end_headers()
        self.connection.settimeout(30)
        self.close_connection = True
        try:
            index = 0
            while True:
                request = self.read_create()
                index += 1
                identity = f"resp-{self.case}-{index}"
                if self.case == "sequence":
                    values = ((2400, 2000, 100, 29), (700, 600, None, 7), (0, 0, 0, 0))[index - 1]
                    self.emit(request, identity, values)
                elif self.case == "missing":
                    self.emit(request, identity, None)
                elif self.case == "failed":
                    self.emit(request, identity, (50, 0, 0, 0) if index == 1 else (600, 500, 0, 6), failed=index == 1)
                elif self.case == "interleaved":
                    second = self.read_create()
                    other = "resp-interleaved-second"
                    for value, rid in ((request, identity), (second, other)):
                        surface["ws_write"](self.wfile, 1, {"type": "response.created", "stream_id": value["stream_id"],
                            "response": {**http["response"](value, None), "id": rid, "status": "in_progress"}})
                    self.emit(second, other, (800, 600, 0, 8), created=False)
                    self.emit(request, identity, (900, 700, None, 9), created=False)
                elif self.case.startswith("parallel-"):
                    self.server.parallel.wait(timeout=10)
                    self.emit(request, "resp-shared", (100, 0, 0, 1) if self.case.endswith("a") else (200, 0, 0, 2))
                elif self.case == "handshake_retry":
                    self.emit(request, identity, (400, 300, 0, 4))
                elif self.case == "warm_turn":
                    self.emit(request, identity, (2, 0, 0, 0) if index == 1 else (1600, 1200, 0, 16))
                elif self.case == "cut":
                    surface["ws_write"](self.wfile, 1, {"type": "response.created", "response": {
                        **http["response"](request, None), "id": identity, "status": "in_progress"}})
                    return
                elif self.case == "ccr":
                    if index == 1:
                        matches = [(item, http["MARKER"].search(item.get("output", ""))) for item in request["input"]
                                   if item.get("type") == "function_call_output" and isinstance(item.get("output"), str)]
                        matches = [(item, match) for item, match in matches if match]
                        require(bool(matches), "ws_ccr_marker_missing")
                        item, match = matches[0]
                        self.server.original = replay.fixtures.output("repetitive_logs", int(item["call_id"].rsplit("-", 1)[1]))
                        self.server.marker = match[1]
                        self.emit(request, identity, (1100, 1000, 0, 11), ccr_hash=match[1])
                    else:
                        outputs = [json.loads(item["output"]) for item in request["input"]
                                   if item.get("type") == "function_call_output" and item.get("call_id") == "call-retrieve"]
                        require(len(outputs) == 1 and outputs[0]["original_content"] == self.server.original,
                                "ws_ccr_original_not_forwarded")
                        self.server.original_forwarded = True
                        self.emit(request, identity, (1200, 1000, 0, 12))
        except (OSError, EOFError):
            return

    def do_POST(self):
        require(self.path == "/v1/responses" and self.headers["X-Proof-Case"] == "fallback", "ws_fallback_path")
        require(self.headers.get("Authorization") == "Bearer " + replay.SENTINEL, "ws_fallback_auth")
        request = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        with self.server.capture_lock:
            self.server.calls.setdefault("fallback", []).append(request)
        value = http["response"](request, (1300, 1000, None, 13))
        frames = [capture["sse_event"]("response.created", response={**value, "status": "in_progress", "usage": None}),
                  capture["sse_event"]("response.completed", response=value)]
        raw = b"".join(frames)
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Content-Length", str(len(raw)))
        self.send_header("Connection", "close")
        self.end_headers()
        self.wfile.write(raw)
        self.close_connection = True


def terminal(connection):
    while True:
        event = json.loads(connection.recv(timeout=40))
        if event.get("type") in {"response.completed", "response.failed", "response.incomplete"}:
            return event


def new_rows(engine, seen, count=1):
    deadline = time.monotonic() + 3
    while True:
        ids = set(probe["request_logs"](engine)) - seen
        rows = capture["records"](engine)
        matched = ids & rows.keys()
        if len(matched) >= count:
            return {identity: rows[identity] for identity in matched}
        require(time.monotonic() < deadline, "ws_outcome_missing")
        time.sleep(0.01)


def ws(engine, case):
    return connect(f"ws://127.0.0.1:{engine.port}/v1/responses", proxy=None, open_timeout=60, close_timeout=5,
        additional_headers={"Authorization": "Bearer " + replay.SENTINEL, "X-Proof-Case": case, "x-headroom-session-id": case})


def summary(rows):
    return [{"surface": row["surface"], "truncated": row["truncated"], "attempts": len(row["attempts"]),
             "usage": row["usage"], "failed": row.get("failed", False)} for row in rows.values()]


def check_one(rows, expected, *, surface=WS_SURFACE):
    require(len(rows) == 1, "ws_outcome_join_not_unique")
    row = next(iter(rows.values()))
    require(row["surface"] == surface and not row["truncated"] and len(row["attempts"]) == 1, "ws_usage_scope")
    require(row["usage"]["input_tokens"] == expected, "ws_usage_counter")
    return row


def exercise(engine, server, binary, run, result):
    with ws(engine, "sequence") as connection:
        for index, request in enumerate((body(), body("gpt-6-luna", "high"), body()), 1):
            seen = set(probe["request_logs"](engine))
            connection.send(replay.encoded(request).decode())
            require(terminal(connection)["type"] == "response.completed", "ws_client_complete")
            rows = new_rows(engine, seen)
            observed_row = check_one(rows, (2400, 700, 0)[index - 1])
            result[f"sequence_{index}"] = summary(rows)
            if index == 1:
                frame = {"port": engine.port, "pid": engine.pids[-1], "generation": engine.generation,
                         "secret": engine.control_token, "directory": str(run),
                         "local_input": observed_row.get("local_input")}
                observed = subprocess.run([str(binary), "--exact", "runtime::metrics_read::tests::owned_websocket_usage_reaches_activity_detail",
                    "--ignored", "--test-threads=1"], input=replay.encoded(frame), capture_output=True, cwd=ROOT, timeout=30,
                    creationflags=subprocess.CREATE_NO_WINDOW)
                require(observed.returncode == 0 and b"1 passed" in observed.stdout, "ws_native_projection_failed")
                result["native_projection"] = "pass"
    require([(item["model"], item["reasoning"]) for item in server.calls["sequence"]] == [
        ("gpt-6.1-sol", {"effort": "xhigh"}), ("gpt-6-luna", {"effort": "high"}), ("gpt-6.1-sol", {"effort": "xhigh"})],
        "ws_model_or_reasoning_changed")
    with ws(engine, "missing") as connection:
        seen = set(probe["request_logs"](engine))
        connection.send(replay.encoded(body()).decode())
        terminal(connection)
        rows = new_rows(engine, seen)
        check_one(rows, None)
        require(all("local_input" not in row for row in rows.values()), "missing_usage_counted_as_local_savings")
        require(all(value is None for value in next(iter(rows.values()))["usage"].values()), "ws_missing_is_zero")
        result["missing"] = summary(rows)
    with ws(engine, "failed") as connection:
        connection.send(replay.encoded(body()).decode())
        require(terminal(connection)["type"] == "response.failed", "ws_failure_changed")
        seen = set(probe["request_logs"](engine))
        connection.send(replay.encoded(body()).decode())
        terminal(connection)
        rows = new_rows(engine, seen)
        row = next(iter(rows.values()))
        require(len(rows) == 1 and row["truncated"] and row["failed"]
                and all(value is None for value in row["usage"].values()), "ws_failed_scope_borrowed")
        result["failed_then_complete"] = summary(rows)
    with ws(engine, "interleaved") as connection:
        seen = set(probe["request_logs"](engine))
        connection.send(replay.encoded(body(stream_id="a")).decode())
        connection.send(replay.encoded(body("gpt-6-luna", "high", stream_id="b")).decode())
        require([terminal(connection)["response"]["model"], terminal(connection)["response"]["model"]]
                == ["gpt-6-luna", "gpt-6.1-sol"], "ws_interleaved_wire")
        rows = new_rows(engine, seen, 2)
        require(len(rows) == 2 and sorted(row["usage"]["input_tokens"] for row in rows.values()) == [800, 900]
                and all(not row["truncated"] and len(row["attempts"]) == 1 for row in rows.values()), "ws_interleaved_counters")
        require(all("local_input" not in row for row in rows.values()), "interleaved_local_savings_guessed")
        result["interleaved"] = summary(rows)
    def parallel(case):
        with ws(engine, case) as connection:
            connection.send(replay.encoded(body()).decode())
            terminal(connection)
    seen = set(probe["request_logs"](engine))
    with ThreadPoolExecutor(max_workers=2) as pool:
        futures = [pool.submit(parallel, case) for case in ("parallel-a", "parallel-b")]
        for future in futures:
            future.result()
    rows = new_rows(engine, seen, 2)
    require(len(rows) == 2 and sorted(row["usage"]["input_tokens"] for row in rows.values()) == [100, 200],
            "ws_parallel_connection_context")
    result["parallel_connections"] = summary(rows)
    for case, expected in (("handshake_retry", 400), ("fallback", 1300)):
        seen = set(probe["request_logs"](engine))
        with ws(engine, case) as connection:
            connection.send(replay.encoded(body()).decode())
            require(terminal(connection)["type"] == "response.completed", "ws_fallback_client_reply")
        rows = new_rows(engine, seen)
        check_one(rows, expected, surface="openai_responses" if case == "fallback" else WS_SURFACE)
        require(server.handshakes[case] > 1 and len(server.calls[case]) == 1, "ws_handshakes_counted_as_inference")
        result[case] = summary(rows)
    with ws(engine, "ccr") as connection:
        from headroom.tokenizers.estimator import EstimatingTokenCounter

        observed = set(probe["request_logs"](engine))
        request = replay.fixtures.conversation("repetitive_logs", "gpt-6.1-sol", "xhigh", 1)[-1]
        request.pop("stream")
        request["type"] = "response.create"
        connection.send(replay.encoded(request).decode())
        first = terminal(connection)
        row = check_one(new_rows(engine, observed), 1100)
        counter = EstimatingTokenCounter()
        before = counter.count_text(replay.encoded(request).decode())
        after = counter.count_text(replay.encoded(server.calls["ccr"][0]).decode())
        require(before > after and row.get("local_input") == {
            "before": before, "after": after, "counter": "headroom_estimator"},
            "ws_local_pair_does_not_match_compressed_input")
        status, _, raw = probe["exchange"](engine.port, "POST", "/v1/retrieve", {"Content-Type": "application/json"},
            replay.encoded({"hash": server.marker}))
        original = json.loads(raw)
        require(status == 200 and original["original_content"] == server.original, "ws_ccr_original_changed")
        connection.send(replay.encoded(body(previous_response_id=first["response"]["id"], input=[{
            "type": "function_call_output", "call_id": "call-retrieve", "output": json.dumps(original)}])).decode())
        require(terminal(connection)["type"] == "response.completed" and server.original_forwarded, "ws_ccr_continuation")
        result["ccr"] = {"original_retrieved_and_forwarded": True, "requests": len(server.calls["ccr"]),
                         "verified_local_pair": row["local_input"], "fixture_provider_input": 1100}
    with ws(engine, "warm_turn") as connection:
        observed = set(probe["request_logs"](engine))
        connection.send(replay.encoded(body(generate=False, input=[])).decode())
        warmup = terminal(connection)
        warm_rows = new_rows(engine, observed)
        require(check_one(warm_rows, 2).get("local_input") is None, "ws_warmup_claimed_measurement")

        request = replay.fixtures.conversation("repetitive_logs", "gpt-6.1-sol", "xhigh", 1)[-1]
        request.pop("stream")
        request.update(type="response.create", previous_response_id=warmup["response"]["id"])
        request["input"].insert(0, {"type": "additional_tools", "tools": request.pop("tools")})
        connection.send(replay.encoded(request).decode())
        require(terminal(connection)["type"] == "response.completed", "ws_warm_turn_not_completed")
        row = check_one(new_rows(engine, observed | warm_rows.keys()), 1600)

        def count_normalized(value):
            carrier, *items = value["input"]
            require(carrier["type"] == "additional_tools" and "tools" not in value, "ws_carrier_not_restored")
            return counter.count_text(replay.encoded({**value, "input": items, "tools": carrier["tools"]}).decode())

        before = count_normalized(request)
        after = count_normalized(server.calls["warm_turn"][1])
        require(before > after and row.get("local_input") == {
            "before": before, "after": after, "counter": "headroom_estimator"},
            "ws_warm_turn_local_pair")
        result["warm_turn"] = {"verified_local_pair": row["local_input"], "fixture_provider_input": 1600}
    seen = set(capture["records"](engine))
    with ws(engine, "cut") as connection:
        connection.send(replay.encoded(body()).decode())
        try:
            while True:
                connection.recv(timeout=40)
        except ConnectionClosed:
            pass
    time.sleep(0.1)
    rows = {key: row for key, row in capture["records"](engine).items() if key not in seen}
    require(all(all(value is None for value in row["usage"].values()) for row in rows.values()), "ws_cut_claimed_usage")
    result["cut"] = {"engine_records": len(rows), "known_usage": False}
    return result


def main():
    require(Path.cwd().resolve() == ROOT == Path(r"D:\Projeler\Headroom"), "fixture_workspace")
    args = argparse.ArgumentParser(description=__doc__)
    args.add_argument("--native-reader", type=Path, required=True)
    binary = args.parse_args().native_reader.resolve(strict=True)
    require(binary.is_relative_to(ROOT / "work/caches/cargo-target"), "native_reader_path")
    controller = replay.load_module("ws_capture_controller", ROOT / "runtime/desktop_controller.py")
    manifest, payload, _ = controller.load_manifest(ROOT / "runtime/manifest.json")
    require(replay.sha(Path(sys.executable)) == manifest["engine"]["python_sha256"], "fixture_python_pin")
    supervisor = replay.load_module("ws_capture_supervisor", payload / manifest["engine"]["runner"])
    plan = json.loads(ccr["PLAN"].read_bytes())
    run = ROOT / "work/proofs/optimization/desktop-ws-usage/runs" / str(uuid.uuid4())
    run.mkdir(parents=True)
    overlay = replay.prepare_fixture_runtime(run, payload)
    inputs = [Path(__file__), Path(sys.executable), binary, ccr["PLAN"], payload / "runtime-manifest.json", *overlay,
        *[ROOT / "runtime" / path.name for path in overlay],
        *[ROOT / "tests/runtime" / name for name in ("responses_usage_capture.py", "anthropic_usage_capture.py")],
        *[ROOT / "bench/optimization" / name for name in ("fixtures.py", "surfaces.py", "replay.py", "anthropic_usage.py", "ccr_policy.py")],
        *[ROOT / "src-tauri/src" / name for name in ("runtime/metrics_read.rs", "metrics/normalize.rs", "metrics/store.rs")]]
    source_map = json.loads((ROOT / "runtime/optimization-sources.json").read_bytes())
    inputs += [payload / "Lib/site-packages/headroom" / name for name in source_map]
    before = replay.fingerprints(inputs)
    server = ThreadingHTTPServer(("127.0.0.1", 0), Fixture)
    server.capture_lock, server.calls, server.handshakes = threading.Lock(), {}, {}
    server.parallel = threading.Barrier(2)
    server.original_forwarded = False
    threading.Thread(target=server.serve_forever, daemon=True).start()
    scenarios, stops, failure = {}, [], None
    engine = probe["UsageEngine"](controller, supervisor, payload, run / "engine", server.server_port, plan)
    try:
        try:
            engine.start()
            require(not capture["records"](engine) and capture["usage_response"](engine, None) == (404, {"error": "not_found"}),
                    "ws_usage_private_access")
            exercise(engine, server, binary, run, scenarios)
        finally:
            engine.close()
            stops.append(engine.stopped)
        restarted = probe["UsageEngine"](controller, supervisor, payload, run / "engine-restarted", server.server_port, plan)
        try:
            restarted.start()
            require(restarted.generation != engine.generation and not capture["records"](restarted)
                and capture["usage_response"](restarted, engine.control_token) == (404, {"error": "not_found"}), "ws_generation_isolation")
        finally:
            restarted.close()
            stops.append(restarted.stopped)
    except Exception as error:
        failure = type(error).__name__
        if isinstance(error, AssertionError) and str(error).isascii() and str(error).replace("_", "").isalnum():
            failure = str(error)
    finally:
        server.shutdown()
        server.server_close()
    after = replay.fingerprints(inputs)
    network = replay.fixture_network_evidence(run)
    passed = failure is None and before == after and network["huggingface_hub_payload_bytes"] == 0
    output = run / "result.json"
    replay.write_new(output, {"passed": passed, "failure": failure, "os": platform.platform(), "scenarios": scenarios,
        "stops": stops, "handshakes": server.handshakes, "fixture_network": network, "before": before, "after": after})
    print(json.dumps({"passed": passed, "failure": failure, "scenarios": len(scenarios), "proof": output.relative_to(ROOT).as_posix()}))
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
